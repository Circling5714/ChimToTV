import Darwin
import XCTest

/// Cổng SOCKS5 nội bộ (Socks5Bridge) và phía khách SOCKS5 (Socks5) — chạy thật trên 127.0.0.1 của máy test.
final class Socks5Tests: XCTestCase {
    /// Máy chủ TCP nhỏ trên 127.0.0.1: mỗi kết nối chạy serve(sock) trên luồng riêng.
    private final class Server {
        let port: Int
        private let fd: Int32

        init(_ serve: @escaping (TcpSocket) -> Void) {
            // Dùng biến cục bộ: khối lệnh (closure) không được nhắc tới self trước khi gán đủ thuộc tính
            let sock = socket(AF_INET, SOCK_STREAM, Int32(IPPROTO_TCP))
            var a = sockaddr_in()
            a.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
            a.sin_family = sa_family_t(AF_INET)
            a.sin_addr.s_addr = inet_addr("127.0.0.1")
            _ = withUnsafePointer(to: &a) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(sock, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
            _ = Darwin.listen(sock, 16)
            var len = socklen_t(MemoryLayout<sockaddr_in>.size)
            _ = withUnsafeMutablePointer(to: &a) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.getsockname(sock, $0, &len) } }
            fd = sock
            port = Int(UInt16(bigEndian: a.sin_port))
            Thread {
                while true {
                    let c = Darwin.accept(sock, nil, nil)
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

    /// Vượt chặn: gói bắt tay TLS bị chia nhỏ khi qua cổng nội bộ nhưng tới đích vẫn đủ và đúng thứ tự.
    func testSplitHelloKeepsBytes() throws {
        let echo = echoServer()
        defer { echo.close() }
        let bridge = try Socks5Bridge(socks: nil, split: true, siteHosts: { [] })
        defer { bridge.close() }
        let s = try Socks5.connect(SocksConfig(host: "127.0.0.1", port: bridge.port), host: "LocalHost", ip: nil, port: echo.port)
        defer { s.close() }
        // Giả gói ClientHello: 16 03 01 … có tên trang ở giữa
        let hello: [UInt8] = [0x16, 0x03, 0x01, 0x00, 0x40] + Array(repeating: 0x2A, count: 30) + Array("localhost".utf8) + Array(repeating: 0x2B, count: 25)
        try s.write(hello)
        s.setReadTimeout(5)
        XCTAssertEqual(try s.readFully(hello.count), hello)
        XCTAssertEqual(try roundTrip(s, "sau đó chuyển bình thường"), "sau đó chuyển bình thường")
    }

    /// Ghép các đoạn của splitPlan thành chữ, ngăn bằng «|» — cùng ca với Socks5BridgeTest.kt / SiteTests.cs.
    private func plan(_ text: String, _ host: String) -> String {
        let b = Array(text.utf8)
        return Socks5Bridge.splitPlan(b, host: host).map { String(decoding: b[$0.start..<($0.start + $0.len)], as: UTF8.self) }.joined(separator: "|")
    }

    func testHttpRequestIsSplitAroundHost() {
        let req = "GET /phim HTTP/1.1\r\nHost: phim.test\r\nAccept: */*\r\n\r\n"
        XCTAssertEqual(plan(req, "phim.test"), "G|ET /phim HTTP/1.1\r\nHo|st: |ph|im|.t|es|t|\r\nAccept: */*\r\n\r\n")
        XCTAssertEqual(plan("SSH-2.0-x\r\n", "phim.test"), "SSH-2.0-x\r\n")
    }

    func testTlsHelloIsSplitAroundName() {
        var hello = [UInt8](repeating: 0, count: 80)
        hello[0] = 0x16; hello[1] = 0x03; hello[2] = 0x01; hello[4] = 75
        hello.replaceSubrange(40..<49, with: Array("phim.test".utf8))
        let p = Socks5Bridge.splitPlan(hello, host: "phim.test")
        XCTAssertEqual(p.first?.start, 0)
        XCTAssertEqual(p.first?.len, 1)
        XCTAssertEqual(p.filter { (40...48).contains($0.start) }.map { $0.len }, [2, 2, 2, 2, 1])
        XCTAssertEqual(p.reduce(0) { $0 + $1.len }, hello.count)
    }

    func testBaseDomainsAndLevel2Memo() {
        XCTAssertEqual(baseDomain("img.phimmoi.net"), "phimmoi.net")
        XCTAssertEqual(baseDomain("www.abc.com.vn"), "abc.com.vn")
        XCTAssertEqual(baseDomain("1.2.3.4"), "1.2.3.4")
        let now: Int64 = 1_000_000_000_000
        Level2Memo.load(["phimmoi.test": now - Level2Memo.keepMs - 1, "kkphim.test": now - 1000], now: now)
        XCTAssertFalse(Level2Memo.needs("phimmoi.test", now: now)) // quá 7 ngày: về Mức 1
        XCTAssertTrue(Level2Memo.needs("www.kkphim.test", now: now)) // theo tên miền gốc
        Level2Memo.clear()
    }

    /// Trang cảnh báo của nhà mạng: cùng một địa chỉ cho hai tên miền gốc khác nhau → đáng ngờ, kể cả với trang đầu tiên.
    func testSameAnswerForDifferentSitesIsSuspicious() {
        Doh.forgetAnswers()
        let warn = [IPAddr(text: "203.0.113.9")!]
        XCTAssertFalse(Doh.sharedAnswer("phimmoi.test", warn))
        XCTAssertFalse(Doh.sharedAnswer("img.phimmoi.test", warn))
        XCTAssertTrue(Doh.sharedAnswer("www.kkphim.test", warn))
        XCTAssertTrue(Doh.sharedAnswer("phimmoi.test", warn))
        XCTAssertFalse(Doh.sharedAnswer("khac.test", [IPAddr(text: "198.51.100.7")!]))
        Doh.forgetAnswers()
    }

    /// Máy đích giả nhà mạng chặn ở Mức 1: kết nối đầu nhận gói bắt tay nhưng không trả lời; kết nối sau trả lời ngay. Cổng nội
    /// bộ phải tự mở lại và gửi lại gói đó — bên gọi nhận được trả lời như không có gì.
    func testHangingAtLevel1RetriesAtLevel2() throws {
        var hello = [UInt8](repeating: 0, count: 80)
        hello[0] = 0x16; hello[1] = 0x03; hello[2] = 0x01; hello[4] = 75
        hello.replaceSubrange(40..<49, with: Array("localhost".utf8))
        let lock = NSLock()
        var accepted = 0
        var seen: [Int] = []
        let target = Server { s in
            lock.lock(); accepted += 1; let first = accepted == 1; lock.unlock()
            s.setReadTimeout(5)
            guard let got = try? s.readFully(80) else { return s.close() }
            lock.lock(); seen.append(got.count); lock.unlock()
            if first { usleep(3_000_000) } else { try? s.write(Array("hello-back".utf8)); usleep(200_000) }
            s.close()
        }
        defer { target.close() }
        Level2Memo.clear()
        let oldWait = Socks5Bridge.firstReplySeconds
        Socks5Bridge.firstReplySeconds = 1
        defer { Socks5Bridge.firstReplySeconds = oldWait; Level2Memo.clear() }
        let bridge = try Socks5Bridge(socks: nil, split: true, siteHosts: { [] })
        defer { bridge.close() }
        let s = try Socks5.connect(SocksConfig(host: "127.0.0.1", port: bridge.port), host: "localhost", ip: nil, port: target.port)
        defer { s.close() }
        try s.write(hello)
        s.setReadTimeout(8)
        XCTAssertEqual(String(decoding: try s.readFully(10), as: UTF8.self), "hello-back")
        lock.lock()
        XCTAssertEqual(seen, [80, 80]) // gửi lại đủ gói bắt tay ở kết nối thứ hai
        lock.unlock()
    }

    func testFindName() {
        var bytes = Array("xxPhimMoir.funyy".utf8)
        let at = bytes.withUnsafeMutableBufferPointer { Socks5Bridge.find(Array("phimmoir.fun".utf8), in: $0.baseAddress!, from: 1, to: $0.count) }
        XCTAssertEqual(at, 2)
    }

    /// Vượt chặn bằng DNS (11/2026): DNS chặn trả 0.0.0.0 / :: → hỏi lại qua DNS mã hoá — cùng ca với Socks5BridgeTest.kt / DohTests.cs.
    func testDohBlockedAnswers() {
        for a in ["0.0.0.0", "::", "127.0.0.1", "::1"] { XCTAssertTrue(Doh.blocked(IPAddr(text: a)!), a) }
        for a in ["104.21.35.220", "2606:4700:3035::6815:23dc", "10.0.0.1"] { XCTAssertFalse(Doh.blocked(IPAddr(text: a)!), a) }
    }

    private func hex(_ s: String) -> [UInt8] {
        let h = Array(s.replacingOccurrences(of: " ", with: ""))
        return stride(from: 0, to: h.count, by: 2).map { UInt8(String(h[$0...$0 + 1]), radix: 16)! }
    }

    /// DNS mã hoá (11/2026): gói DNS, trả lời HTTP, thứ tự máy chủ — cùng ca với DohTest.kt / DohTests.cs.
    func testDnsQueryForA() {
        XCTAssertEqual(DnsWire.query("a.bc"), hex("0000 0100 0001 0000 0000 0000 01 61 02 6263 00 0001 0001"))
    }

    func testDnsAnswerWithCnameAndCompression() {
        let reply = hex("0000 8180 0001 0003 0000 0000" + "03 777777 01 78 02 766e 00 0001 0001" +
                        "c00c 0005 0001 00000e10 0004 c010 c010" + "c010 0001 0001 0000012c 0004 68 15 23 dc" +
                        "c010 0001 0001 0000003c 0004 ac 43 b4 3b")
        // mọi bản ghi A (máy chủ NextDNS gần nhất cần cả danh sách); địa chỉ dùng = cái đầu, TTL nhỏ nhất
        XCTAssertEqual(DnsWire.answer(reply, id: 0),
                       .found(IPAddr(text: "104.21.35.220")!, ttl: 60, all: [IPAddr(text: "104.21.35.220")!, IPAddr(text: "172.67.180.59")!]))
    }

    func testDnsNoDomainNoIpv4AndWrongId() {
        XCTAssertEqual(DnsWire.answer(hex("0000 8183 0001 0000 0000 0000 01 78 00 0001 0001"), id: 0), .noDomain)
        let empty = hex("0000 8180 0001 0000 0000 0000 01 78 00 0001 0001")
        XCTAssertEqual(DnsWire.answer(empty, id: 0), .noIpv4)
        XCTAssertEqual(DnsWire.answer(empty, id: 0x1234), .failed) // DoT: id khác → bỏ
        XCTAssertEqual(DnsWire.answer(hex("0000 8182 0001 0000 0000 0000 01 78 00 0001 0001"), id: 0), .failed)
    }

    func testDohHttpBodyPlainAndChunked() {
        XCTAssertEqual(DohHttp.body(Array("HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: 3\r\n\r\nabc".utf8)), Array("abc".utf8))
        XCTAssertEqual(DohHttp.body(Array("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n3;x=1\r\ncde\r\n0\r\n\r\n".utf8)), Array("abcde".utf8))
        XCTAssertNil(DohHttp.body(Array("HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".utf8)))
    }

    func testDohChainOrder() {
        XCTAssertEqual(Doh.chain("abc123").map(\.label), ["DoH NextDNS", "DoH Cloudflare", "DoH Google", "DoT NextDNS", "DoT Cloudflare", "DoT Google"])
        XCTAssertEqual(Doh.chain("abc123")[0].path, "/abc123")
        XCTAssertEqual(Doh.chain("abc123")[3].host, "abc123.dns.nextdns.io")
        XCTAssertEqual(Doh.chain("").map(\.label), ["DoH Cloudflare", "DoH Google", "DoT Cloudflare", "DoT Google"])
    }

    func testDohDeadServersFallBackToDevice() throws {
        // Mọi máy chủ không tới được (cổng đóng trên máy) → nil: cổng nội bộ dùng DNS của máy, không ném lỗi
        let dead = [Doh.Server(label: "Chết", kind: .dot, host: "dns.test", path: "", ips: ["127.0.0.1"])]
        XCTAssertNil(try Doh.resolve("phim.test", servers: dead))
        XCTAssertNil(try Doh.resolve("localhost", servers: dead))
    }

    func testNetDefaults() {
        let suite = "net-defaults-\(UUID().uuidString)"
        Prefs.store = UserDefaults(suiteName: suite)!
        defer { Prefs.store.removePersistentDomain(forName: suite); Prefs.store = .standard }
        Prefs.applyNetDefaults()
        XCTAssertTrue(Prefs.unblock && Prefs.unblockLevel == 1 && Prefs.secureDns)
        Prefs.unblock = false
        Prefs.secureDns = false
        Prefs.applyNetDefaults() // làm một lần: người xem tắt thì giữ tắt
        XCTAssertFalse(Prefs.unblock || Prefs.secureDns)
        let suite2 = "net-defaults2-\(UUID().uuidString)"
        Prefs.store = UserDefaults(suiteName: suite2)!
        defer { Prefs.store.removePersistentDomain(forName: suite2) }
        Prefs.store.set(true, forKey: "unblock")
        Prefs.store.set(2, forKey: "unblock_level")
        Prefs.applyNetDefaults() // máy đang mức 2 thì giữ mức 2
        XCTAssertEqual(Prefs.unblockLevel, 2)
        XCTAssertTrue(Prefs.secureDns)
    }

    func testDohKeepAliveReadsOneReplyAtATime() throws {
        // Ba trả lời liền nhau trên một kết nối (keep-alive): đọc đúng từng cái; máy chủ báo đóng thì không dùng lại
        let src = ArraySource(Array(("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc" +
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nde\r\n0\r\n\r\n" +
            "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\nf").utf8))
        let r1 = try DohHttp.read(src), r2 = try DohHttp.read(src), r3 = try DohHttp.read(src)
        XCTAssertEqual([r1.body, r2.body, r3.body], [Array("abc".utf8), Array("de".utf8), Array("f".utf8)])
        XCTAssertEqual([r1.keepAlive, r2.keepAlive, r3.keepAlive], [true, true, false])
    }

    func testDohOnlyNextDnsIsSteered() {
        XCTAssertEqual(Doh.chain("abc123").map(\.steer), [true, false, false, true, false, false])
        XCTAssertEqual(Doh.ipsFor(Doh.chain("abc123")[4]), ["1.1.1.1", "1.0.0.1"])
    }

    func testDohSkipsIpAndLocalhost() {
        XCTAssertNil(Doh.unblocked("127.0.0.1"))
        XCTAssertNil(Doh.unblocked("LocalHost"))
        XCTAssertNil(Doh.unblocked("[::1]"))
    }
}
