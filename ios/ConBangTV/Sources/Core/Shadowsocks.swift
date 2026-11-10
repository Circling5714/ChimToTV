import Foundation
import CryptoKit
import CommonCrypto
#if canImport(Darwin)
import Darwin
#endif

/// Máy chủ Shadowsocks (Cài đặt → Proxy → Kiểu: Shadowsocks), vd lối vào Shadowsocks của 3X-UI / Xray (bản Android:
/// Shadowsocks.kt). password: SS 2022 = các khoá base64 cách nhau bằng ":" (3X-UI luôn nhiều người dùng:
/// "khoá-máy-chủ:khoá-người-dùng"); kiểu cũ (aes-256-gcm, chacha20-ietf-poly1305…) = mật khẩu.
struct SsConfig: Equatable {
    var host: String
    var port: Int
    var method: String
    var password: String
    /// true: máy chủ phân giải tên miền (gửi tên miền); false: máy tự phân giải rồi gửi IP.
    var remoteDns = true
    /// true: chỉ trang phim đi qua máy chủ; video / ảnh / cập nhật đi thẳng.
    var sitesOnly = false
}

/// Kiểu mã hoá hỗ trợ (TCP). keySize = số byte khoá = số byte salt.
enum SsMethod: String, CaseIterable {
    case b3Aes256 = "2022-blake3-aes-256-gcm"
    case b3Aes128 = "2022-blake3-aes-128-gcm"
    case b3Chacha = "2022-blake3-chacha20-poly1305"
    case aes256 = "aes-256-gcm"
    case aes128 = "aes-128-gcm"
    case chacha = "chacha20-ietf-poly1305"

    static let defaultMethod = SsMethod.b3Aes256

    var id: String { rawValue }
    var keySize: Int { self == .b3Aes128 || self == .aes128 ? 16 : 32 }
    var isChacha: Bool { self == .b3Chacha || self == .chacha }
    var is2022: Bool { rawValue.hasPrefix("2022-") }

    /// Theo tên (không phân biệt hoa / thường; trống = mặc định); "chacha20-poly1305" của Xray / 3X-UI = chacha20-ietf-poly1305.
    static func of(_ name: String) -> SsMethod? {
        let n = name.trimmingCharacters(in: .whitespaces).lowercased()
        if n.isEmpty { return defaultMethod }
        if n == "chacha20-poly1305" { return .chacha }
        return SsMethod(rawValue: n)
    }

    static var names: String { allCases.map(\.rawValue).joined(separator: ", ") }
}

/// Lỗi Shadowsocks: câu báo cho người xem (sai khoá, máy chủ không trả lời…).
struct SsError: LocalizedError {
    let message: String
    init(_ m: String) { message = m }
    var errorDescription: String? { message }
}

