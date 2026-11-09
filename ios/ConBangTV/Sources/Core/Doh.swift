import Foundation
import Network

/// DNS mã hoá (từ 1.10.1 bật sẵn; bản TV: Doh.kt, Windows: Doh.cs): hỏi địa chỉ tên miền qua DoH (RFC 8484) rồi DoT
/// (RFC 7858), mỗi giao thức lần lượt NextDNS (hồ sơ riêng của app — mã lấy từ secret NEXTDNS_ID của repo lúc dựng trên
/// GitHub, ghi vào Info.plist «CBNextDnsId», không có trong code) → Cloudflare → Google; lỗi hết thì cổng nội bộ dùng DNS
/// của máy — app không bao giờ hỏng vì DNS mã hoá bị chặn. Máy chủ gọi bằng IP cố định, kiểm chứng chỉ theo tên máy chủ, đi
/// thẳng (Network.framework, không qua proxy / cổng nội bộ). Máy chủ lỗi thì nghỉ 2 phút. Trả lời «không có tên miền»
/// (NXDOMAIN — vd hồ sơ NextDNS chặn) là trả lời thật: không hỏi máy chủ sau.
///
/// Tắt DNS mã hoá mà đang Vượt chặn: DNS của máy không trả lời / trả địa chỉ chặn (0.0.0.0, ::…) thì vẫn hỏi lại qua chuỗi
/// này (unblocked) — vd ảnh bìa CLB Phim Xưa (clbphimxua.com) bị bộ lọc DNS trong nhà chặn.
enum Doh {
    enum Kind { case doh, dot }

    struct Server {
        let label: String
        let kind: Kind
        let host: String
        let path: String
        let ips: [String]
        var port: Int { kind == .dot ? 853 : 443 }
    }

    private static let nextIps = ["45.90.28.0", "45.90.30.0"]
    private static let googleIps = ["8.8.8.8", "8.8.4.4"]

    /// Thứ tự hỏi: DoH NextDNS → Cloudflare → Google, rồi DoT cũng vậy (không có mã NextDNS thì bỏ NextDNS).
    static func chain(_ nextId: String) -> [Server] {
        let id = nextId.trimmingCharacters(in: .whitespacesAndNewlines)
        var l: [Server] = []
        if !id.isEmpty { l.append(Server(label: "DoH NextDNS", kind: .doh, host: "dns.nextdns.io", path: "/\(id)", ips: nextIps)) }
        l.append(Server(label: "DoH Cloudflare", kind: .doh, host: "dns.cloudflare.com", path: "/dns-query", ips: ["104.16.248.249", "104.16.249.249"]))
        l.append(Server(label: "DoH Google", kind: .doh, host: "dns.google", path: "/dns-query", ips: googleIps))
        if !id.isEmpty { l.append(Server(label: "DoT NextDNS", kind: .dot, host: "\(id).dns.nextdns.io", path: "", ips: nextIps)) }
        l.append(Server(label: "DoT Cloudflare", kind: .dot, host: "1dot1dot1dot1.cloudflare-dns.com", path: "", ips: ["1.1.1.1", "1.0.0.1"]))
        l.append(Server(label: "DoT Google", kind: .dot, host: "dns.google", path: "", ips: googleIps))
        return l
    }

    static let servers: [Server] = {
        let id = (Bundle.main.object(forInfoDictionaryKey: "CBNextDnsId") as? String ?? "").trimmingCharacters(in: .whitespaces)
        return chain(id.range(of: "^[a-z0-9]+$", options: .regularExpression) != nil ? id : "")
    }()

    /// Tên các nhà cung cấp theo thứ tự, để hiện trong Cài đặt: "NextDNS → Cloudflare → Google".
    static var providers: String {
        var seen: [String] = []
        for s in servers { let p = String(s.label.split(separator: " ").last ?? ""); if !seen.contains(p) { seen.append(p) } }
        return seen.joined(separator: " → ")
    }

    private static let connectTimeout: TimeInterval = 2.5
    private static let readTimeout: TimeInterval = 3
    private static let downFor: TimeInterval = 120

