import CommonCrypto
import CryptoKit
import Darwin
import XCTest

/// Shadowsocks (Shadowsocks.swift): BLAKE3, khoá, link ss://, và kết nối thật trên 127.0.0.1 với máy chủ thử viết riêng
/// theo đặc tả (như ShadowsocksTest.kt của bản Android).
final class ShadowsocksTests: XCTestCase {
    private func hex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
    private func input(_ n: Int) -> [UInt8] { (0..<n).map { UInt8($0 % 251) } }
    private func b64(_ b: [UInt8]) -> String { Data(b).base64EncodedString() }

    // Bộ số chuẩn của BLAKE3 (BLAKE3-team/BLAKE3 test_vectors.json — 32 byte đầu của hash / derive_key)
    func testBlake3OfficialVectors() {
        let ctx = "BLAKE3 2019-12-27 16:29:52 test vectors context"
        let cases: [(Int, String, String)] = [
            (0, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", "2cc39783c223154fea8dfb7c1b1660f2ac2dcbd1c1de8277b0b0dd39b7e50d7d"),
            (1, "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213", "b3e2e340a117a499c6cf2398a19ee0d29cca2bb7404c73063382693bf66cb06c"),
            (63, "e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b", "b6451e30b953c206e34644c6803724e9d2725e0893039cfc49584f991f451af3"),
            (64, "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98", "a5c4a7053fa86b64746d4bb688d06ad1f02a18fce9afd3e818fefaa7126bf73e"),
            (65, "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee", "51fd05c3c1cfbc8ed67d139ad76f5cf8236cd2acd26627a30c104dfd9d3ff8a8"),
            (1023, "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11", "74a16c1c3d44368a86e1ca6df64be6a2f64cce8f09220787450722d85725dea5"),
            (1024, "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7", "7356cd7720d5b66b6d0697eb3177d9f8d73a4a5c5e968896eb6a689684302706"),
        ]
        for (n, h, d) in cases {
            XCTAssertEqual(hex(Blake3.hash(input(n))), h, "hash \(n)")
            XCTAssertEqual(hex(Blake3.deriveKey(ctx, input(n))), d, "derive_key \(n)")
        }
        XCTAssertEqual(hex(Blake3.hash(input(63), outLen: 16)), String(cases[2].1.prefix(32)))
        XCTAssertEqual(hex(Blake3.deriveKey(ctx, input(64), outLen: 16)), String(cases[3].2.prefix(32)))
    }

    // Số kiểm bằng Python (hashlib MD5 + cryptography HKDF-SHA1)
    func testLegacyKeyDerivation() {
        let k = Shadowsocks.evpBytesToKey(Array("test-password".utf8), 32)
        XCTAssertEqual(hex(k), "dfb450efddbb5387197c84460623675b69f2cebcd8ef520a3dfddef7c3d540b2")
        XCTAssertEqual(hex(Shadowsocks.evpBytesToKey(Array("test-password".utf8), 16)), "dfb450efddbb5387197c84460623675b")
        XCTAssertEqual(hex(Shadowsocks.subkey(.aes256, k, (0..<32).map { UInt8($0) })),
                       "7b6ca12d45ef79467432717aaa6a6ae6e1dd046a505fb2ebaaac602210919595")
    }

    func testBase64BothAlphabets() {
        let raw = (0..<32).map { UInt8(($0 * 37 + 251) & 0xFF) }
        XCTAssertEqual(SsBase64.decode(b64(raw)), raw)
        let url = b64(raw).replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        XCTAssertEqual(SsBase64.decode(url), raw)
        XCTAssertNil(SsBase64.decode("abc!"))
        XCTAssertNil(SsBase64.decode(""))
    }

    private let serverPsk = (0..<32).map { UInt8($0 + 1) }
    private let userPsk = (0..<32).map { UInt8(100 + $0) }

    private func enc(_ s: String) -> String { s.addingPercentEncoding(withAllowedCharacters: .alphanumerics)! }

    func testParse3xUi2022Link() throws {
        let link = "ss://2022-blake3-aes-256-gcm:\(enc(b64(serverPsk))):\(enc(b64(userPsk)))@vpn.example.com:24443?type=tcp#Nh%C3%A0"
        let c = try SsLink.parse(link)
        XCTAssertEqual(c.host, "vpn.example.com")
        XCTAssertEqual(c.port, 24443)
        XCTAssertEqual(c.method, "2022-blake3-aes-256-gcm")
        XCTAssertEqual(c.password, "\(b64(serverPsk)):\(b64(userPsk))")
        let keys = try Shadowsocks.keys(.b3Aes256, c.password)
        XCTAssertEqual(keys, [serverPsk, userPsk])
    }

    func testParseSip002AndOldLinks() throws {
        func url(_ s: String) -> String {
            Data(s.utf8).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_")
                .replacingOccurrences(of: "=", with: "")
        }
        let a = try SsLink.parse("ss://" + url("aes-256-gcm:mật khẩu") + "@1.2.3.4:8388/?type=tcp#x")
        XCTAssertEqual(a, SsConfig(host: "1.2.3.4", port: 8388, method: "aes-256-gcm", password: "mật khẩu"))
        let b = try SsLink.parse("ss://" + b64(Array("chacha20-poly1305:pa:ss@host.vn:443".utf8)) + "#tên")
        XCTAssertEqual(b, SsConfig(host: "host.vn", port: 443, method: "chacha20-ietf-poly1305", password: "pa:ss"))
        let c = try SsLink.parse("ss://" + url("2022-blake3-aes-128-gcm:\(b64([UInt8](repeating: 0, count: 16)))") + "@[2001:db8::1]:8443")
        XCTAssertEqual(c.host, "2001:db8::1")
        XCTAssertEqual(c.port, 8443)
    }

    func testParseRejectsUnsupported() {
        let ok = "ss://2022-blake3-aes-256-gcm:\(b64(serverPsk))@h:1"
        let cases: [(String, String)] = [
            ("\(ok)?type=ws&path=%2F", "ws"),
            ("\(ok)?type=tcp&security=tls", "tls"),
            ("\(ok)?plugin=obfs-local%3Bobfs%3Dhttp", "plugin"),
            ("ss://2022-blake3-aes-256-gcm:\(b64([UInt8](repeating: 0, count: 16)))@h:1", "32 byte"),
            ("ss://rc4-md5:abc@h:1", "chưa hỗ trợ"),
            ("ss://2022-blake3-chacha20-poly1305:\(b64(serverPsk)):\(b64(userPsk))@h:1", "nhiều người dùng"),
        ]
        for (link, word) in cases {
            XCTAssertThrowsError(try SsLink.parse(link), link) { e in
                XCTAssertTrue((e as? SsError)?.message.contains(word) == true, "\(link) → \(e)")
            }
        }
    }

    // MARK: Máy chủ thử (viết riêng theo đặc tả, không dùng SsConnection)

    private final class Listener {
        let port: Int
        private let fd: Int32

        init(_ serve: @escaping (TcpSocket) -> Void) {
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

    /// Kết quả máy chủ thấy: đích + dữ liệu đầu.
    private final class Seen {
        private let lock = NSLock()
        private let ready = DispatchSemaphore(value: 0)
        private var value: (String, [UInt8])?
        func set(_ t: String, _ d: [UInt8]) { lock.lock(); value = (t, d); lock.unlock(); ready.signal() }
        func wait() -> (String, [UInt8])? { _ = ready.wait(timeout: .now() + 5); lock.lock(); defer { lock.unlock() }; return value }
    }

    private static func nonce(_ n: UInt64) -> [UInt8] { (0..<12).map { $0 < 8 ? UInt8((n >> (8 * UInt64($0))) & 0xFF) : 0 } }

    private static func open(_ m: SsMethod, _ key: [UInt8], _ n: UInt64, _ c: [UInt8]) throws -> [UInt8] {
        let body = c[0..<(c.count - 16)], tag = c[(c.count - 16)...]
        if m.isChacha {
            return Array(try ChaChaPoly.open(try ChaChaPoly.SealedBox(nonce: try ChaChaPoly.Nonce(data: nonce(n)), ciphertext: body, tag: tag),
                                             using: SymmetricKey(data: key)))
        }
        return Array(try AES.GCM.open(try AES.GCM.SealedBox(nonce: try AES.GCM.Nonce(data: nonce(n)), ciphertext: body, tag: tag),
                                      using: SymmetricKey(data: key)))
    }

    private static func seal(_ m: SsMethod, _ key: [UInt8], _ n: UInt64, _ p: [UInt8]) throws -> [UInt8] {
        if m.isChacha {
            let b = try ChaChaPoly.seal(p, using: SymmetricKey(data: key), nonce: try ChaChaPoly.Nonce(data: nonce(n)))
            return Array(b.ciphertext) + Array(b.tag)
        }
        let b = try AES.GCM.seal(p, using: SymmetricKey(data: key), nonce: try AES.GCM.Nonce(data: nonce(n)))
        return Array(b.ciphertext) + Array(b.tag)
    }

    private static func ecbDecrypt(_ key: [UInt8], _ block: [UInt8]) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: 32)
        var moved = 0
        _ = CCCrypt(CCOperation(kCCDecrypt), CCAlgorithm(kCCAlgorithmAES), CCOptions(kCCOptionECBMode), key, key.count, nil, block, 16,
                    &out, out.count, &moved)
        return Array(out.prefix(16))
    }

    private static func describe(_ a: [UInt8]) -> String {
        let port = Int(a[a.count - 2]) << 8 | Int(a[a.count - 1])
        let host = a[0] == 3 ? String(decoding: a[2..<(2 + Int(a[1]))], as: UTF8.self) : IPAddr(Array(a[1..<(a.count - 2)]))!.text
        return "\(host):\(port)"
    }

    private static func addrLen(_ v: [UInt8]) -> Int { v[0] == 1 ? 7 : v[0] == 4 ? 19 : 4 + Int(v[1]) }

    /// Nhận một kết nối: kiểm phần đầu, ghi lại đích + dữ liệu đầu, trả "echo:" + dữ liệu đầu, rồi vang lại từng chunk.
    private func ssServer(_ m: SsMethod, identity: [UInt8]?, users: [[UInt8]], seen: Seen) -> Listener {
        Listener { s in
            defer { s.close() }
            do {
                let salt = try s.readFully(m.keySize)
                var psk = users[0]
                if let id = identity {
                    let h = Self.ecbDecrypt(Blake3.deriveKey("shadowsocks 2022 identity subkey", id + salt, outLen: m.keySize), try s.readFully(16))
                    guard let u = users.first(where: { Blake3.hash($0, outLen: 16) == h }) else { return } // người dùng lạ: đóng
                    psk = u
                }
                let key = Shadowsocks.subkey(m, psk, salt)
                var rn: UInt64 = 0
                var addr: [UInt8]
                var first: [UInt8]
                if m.is2022 {
                    let fixed = try Self.open(m, key, rn, try s.readFully(27)); rn += 1
                    XCTAssertEqual(fixed[0], 0)
                    var ts: UInt64 = 0
                    for i in 1...8 { ts = ts << 8 | UInt64(fixed[i]) }
                    XCTAssertLessThanOrEqual(abs(Int64(Date().timeIntervalSince1970) - Int64(ts)), 30)
                    let v = try Self.open(m, key, rn, try s.readFully((Int(fixed[9]) << 8 | Int(fixed[10])) + 16)); rn += 1
                    let al = Self.addrLen(v)
                    addr = Array(v[0..<al])
                    let pad = Int(v[al]) << 8 | Int(v[al + 1])
                    XCTAssertTrue(pad > 0 || v.count > al + 2)
                    first = Array(v[(al + 2 + pad)...])
                } else {
                    let l = try Self.open(m, key, rn, try s.readFully(18)); rn += 1
                    let v = try Self.open(m, key, rn, try s.readFully((Int(l[0]) << 8 | Int(l[1])) + 16)); rn += 1
                    let al = Self.addrLen(v)
                    addr = Array(v[0..<al])
                    first = Array(v[al...])
                }
                seen.set(Self.describe(addr), first)
                let rsalt = Shadowsocks.random(m.keySize)
                let wkey = Shadowsocks.subkey(m, psk, rsalt)
                var wn: UInt64 = 0
                let reply = Array("echo:".utf8) + first
                var out = rsalt
                if m.is2022 {
                    var h: [UInt8] = [1]
                    let ts = UInt64(Date().timeIntervalSince1970)
                    for i in (0..<8).reversed() { h.append(UInt8((ts >> (8 * UInt64(i))) & 0xFF)) }
                    h += salt + [UInt8(reply.count >> 8), UInt8(reply.count & 0xFF)]
                    out += try Self.seal(m, wkey, wn, h); wn += 1
                } else {
                    out += try Self.seal(m, wkey, wn, [UInt8(reply.count >> 8), UInt8(reply.count & 0xFF)]); wn += 1
                }
                out += try Self.seal(m, wkey, wn, reply); wn += 1
                try s.write(out)
                while true {
                    var lc = [UInt8](repeating: 0, count: 18)
                    var got = 0
                    while got < 18 {
                        let r = try lc.withUnsafeMutableBytes { try s.read($0.baseAddress! + got, 18 - got) }
                        if r == 0 { return }
                        got += r
                    }
                    let l = try Self.open(m, key, rn, lc); rn += 1
                    let n = Int(l[0]) << 8 | Int(l[1])
                    let data = try Self.open(m, key, rn, try s.readFully(n + 16)); rn += 1
                    let a = try Self.seal(m, wkey, wn, [UInt8(n >> 8), UInt8(n & 0xFF)]); wn += 1
                    let b = try Self.seal(m, wkey, wn, data); wn += 1
                    try s.write(a + b)
                }
            } catch {}
        }
    }

    private func readAll(_ u: Upstream, _ n: Int) throws -> [UInt8] {
        var out = [UInt8](repeating: 0, count: n)
        var got = 0
        while got < n {
            let r = try out.withUnsafeMutableBytes { try u.read($0.baseAddress! + got, n - got) }
            if r == 0 { throw SocketError(message: "hết") }
            got += r
        }
        return out
    }

    private func roundTrip(_ m: SsMethod, _ password: String, _ server: Listener, _ seen: Seen, _ initial: String) throws {
        let c = SsConfig(host: "127.0.0.1", port: server.port, method: m.id, password: password)
        let up = try Shadowsocks.connect(c, host: "phim.example", ip: nil, port: 443)
        up.socket.setReadTimeout(5)
        let first = Array(initial.utf8)
        try first.withUnsafeBytes { try up.write($0.baseAddress!, first.count) }
        let got = seen.wait()
        XCTAssertEqual(got?.0, "phim.example:443")
        XCTAssertEqual(got?.1, first)
        XCTAssertEqual(try readAll(up, 5 + first.count), Array("echo:".utf8) + first)
        // Chunk lớn (quá 0x3FFF với kiểu cũ → chia nhiều chunk)
        let big = (0..<40_000).map { UInt8($0 % 97) }
        try big.withUnsafeBytes { try up.write($0.baseAddress!, big.count) }
        XCTAssertEqual(try readAll(up, big.count), big)
        up.socket.shutdownWrite()
        var one = [UInt8](repeating: 0, count: 1)
        XCTAssertEqual(try one.withUnsafeMutableBytes { try up.read($0.baseAddress!, 1) }, 0)
        up.socket.close()
    }

    func testSs2022MultiUserRoundTrip() throws {
        let seen = Seen()
        let s = ssServer(.b3Aes256, identity: serverPsk, users: [[UInt8](repeating: 7, count: 32), userPsk], seen: seen)
        defer { s.close() }
        try roundTrip(.b3Aes256, "\(b64(serverPsk)):\(b64(userPsk))", s, seen, "GET / HTTP/1.1\r\n\r\n")
    }

    func testSs2022Aes128AndChachaSingleUser() throws {
        let k = (0..<16).map { UInt8($0 * 3) }
        let seen = Seen()
        let s = ssServer(.b3Aes128, identity: nil, users: [k], seen: seen)
        defer { s.close() }
        try roundTrip(.b3Aes128, b64(k), s, seen, "xin chào")
        let seen2 = Seen()
        let s2 = ssServer(.b3Chacha, identity: nil, users: [userPsk], seen: seen2)
        defer { s2.close() }
        try roundTrip(.b3Chacha, b64(userPsk), s2, seen2, "chacha")
    }

    func testLegacyAeadRoundTrip() throws {
        for m in [SsMethod.aes256, .aes128, .chacha] {
            let seen = Seen()
            let s = ssServer(m, identity: nil, users: [Shadowsocks.evpBytesToKey(Array("mật khẩu".utf8), m.keySize)], seen: seen)
            defer { s.close() }
            try roundTrip(m, "mật khẩu", s, seen, "legacy \(m.id)")
        }
    }

    func testWrongUserKeyReportsClearError() throws {
        let seen = Seen()
        let s = ssServer(.b3Aes256, identity: serverPsk, users: [userPsk], seen: seen)
        defer { s.close() }
        let c = SsConfig(host: "127.0.0.1", port: s.port, method: SsMethod.b3Aes256.id,
                         password: "\(b64(serverPsk)):\(b64([UInt8](repeating: 9, count: 32)))")
        let up = try Shadowsocks.connect(c, host: "phim.example", ip: nil, port: 443)
        up.socket.setReadTimeout(5)
        let hello = Array("hello".utf8)
        try hello.withUnsafeBytes { try up.write($0.baseAddress!, hello.count) }
        var one = [UInt8](repeating: 0, count: 1)
        XCTAssertThrowsError(try one.withUnsafeMutableBytes { try up.read($0.baseAddress!, 1) }) { e in
            XCTAssertTrue((e as? SsError)?.message.contains("sai khoá") == true, "\(e)")
        }
        up.socket.close()
    }

    /// Cổng nội bộ đi qua Shadowsocks: gói bắt tay TLS tới máy chủ nguyên một mẩu (không chia nhỏ), đúng đích.
    func testBridgeRelaysThroughShadowsocks() throws {
        let seen = Seen()
        let s = ssServer(.b3Aes256, identity: serverPsk, users: [userPsk], seen: seen)
        defer { s.close() }
        let cfg = SsConfig(host: "127.0.0.1", port: s.port, method: SsMethod.b3Aes256.id, password: "\(b64(serverPsk)):\(b64(userPsk))")
        let b = try Socks5Bridge(socks: nil, ss: cfg, split: true, siteHosts: { [] })
        defer { b.close() }
        let k = try TcpSocket.connect(host: "127.0.0.1", port: b.port)
        defer { k.close() }
        k.setReadTimeout(5)
        try k.write([5, 1, 0])
        XCTAssertEqual(try k.readFully(2), [5, 0])
        let host = Array("phim.example".utf8)
        try k.write([5, 1, 0, 3, UInt8(host.count)] + host + [1, 0xBB])
        XCTAssertEqual(try k.readFully(10)[1], 0)
        let hello: [UInt8] = [0x16, 0x03, 0x01] + host
        try k.write(hello)
        let got = seen.wait()
        XCTAssertEqual(got?.0, "phim.example:443")
        XCTAssertEqual(got?.1, hello)
        XCTAssertEqual(Array(try k.readFully(5 + hello.count).prefix(5)), Array("echo:".utf8))
    }

    /// Bên gọi chưa gửi gì: sau 0,3 s cổng nội bộ gửi phần đầu (có đệm) để máy chủ mở kết nối.
    func testBridgeSendsHeaderWhenClientIsSilent() throws {
        let seen = Seen()
        let s = ssServer(.b3Aes256, identity: serverPsk, users: [userPsk], seen: seen)
        defer { s.close() }
        let cfg = SsConfig(host: "127.0.0.1", port: s.port, method: SsMethod.b3Aes256.id, password: "\(b64(serverPsk)):\(b64(userPsk))")
        let b = try Socks5Bridge(socks: nil, ss: cfg, siteHosts: { [] })
        defer { b.close() }
        let k = try TcpSocket.connect(host: "127.0.0.1", port: b.port)
        defer { k.close() }
        k.setReadTimeout(5)
        try k.write([5, 1, 0])
        _ = try k.readFully(2)
        try k.write([5, 1, 0, 1, 10, 0, 0, 1, 0, 25])
        _ = try k.readFully(10)
        let got = seen.wait()
        XCTAssertEqual(got?.0, "10.0.0.1:25")
        XCTAssertEqual(got?.1, [])
    }
}