/// Đọc link ss:// (SIP002, cả kiểu 3X-UI cho SS 2022: "ss://method:khoá-máy-chủ:khoá-người-dùng@host:cổng?type=tcp#tên").
enum SsLink {
    static func parse(_ link: String, remoteDns: Bool = true, sitesOnly: Bool = false) throws -> SsConfig {
        var s = link.trimmingCharacters(in: .whitespacesAndNewlines)
        guard s.lowercased().hasPrefix("ss://") else { throw SsError("Link Shadowsocks phải bắt đầu bằng ss://") }
        s = String(s.dropFirst(5))
        if let h = s.firstIndex(of: "#") { s = String(s[..<h]) }
        var query = ""
        if let q = s.firstIndex(of: "?") {
            query = String(s[s.index(after: q)...])
            s = String(s[..<q])
        }
        while s.hasSuffix("/") { s.removeLast() }
        for p in query.split(separator: "&") {
            let k = p.split(separator: "=", maxSplits: 1).first.map { String($0).lowercased() } ?? ""
            let v = (p.split(separator: "=", maxSplits: 1).dropFirst().first.map { decode(String($0)) } ?? "").lowercased()
            if k == "type", !v.isEmpty, v != "tcp" {
                throw SsError("Link dùng truyền tải «\(v)» — app chỉ hỗ trợ Shadowsocks qua TCP thường (3X-UI: Transmission = TCP, không TLS).")
            }
            if k == "security", !v.isEmpty, v != "none" {
                throw SsError("Link dùng «security=\(v)» — app chỉ hỗ trợ Shadowsocks qua TCP thường (3X-UI: Security = None).")
            }
            if k == "plugin", !v.isEmpty {
                throw SsError("Link dùng plugin (\(v.split(separator: ";").first ?? "")) — app chưa hỗ trợ plugin.")
            }
            if k == "headertype", !v.isEmpty, v != "none" {
                throw SsError("Link dùng ngụy trang HTTP — app chưa hỗ trợ (3X-UI: TCP header = None).")
            }
        }
        // Kiểu cũ: ss://base64(method:pass@host:port)
        if !s.contains("@") {
            guard let d = SsBase64.decode(s), let t = String(bytes: d, encoding: .utf8) else { throw SsError("Link ss:// không đọc được.") }
            s = t
        }
        guard let at = s.lastIndex(of: "@") else { throw SsError("Link ss:// thiếu máy chủ / cổng.") }
        let user = String(s[..<at])
        let hostPort = String(s[s.index(after: at)...])
        var info: String
        if user.contains(":") || user.contains("%") {
            info = decode(user)
        } else if let d = SsBase64.decode(user), let t = String(bytes: d, encoding: .utf8), t.contains(":") {
            info = t
        } else {
            info = decode(user)
        }
        guard let colon = info.firstIndex(of: ":") else { throw SsError("Link ss:// thiếu kiểu mã hoá / khoá.") }
        let method = String(info[..<colon])
        let password = String(info[info.index(after: colon)...])
        guard let (host, port) = splitHostPort(hostPort) else { throw SsError("Link ss:// thiếu máy chủ / cổng.") }
        guard let m = SsMethod.of(method) else { throw SsError("Kiểu mã hoá «\(method)» chưa hỗ trợ. App hỗ trợ: \(SsMethod.names).") }
        _ = try Shadowsocks.keys(m, password) // kiểm tra khoá ngay lúc dán
        return SsConfig(host: host, port: port, method: m.id, password: password, remoteDns: remoteDns, sitesOnly: sitesOnly)
    }

    /// "host:cổng" hoặc "[ipv6]:cổng" → (host, cổng).
    static func splitHostPort(_ s: String) -> (String, Int)? {
        let t = s.trimmingCharacters(in: .whitespaces)
        var host: String
        var port: String
        if t.hasPrefix("[") {
            guard let close = t.firstIndex(of: "]") else { return nil }
            host = String(t[t.index(after: t.startIndex)..<close])
            let rest = t[t.index(after: close)...]
            guard rest.hasPrefix(":") else { return nil }
            port = String(rest.dropFirst())
        } else {
            guard let c = t.lastIndex(of: ":") else { return nil }
            host = String(t[..<c])
            port = String(t[t.index(after: c)...])
        }
        guard let p = Int(port), !host.isEmpty, (1...65535).contains(p) else { return nil }
        return (host, p)
    }

    private static func decode(_ s: String) -> String {
        s.removingPercentEncoding ?? s
    }
}

/// Base64 thường lẫn base64url, có hoặc không dấu "=" (khoá 3X-UI, link SIP002). nil = không phải base64.
enum SsBase64 {
    static func decode(_ s: String) -> [UInt8]? {
        var t = s.trimmingCharacters(in: .whitespaces)
        while t.hasSuffix("=") { t.removeLast() }
        if t.isEmpty { return nil }
        var out: [UInt8] = []
        out.reserveCapacity(t.utf8.count * 3 / 4)
        var acc: UInt32 = 0
        var bits = 0
        for ch in t.utf8 {
            let v: UInt32
            switch ch {
            case 65...90: v = UInt32(ch - 65)
            case 97...122: v = UInt32(ch - 97 + 26)
            case 48...57: v = UInt32(ch - 48 + 52)
            case 43, 45: v = 62 // + -
            case 47, 95: v = 63 // / _
            default: return nil
            }
            acc = (acc << 6) | v
            bits += 6
            if bits >= 8 {
                bits -= 8
                out.append(UInt8((acc >> UInt32(bits)) & 0xFF))
            }
        }
        if bits >= 6 { return nil }
        return out
    }
}

