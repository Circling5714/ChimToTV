import Darwin
import XCTest

/// Cổng SOCKS5 nội bộ (Socks5Bridge) và phía khách SOCKS5 (Socks5) — chạy thật trên 127.0.0.1 của máy test.
final class Socks5Tests: XCTestCase {
    /// Máy chủ TCP nhỏ trên 127.0.0.1: mỗi kết nối chạy serve(sock) trên luồng riêng.
    private final class Server {
        let port: Int
        private let fd: Int32

        init(_ serve: @escaping (TcpSocket) -> Void) {
            fd = socket(AF_INET, SOCK_STREAM, Int32(IPPROTO_TCP))
            var a = sockaddr_in()
            a.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
            a.sin_family = sa_family_t(AF_INET)
            a.sin_addr.s_addr = inet_addr("127.0.0.1")
            _ = withUnsafePointer(to: &a) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
            _ = Darwin.listen(fd, 16)
            var len = socklen_t(MemoryLayout<sockaddr_in>.size)
            _ = withUnsafeMutablePointer(to: &a) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.getsockname(fd, $0, &len) } }
            port = Int(UInt16(bigEndian: a.sin_port))
            let fd = self.fd
            Thread {
                while true {
                    let c = Darwin.accept(fd, nil, nil)
                    if c < 0 { return }
                    let s = TcpSocket(fd: c)
                    Thread { serve(s) }.start()
                }
            }.start()
        }

        func close() { Darwin.shutdown(fd, SHUT_RDWR); Darwin.close(fd) }
    }

    /// Trả lại đúng những gì nhận được.
    private func echoServer() -> Server {
        Server { s in
            var buf = [UInt8](repeating: 0, count: 4096)
            while let n = try? buf.withUnsafeMutableBytes({ try s.read($0.baseAddress!, 4096) }), n > 0 {
                try? s.write(Array(buf[0..<n]))
            }
            s.close()
        }
    }

    /// Proxy SOCKS5 thật có đăng nhập (RFC 1929) — đúng tên / mật khẩu thì nối thẳng tới đích.
    private func upstream(user: String, pass: String, seen: @escaping (String) -> Void = { _ in }) -> Server {
        Server { s in
            do {
                guard try s.readByte() == 5 else { return s.close() }
                let methods = try s.readFully(Int(try s.readByte()))
                guard methods.contains(2) else { try s.write([5, 0xFF]); return s.close() }
                try s.write([5, 2])
                _ = try s.readByte()
                let u = String(decoding: try s.readFully(Int(try s.readByte())), as: UTF8.self)
                let p = String(decoding: try s.readFully(Int(try s.readByte())), as: UTF8.self)
                guard u == user, p == pass else { try s.write([1, 1]); return s.close() }
                try s.write([1, 0])
                _ = try s.readFully(3)
                var host = ""
                switch try s.readByte() {
                case 1: host = IPAddr(try s.readFully(4))!.text
                case 3: host = String(decoding: try s.readFully(Int(try s.readByte())), as: UTF8.self)
                default: return s.close()
                }
                let pb = try s.readFully(2)
                let port = Int(pb[0]) << 8 | Int(pb[1])
                seen("\(host):\(port)")
                let up = try TcpSocket.connect(host: host, port: port)
                try s.write([5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
                // Chuyển tiếp hai chiều đơn giản
                Thread {
                    var b = [UInt8](repeating: 0, count: 4096)
                    while let n = try? b.withUnsafeMutableBytes({ try up.read($0.baseAddress!, 4096) }), n > 0 { try? s.write(Array(b[0..<n])) }
                    s.close()
                }.start()
                var b = [UInt8](repeating: 0, count: 4096)
                while let n = try? b.withUnsafeMutableBytes({ try s.read($0.baseAddress!, 4096) }), n > 0 { try? up.write(Array(b[0..<n])) }
                up.close()
            } catch {
                s.close()
            }
        }
    }

    private func roundTrip(_ s: TcpSocket, _ text: String) throws -> String {
        try s.write(Array(text.utf8))
        s.setReadTimeout(5)
        return String(decoding: try s.readFully(text.utf8.count), as: UTF8.self)
    }

    func testParsePastedProxy() throws {
        let c = try AppProxy.parse(host: " socks5://tom:mat:khau@1.2.3.4:1081/ ", port: "", user: "", pass: "").get()
        XCTAssertEqual(c.host, "1.2.3.4")
        XCTAssertEqual(c.port, 1081)
        XCTAssertEqual(c.user, "tom")
        XCTAssertEqual(c.pass, "mat:khau")
        let d = try AppProxy.parse(host: "proxy.example.com", port: "1080", user: "u", pass: "p").get()
        XCTAssertEqual(d.host, "proxy.example.com")
        XCTAssertEqual(d.port, 1080)
        XCTAssertEqual(d.user, "u")
        XCTAssertThrowsError(try AppProxy.parse(host: "1.2.3.4", port: "", user: "", pass: "").get())
        XCTAssertThrowsError(try AppProxy.parse(host: "", port: "1080", user: "", pass: "").get())
        XCTAssertThrowsError(try AppProxy.parse(host: "1.2.3.4", port: "70000", user: "", pass: "").get())
    }

    func testIPAddr() {
        XCTAssertEqual(IPAddr(text: "10.0.0.1")?.bytes, [10, 0, 0, 1])
        XCTAssertEqual(IPAddr(text: "::1")?.bytes.count, 16)
        XCTAssertEqual(IPAddr(text: "::1")?.text, "::1")
        XCTAssertNil(IPAddr(text: "yanhh3d.ee"))
    }

    /// Không có proxy thật: cổng nội bộ nối thẳng tới đích và chuyển dữ liệu hai chiều.
    func testBridgeDirect() throws {
        let echo = echoServer()
        defer { echo.close() }
        let bridge = try Socks5Bridge(socks: nil, siteHosts: { [] })
        defer { bridge.close() }
        let s = try Socks5.connect(SocksConfig(host: "127.0.0.1", port: bridge.port), host: "127.0.0.1", ip: nil, port: echo.port)
        defer { s.close() }
        XCTAssertEqual(try roundTrip(s, "xin chào côn bằng"), "xin chào côn bằng")
        XCTAssertEqual(bridge.statsCopy["127.0.0.1:\(echo.port)"], [0, 1, 0])
    }

    /// Có proxy thật cần đăng nhập: cổng nội bộ tự đăng nhập rồi chuyển dữ liệu; tên miền gửi nguyên cho proxy (DNS của proxy).
    func testBridgeViaUpstreamWithLogin() throws {
        let echo = echoServer()
        defer { echo.close() }
        var targets: [String] = []
        let lock = NSLock()
        let up = upstream(user: "bo", pass: "bí mật") { t in lock.lock(); targets.append(t); lock.unlock() }
        defer { up.close() }
        let bridge = try Socks5Bridge(socks: SocksConfig(host: "127.0.0.1", port: up.port, user: "bo", pass: "bí mật"), siteHosts: { [] })
        defer { bridge.close() }
        let s = try Socks5.connect(SocksConfig(host: "127.0.0.1", port: bridge.port), host: "localhost", ip: nil, port: echo.port)
        defer { s.close() }
        XCTAssertEqual(try roundTrip(s, "qua proxy"), "qua proxy")
        lock.lock()
        XCTAssertEqual(targets, ["localhost:\(echo.port)"])
        lock.unlock()
        XCTAssertNil(bridge.recentError)
    }

    /// Sai mật khẩu: bên gọi nhận lỗi, cổng nội bộ ghi lại lỗi để báo người xem.
    func testBridgeWrongPassword() throws {
        let up = upstream(user: "bo", pass: "dung")
        defer { up.close() }
        let bridge = try Socks5Bridge(socks: SocksConfig(host: "127.0.0.1", port: up.port, user: "bo", pass: "sai"), siteHosts: { [] })
        defer { bridge.close() }
        XCTAssertThrowsError(try Socks5.connect(SocksConfig(host: "127.0.0.1", port: bridge.port), host: "example.com", ip: nil, port: 443))
        XCTAssertEqual(bridge.recentError, "proxy báo sai tên đăng nhập / mật khẩu")
    }

    /// Chỉ trang phim: tên miền trang phim đi qua proxy, nơi khác đi thẳng.
    func testSitesOnly() throws {
        let echo = echoServer()
        defer { echo.close() }
        let up = upstream(user: "", pass: "")
        defer { up.close() }
        // Proxy không đăng nhập được (upstream đòi đăng nhập) → nếu "localhost" bị đưa qua proxy thì sẽ lỗi
        let bridge = try Socks5Bridge(socks: SocksConfig(host: "127.0.0.1", port: up.port, sitesOnly: true), siteHosts: { ["yanhh3d.ee"] })
        defer { bridge.close() }
        let s = try Socks5.connect(SocksConfig(host: "127.0.0.1", port: bridge.port), host: "localhost", ip: nil, port: echo.port)
        defer { s.close() }
        XCTAssertEqual(try roundTrip(s, "đi thẳng"), "đi thẳng")
        XCTAssertEqual(bridge.statsCopy["localhost:\(echo.port)"], [0, 1, 0])
    }
}
