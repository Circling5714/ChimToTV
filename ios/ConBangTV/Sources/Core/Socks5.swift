import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// Proxy SOCKS5 thật mà người xem nhập trong Cài đặt (bản Android: SocksConfig trong Socks5.kt).
struct SocksConfig: Equatable {
    var host: String
    var port: Int
    var user = ""
    var pass = ""
    /// true: proxy phân giải tên miền (gửi tên miền); false: iPad tự phân giải rồi gửi IP cho proxy.
    var remoteDns = true
    /// true: chỉ trang phim (trong danh sách) đi qua proxy; video / ảnh CDN, GitHub… đi thẳng mạng nhà.
    var sitesOnly = false
}

/// Lỗi khi mở kết nối; code = mã trả lời SOCKS5 gửi lại bên gọi; viaProxy = lỗi ở phía proxy (để báo người xem).
struct SocksError: LocalizedError {
    let code: UInt8
    let message: String
    var viaProxy = true
    var errorDescription: String? { message }
}

/// Lỗi đọc / ghi trên một kết nối đã mở.
struct SocketError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// Địa chỉ IP (4 hoặc 16 byte).
struct IPAddr: Equatable {
    let bytes: [UInt8]
    var isV4: Bool { bytes.count == 4 }

    init?(_ bytes: [UInt8]) {
        guard bytes.count == 4 || bytes.count == 16 else { return nil }
        self.bytes = bytes
    }

    /// "1.2.3.4" / "2001:db8::1" → IPAddr; không phải IP thì nil.
    init?(text: String) {
        var v4 = in_addr()
        if inet_pton(AF_INET, text, &v4) == 1 {
            self.bytes = withUnsafeBytes(of: &v4) { Array($0) }
            return
        }
        var v6 = in6_addr()
        if inet_pton(AF_INET6, text.trimmingCharacters(in: CharacterSet(charactersIn: "[]")), &v6) == 1 {
            self.bytes = withUnsafeBytes(of: &v6) { Array($0) }
            return
        }
        return nil
    }

    var text: String {
        var buf = [CChar](repeating: 0, count: Int(INET6_ADDRSTRLEN))
        bytes.withUnsafeBytes { p in _ = inet_ntop(isV4 ? AF_INET : AF_INET6, p.baseAddress, &buf, socklen_t(buf.count)) }
        return String(cString: buf)
    }
}

/// Ổ cắm TCP chặn (POSIX), dùng như java.net.Socket của bản Android: mỗi kết nối chạy trên luồng riêng.
/// close() chỉ ngắt kết nối (đánh thức luồng đang chờ đọc); số hiệu ổ cắm trả lại hệ thống khi không còn ai giữ
/// đối tượng — để luồng khác không lỡ đọc / ghi vào một ổ cắm mới trùng số.
final class TcpSocket {
    let fd: Int32
    private let lock = NSLock()
    private var shut = false

    /// TTL (IPv4) / hop limit (IPv6) đang dùng — cho Vượt chặn mức 2. Tầng nào không áp dụng cho socket này thì nil.
    func ttl() -> (v4: Int32?, v6: Int32?) {
        var a: Int32 = 0, b: Int32 = 0
        var la = socklen_t(MemoryLayout<Int32>.size), lb = la
        let v4 = getsockopt(fd, IPPROTO_IP, IP_TTL, &a, &la) == 0 ? a : nil
        let v6 = getsockopt(fd, Int32(IPPROTO_IPV6), IPV6_UNICAST_HOPS, &b, &lb) == 0 ? b : nil
        return (v4, v6)
    }

    /// Đặt TTL / hop limit (tầng nào nil thì bỏ qua); true nếu đặt được ít nhất một tầng.
    @discardableResult
    func setTTL(v4: Int32?, v6: Int32?) -> Bool {
        var ok = false
        if var a = v4, setsockopt(fd, IPPROTO_IP, IP_TTL, &a, socklen_t(MemoryLayout<Int32>.size)) == 0 { ok = true }
        if var b = v6, setsockopt(fd, Int32(IPPROTO_IPV6), IPV6_UNICAST_HOPS, &b, socklen_t(MemoryLayout<Int32>.size)) == 0 { ok = true }
        return ok
    }

    init(fd: Int32) {
        self.fd = fd
        var on: Int32 = 1
        // Ghi vào kết nối bên kia đã đóng: báo lỗi chứ không gửi SIGPIPE làm sập app
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
        setsockopt(fd, Int32(IPPROTO_TCP), TCP_NODELAY, &on, socklen_t(MemoryLayout<Int32>.size))
    }

    deinit { Darwin.close(fd) }

    /// Kết nối TCP tới host (tên miền hoặc IP) — thử lần lượt các địa chỉ, ưu tiên IPv4.
    static func connect(host: String, port: Int, timeout: Int = 10) throws -> TcpSocket {
        let addrs = try IPAddr(text: host).map { [$0] } ?? Socks5.resolveAll(host)
        var last: Error = SocksError(code: 4, message: "không kết nối được \(host):\(port)")
        for a in addrs {
            do { return try connect(a, port: port, timeout: timeout) } catch { last = error }
        }
        throw last
    }