/// BLAKE3 — chỉ đầu vào ≤ 1024 byte (một chunk) và đầu ra ≤ 32 byte: đủ cho khoá của Shadowsocks 2022. Kiểm bằng bộ số
/// chuẩn của BLAKE3 trong ShadowsocksTests.
enum Blake3 {
    private static let iv: [UInt32] = [0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19]
    private static let perm = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]

    static func hash(_ input: [UInt8], outLen: Int = 32) -> [UInt8] { root(iv, input, 0, outLen) }

    static func deriveKey(_ context: String, _ material: [UInt8], outLen: Int = 32) -> [UInt8] {
        let ck = root(iv, Array(context.utf8), 32, 32)
        let words = (0..<8).map { i in UInt32(ck[i * 4]) | UInt32(ck[i * 4 + 1]) << 8 | UInt32(ck[i * 4 + 2]) << 16 | UInt32(ck[i * 4 + 3]) << 24 }
        return root(words, material, 64, outLen)
    }

    private static func root(_ key: [UInt32], _ input: [UInt8], _ flags: UInt32, _ outLen: Int) -> [UInt8] {
        precondition(input.count <= 1024 && outLen >= 1 && outLen <= 32)
        var cv = key
        let blocks = max(1, (input.count + 63) / 64)
        for b in 0..<blocks {
            let start = b * 64
            let len = min(64, input.count - start)
            var m = [UInt32](repeating: 0, count: 16)
            for i in 0..<max(0, len) { m[i >> 2] |= UInt32(input[start + i]) << (8 * UInt32(i & 3)) }
            var f = flags
            if b == 0 { f |= 1 }
            if b == blocks - 1 { f |= 2 | 8 }
            cv = compress(cv, m, UInt32(max(0, len)), f)
        }
        return (0..<outLen).map { UInt8((cv[$0 >> 2] >> (8 * UInt32($0 & 3))) & 0xFF) }
    }

    private static func compress(_ cv: [UInt32], _ block: [UInt32], _ len: UInt32, _ flags: UInt32) -> [UInt32] {
        var s = cv + [iv[0], iv[1], iv[2], iv[3], 0, 0, len, flags]
        var m = block
        for r in 0..<7 {
            g(&s, 0, 4, 8, 12, m[0], m[1]); g(&s, 1, 5, 9, 13, m[2], m[3])
            g(&s, 2, 6, 10, 14, m[4], m[5]); g(&s, 3, 7, 11, 15, m[6], m[7])
            g(&s, 0, 5, 10, 15, m[8], m[9]); g(&s, 1, 6, 11, 12, m[10], m[11])
            g(&s, 2, 7, 8, 13, m[12], m[13]); g(&s, 3, 4, 9, 14, m[14], m[15])
            if r < 6 { m = perm.map { m[$0] } }
        }
        return (0..<8).map { s[$0] ^ s[$0 + 8] }
    }

    private static func rotr(_ x: UInt32, _ n: UInt32) -> UInt32 { (x >> n) | (x << (32 - n)) }

    private static func g(_ s: inout [UInt32], _ a: Int, _ b: Int, _ c: Int, _ d: Int, _ x: UInt32, _ y: UInt32) {
        s[a] = s[a] &+ s[b] &+ x
        s[d] = rotr(s[d] ^ s[a], 16)
        s[c] = s[c] &+ s[d]
        s[b] = rotr(s[b] ^ s[c], 12)
        s[a] = s[a] &+ s[b] &+ y
        s[d] = rotr(s[d] ^ s[a], 8)
        s[c] = s[c] &+ s[d]
        s[b] = rotr(s[b] ^ s[c], 7)
    }
}