    private static let lock = NSLock()
    /// Địa chỉ đã hỏi; addr nil = tên miền không có (NXDOMAIN).
    private static var cache: [String: (addr: IPAddr?, until: Date)] = [:]
    private static var downUntil: [String: Date] = [:]
    private static var nextIp: [String: Int] = [:]
    private static var inflight: [String: DispatchGroup] = [:]

    #if DEVTOOLS
    /// Chỉ bản thử (cổng gỡ lỗi /dnsblock): coi DNS của máy trả 0.0.0.0 cho các tên miền này, để thử Vượt chặn.
    private static var debugBlock: Set<String> = []
    /// Chỉ bản thử (cổng gỡ lỗi /dohfail): coi các máy chủ này như bị chặn, để thử chuỗi dự phòng.
    private static var debugFail: Set<String> = []
    #endif

    /// Kết quả hỏi một máy chủ (máy chủ lỗi thì ném lỗi).
    enum Answer: Equatable {
        case found(IPAddr, ttl: Int)
        /// NXDOMAIN: tên miền không có (hoặc hồ sơ DNS chặn) — không hỏi máy chủ khác.
        case noDomain
        /// Có tên miền nhưng không có bản ghi A (vd chỉ có IPv6) — để DNS của máy thử.
        case noIpv4
        /// SERVFAIL / REFUSED / gói hỏng — hỏi máy chủ sau.
        case failed
    }

    private static func noDomainError(_ name: String) -> SocksError {
        SocksError(code: 4, message: "không tìm thấy tên miền \(name)", viaProxy: false)
    }