    static func connect(_ ip: IPAddr, port: Int, timeout: Int = 10) throws -> TcpSocket {
        let fd = socket(ip.isV4 ? AF_INET : AF_INET6, SOCK_STREAM, Int32(IPPROTO_TCP))
        guard fd >= 0 else { throw SocksError(code: 1, message: "không mở được ổ cắm (\(errno))") }
        let s = TcpSocket(fd: fd)
        let flags = fcntl(fd, F_GETFL, 0)
        _ = fcntl(fd, F_SETFL, flags | O_NONBLOCK)
        var r: Int32
        if ip.isV4 {
            var a = sockaddr_in()
            a.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
            a.sin_family = sa_family_t(AF_INET)
            a.sin_port = in_port_t(UInt16(truncatingIfNeeded: port).bigEndian)
            withUnsafeMutableBytes(of: &a.sin_addr) { $0.copyBytes(from: ip.bytes) }
            r = withUnsafePointer(to: &a) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
            }
        } else {
            var a = sockaddr_in6()
            a.sin6_len = UInt8(MemoryLayout<sockaddr_in6>.size)
            a.sin6_family = sa_family_t(AF_INET6)
            a.sin6_port = in_port_t(UInt16(truncatingIfNeeded: port).bigEndian)
            withUnsafeMutableBytes(of: &a.sin6_addr) { $0.copyBytes(from: ip.bytes) }
            r = withUnsafePointer(to: &a) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in6>.size)) }
            }
        }
        if r != 0 {
            guard errno == EINPROGRESS else { throw SocksError(code: 5, message: "\(ip.text):\(port) từ chối kết nối") }
            var p = pollfd(fd: fd, events: Int16(POLLOUT), revents: 0)
            let pr = poll(&p, 1, Int32(timeout * 1000))
            if pr <= 0 { throw SocksError(code: 6, message: "hết thời gian kết nối \(ip.text):\(port)") }
            var err: Int32 = 0
            var len = socklen_t(MemoryLayout<Int32>.size)
            getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &len)
            if err != 0 {
                throw SocksError(code: err == ECONNREFUSED ? 5 : 4, message: "không kết nối được \(ip.text):\(port) (\(String(cString: strerror(err))))")
            }
        }
        _ = fcntl(fd, F_SETFL, flags)
        return s
    }

    /// Hẹn giờ đọc, giây (0 = chờ mãi).
    func setReadTimeout(_ seconds: Int) {
        var tv = timeval(tv_sec: seconds, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
    }

    /// Đọc tối đa n byte vào buf; 0 = bên kia đã đóng.
    func read(_ buf: UnsafeMutableRawPointer, _ n: Int) throws -> Int {
        while true {
            let r = recv(fd, buf, n, 0)
            if r >= 0 { return r }
            if errno == EINTR { continue }
            throw SocketError(message: errno == EAGAIN ? "hết thời gian chờ trả lời" : "lỗi đọc (\(errno))")
        }
    }

    func readFully(_ n: Int) throws -> [UInt8] {
        var out = [UInt8](repeating: 0, count: n)
        var got = 0
        while got < n {
            let r = try out.withUnsafeMutableBytes { try read($0.baseAddress! + got, n - got) }
            if r == 0 { throw SocketError(message: "kết nối bị đóng giữa chừng") }
            got += r
        }
        return out
    }

    func readByte() throws -> UInt8 { try readFully(1)[0] }

    func write(_ bytes: [UInt8]) throws {
        if bytes.isEmpty { return }
        try bytes.withUnsafeBytes { try write($0.baseAddress!, bytes.count) }
    }

    func write(_ p: UnsafeRawPointer, _ n: Int) throws {
        var sent = 0
        while sent < n {
            let r = send(fd, p + sent, n - sent, 0)
            if r < 0 {
                if errno == EINTR { continue }
                throw SocketError(message: "lỗi ghi (\(errno))")
            }
            sent += r
        }
    }

    /// Hết dữ liệu gửi đi: báo bên kia (đóng nửa ghi).
    func shutdownWrite() { Darwin.shutdown(fd, SHUT_WR) }

    /// Chờ tối đa ms mili giây có dữ liệu để đọc (hoặc bên kia đóng); false = hết giờ.
    func waitReadable(ms: Int) -> Bool {
        var p = pollfd(fd: fd, events: Int16(POLLIN), revents: 0)
        return poll(&p, 1, Int32(ms)) > 0
    }

    func close() {
        lock.lock()
        defer { lock.unlock() }
        guard !shut else { return }
        shut = true
        Darwin.shutdown(fd, SHUT_RDWR)
    }
}

/// Phía khách SOCKS5 (RFC 1928, đăng nhập theo RFC 1929): đi qua proxy thật tới một đích (bản Android: Socks5.kt).
enum Socks5 {
    static let replies: [UInt8: String] = [
        1: "lỗi chung", 2: "proxy không cho phép", 3: "mạng không tới được", 4: "máy đích không tới được",
        5: "máy đích từ chối", 6: "hết thời gian", 7: "lệnh không hỗ trợ", 8: "kiểu địa chỉ không hỗ trợ",
    ]

    /// Kết nối tới host:port qua proxy c. ip != nil: đích đã là IP. DNS của proxy: gửi tên miền để proxy phân giải;
    /// DNS của iPad: tự phân giải ở đây rồi gửi IP.
    static func connect(_ c: SocksConfig, host: String, ip: IPAddr?, port: Int) throws -> TcpSocket {
        let s: TcpSocket
        do {
            s = try TcpSocket.connect(host: c.host, port: c.port, timeout: 10)
        } catch {
            let notFound = (error as? SocksError)?.code == 4 && IPAddr(text: c.host) == nil
            throw SocksError(code: 1, message: notFound ? "không tìm thấy proxy \(c.host)" : "không kết nối được proxy \(c.host):\(c.port)")
        }
        do {
            s.setReadTimeout(15)
            try handshake(s, c, host: host, ip: ip, port: port)
            s.setReadTimeout(0)
            return s
        } catch let e as SocksError {
            s.close()
            throw e
        } catch {
            s.close()
            throw SocksError(code: 1, message: "proxy \(c.host):\(c.port) ngắt kết nối (\(error.localizedDescription))")
        }
    }

