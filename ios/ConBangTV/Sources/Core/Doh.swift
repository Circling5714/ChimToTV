import Foundation
import Network

/// DNS mã hoá (từ 1.11 bật sẵn; bản TV: Doh.kt, Windows: Doh.cs): hỏi địa chỉ tên miền qua DoH (RFC 8484) rồi DoT
/// (RFC 7858), mỗi giao thức lần lượt NextDNS (hồ sơ riêng của app — mã lấy từ secret NEXTDNS_ID của repo lúc dựng trên
/// GitHub, ghi vào Info.plist «CBNextDnsId», không có trong code) → Cloudflare → Google; lỗi hết thì cổng nội bộ dùng DNS
/// của máy — app không bao giờ hỏng vì DNS mã hoá bị chặn. Máy chủ gọi bằng IP, kiểm chứng chỉ theo tên máy chủ, đi thẳng
/// (Network.framework, không qua proxy / cổng nội bộ). Từ 1.11.1:
/// - NextDNS dùng máy chủ gần nhất: IP anycast từ mạng Viettel bị đưa sang Hồng Kông → lần đầu hỏi chính NextDNS tên
///   «dns.nextdns.io» (NextDNS trả máy chủ gần IP của mạng này nhất), không được thì DNS của máy; nhớ 6 giờ; anycast đứng sau.
/// - Giữ kết nối TLS tới máy chủ DNS để dùng lại (DoT; DoH bằng HTTP/1.1 keep-alive): mở kết nối mới ~140–220 ms, dùng lại
///   ~20–40 ms. Kết nối nghỉ quá 90 giây thì bỏ; kết nối cũ bị máy chủ đóng thì mở kết nối mới ngay.
/// Máy chủ lỗi thì nghỉ 2 phút. Trả lời «không có tên miền» (NXDOMAIN — vd hồ sơ NextDNS chặn) là trả lời thật: không hỏi
/// máy chủ sau.
///
/// Tắt DNS mã hoá mà đang Vượt chặn: DNS của máy không trả lời / trả địa chỉ chặn (0.0.0.0, ::…) thì vẫn hỏi lại qua chuỗi
/// này (unblocked) — vd ảnh bìa CLB Phim Xưa (clbphimxua.com) bị bộ lọc DNS trong nhà chặn.
enum Doh {
    enum Kind { case doh, dot }

    /// steer = NextDNS: IP lấy theo máy chủ gần nhất (nextPops) rồi mới tới ips cố định (anycast).
    struct Server {
        let label: String
        let kind: Kind
        let host: String
        let path: String
        let ips: [String]
        var steer = false
        var port: Int { kind == .dot ? 853 : 443 }
    }

    private static let nextIps = ["45.90.28.0", "45.90.30.0"]
    private static let googleIps = ["8.8.8.8", "8.8.4.4"]