    /// Địa chỉ IPv4 qua chuỗi DNS mã hoá (nhớ theo TTL, 30 giây – 10 phút). nil = IP / localhost, hoặc mọi máy chủ lỗi /
    /// không có bản ghi A → bên gọi dùng DNS của máy. Tên miền không có → SocksError 4. Chạy ở luồng nền (chờ mạng).
    static func resolve(_ host: String, servers: [Server]? = nil) throws -> IPAddr? {
        let name = host.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "[]")).trimmingSuffix(".")
        if IPAddr(text: name) != nil || name == "localhost" { return nil }
        lock.lock()
        if let hit = cache[name], hit.until > Date() {
            lock.unlock()
            guard let a = hit.addr else { throw noDomainError(name) }
            return a
        }
        // Nhiều kết nối cùng lúc tới một tên miền (ảnh bìa, API…): chỉ hỏi một lần, các kết nối kia chờ kết quả đó
        if let running = inflight[name] {
            lock.unlock()
            _ = running.wait(timeout: .now() + 30)
            lock.lock()
            let hit = cache[name]
            lock.unlock()
            guard let hit = hit, hit.until > Date() else { return nil }
            guard let a = hit.addr else { throw noDomainError(name) }
            return a
        }
        let group = DispatchGroup()
        group.enter()
        inflight[name] = group
        lock.unlock()
        defer {
            lock.lock()
            inflight[name] = nil
            lock.unlock()
            group.leave()
        }
        return try lookup(name, servers ?? self.servers)
    }

    private static func lookup(_ name: String, _ servers: [Server]) throws -> IPAddr? {
        for s in servers {
            lock.lock()
            let down = (downUntil[s.label] ?? .distantPast) > Date()
            lock.unlock()
            if down { continue }
            let answer: Answer
            do {
                answer = try ask(s, name)
            } catch {
                // Máy chủ bị chặn / không trả lời: nghỉ 2 phút, lần sau thử IP còn lại của nó
                lock.lock()
                downUntil[s.label] = Date().addingTimeInterval(downFor)
                nextIp[s.label, default: 0] += 1
                lock.unlock()
                Log.d("dns", "\(s.label) lỗi: \(error.localizedDescription)")
                continue
            }
            switch answer {
            case let .found(addr, ttl):
                lock.lock()
                cache[name] = (addr, Date().addingTimeInterval(TimeInterval(min(max(ttl, 30), 600))))
                lock.unlock()
                #if DEVTOOLS
                Log.d("dns", "\(name) → \(addr.text) (\(s.label))")
                #endif
                return addr
            case .noDomain:
                lock.lock()
                cache[name] = (nil, Date().addingTimeInterval(30)) // nhớ 30 giây: kết nối khác khỏi hỏi lại
                lock.unlock()
                throw noDomainError(name)
            case .noIpv4:
                return nil
            case .failed:
                continue
            }
        }
        return nil
    }

    /// Hỏi một máy chủ (một trong các IP của nó).
    static func ask(_ s: Server, _ name: String) throws -> Answer {
        lock.lock()
        #if DEVTOOLS
        let fake = debugFail.contains(s.label)
        #else
        let fake = false
        #endif
        let ip = s.ips[(nextIp[s.label] ?? 0) % s.ips.count]
        lock.unlock()
        if fake { throw SocketError(message: "giả bị chặn") }
        let id = s.kind == .dot ? Int.random(in: 0..<0x10000) : 0 // DoH khuyên id 0 (dễ lưu đệm)
        guard let reply = try exchange(s, ip: ip, query: DnsWire.query(name, id: id)) else { return .failed }
        return DnsWire.answer(reply, id: id)
    }

    private static func exchange(_ s: Server, ip: String, query: [UInt8]) throws -> [UInt8]? {
        let link = TlsLink(ip: ip, port: s.port, serverName: s.host)
        defer { link.close() }
        try link.open(timeout: connectTimeout)
        let deadline = Date().addingTimeInterval(readTimeout)
        if s.kind == .dot {
            try link.send([UInt8(query.count >> 8), UInt8(query.count & 0xFF)] + query, deadline: deadline)
            let len = try link.read(exactly: 2, deadline: deadline)
            return try link.read(exactly: Int(len[0]) << 8 | Int(len[1]), deadline: deadline)
        }
        let head = "POST \(s.path) HTTP/1.1\r\nHost: \(s.host)\r\nContent-Type: application/dns-message\r\n" +
            "Accept: application/dns-message\r\nContent-Length: \(query.count)\r\nConnection: close\r\n\r\n"
        try link.send(Array(head.utf8) + query, deadline: deadline)
        return DohHttp.body(try link.readAll(limit: 70_000, deadline: deadline))
    }

    static func clearCache() {
        lock.lock()
        cache = [:]
        downUntil = [:]
        lock.unlock()
    }

    #if DEVTOOLS
    static func setDebugBlock(_ hosts: Set<String>) {
        lock.lock()
        debugBlock = hosts
        cache = [:]
        lock.unlock()
    }

    static func setDebugFail(_ labels: Set<String>) {
        lock.lock()
        debugFail = labels
        cache = [:]
        downUntil = [:]
        lock.unlock()
    }

    static var debugFailLabels: Set<String> { lock.lock(); defer { lock.unlock() }; return debugFail }
    #endif

    // MARK: - Vượt chặn khi tắt DNS mã hoá

    /// Địa chỉ DNS chặn trả: trỏ về chính máy chứ không phải trang thật.
    static func blocked(_ a: IPAddr) -> Bool {
        if a.isV4 { return a.bytes.allSatisfy { $0 == 0 } || a.bytes[0] == 127 }
        return a.bytes.allSatisfy { $0 == 0 } || a.bytes == [UInt8](repeating: 0, count: 15) + [1]
    }

    private static func debugBlocked(_ name: String) -> Bool {
        #if DEVTOOLS
        lock.lock()
        defer { lock.unlock() }
        return debugBlock.contains(name)
        #else
        return false
        #endif
    }

    /// DNS của máy không trả lời / chỉ trả địa chỉ chặn → địa chỉ qua DNS mã hoá; nil = cứ dùng DNS của máy.
    static func unblocked(_ host: String) -> IPAddr? {
        let name = host.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "[]")).trimmingSuffix(".")
        if IPAddr(text: name) != nil || name == "localhost" { return nil }
        let device = debugBlocked(name) ? [IPAddr([0, 0, 0, 0])!] : ((try? Socks5.resolveAll(name)) ?? [])
        if device.contains(where: { !blocked($0) }) { return nil }
        return try? resolve(name)
    }

    /// Địa chỉ cho một kết nối mà máy tự phân giải (đi thẳng, hoặc proxy dùng «DNS của máy»): DNS mã hoá nếu bật, không
    /// thì Vượt chặn DNS; nil = dùng DNS của máy như thường.
    static func forConnect(_ host: String, secure: Bool, split: Bool) throws -> IPAddr? {
        if secure { return try resolve(host) }
        return split ? unblocked(host) : nil
    }
}