    private static func handshake(_ s: TcpSocket, _ c: SocksConfig, host: String, ip: IPAddr?, port: Int) throws {
        let login = !c.user.isEmpty
        try s.write(login ? [5, 2, 0, 2] : [5, 1, 0])
        guard try s.readByte() == 5 else { throw SocksError(code: 1, message: "\(c.host):\(c.port) không phải proxy SOCKS5") }
        switch try s.readByte() {
        case 0: break
        case 2:
            guard login else { throw SocksError(code: 1, message: "proxy cần tên đăng nhập / mật khẩu") }
            let u = Array(c.user.utf8), p = Array(c.pass.utf8)
            guard u.count <= 255, p.count <= 255 else { throw SocksError(code: 1, message: "tên đăng nhập / mật khẩu quá dài (tối đa 255 byte)") }
            try s.write([1, UInt8(u.count)] + u + [UInt8(p.count)] + p)
            _ = try s.readByte()
            guard try s.readByte() == 0 else { throw SocksError(code: 1, message: "proxy báo sai tên đăng nhập / mật khẩu") }
        default:
            throw SocksError(code: 1, message: login ? "proxy không nhận cách đăng nhập này" : "proxy cần tên đăng nhập / mật khẩu")
        }
        try s.write([5, 1, 0] + address(c, host: host, ip: ip) + [UInt8(port >> 8 & 0xFF), UInt8(port & 0xFF)])
        guard try s.readByte() == 5 else { throw SocksError(code: 1, message: "proxy trả lời sai giao thức") }
        let rep = try s.readByte()
        _ = try s.readByte()
        switch try s.readByte() { // địa chỉ proxy gắn cho kết nối — không dùng
        case 1: _ = try s.readFully(4)
        case 3: _ = try s.readFully(Int(try s.readByte()))
        case 4: _ = try s.readFully(16)
        default: break
        }
        _ = try s.readFully(2)
        // Mã 2 khi có đăng nhập: nhiều proxy (vd 3proxy) nhận mọi mật khẩu ở bước đăng nhập rồi mới từ chối ở đây —
        // thường là sai tên đăng nhập / mật khẩu (đo trên proxy nhà của người dùng)
        if rep == 2 && login {
            throw SocksError(code: rep, message: "proxy không cho mở \(host):\(port) — thường do sai tên đăng nhập / mật khẩu (kiểm tra chữ hoa / chữ thường), hoặc proxy chặn trang này")
        }
        if rep != 0 { throw SocksError(code: rep, message: "proxy không mở được \(host):\(port) (\(replies[rep] ?? "mã \(rep)"))") }
    }

    private static func address(_ c: SocksConfig, host: String, ip: IPAddr?) throws -> [UInt8] {
        let a = try ip ?? (c.remoteDns ? nil : resolve(host))
        if let a = a { return [a.isV4 ? 1 : 4] + a.bytes }
        let h = Array(host.utf8.prefix(255))
        return [3, UInt8(h.count)] + h
    }

    /// DNS của iPad; ưu tiên IPv4 (nhiều mạng nhà không có IPv6).
    static func resolve(_ host: String) throws -> IPAddr { try resolveAll(host)[0] }

    static func resolveAll(_ host: String) throws -> [IPAddr] {
        var hints = addrinfo()
        hints.ai_family = AF_UNSPEC
        hints.ai_socktype = SOCK_STREAM
        var res: UnsafeMutablePointer<addrinfo>?
        guard getaddrinfo(host, nil, &hints, &res) == 0, let first = res else {
            throw SocksError(code: 4, message: "Máy không phân giải được \(host)")
        }
        defer { freeaddrinfo(res) }
        var v4: [IPAddr] = [], v6: [IPAddr] = []
        for p in sequence(first: first, next: { $0.pointee.ai_next }) {
            guard let sa = p.pointee.ai_addr else { continue }
            if sa.pointee.sa_family == sa_family_t(AF_INET) {
                let a = sa.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { $0.pointee.sin_addr }
                if let ip = withUnsafeBytes(of: a, { IPAddr(Array($0)) }), !v4.contains(ip) { v4.append(ip) }
            } else if sa.pointee.sa_family == sa_family_t(AF_INET6) {
                let a = sa.withMemoryRebound(to: sockaddr_in6.self, capacity: 1) { $0.pointee.sin6_addr }
                if let ip = withUnsafeBytes(of: a, { IPAddr(Array($0)) }), !v6.contains(ip) { v6.append(ip) }
            }
        }
        let all = v4 + v6
        guard !all.isEmpty else { throw SocksError(code: 4, message: "Máy không phân giải được \(host)") }
        return all
    }
}

/// Tên miền gốc, để nhớ theo trang chứ không theo từng tên con: img.phimmoi.net → phimmoi.net, www.abc.com.vn → abc.com.vn.
/// Địa chỉ IP giữ nguyên. Như baseDomain của bản TV.
func baseDomain(_ host: String) -> String {
    let h = host.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "[]")).trimmingSuffix(".")
    if IPAddr(text: h) != nil { return h }
    let labels = h.split(separator: ".").map(String.init)
    guard labels.count > 2 else { return h }
    let secondLevel: Set<String> = ["com", "net", "org", "edu", "gov", "co", "ac", "info", "biz", "name", "pro", "int", "health"]
    let three = labels[labels.count - 1].count == 2 && secondLevel.contains(labels[labels.count - 2])
    return labels.suffix(three ? 3 : 2).joined(separator: ".")
}