    /// Thứ tự hỏi: DoH NextDNS → Cloudflare → Google, rồi DoT cũng vậy (không có mã NextDNS thì bỏ NextDNS).
    static func chain(_ nextId: String) -> [Server] {
        let id = nextId.trimmingCharacters(in: .whitespacesAndNewlines)
        var l: [Server] = []
        if !id.isEmpty { l.append(Server(label: "DoH NextDNS", kind: .doh, host: "dns.nextdns.io", path: "/\(id)", ips: nextIps, steer: true)) }
        l.append(Server(label: "DoH Cloudflare", kind: .doh, host: "dns.cloudflare.com", path: "/dns-query", ips: ["104.16.248.249", "104.16.249.249"]))
        l.append(Server(label: "DoH Google", kind: .doh, host: "dns.google", path: "/dns-query", ips: googleIps))
        if !id.isEmpty { l.append(Server(label: "DoT NextDNS", kind: .dot, host: "\(id).dns.nextdns.io", path: "", ips: nextIps, steer: true)) }
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
    private static let idleFor: TimeInterval = 90
    private static let maxIdle = 4
    private static let popsFor: TimeInterval = 6 * 3600

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
        /// all = mọi bản ghi A (theo thứ tự máy chủ trả); địa chỉ đầu = cái dùng.
        case found(IPAddr, ttl: Int, all: [IPAddr])
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
            let index = nextIp[s.label] ?? 0
            lock.unlock()
            if down { continue }
            let ips = ipsFor(s)
            let ip = ips[index % ips.count]
            let t0 = Date()
            let answer: Answer
            do {
                answer = try ask(s, ip: ip, name)
            } catch {
                // Máy chủ bị chặn / không trả lời: nghỉ 2 phút, lần sau thử IP kế của nó
                lock.lock()
                downUntil[s.label] = Date().addingTimeInterval(downFor)
                nextIp[s.label, default: 0] += 1
                lock.unlock()
                Log.d("dns", "\(s.label) · \(ip) lỗi: \(error.localizedDescription)")
                continue
            }
            switch answer {
            case let .found(addr, ttl, _):
                lock.lock()
                cache[name] = (addr, Date().addingTimeInterval(TimeInterval(min(max(ttl, 30), 600))))
                lock.unlock()
                #if DEVTOOLS
                Log.d("dns", "\(name) → \(addr.text) (\(s.label) · \(ip), \(Int(Date().timeIntervalSince(t0) * 1000)) ms)")
                #else
                _ = t0
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

    // MARK: - NextDNS: máy chủ gần nhất

    private static var pops: [String] = []
    private static var popsUntil = Date.distantPast
    private static let popsLock = NSLock()

    /// IP để gọi máy chủ s: NextDNS = máy chủ gần nhất rồi anycast; còn lại = IP cố định.
    static func ipsFor(_ s: Server) -> [String] {
        guard s.steer else { return s.ips }
        var out: [String] = []
        for ip in nextPops() + s.ips where !out.contains(ip) { out.append(ip) }
        return out
    }

    private static func nextPops() -> [String] {
        popsLock.lock()
        defer { popsLock.unlock() }
        if Date() < popsUntil { return pops }
        let found = discoverPops()
        pops = found
        popsUntil = Date().addingTimeInterval(found.isEmpty ? 600 : popsFor)
        Log.d("dns", "NextDNS gần nhất: " + (found.isEmpty ? "(không tìm được — dùng anycast)" : found.joined(separator: ", ")))
        return found
    }

    /// Hỏi chính NextDNS (anycast, đã mã hoá) tên «dns.nextdns.io» — NextDNS trả máy chủ gần IP của mạng này; không được thì DNS máy.
    private static func discoverPops() -> [String] {
        guard let any = servers.first(where: { $0.steer && $0.kind == .doh }) ?? servers.first(where: { $0.steer }) else { return [] }
        for ip in any.ips {
            if case let .found(_, _, all)? = try? ask(any, ip: ip, "dns.nextdns.io") {
                var out: [String] = []
                for a in all where a.isV4 && !blocked(a) && !out.contains(a.text) { out.append(a.text) }
                return out
            }
        }
        var out: [String] = []
        for a in (try? Socks5.resolveAll("dns.nextdns.io")) ?? [] where a.isV4 && !blocked(a) && !out.contains(a.text) { out.append(a.text) }
        return out
    }

    /// Hỏi máy chủ s ở địa chỉ ip.
    static func ask(_ s: Server, ip: String, _ name: String) throws -> Answer {
        #if DEVTOOLS
        lock.lock()
        let fake = debugFail.contains(s.label)
        lock.unlock()
        if fake { throw SocketError(message: "giả bị chặn") }
        #endif
        let id = s.kind == .dot ? Int.random(in: 0..<0x10000) : 0 // DoH khuyên id 0 (dễ lưu đệm)
        guard let reply = try exchange(s, ip: ip, query: DnsWire.query(name, id: id)) else { return .failed }
        return DnsWire.answer(reply, id: id)
    }

    // MARK: - Kết nối dùng lại

    private static var idle: [String: [TlsLink]] = [:]

    private static func take(_ key: String) -> TlsLink? {
        lock.lock()
        defer { lock.unlock() }
        while var list = idle[key], !list.isEmpty {
            let l = list.removeLast()
            idle[key] = list
            if Date().timeIntervalSince(l.lastUsed) < idleFor && l.usable { return l }
            l.close()
        }
        return nil
    }

    private static func give(_ l: TlsLink) {
        l.lastUsed = Date()
        lock.lock()
        defer { lock.unlock() }
        var list = idle[l.key] ?? []
        if list.count >= maxIdle { l.close() } else { list.append(l); idle[l.key] = list }
    }

    /// Một lần hỏi: dùng lại kết nối đang nghỉ (hỏng thì mở mới ngay), không có thì mở kết nối mới.
    private static func exchange(_ s: Server, ip: String, query: [UInt8]) throws -> [UInt8]? {
        let key = "\(s.label)|\(ip)"
        if let old = take(key) {
            do {
                return try roundTrip(s, old, query)
            } catch {
                // máy chủ đã đóng kết nối cũ — không phải máy chủ lỗi, mở kết nối mới bên dưới
            }
        }
        let link = TlsLink(key: key, ip: ip, port: s.port, serverName: s.host)
        do {
            try link.open(timeout: connectTimeout)
        } catch {
            link.close()
            throw error
        }
        return try roundTrip(s, link, query)
    }

    /// Gửi một câu hỏi trên kết nối, đọc đúng một trả lời rồi trả kết nối về chỗ nghỉ (lỗi / máy chủ báo đóng thì đóng).
    private static func roundTrip(_ s: Server, _ link: TlsLink, _ query: [UInt8]) throws -> [UInt8]? {
        var keep = false
        defer { if keep { give(link) } else { link.close() } }
        link.deadline = Date().addingTimeInterval(readTimeout)
        if s.kind == .dot {
            try link.send([UInt8(query.count >> 8), UInt8(query.count & 0xFF)] + query)
            let len = try link.read(exactly: 2)
            let reply = try link.read(exactly: Int(len[0]) << 8 | Int(len[1]))
            keep = true
            return reply
        }
        let head = "POST \(s.path) HTTP/1.1\r\nHost: \(s.host)\r\nContent-Type: application/dns-message\r\n" +
            "Accept: application/dns-message\r\nContent-Length: \(query.count)\r\n\r\n"
        try link.send(Array(head.utf8) + query)
        let r = try DohHttp.read(link)
        keep = r.keepAlive
        return r.status == 200 ? r.body : nil
    }

    static func clearCache() {
        lock.lock()
        cache = [:]
        downUntil = [:]
        lock.unlock()
    }

    #if DEVTOOLS
    private static var debugBlockAddr = IPAddr([0, 0, 0, 0])!

    static func setDebugBlock(_ hosts: Set<String>, ip: IPAddr? = nil) {
        lock.lock()
        debugBlock = hosts
        debugBlockAddr = ip ?? IPAddr([0, 0, 0, 0])!
        cache = [:]
        lock.unlock()
        forgetAnswers()
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

    /// DNS của máy không trả lời / trả địa chỉ chặn / trả địa chỉ đáng ngờ → địa chỉ qua DNS mã hoá; nil = cứ dùng DNS của máy.
    /// Xét mọi địa chỉ trong câu trả lời (có một địa chỉ chặn là cả câu trả lời không dùng được — đo DNS Viettel 13/11/2026: tên
    /// miền bị chặn trả 127.0.0.1).
    static func unblocked(_ host: String) -> IPAddr? {
        let name = host.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "[]")).trimmingSuffix(".")
        if IPAddr(text: name) != nil || name == "localhost" { return nil }
        let device = debugBlocked(name) ? [debugBlockIP] : ((try? Socks5.resolveAll(name)) ?? [])
        if !device.isEmpty && !device.contains(where: blocked) && !sharedAnswer(name, device) { return nil }
        return try? resolve(name)
    }

    private static var answeredFor: [String: String] = [:] // địa chỉ → tên miền gốc đầu tiên nhận địa chỉ đó
    private static var suspects: Set<String> = [] // địa chỉ đã thấy trả cho hai tên miền gốc khác nhau

    /// Trang cảnh báo của nhà mạng: DNS chặn trả cùng một địa chỉ (máy chủ trang cảnh báo) cho mọi trang bị chặn. Câu trả lời có
    /// địa chỉ đã trả cho một tên miền gốc khác thì đáng ngờ → Vượt chặn hỏi lại DNS mã hoá cho chắc; từ đó mọi trang nhận địa
    /// chỉ ấy (kể cả trang đầu tiên) cũng vậy. Hai trang dùng chung CDN cũng trùng — DNS mã hoá trả địa chỉ đúng, không hại gì.
    static func sharedAnswer(_ host: String, _ all: [IPAddr]) -> Bool {
        let base = baseDomain(host)
        lock.lock()
        defer { lock.unlock() }
        if answeredFor.count > 512 { answeredFor = [:] }
        var shared = false
        for a in all {
            let k = a.text
            if suspects.contains(k) {
                shared = true
            } else if let prev = answeredFor[k] {
                if prev != base {
                    suspects.insert(k)
                    shared = true
                }
            } else {
                answeredFor[k] = base
            }
        }
        return shared
    }

    static func forgetAnswers() {
        lock.lock()
        answeredFor = [:]
        suspects = []
        lock.unlock()
    }

    /// Chỉ bản thử (/dnsblock?ip=…): địa chỉ DNS của máy giả trả cho các tên miền bị giả chặn (mặc định 0.0.0.0).
    private static var debugBlockIP: IPAddr {
        #if DEVTOOLS
        lock.lock()
        defer { lock.unlock() }
        return debugBlockAddr
        #else
        return IPAddr([0, 0, 0, 0])!
        #endif
    }

    /// Địa chỉ cho một kết nối mà máy tự phân giải (đi thẳng, hoặc proxy dùng «DNS của máy»): DNS mã hoá nếu bật, không
    /// thì Vượt chặn DNS; nil = dùng DNS của máy như thường.
    static func forConnect(_ host: String, secure: Bool, split: Bool) throws -> IPAddr? {
        if secure { return try resolve(host) }
        return split ? unblocked(host) : nil
    }
}

/// Nguồn byte để đọc trả lời HTTP (kết nối TLS, hoặc mảng byte khi test).
protocol ByteSource: AnyObject {
    func read(exactly n: Int) throws -> [UInt8]
    func readLine() throws -> String
    func readToEnd(limit: Int) throws -> [UInt8]
}

/// Mảng byte đã nhận trọn (test).
final class ArraySource: ByteSource {
    private var b: [UInt8]
    init(_ b: [UInt8]) { self.b = b }
    func read(exactly n: Int) throws -> [UInt8] {
        guard b.count >= n else { throw SocketError(message: "hết dữ liệu") }
        defer { b.removeFirst(n) }
        return Array(b[0..<n])
    }
    func readLine() throws -> String {
        guard let i = b.firstIndex(of: 10) else { throw SocketError(message: "hết dữ liệu") }
        defer { b.removeFirst(i + 1) }
        return String(decoding: b[0..<i], as: UTF8.self).trimmingSuffix("\r")
    }
    func readToEnd(limit: Int) throws -> [UInt8] {
        defer { b = [] }
        return b
    }
}

/// Một kết nối TLS tới máy chủ DNS: IP cố định, SNI + kiểm chứng chỉ theo tên máy chủ; dùng đồng bộ ở luồng nền, giữ
/// được để dùng lại. deadline = hạn của lần hỏi đang làm.
final class TlsLink: ByteSource {
    let key: String
    var lastUsed = Date()
    var deadline = Date().addingTimeInterval(3)
    private let conn: NWConnection
    private let queue = DispatchQueue(label: "vn.chimto.doh")
    private var buf: [UInt8] = []
    private var ended = false