/// Một kết nối TLS tới máy chủ DNS: IP cố định, SNI + kiểm chứng chỉ theo tên máy chủ; dùng đồng bộ ở luồng nền.
private final class TlsLink {
    private let conn: NWConnection
    private let queue = DispatchQueue(label: "vn.chimto.doh")

    init(ip: String, port: Int, serverName: String) {
        let tls = NWProtocolTLS.Options()
        sec_protocol_options_set_tls_server_name(tls.securityProtocolOptions, serverName)
        let tcp = NWProtocolTCP.Options()
        tcp.noDelay = true
        let params = NWParameters(tls: tls, tcp: tcp)
        params.preferNoProxies = true
        conn = NWConnection(host: NWEndpoint.Host(ip), port: NWEndpoint.Port(rawValue: UInt16(port))!, using: params)
    }

    func open(timeout: TimeInterval) throws {
        let done = DispatchSemaphore(value: 0)
        let lock = NSLock()
        var result: Error??
        conn.stateUpdateHandler = { st in
            lock.lock()
            defer { lock.unlock() }
            guard result == nil else { return }
            switch st {
            case .ready: result = .some(nil); done.signal()
            case let .failed(e), let .waiting(e): result = .some(e); done.signal()
            case .cancelled: result = .some(SocketError(message: "đã huỷ")); done.signal()
            default: break
            }
        }
        conn.start(queue: queue)
        if done.wait(timeout: .now() + timeout) == .timedOut { throw SocketError(message: "hết giờ kết nối") }
        lock.lock()
        let r = result
        lock.unlock()
        if case let .some(.some(e)) = r { throw e }
    }

    func send(_ bytes: [UInt8], deadline: Date) throws {
        let done = DispatchSemaphore(value: 0)
        var err: Error?
        conn.send(content: Data(bytes), completion: .contentProcessed { e in err = e; done.signal() })
        if done.wait(timeout: .now() + max(0.1, deadline.timeIntervalSinceNow)) == .timedOut { throw SocketError(message: "hết giờ gửi") }
        if let e = err { throw e }
    }

    /// Đọc một lần: (dữ liệu, đã hết luồng).
    private func receive(max: Int, deadline: Date) throws -> ([UInt8], Bool) {
        let done = DispatchSemaphore(value: 0)
        var out: ([UInt8], Bool, Error?) = ([], false, nil)
        conn.receive(minimumIncompleteLength: 1, maximumLength: max) { data, _, complete, error in
            out = (data.map { [UInt8]($0) } ?? [], complete, error)
            done.signal()
        }
        if done.wait(timeout: .now() + Swift.max(0.1, deadline.timeIntervalSinceNow)) == .timedOut { throw SocketError(message: "hết giờ đọc") }
        if let e = out.2 { throw e }
        return (out.0, out.1)
    }

    func read(exactly n: Int, deadline: Date) throws -> [UInt8] {
        var got: [UInt8] = []
        while got.count < n {
            let (d, end) = try receive(max: n - got.count, deadline: deadline)
            got += d
            if end && got.count < n { throw SocketError(message: "máy chủ ngắt giữa chừng") }
        }
        return got
    }

    func readAll(limit: Int, deadline: Date) throws -> [UInt8] {
        var got: [UInt8] = []
        while true {
            let (d, end) = try receive(max: 16_384, deadline: deadline)
            got += d
            if got.count > limit { throw SocketError(message: "trả lời DNS quá dài") }
            if end { return got }
        }
    }

    func close() { conn.cancel() }
}