/// AEAD một chiều của phiên: khoá phiên + nonce 12 byte little-endian, tăng sau mỗi lần mã hoá / giải mã.
final class SsAead {
    private let key: SymmetricKey
    private let chacha: Bool
    private var nonce = [UInt8](repeating: 0, count: 12)

    init(_ m: SsMethod, key: [UInt8]) {
        self.key = SymmetricKey(data: key)
        chacha = m.isChacha
    }

    func seal(_ p: [UInt8]) throws -> [UInt8] {
        defer { inc() }
        if chacha {
            let box = try ChaChaPoly.seal(p, using: key, nonce: try ChaChaPoly.Nonce(data: nonce))
            return Array(box.ciphertext) + Array(box.tag)
        }
        let box = try AES.GCM.seal(p, using: key, nonce: try AES.GCM.Nonce(data: nonce))
        return Array(box.ciphertext) + Array(box.tag)
    }

    /// Sai thẻ xác thực (sai khoá / dữ liệu hỏng) → lỗi.
    func open(_ c: [UInt8]) throws -> [UInt8] {
        defer { inc() }
        let body = c[0..<(c.count - 16)], tag = c[(c.count - 16)...]
        if chacha {
            let box = try ChaChaPoly.SealedBox(nonce: try ChaChaPoly.Nonce(data: nonce), ciphertext: body, tag: tag)
            return Array(try ChaChaPoly.open(box, using: key))
        }
        let box = try AES.GCM.SealedBox(nonce: try AES.GCM.Nonce(data: nonce), ciphertext: body, tag: tag)
        return Array(try AES.GCM.open(box, using: key))
    }

    private func inc() {
        for i in 0..<12 {
            nonce[i] &+= 1
            if nonce[i] != 0 { break }
        }
    }
}

/// Một đầu của kết nối mà cổng nội bộ chép dữ liệu qua lại: ổ cắm thường, hoặc Shadowsocks chồng trên ổ cắm.
protocol Upstream: AnyObject {
    var tcp: TcpSocket { get }
    /// Đọc tối đa n byte; 0 = hết.
    func read(_ buf: UnsafeMutableRawPointer, _ n: Int) throws -> Int
    func write(_ p: UnsafeRawPointer, _ n: Int) throws
}

extension TcpSocket: Upstream {
    var tcp: TcpSocket { self }
}

/// Kết nối qua máy chủ Shadowsocks. Phần đầu (salt, phần nhận dạng người dùng, địa chỉ đích) gửi chung một lần ghi với
/// dữ liệu đầu tiên của bên gọi; start() gửi phần đầu không kèm dữ liệu (thêm đệm ngẫu nhiên) khi bên gọi chưa nói gì.
final class SsConnection: Upstream {
    let tcp: TcpSocket
    private let m: SsMethod
    private let keys: [[UInt8]]
    private let target: [UInt8]
    let salt: [UInt8]
    private let enc: SsAead
    private(set) var started = false
    private var dec: SsAead?
    // Đã giải mã, chưa trả cho bên gọi
    private var plain: [UInt8] = []
    private var plainPos = 0
    // Đọc thô từ ổ cắm
    private var raw = [UInt8](repeating: 0, count: 32 * 1024)
    private var rawPos = 0, rawEnd = 0

    init(socket: TcpSocket, method: SsMethod, keys: [[UInt8]], target: [UInt8]) {
        tcp = socket
        m = method
        self.keys = keys
        self.target = target
        salt = Shadowsocks.random(method.keySize)
        enc = SsAead(method, key: Shadowsocks.subkey(method, keys[keys.count - 1], salt))
    }

    // MARK: Gửi

    func write(_ p: UnsafeRawPointer, _ n: Int) throws {
        var data = Array(UnsafeRawBufferPointer(start: p, count: n))[...]
        if !started {
            let k = try header(Array(data))
            data = data.dropFirst(k)
        }
        let maxChunk = m.is2022 ? 0xFFFF : 0x3FFF
        while !data.isEmpty {
            let k = min(data.count, maxChunk)
            let part = Array(data.prefix(k))
            let out = try enc.seal([UInt8(k >> 8), UInt8(k & 0xFF)]) + enc.seal(part)
            try tcp.write(out)
            data = data.dropFirst(k)
        }
    }