/// Vượt chặn tự lên Mức 2 (11/2026, như bản TV): đang Mức 1 mà trang không trả lời gói bắt tay (nhà mạng đã ghép được các
/// mẩu và chặn) thì cổng nội bộ tự thử lại theo Mức 2 (Socks5Bridge.opening); lọt thì nhớ trang (tên miền gốc) dùng Mức 2
/// trong 7 ngày — các kết nối sau đi Mức 2 ngay, khỏi chờ. Nhà mạng thôi chặn thì sau 7 ngày tự về Mức 1.
enum Level2Memo {
    static let keepMs: Int64 = 7 * 24 * 3600 * 1000
    private static let lock = NSLock()
    private static var hosts: [String: Int64] = [:]
    /// Lưu xuống máy (AppProxy gắn vào Prefs); nil = chỉ giữ trong bộ nhớ (test).
    static var save: (([String: Int64]) -> Void)?

    static func nowMs() -> Int64 { Int64(Date().timeIntervalSince1970 * 1000) }

    static func load(_ saved: [String: Int64], now: Int64 = nowMs()) {
        lock.lock()
        hosts = saved.filter { now - $0.value >= 0 && now - $0.value < keepMs }
        lock.unlock()
    }

    static func needs(_ host: String, now: Int64 = nowMs()) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard let t = hosts[baseDomain(host)] else { return false }
        return now - t >= 0 && now - t < keepMs
    }

    static func remember(_ host: String, now: Int64 = nowMs()) {
        lock.lock()
        hosts[baseDomain(host)] = now
        let copy = hosts
        lock.unlock()
        save?(copy)
    }

    static var all: [String: Int64] { lock.lock(); defer { lock.unlock() }; return hosts }

    static func clear() {
        lock.lock()
        hosts = [:]
        lock.unlock()
        save?([:])
    }
}

/// Cổng SOCKS5 nội bộ (chỉ nghe 127.0.0.1, không mật khẩu) đứng giữa app và proxy thật — xem AppProxy.
/// socks = proxy của cổng này (nil = đi thẳng; đổi cài đặt thì AppProxy mở cổng mới); siteHosts() = tên miền các
/// trang phim, đọc lại mỗi kết nối (chế độ "chỉ trang phim": đích khác đi thẳng). Proxy lỗi thì trả lỗi cho bên gọi,
/// không tự đi thẳng. split = vượt chặn nhà mạng: chia nhỏ gói bắt tay HTTPS đầu tiên của mỗi kết nối (writeSplit, opening).
final class Socks5Bridge {
    let port: Int
    let socks: SocksConfig?
    /// Máy chủ Shadowsocks (thay cho SOCKS5) — xem Shadowsocks.swift.
    let ss: SsConfig?
    let split: Bool
    /// DNS mã hoá (Doh: DoH rồi DoT, lỗi hết thì DNS của máy) cho tên miền máy tự phân giải.
    let dns: Bool
    /// Vượt chặn mức 2: byte đầu của gói bắt tay gửi với TTL 1 (đảo thứ tự) — xem writeSplit.
    let disorder: Bool
    private let server: Int32
    private let siteHosts: () -> [String]
    private let lock = NSLock()
    private var open: [ObjectIdentifier: TcpSocket] = [:]
    private var closed = false
    /// Ổ cắm nghe đã hỏng (app nằm nền lâu: iOS thu hồi ổ cắm — "defunct"); AppProxy.revive mở cổng mới.
    private var dead = false

    /// Lỗi proxy gần nhất và thời điểm (ghép vào thông báo lỗi cho người xem).
    private(set) var lastError: String?
    private(set) var lastErrorAt = Date.distantPast
    /// Số kết nối đã mở theo đích ("host:cổng" → [qua proxy, đi thẳng, lỗi]) — cho cổng gỡ lỗi.
    private(set) var stats: [String: [Int]] = [:]