/// Gói DNS (RFC 1035): hỏi bản ghi A, đọc trả lời.
enum DnsWire {
    static func query(_ name: String, id: Int = 0) -> [UInt8] {
        var q: [UInt8] = [UInt8(id >> 8 & 0xFF), UInt8(id & 0xFF), 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0] // RD, 1 câu hỏi
        for label in name.trimmingSuffix(".").split(separator: ".") {
            let b = Array(label.utf8.prefix(63))
            q.append(UInt8(b.count))
            q += b
        }
        return q + [0, 0, 1, 0, 1] // hết tên, kiểu A, lớp IN
    }

    /// Trả lời → Answer; id khác (DoT) hay gói hỏng → failed.
    static func answer(_ b: [UInt8], id: Int) -> Doh.Answer {
        guard b.count >= 12 else { return .failed }
        func u16(_ i: Int) -> Int { Int(b[i]) << 8 | Int(b[i + 1]) }
        guard u16(0) == id else { return .failed }
        let rcode = Int(b[3] & 0x0F)
        if rcode == 3 { return .noDomain }
        guard rcode == 0 else { return .failed }
        var i = 12
        for _ in 0..<u16(4) { i = skipName(b, i) + 4 }
        var found: IPAddr?
        var ttl = Int.max
        for _ in 0..<u16(6) {
            i = skipName(b, i)
            guard i + 10 <= b.count else { return .failed }
            let type = u16(i)
            let t = Int(b[i + 4]) << 24 | Int(b[i + 5]) << 16 | Int(b[i + 6]) << 8 | Int(b[i + 7])
            let len = u16(i + 8)
            i += 10
            guard i + len <= b.count else { return .failed }
            if type == 1 && len == 4 {
                if found == nil { found = IPAddr(Array(b[i..<(i + 4)])) }
                ttl = min(ttl, t)
            }
            i += len
        }
        guard let f = found else { return .noIpv4 }
        return .found(f, ttl: ttl == Int.max ? 60 : ttl)
    }

    /// Bỏ qua một tên (có thể nén bằng con trỏ 0xC0).
    private static func skipName(_ b: [UInt8], _ from: Int) -> Int {
        var i = from
        while i < b.count {
            let l = Int(b[i])
            if l == 0 { return i + 1 }
            if l & 0xC0 == 0xC0 { return i + 2 }
            i += 1 + l
        }
        return b.count
    }
}

/// Đọc trả lời HTTP/1.1 đã nhận trọn (Connection: close): thân khi mã 200, nil nếu mã khác.
enum DohHttp {
    static func body(_ r: [UInt8]) -> [UInt8]? {
        guard r.count >= 4, let sep = (0...(r.count - 4)).first(where: { r[$0] == 13 && r[$0 + 1] == 10 && r[$0 + 2] == 13 && r[$0 + 3] == 10 })
        else { return nil }
        let head = String(decoding: r[0..<sep], as: UTF8.self)
        let first = head.components(separatedBy: "\r\n").first ?? ""
        guard first.split(separator: " ").dropFirst().first == "200" else { return nil }
        let body = Array(r[(sep + 4)...])
        let lower = head.lowercased()
        if lower.range(of: "(?m)^transfer-encoding:\\s*chunked", options: .regularExpression) != nil { return dechunk(body) }
        if let m = lower.range(of: "(?m)^content-length:\\s*\\d+", options: .regularExpression),
           let len = Int(lower[m].split(separator: ":").last?.trimmingCharacters(in: .whitespaces) ?? ""), len <= body.count {
            return Array(body[0..<len])
        }
        return body
    }

    private static func dechunk(_ b: [UInt8]) -> [UInt8] {
        var out: [UInt8] = []
        var i = 0
        while i < b.count {
            guard let eol = (i..<max(i, b.count - 1)).first(where: { b[$0] == 13 && b[$0 + 1] == 10 }) else { break }
            let size = String(decoding: b[i..<eol], as: UTF8.self).split(separator: ";").first?.trimmingCharacters(in: .whitespaces) ?? ""
            guard let n = Int(size, radix: 16), n > 0, eol + 2 + n <= b.count else { break }
            out += b[(eol + 2)..<(eol + 2 + n)]
            i = eol + 2 + n + 2
        }
        return out
    }
}