    /// Gửi phần đầu ngay (chưa có dữ liệu của bên gọi).
    func start() throws {
        if !started { _ = try header([]) }
    }

    /// Ghi phần đầu kèm tối đa một phần dữ liệu; trả số byte dữ liệu đã gửi kèm.
    private func header(_ b: [UInt8]) throws -> Int {
        started = true
        var out = salt
        let sent: Int
        if m.is2022 {
            // Phần nhận dạng (EIH): mỗi khoá trước khoá người dùng mã hoá 16 byte đầu BLAKE3(khoá kế tiếp) bằng AES-ECB
            for i in 0..<(keys.count - 1) {
                let sub = Blake3.deriveKey("shadowsocks 2022 identity subkey", keys[i] + salt, outLen: m.keySize)
                out += try Shadowsocks.aesEcb(sub, Blake3.hash(keys[i + 1], outLen: 16))
            }
            let padding = b.count < 900 ? Int.random(in: 1...900) : 0
            sent = min(b.count, 0xFFFF - target.count - 2 - padding)
            let varLen = target.count + 2 + padding + sent
            var fixed: [UInt8] = [0]
            let ts = UInt64(Date().timeIntervalSince1970)
            for i in (0..<8).reversed() { fixed.append(UInt8((ts >> (8 * UInt64(i))) & 0xFF)) }
            fixed += [UInt8(varLen >> 8), UInt8(varLen & 0xFF)]
            var vh = target
            vh += [UInt8(padding >> 8), UInt8(padding & 0xFF)]
            vh += Shadowsocks.random(padding)
            vh += b[0..<sent]
            out += try enc.seal(fixed)
            out += try enc.seal(vh)
        } else {
            // Kiểu cũ: chunk đầu = địa chỉ đích + dữ liệu đầu
            sent = min(b.count, 0x3FFF - target.count)
            let first = target + b[0..<sent]
            out += try enc.seal([UInt8(first.count >> 8), UInt8(first.count & 0xFF)])
            out += try enc.seal(first)
        }
        try tcp.write(out) // một lần ghi (chuẩn SS 2022: không để lộ cỡ từng phần)
        return sent
    }

    // MARK: Nhận

    func read(_ buf: UnsafeMutableRawPointer, _ n: Int) throws -> Int {
        while plainPos >= plain.count {
            if try !fill() { return 0 }
        }
        let k = min(n, plain.count - plainPos)
        plain.withUnsafeBytes { buf.copyMemory(from: $0.baseAddress! + plainPos, byteCount: k) }
        plainPos += k
        return k
    }

    /// Chunk tiếp theo; false = máy chủ đóng kết nối (hết dữ liệu).
    private func fill() throws -> Bool {
        guard let d = dec else { return try begin() }
        guard let lc = try readExact(18, eofOk: true) else { return false }
        let l = try openOrFail(d, lc, "dữ liệu từ máy chủ Shadowsocks bị hỏng")
        try payload(d, Int(l[0]) << 8 | Int(l[1]))
        return true
    }

    private func payload(_ d: SsAead, _ n: Int) throws {
        guard let c = try readExact(n + 16, eofOk: false) else { return }
        plain = try openOrFail(d, c, "dữ liệu từ máy chủ Shadowsocks bị hỏng")
        plainPos = 0
    }