    init(key: String, ip: String, port: Int, serverName: String) {
        self.key = key
        let tls = NWProtocolTLS.Options()
        sec_protocol_options_set_tls_server_name(tls.securityProtocolOptions, serverName)
        let tcp = NWProtocolTCP.Options()
        tcp.noDelay = true
        let params = NWParameters(tls: tls, tcp: tcp)
        params.preferNoProxies = true
        conn = NWConnection(host: NWEndpoint.Host(ip), port: NWEndpoint.Port(rawValue: UInt16(port))!, using: params)
    }

    /// Còn dùng được (máy chủ chưa đóng, chưa đọc tới hết luồng).
    var usable: Bool { !ended && conn.state == .ready }

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

    func send(_ bytes: [UInt8]) throws {
        let done = DispatchSemaphore(value: 0)
        var err: Error?
        conn.send(content: Data(bytes), completion: .contentProcessed { e in err = e; done.signal() })
        if done.wait(timeout: .now() + max(0.1, deadline.timeIntervalSinceNow)) == .timedOut { throw SocketError(message: "hết giờ gửi") }
        if let e = err { throw e }
    }

    /// Nhận thêm vào bộ đệm; false nếu máy chủ đã đóng.
    private func fill() throws -> Bool {
        if ended { return false }
        let done = DispatchSemaphore(value: 0)
        var out: ([UInt8], Bool, Error?) = ([], false, nil)
        conn.receive(minimumIncompleteLength: 1, maximumLength: 16_384) { data, _, complete, error in
            out = (data.map { [UInt8]($0) } ?? [], complete, error)
            done.signal()
        }
        if done.wait(timeout: .now() + max(0.1, deadline.timeIntervalSinceNow)) == .timedOut { throw SocketError(message: "hết giờ đọc") }
        if let e = out.2 { throw e }
        buf += out.0
        if out.1 { ended = true }
        return !out.0.isEmpty || !out.1
    }