    /// port = cổng muốn dùng (0 = tuỳ máy chọn); mở lại cổng cũ sau khi iOS thu hồi thì WebKit / URLSession khỏi đổi gì.
    init(socks: SocksConfig?, ss: SsConfig? = nil, split: Bool = false, disorder: Bool = false, dns: Bool = false, port wanted: Int = 0,
         siteHosts: @escaping () -> [String]) throws {
        self.socks = socks
        self.ss = ss
        self.split = split
        self.dns = dns
        self.disorder = split && disorder
        self.siteHosts = siteHosts
        let fd = socket(AF_INET, SOCK_STREAM, Int32(IPPROTO_TCP))
        guard fd >= 0 else { throw SocksError(code: 1, message: "không mở được cổng nội bộ (\(errno))") }
        var one: Int32 = 1
        _ = setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, socklen_t(MemoryLayout<Int32>.size))
        var a = sockaddr_in()
        a.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        a.sin_family = sa_family_t(AF_INET)
        a.sin_addr.s_addr = inet_addr("127.0.0.1")
        func bindTo(_ p: Int) -> Bool {
            a.sin_port = UInt16(p).bigEndian
            return withUnsafePointer(to: &a) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
            } == 0
        }
        let ok = (wanted > 0 && bindTo(wanted) || bindTo(0)) && Darwin.listen(fd, 64) == 0
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        guard ok, withUnsafeMutablePointer(to: &a, {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.getsockname(fd, $0, &len) }
        }) == 0 else {
            Darwin.close(fd)
            throw SocksError(code: 1, message: "không mở được cổng nội bộ (\(errno))")
        }
        server = fd
        port = Int(UInt16(bigEndian: a.sin_port))
        let t = Thread { [weak self] in self?.acceptLoop() }
        t.name = "chimto-socks"
        t.start()
    }

    /// Tắt proxy: đóng cổng và mọi kết nối đang mở (WebView / URLSession mở lại kết nối mới theo cài đặt mới).
    func close() {
        lock.lock()
        closed = true
        let all = Array(open.values)
        open.removeAll()
        lock.unlock()
        all.forEach { $0.close() }
    }

    private var isClosed: Bool {
        lock.lock()
        defer { lock.unlock() }
        return closed
    }

    /// Cổng gỡ lỗi: ngắt ổ cắm nghe (accept lỗi mãi như ổ cắm bị iOS thu hồi).
    func debugBreak() {
        // Thay ổ cắm nghe bằng /dev/null: cổng thôi nhận kết nối, accept báo ENOTSOCK — như ổ cắm bị thu hồi
        let null = Darwin.open("/dev/null", O_RDONLY)
        guard null >= 0 else { return }
        _ = Darwin.dup2(null, server)
        Darwin.close(null)
    }

    /// Cổng còn nhận kết nối: chưa đóng, vòng nhận chưa thấy ổ cắm hỏng, và thử kết nối vào chính cổng được (app nằm nền
    /// lâu thì iOS thu hồi ổ cắm nghe — mọi kết nối của WebKit / URLSession qua cổng báo lỗi).
    var listening: Bool {
        lock.lock()
        let bad = closed || dead
        lock.unlock()
        guard !bad, let lo = IPAddr(text: "127.0.0.1"), let s = try? TcpSocket.connect(lo, port: port, timeout: 1) else { return false }
        s.close()
        return true
    }

    private func acceptLoop() {
        // Chờ có kết nối tối đa 0,5 s mỗi lượt để biết cổng đã bị tắt (accept() đang chờ không tự thức khi đóng ổ cắm)
        var fails = 0
        while !isClosed {
            var p = pollfd(fd: server, events: Int16(POLLIN), revents: 0)
            guard poll(&p, 1, 500) > 0 else { continue }
            let c = Darwin.accept(server, nil, nil)
            if c < 0 {
                let e = errno
                // Lỗi tạm (hết tài nguyên, bị ngắt): nghỉ chút rồi nhận tiếp
                if e == EINTR || e == EAGAIN || e == EMFILE || e == ENFILE || e == ENOBUFS || e == ENOMEM {
                    fails = 0
                    usleep(20_000)
                    continue
                }
                // Ổ cắm nghe hỏng (iOS thu hồi lúc app nằm nền): poll báo có việc mà accept lỗi liền 20 lần — thôi, khỏi
                // quay vòng ăn CPU; AppProxy.revive mở lại cổng khi app quay lại màn hình
                fails += 1
                if fails >= 20 {
                    lock.lock()
                    dead = true
                    lock.unlock()
                    Log.d("Proxy", "cổng nội bộ \(port) hỏng (accept lỗi \(e)) — chờ mở lại")
                    break
                }
                usleep(50_000)
                continue
            }
            fails = 0
            let client = TcpSocket(fd: c)
            let t = Thread { [weak self] in self?.handle(client) }
            t.name = "chimto-socks-conn"
            t.start()
        }
        Darwin.close(server)
    }

    private func track(_ s: TcpSocket) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if closed { return false }
        open[ObjectIdentifier(s)] = s
        return true
    }

    private func untrack(_ s: TcpSocket) {
        lock.lock()
        open[ObjectIdentifier(s)] = nil
        lock.unlock()
        s.close()
    }

    private func count(_ key: String, _ i: Int) {
        lock.lock()
        var v = stats[key] ?? [0, 0, 0]
        v[i] += 1
        stats[key] = v
        lock.unlock()
    }

    private func handle(_ client: TcpSocket) {
        guard track(client) else { return client.close() }
        var up: Upstream?
        defer {
            untrack(client)
            if let u = up { untrack(u.tcp) }
        }
        do {
            client.setReadTimeout(15)
            // Lời chào: cổng nội bộ không đòi mật khẩu (WebKit / URLSession không đăng nhập SOCKS5 qua cổng này)
            guard try client.readByte() == 5 else { return }
            let methods = try client.readFully(Int(try client.readByte()))
            guard methods.contains(0) else { return try client.write([5, 0xFF]) }
            try client.write([5, 0])
            // Yêu cầu: VER CMD RSV ATYP ĐỊA-CHỈ CỔNG — chỉ hỗ trợ CONNECT (TCP)
            guard try client.readByte() == 5 else { return }
            let cmd = try client.readByte()
            _ = try client.readByte()
            var host: String
            var ip: IPAddr?
            switch try client.readByte() {
            case 1: ip = IPAddr(try client.readFully(4)); host = ip?.text ?? ""
            case 3:
                host = String(decoding: try client.readFully(Int(try client.readByte())), as: UTF8.self)
                ip = IPAddr(text: host)
            case 4: ip = IPAddr(try client.readFully(16)); host = ip?.text ?? ""
            default: return try reply(client, 8)
            }
            let pb = try client.readFully(2)
            let port = Int(pb[0]) << 8 | Int(pb[1])
            guard cmd == 1 else { return try reply(client, 7) }
            let viaProxy = (socks?.sitesOnly ?? ss?.sitesOnly).map { !$0 || isSite(host) } ?? false
            do {
                up = try route(host, ip, port, viaProxy: viaProxy)
            } catch let e as SocksError {
                count("\(host):\(port)", 2)
                if e.viaProxy { noteError(e.message) }
                return try reply(client, e.code)
            }
            count("\(host):\(port)", viaProxy ? 0 : 1)
            guard let u = up, track(u.tcp) else { return }
            try reply(client, 0)
            client.setReadTimeout(0)
            // Qua Shadowsocks: nhà mạng chỉ thấy dữ liệu đã mã hoá, không đọc được tên trang — khỏi chia nhỏ
            let hello: (host: String, viaProxy: Bool, disorder: Bool)? = split && !(viaProxy && ss != nil) ? (host, viaProxy, disorder) : nil
            guard let h = hello, let t = u as? TcpSocket else { return relay(client, u, hello: hello) }
            guard let opened = try opening(client, t, host: host, ip: ip, port: port, viaProxy: viaProxy) else { return }
            let ready = opened.0
            if ready !== t { up = ready }
            relay(client, ready, hello: opened.1 ? h : nil)
        } catch {
            // bên gọi đóng giữa chừng, hoặc proxy vừa bị tắt — bỏ qua
        }
    }

    private func route(_ host: String, _ ip: IPAddr?, _ port: Int, viaProxy: Bool) throws -> Upstream {
        if viaProxy, let s = socks {
            // DNS của iPad (không phải của proxy): DNS mã hoá, hoặc Vượt chặn khi DNS của máy bị chặn
            let real = try (ip == nil && !s.remoteDns ? Doh.forConnect(host, secure: dns, split: split) : nil)
            return try Socks5.connect(s, host: host, ip: ip ?? real, port: port)
        }
        if viaProxy, let s = ss {
            let real = try (ip == nil && !s.remoteDns ? Doh.forConnect(host, secure: dns, split: split) ?? Socks5.resolve(host) : nil)
            return try Shadowsocks.connect(s, host: host, ip: ip ?? real, port: port)
        }
        // Đi thẳng: lỗi DNS / kết nối như kết nối thẳng thường (không phải lỗi proxy)
        do {
            if let ip = ip { return try TcpSocket.connect(ip, port: port) }
            // DNS mã hoá (mặc định), hoặc Vượt chặn: DNS của máy chặn tên miền (trả 0.0.0.0…) → địa chỉ hỏi qua DNS mã hoá;
            // nil = DNS của máy
            if let real = try Doh.forConnect(host, secure: dns, split: split) { return try TcpSocket.connect(real, port: port) }
            return try TcpSocket.connect(host: host, port: port)
        } catch let e as SocksError {
            throw SocksError(code: e.code == 4 ? 4 : 5, message: e.message, viaProxy: false)
        }
    }

    private func isSite(_ host: String) -> Bool {
        let h = host.lowercased().trimmingSuffix(".")
        return siteHosts().contains { h == $0 || h.hasSuffix("." + $0) }
    }

    private func noteError(_ message: String) {
        lock.lock()
        lastError = message
        lastErrorAt = Date()
        lock.unlock()
    }

    /// Lỗi proxy trong 30 giây gần đây.
    var recentError: String? {
        lock.lock()
        defer { lock.unlock() }
        return Date().timeIntervalSince(lastErrorAt) < 30 ? lastError : nil
    }

    var statsCopy: [String: [Int]] {
        lock.lock()
        defer { lock.unlock() }
        return stats
    }

    private func reply(_ c: TcpSocket, _ code: UInt8) throws { try c.write([5, code, 0, 1, 0, 0, 0, 0, 0, 0]) }

    // MARK: - Vượt chặn: cắt gói đầu, tự lên Mức 2

    /// Gói đầu của bên gọi là gói bắt tay TLS (ClientHello).
    static func isTlsHello(_ b: [UInt8]) -> Bool { b.count > 5 && b[0] == 0x16 && b[1] == 0x03 }

    private static let httpMethods = ["GET ", "POST ", "HEAD ", "PUT ", "DELETE ", "OPTIONS ", "PATCH "].map { Array($0.utf8) }

    /// Gói đầu là một yêu cầu HTTP thường (không mã hoá).
    static func isHttpRequest(_ b: [UInt8]) -> Bool { httpMethods.contains { m in b.count > m.count && Array(b[0..<m.count]) == m } }

    /// Cách cắt gói đầu của bên gọi để nhà mạng không đọc được tên trang: danh sách đoạn (đầu, dài), mỗi đoạn gửi thành một gói
    /// TCP (TCP_NODELAY). Gói bắt tay TLS: byte đầu riêng, phần trước tên trang, tên trang thành từng mẩu 2 byte, phần còn lại.
    /// Yêu cầu HTTP thường (từ 11/2026 — trước chỉ cắt TLS): byte đầu riêng, tới giữa chữ «Host», phần còn lại của «Host: », giá
    /// trị Host thành từng mẩu 2 byte, phần còn lại. Gói khác: một đoạn (gửi nguyên). Như splitPlan của bản TV.
    static func splitPlan(_ b: [UInt8], host: String) -> [(start: Int, len: Int)] {
        var out: [(start: Int, len: Int)] = []
        func add(_ s: Int, _ e: Int) { if e > s { out.append((s, e - s)) } }
        func pieces(_ s: Int, _ e: Int) { var k = s; while k < e { add(k, min(k + 2, e)); k += 2 } }
        let n = b.count
        guard n > 0 else { return [] }
        return b.withUnsafeBufferPointer { p -> [(start: Int, len: Int)] in
            let base = UnsafeMutablePointer(mutating: p.baseAddress!)
            if isTlsHello(b) {
                add(0, 1)
                let name = Array(host.lowercased().utf8)
                if name.count >= 2, let s = find(name, in: base, from: 1, to: n) {
                    add(1, s)
                    pieces(s, s + name.count)
                    add(s + name.count, n)
                } else {
                    add(1, n)
                }
            } else if isHttpRequest(b) {
                add(0, 1)
                if let h = find(Array("\r\nhost:".utf8), in: base, from: 0, to: n) {
                    let k = h + 2 // chữ «Host»
                    var v = k + 5
                    while v < n && (b[v] == 0x20 || b[v] == 0x09) { v += 1 }
                    var e = v
                    while e < n && b[e] != 0x0D && b[e] != 0x0A { e += 1 }
                    add(1, k + 2)
                    add(k + 2, v)
                    pieces(v, e)
                    add(e, n)
                } else {
                    add(1, n)
                }
            } else {
                add(0, n)
            }
            return out
        }
    }

    /// Vượt chặn theo tên trang (SNI): nhà mạng đọc tên trang trong gói bắt tay HTTPS đầu tiên (TLS ClientHello) để chặn
    /// (vd phimmoir.fun — kết nối được nhưng bắt tay treo 15 giây rồi đứt). Đo trên mạng nhà: gửi nguyên hoặc cắt 2–3 mẩu lớn
    /// đều bị chặn; gửi riêng byte đầu là lọt. Gửi từng đoạn của splitPlan — mỗi lần ghi là một gói (TCP_NODELAY). Qua proxy:
    /// chờ chút sau byte đầu để proxy cũng gửi nó thành gói riêng.
    ///
    /// Mức 2 (disorder, đi thẳng): đoạn đầu (1 byte) gửi với TTL 1 — chết ở router đầu tiên (trong nhà), không tới nhà mạng;
    /// phần còn lại đi bình thường, trang giữ chờ phần thiếu; hệ điều hành thấy byte đầu chưa được xác nhận thì tự gửi lại với
    /// TTL đã trả về như cũ → byte đầu tới sau. Đo trên TV (Linux): gửi lại sau ~11 ms. Qua proxy thì không dùng. Gói không cắt
    /// được (một đoạn) thì gửi nguyên. Trả về true nếu đã gửi đảo thứ tự.
    @discardableResult
    static func writeSplit(_ to: TcpSocket, _ b: [UInt8], plan: [(start: Int, len: Int)], viaProxy: Bool, disorder: Bool) throws -> Bool {
        var applied = false
        try b.withUnsafeBytes { raw in
            let p = raw.baseAddress!
            for (i, seg) in plan.enumerated() {
                let old = i == 0 && plan.count > 1 && disorder && !viaProxy ? to.ttl() : (v4: nil, v6: nil)
                if (old.v4 != nil || old.v6 != nil) && to.setTTL(v4: old.v4 == nil ? nil : 1, v6: old.v6 == nil ? nil : 1) {
                    defer { to.setTTL(v4: old.v4, v6: old.v6) } // gói gửi lại (và mọi gói sau) đi với TTL cũ
                    try to.write(p + seg.start, seg.len) // TCP_NODELAY: hệ điều hành gửi ngay trong lúc ghi, với TTL 1
                    applied = true
                    disorderLock.lock()
                    disorderCount += 1
                    disorderLock.unlock()
                } else {
                    try to.write(p + seg.start, seg.len)
                }
                if i == 0 && plan.count > 1 && viaProxy { usleep(40_000) }
            }
        }
        return applied
    }

    /// Ở Mức 1, chờ trang trả lời gói bắt tay bao nhiêu giây thì coi là bị chặn và thử Mức 2 (test đặt ngắn).
    static var firstReplySeconds = 4
    /// Ở Mức 2 (cách cuối cùng): chờ lâu hơn lúc nhà mạng giữ kết nối treo (đo: 15 giây).
    static let level2ReplySeconds = 20

    /// Số lần tự lên Mức 2 vì trang không trả lời ở Mức 1 — cho cổng gỡ lỗi.
    private(set) static var autoLevel2 = 0

    #if DEVTOOLS
    /// Chỉ bản thử (cổng gỡ lỗi /unblockhang): ở Mức 1, gửi nguyên gói bắt tay tới các trang này — như nhà mạng ghép được các
    /// mẩu — để thử tự lên Mức 2 với chính máy soi của nhà mạng.
    static var debugWholeHello: Set<String> = []
    #endif

    /// Gói đầu của bên gọi (đi kèm Vượt chặn): đọc trọn (gói bắt tay TLS có thể dài hơn một lần đọc), cắt nhỏ rồi gửi. Đi thẳng,
    /// gói bắt tay TLS, đang Mức 1: gửi xong mà trang im quá firstReplySeconds hoặc cắt kết nối trước khi trả byte nào → kết nối
    /// mới, gửi lại theo Mức 2; lọt thì nhớ trang (Level2Memo). Bên gọi chưa nhận byte nào của trang nên gửi lại không sai.
    /// Trả về (kết nối dùng tiếp, còn gói chờ cắt — bên gọi chưa gửi gì vì giao thức để máy chủ nói trước); nil = không mở được.
    private func opening(_ client: TcpSocket, _ u: TcpSocket, host: String, ip: IPAddr?, port: Int, viaProxy: Bool) throws -> (TcpSocket, Bool)? {
        guard let b = try readFirst(client) else { return nil }
        if b.isEmpty { return (u, true) }
        let plan = Self.splitPlan(b, host: host)
        if viaProxy || !Self.isTlsHello(b) {
            try Self.writeSplit(u, b, plan: plan, viaProxy: viaProxy, disorder: disorder)
            return (u, false)
        }
        let level2 = disorder || Level2Memo.needs(host)
        var whole = false
        #if DEVTOOLS
        whole = !level2 && Self.debugWholeHello.contains(host.lowercased().trimmingSuffix("."))
        #endif
        if whole { try u.write(b) } else { try Self.writeSplit(u, b, plan: plan, viaProxy: false, disorder: level2) }
        if try firstReply(u, client, seconds: level2 ? Self.level2ReplySeconds : Self.firstReplySeconds) { return (u, false) }
        if level2 { return nil }
        // Mức 1 không lọt: kết nối mới, gửi lại theo Mức 2
        untrack(u)
        guard let u2 = (try? route(host, ip, port, viaProxy: false)) as? TcpSocket, track(u2) else { return nil }
        let applied = try Self.writeSplit(u2, b, plan: plan, viaProxy: false, disorder: true)
        let ok = try firstReply(u2, client, seconds: Self.level2ReplySeconds)
        Log.d("Proxy", "vượt chặn: \(host) không trả lời ở mức 1 → mức 2: \(ok ? "lọt" : "vẫn không được")" + (applied ? "" : " (không đặt được TTL)"))
        guard ok else {
            untrack(u2)
            return nil
        }
        if applied {
            Level2Memo.remember(host)
            Self.disorderLock.lock()
            Self.autoLevel2 += 1
            Self.disorderLock.unlock()
        }
        return (u2, false)
    }

    /// Đọc gói đầu của bên gọi: im quá 0,3 s thì [] (máy chủ nói trước); gói bắt tay TLS thì đọc trọn bản ghi đầu; nil = đã đóng.
    private func readFirst(_ client: TcpSocket) throws -> [UInt8]? {
        guard client.waitReadable(ms: 300) else { return [] }
        let size = 17 * 1024
        var b = [UInt8](repeating: 0, count: size)
        var n = try b.withUnsafeMutableBytes { try client.read($0.baseAddress!, size) }
        if n == 0 { return nil }
        if n >= 5 && b[0] == 0x16 && b[1] == 0x03 {
            let need = min(size, 5 + (Int(b[3]) << 8 | Int(b[4])))
            while n < need && client.waitReadable(ms: 2000) {
                let m = try b.withUnsafeMutableBytes { try client.read($0.baseAddress! + n, need - n) }
                if m == 0 { break }
                n += m
            }
        }
        return Array(b[0..<n])
    }

    /// Chờ trang trả lời (tối đa seconds giây); có thì chuyển ngay cho bên gọi. false = trang im / cắt kết nối.
    private func firstReply(_ u: TcpSocket, _ client: TcpSocket, seconds: Int) throws -> Bool {
        let size = 16 * 1024
        var b = [UInt8](repeating: 0, count: size)
        guard u.waitReadable(ms: seconds * 1000) else { return false }
        let m = (try? b.withUnsafeMutableBytes { try u.read($0.baseAddress!, size) }) ?? 0
        guard m > 0 else { return false }
        try client.write(Array(b[0..<m])) // lỗi ở đây: bên gọi đã đóng → handle đóng hết
        return true
    }

    /// Số gói bắt tay đã gửi đảo thứ tự (mức 2) — cho cổng gỡ lỗi.
    private(set) static var disorderCount = 0
    private static let disorderLock = NSLock()

    /// Vị trí đầu tiên của name (chữ thường) trong b[from..<to], không phân biệt hoa / thường.
    static func find(_ name: [UInt8], in b: UnsafeMutablePointer<UInt8>, from: Int, to: Int) -> Int? {
        guard to - from >= name.count else { return nil }
        var s = from
        while s + name.count <= to {
            var k = 0
            while k < name.count {
                let c = b[s + k]
                if (c >= 0x41 && c <= 0x5A ? c | 0x20 : c) != name[k] { break }
                k += 1
            }
            if k == name.count { return s }
            s += 1
        }
        return nil
    }

    /// Chép hai chiều tới khi hết; một chiều lỗi thì đóng cả hai cho chiều kia khỏi treo. hello != nil: gói đầu tiên
    /// từ bên gọi (a → b) là gói bắt tay HTTPS thì chia nhỏ (vượt chặn nhà mạng).
    private func relay(_ a: TcpSocket, _ b: Upstream, hello: (host: String, viaProxy: Bool, disorder: Bool)?) {
        let done = DispatchSemaphore(value: 0)
        let t = Thread { [weak self] in
            self?.pipe(b, a, hello: nil)
            done.signal()
        }
        t.name = "chimto-socks-pipe"
        t.start()
        pipe(a, b, hello: hello)
        done.wait()
    }

    private func pipe(_ from: Upstream, _ to: Upstream, hello: (host: String, viaProxy: Bool, disorder: Bool)?) {
        let size = 16 * 1024
        let buf = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 8)
        defer { buf.deallocate() }
        do {
            var first = hello
            // Shadowsocks: phần đầu gửi cùng dữ liệu đầu tiên của bên gọi (HTTPS: gói bắt tay). Bên gọi im quá 0,3 s (giao
            // thức máy chủ nói trước) thì gửi phần đầu trước để máy chủ mở kết nối tới đích
            if let ss = to as? SsConnection, !ss.started, !from.tcp.waitReadable(ms: 300) { try ss.start() }
            while true {
                let n = try from.read(buf, size)
                if n == 0 { break }
                if let h = first, let t = to as? TcpSocket {
                    first = nil
                    let b = Array(UnsafeBufferPointer(start: buf.assumingMemoryBound(to: UInt8.self), count: n))
                    try Self.writeSplit(t, b, plan: Self.splitPlan(b, host: h.host), viaProxy: h.viaProxy, disorder: h.disorder)
                    continue
                }
                try to.write(buf, n)
            }
            // Hết dữ liệu: báo bên kia (đóng nửa ghi); chiều ngược lại chờ tối đa 60 s im lặng rồi thôi
            to.tcp.shutdownWrite()
            to.tcp.setReadTimeout(60)
        } catch {
            if let e = error as? SsError { noteError(e.message) }
            from.tcp.close()
            to.tcp.close()
        }
    }
}