    private func begin() throws -> Bool {
        // Máy chủ không nhận ra khoá thì đóng / ngắt / im lặng — chưa có trả lời nào là coi như bị từ chối
        let rs: [UInt8]
        do {
            guard let s = try readExact(m.keySize, eofOk: true) else { throw SsError(Shadowsocks.noReply(m)) }
            rs = s
        } catch is SocketError {
            throw SsError(Shadowsocks.noReply(m))
        }
        let d = SsAead(m, key: Shadowsocks.subkey(m, keys[keys.count - 1], rs))
        dec = d
        if !m.is2022 { return try fill() }
        let hl = 1 + 8 + m.keySize + 2
        guard let c = try readExact(hl + 16, eofOk: false) else { return false }
        let h = try openOrFail(d, c, "không giải mã được trả lời của máy chủ Shadowsocks — sai khoá hoặc sai kiểu mã hoá")
        guard h[0] == 1, Array(h[9..<(9 + m.keySize)]) == salt else { throw SsError("máy chủ Shadowsocks trả lời sai giao thức") }
        var ts: UInt64 = 0
        for i in 1...8 { ts = ts << 8 | UInt64(h[i]) }
        let skew = abs(Int64(Date().timeIntervalSince1970) - Int64(bitPattern: ts))
        if skew > 30 { throw SsError("giờ trên máy lệch \(skew) giây so với máy chủ (Shadowsocks 2022 cho lệch tối đa 30 giây)") }
        try payload(d, Int(h[hl - 2]) << 8 | Int(h[hl - 1]))
        return true
    }

    private func openOrFail(_ d: SsAead, _ c: [UInt8], _ message: String) throws -> [UInt8] {
        do { return try d.open(c) } catch { throw SsError(message) }
    }

    /// Đọc đúng n byte (qua bộ đệm); nil = máy chủ đóng kết nối trước byte đầu và eofOk.
    private func readExact(_ n: Int, eofOk: Bool) throws -> [UInt8]? {
        var out: [UInt8] = []
        out.reserveCapacity(n)
        while out.count < n {
            if rawPos >= rawEnd {
                let r = try raw.withUnsafeMutableBytes { try tcp.read($0.baseAddress!, $0.count) }
                if r == 0 {
                    if out.isEmpty && eofOk { return nil }
                    throw SocketError(message: "máy chủ Shadowsocks ngắt kết nối giữa chừng")
                }
                rawPos = 0
                rawEnd = r
            }
            let k = min(n - out.count, rawEnd - rawPos)
            out += raw[rawPos..<(rawPos + k)]
            rawPos += k
        }
        return out
    }
}

/// Phía khách Shadowsocks (TCP): SS 2022 (SIP022, có phần nhận dạng người dùng cho máy chủ nhiều người dùng) + AEAD cũ.
enum Shadowsocks {
    static func random(_ n: Int) -> [UInt8] {
        var b = [UInt8](repeating: 0, count: n)
        if n > 0 { _ = SecRandomCopyBytes(kSecRandomDefault, n, &b) }
        return b
    }