    func read(exactly n: Int) throws -> [UInt8] {
        while buf.count < n {
            if try !fill() { throw SocketError(message: "máy chủ ngắt giữa chừng") }
        }
        defer { buf.removeFirst(n) }
        return Array(buf[0..<n])
    }

    func readLine() throws -> String {
        while true {
            if let i = buf.firstIndex(of: 10) {
                defer { buf.removeFirst(i + 1) }
                return String(decoding: buf[0..<i], as: UTF8.self).trimmingSuffix("\r")
            }
            if buf.count > 16_384 { throw SocketError(message: "dòng quá dài") }
            if try !fill() { throw SocketError(message: "máy chủ ngắt giữa chừng") }
        }
    }

    func readToEnd(limit: Int) throws -> [UInt8] {
        while try fill() {
            if buf.count > limit { throw SocketError(message: "trả lời DNS quá dài") }
        }
        defer { buf = [] }
        return buf
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
        var all: [IPAddr] = []
        var ttl = Int.max
        for _ in 0..<u16(6) {
            i = skipName(b, i)
            guard i + 10 <= b.count else { return .failed }
            let type = u16(i)
            let t = Int(b[i + 4]) << 24 | Int(b[i + 5]) << 16 | Int(b[i + 6]) << 8 | Int(b[i + 7])
            let len = u16(i + 8)
            i += 10
            guard i + len <= b.count else { return .failed }
            if type == 1 && len == 4, let a = IPAddr(Array(b[i..<(i + 4)])) {
                all.append(a)
                ttl = min(ttl, t)
            }
            i += len
        }
        guard let f = all.first else { return .noIpv4 }
        return .found(f, ttl: ttl == Int.max ? 60 : ttl, all: all)
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

/// Trả lời HTTP/1.1 của máy chủ DoH: đọc đúng một trả lời trên kết nối (để dùng lại kết nối).
enum DohHttp {
    struct Reply {
        let status: Int
        let body: [UInt8]
        let keepAlive: Bool
    }

    /// Đọc dòng trạng thái + đầu trả lời rồi thân theo Content-Length / chunked; không có độ dài thì đọc tới khi máy chủ đóng.
    static func read(_ src: ByteSource) throws -> Reply {
        let first = try src.readLine()
        guard let status = Int(first.split(separator: " ").dropFirst().first ?? "") else { throw SocketError(message: "trả lời HTTP sai") }
        var headers: [String: String] = [:]
        while true {
            let l = try src.readLine()
            if l.isEmpty { break }
            if let c = l.firstIndex(of: ":") {
                headers[l[..<c].lowercased()] = l[l.index(after: c)...].trimmingCharacters(in: .whitespaces).lowercased()
            }
            if headers.count > 100 { throw SocketError(message: "đầu trả lời quá dài") }
        }
        let keep = headers["connection"] != "close"
        if headers["transfer-encoding"]?.contains("chunked") == true {
            var out: [UInt8] = []
            while true {
                let size = try src.readLine().split(separator: ";").first.map { $0.trimmingCharacters(in: .whitespaces) } ?? ""
                guard let n = Int(size, radix: 16) else { throw SocketError(message: "chunk sai") }
                if n == 0 {
                    while try !src.readLine().isEmpty {} // bỏ phần đuôi (trailer)
                    return Reply(status: status, body: out, keepAlive: keep)
                }
                guard out.count + n <= 70_000 else { throw SocketError(message: "trả lời DNS quá dài") }
                out += try src.read(exactly: n)
                _ = try src.readLine() // CRLF sau mỗi khúc
            }
        }
        guard let len = headers["content-length"].flatMap({ Int($0) }) else {
            return Reply(status: status, body: try src.readToEnd(limit: 70_000), keepAlive: false)
        }
        guard len <= 70_000 else { throw SocketError(message: "trả lời DNS quá dài") }
        return Reply(status: status, body: try src.read(exactly: len), keepAlive: keep)
    }

    /// Trả lời đã nhận trọn: thân khi mã 200, nil nếu mã khác / hỏng (để test).
    static func body(_ r: [UInt8]) -> [UInt8]? {
        guard let reply = try? read(ArraySource(r)), reply.status == 200 else { return nil }
        return reply.body
    }
}
