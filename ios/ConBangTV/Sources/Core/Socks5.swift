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
            throw SocksError(code: 4, message: "iPad không phân giải được \(host)")
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
        guard !all.isEmpty else { throw SocksError(code: 4, message: "iPad không phân giải được \(host)") }
        return all
    }
}

/// Cổng SOCKS5 nội bộ (chỉ nghe 127.0.0.1, không mật khẩu) đứng giữa app và proxy thật — xem AppProxy.
/// socks = proxy của cổng này (nil = đi thẳng; đổi cài đặt thì AppProxy mở cổng mới); siteHosts() = tên miền các
/// trang phim, đọc lại mỗi kết nối (chế độ "chỉ trang phim": đích khác đi thẳng). Proxy lỗi thì trả lỗi cho bên gọi,
/// không tự đi thẳng.
final class Socks5Bridge {
    let port: Int
    let socks: SocksConfig?
    private let server: Int32
    private let siteHosts: () -> [String]
    private let lock = NSLock()
    private var open: [ObjectIdentifier: TcpSocket] = [:]
    private var closed = false

    /// Lỗi proxy gần nhất và thời điểm (ghép vào thông báo lỗi cho người xem).
    private(set) var lastError: String?
    private(set) var lastErrorAt = Date.distantPast
    /// Số kết nối đã mở theo đích ("host:cổng" → [qua proxy, đi thẳng, lỗi]) — cho cổng gỡ lỗi.
    private(set) var stats: [String: [Int]] = [:]

    init(socks: SocksConfig?, siteHosts: @escaping () -> [String]) throws {
        self.socks = socks
        self.siteHosts = siteHosts
        let fd = socket(AF_INET, SOCK_STREAM, Int32(IPPROTO_TCP))
        guard fd >= 0 else { throw SocksError(code: 1, message: "không mở được cổng nội bộ (\(errno))") }
        var a = sockaddr_in()
        a.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        a.sin_family = sa_family_t(AF_INET)
        a.sin_port = 0
        a.sin_addr.s_addr = inet_addr("127.0.0.1")
        let ok = withUnsafePointer(to: &a) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
        } == 0 && listen(fd, 64) == 0
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        guard ok, withUnsafeMutablePointer(to: &a, {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &len) }
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

    private func acceptLoop() {
        // Chờ có kết nối tối đa 0,5 s mỗi lượt để biết cổng đã bị tắt (accept() đang chờ không tự thức khi đóng ổ cắm)
        while !isClosed {
            var p = pollfd(fd: server, events: Int16(POLLIN), revents: 0)
            guard poll(&p, 1, 500) > 0 else { continue }
            let c = accept(server, nil, nil)
            if c < 0 { continue }
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
        var up: TcpSocket?
        defer {
            untrack(client)
            if let u = up { untrack(u) }
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
            let viaProxy = socks.map { !$0.sitesOnly || isSite(host) } ?? false
            do {
                up = try route(host, ip, port, viaProxy: viaProxy)
            } catch let e as SocksError {
                count("\(host):\(port)", 2)
                if e.viaProxy { noteError(e.message) }
                return try reply(client, e.code)
            }
            count("\(host):\(port)", viaProxy ? 0 : 1)
            guard let u = up, track(u) else { return }
            try reply(client, 0)
            client.setReadTimeout(0)
            relay(client, u)
        } catch {
            // bên gọi đóng giữa chừng, hoặc proxy vừa bị tắt — bỏ qua
        }
    }

    private func route(_ host: String, _ ip: IPAddr?, _ port: Int, viaProxy: Bool) throws -> TcpSocket {
        if viaProxy, let s = socks { return try Socks5.connect(s, host: host, ip: ip, port: port) }
        // Đi thẳng: lỗi DNS / kết nối như kết nối thẳng thường (không phải lỗi proxy)
        do {
            if let ip = ip { return try TcpSocket.connect(ip, port: port) }
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

    /// Chép hai chiều tới khi hết; một chiều lỗi thì đóng cả hai cho chiều kia khỏi treo.
    private func relay(_ a: TcpSocket, _ b: TcpSocket) {
        let done = DispatchSemaphore(value: 0)
        let t = Thread { [weak self] in
            self?.pipe(b, a)
            done.signal()
        }
        t.name = "chimto-socks-pipe"
        t.start()
        pipe(a, b)
        done.wait()
    }

    private func pipe(_ from: TcpSocket, _ to: TcpSocket) {
        let size = 16 * 1024
        let buf = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 8)
        defer { buf.deallocate() }
        do {
            while true {
                let n = try from.read(buf, size)
                if n == 0 { break }
                try to.write(buf, n)
            }
            // Hết dữ liệu: báo bên kia (đóng nửa ghi); chiều ngược lại chờ tối đa 60 s im lặng rồi thôi
            to.shutdownWrite()
            to.setReadTimeout(60)
        } catch {
            from.close()
            to.close()
        }
    }
}