    /// Khoá từ mật khẩu. SS 2022: mỗi phần (cách nhau bằng ":") là khoá base64 đúng cỡ — phần cuối là khoá người dùng, các
    /// phần trước là khoá nhận dạng (khoá máy chủ của 3X-UI). Kiểu cũ: EVP_BytesToKey(MD5). Sai → SsError.
    static func keys(_ m: SsMethod, _ password: String) throws -> [[UInt8]] {
        guard !password.isEmpty else { throw SsError("Chưa nhập khoá / mật khẩu Shadowsocks.") }
        if !m.is2022 { return [evpBytesToKey(Array(password.utf8), m.keySize)] }
        let parts = password.split(separator: ":", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespaces) }
        if parts.count > 1 && m.isChacha {
            throw SsError("\(m.id) không dùng được với máy chủ nhiều người dùng (khoá dạng «khoá-máy-chủ:khoá-người-dùng») — chọn 2022-blake3-aes-256-gcm.")
        }
        return try parts.enumerated().map { i, p in
            let which = parts.count == 1 ? "Khoá" : i == parts.count - 1 ? "Khoá người dùng" : "Khoá máy chủ"
            guard let k = SsBase64.decode(p), k.count == m.keySize else {
                throw SsError("\(which) không đúng: \(m.id) cần khoá base64 dài \(m.keySize) byte (\((m.keySize + 2) / 3 * 4) ký tự).")
            }
            return k
        }
    }

    static func subkey(_ m: SsMethod, _ key: [UInt8], _ salt: [UInt8]) -> [UInt8] {
        if m.is2022 { return Blake3.deriveKey("shadowsocks 2022 session subkey", key + salt, outLen: m.keySize) }
        let k = HKDF<Insecure.SHA1>.deriveKey(inputKeyMaterial: SymmetricKey(data: key), salt: salt, info: Array("ss-subkey".utf8),
                                               outputByteCount: m.keySize)
        return k.withUnsafeBytes { Array($0) }
    }

    static func evpBytesToKey(_ pass: [UInt8], _ size: Int) -> [UInt8] {
        var out: [UInt8] = []
        var prev: [UInt8] = []
        while out.count < size {
            prev = Array(Insecure.MD5.hash(data: prev + pass))
            out += prev
        }
        return Array(out.prefix(size))
    }

    /// Mã hoá đúng một khối 16 byte bằng AES-ECB (phần nhận dạng người dùng của SS 2022).
    static func aesEcb(_ key: [UInt8], _ block: [UInt8]) throws -> [UInt8] {
        var out = [UInt8](repeating: 0, count: 16 + kCCBlockSizeAES128)
        var moved = 0
        let st = CCCrypt(CCOperation(kCCEncrypt), CCAlgorithm(kCCAlgorithmAES), CCOptions(kCCOptionECBMode), key, key.count, nil,
                         block, block.count, &out, out.count, &moved)
        guard st == kCCSuccess, moved == 16 else { throw SsError("không mã hoá được phần nhận dạng (\(st))") }
        return Array(out.prefix(16))
    }

    static func noReply(_ m: SsMethod) -> String {
        "máy chủ Shadowsocks không trả lời — thường do sai khoá / sai kiểu mã hoá" + (m.is2022 ? ", hoặc giờ trên máy lệch quá 30 giây" : "") +
            " (kiểm tra lại link ss:// trong 3X-UI)"
    }

    /// Địa chỉ đích dạng SOCKS5 (ATYP + địa chỉ + cổng).
    static func target(host: String, ip: IPAddr?, port: Int) throws -> [UInt8] {
        var a: [UInt8]
        if let ip = ip {
            a = (ip.bytes.count == 4 ? [1] : [4]) + ip.bytes
        } else {
            let h = Array(host.utf8)
            guard h.count <= 255 else { throw SocksError(code: 1, message: "tên miền quá dài") }
            a = [3, UInt8(h.count)] + h
        }
        return a + [UInt8(port >> 8), UInt8(port & 0xFF)]
    }

    private static let cacheLock = NSLock()
    private static var cached: (SsMethod, String, [[UInt8]])?

    /// Mở kết nối tới host:port qua máy chủ c (ip: đích đã là IP / đã phân giải). Chưa gửi gì: phần đầu đi cùng dữ liệu đầu
    /// tiên. Máy chủ không xác nhận kết nối (giao thức không có bước này) — sai khoá chỉ lộ ra khi đọc trả lời.
    static func connect(_ c: SsConfig, host: String, ip: IPAddr?, port: Int) throws -> SsConnection {
        guard let m = SsMethod.of(c.method) else { throw SocksError(code: 1, message: "kiểu mã hoá Shadowsocks «\(c.method)» chưa hỗ trợ") }
        let keys: [[UInt8]]
        cacheLock.lock()
        if let k = cached, k.0 == m, k.1 == c.password {
            keys = k.2
            cacheLock.unlock()
        } else {
            cacheLock.unlock()
            do {
                keys = try Shadowsocks.keys(m, c.password)
            } catch let e as SsError {
                throw SocksError(code: 1, message: e.message)
            }
            cacheLock.lock()
            cached = (m, c.password, keys)
            cacheLock.unlock()
        }
        let t = try target(host: host, ip: ip, port: port)
        let s: TcpSocket
        do {
            s = try TcpSocket.connect(host: c.host, port: c.port)
        } catch {
            throw SocksError(code: 1, message: "không kết nối được máy chủ Shadowsocks \(c.host):\(c.port)")
        }
        return SsConnection(socket: s, method: m, keys: keys, target: t)
    }
}
