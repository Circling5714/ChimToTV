import Foundation

/// Đọc một giá trị JSON bắt đầu tại một vị trí trong chuỗi dài, dừng ngay sau giá trị đó (bản Android dùng JSONTokener).
/// Dữ liệu Next.js của trang (self.__next_f.push) là văn bản dài lẫn nhiều thứ; app chỉ bóc từng đối tượng cần.
/// Vị trí tính theo đơn vị UTF-16 (khớp NSString / NSRegularExpression).
/// Kết quả: [String: Any], [Any], String, Int, Double, Bool, NSNull.
struct JSONScan {
    private let u: [UInt16]
    private var i: Int

    private struct Bad: Error {}

    static func value(_ units: [UInt16], at index: Int) -> Any? {
        guard index >= 0, index < units.count else { return nil }
        var p = JSONScan(u: units, i: index)
        return try? p.parseValue()
    }

    static func value(_ text: String, at index: Int) -> Any? { value(Array(text.utf16), at: index) }

    private init(u: [UInt16], i: Int) {
        self.u = u
        self.i = i
    }

    private mutating func skipWhitespace() {
        while i < u.count, u[i] == 0x20 || u[i] == 0x0A || u[i] == 0x0D || u[i] == 0x09 { i += 1 }
    }

    private mutating func parseValue() throws -> Any {
        skipWhitespace()
        guard i < u.count else { throw Bad() }
        switch u[i] {
        case 0x7B: return try parseObject()
        case 0x5B: return try parseArray()
        case 0x22: return try parseString()
        case 0x74: try literal("true"); return true
        case 0x66: try literal("false"); return false
        case 0x6E: try literal("null"); return NSNull()
        default: return try parseNumber()
        }
    }

    private mutating func literal(_ word: String) throws {
        for c in word.utf16 {
            guard i < u.count, u[i] == c else { throw Bad() }
            i += 1
        }
    }

    private mutating func parseObject() throws -> [String: Any] {
        i += 1 // {
        var out: [String: Any] = [:]
        skipWhitespace()
        if i < u.count, u[i] == 0x7D { i += 1; return out }
        while true {
            skipWhitespace()
            guard i < u.count, u[i] == 0x22 else { throw Bad() }
            let key = try parseString()
            skipWhitespace()
            guard i < u.count, u[i] == 0x3A else { throw Bad() } // :
            i += 1
            let v = try parseValue()
            if out[key] == nil { out[key] = v }
            skipWhitespace()
            guard i < u.count else { throw Bad() }
            if u[i] == 0x2C { i += 1; continue } // ,
            if u[i] == 0x7D { i += 1; return out } // }
            throw Bad()
        }
    }

    private mutating func parseArray() throws -> [Any] {
        i += 1 // [
        var out: [Any] = []
        skipWhitespace()
        if i < u.count, u[i] == 0x5D { i += 1; return out }
        while true {
            out.append(try parseValue())
            skipWhitespace()
            guard i < u.count else { throw Bad() }
            if u[i] == 0x2C { i += 1; continue }
            if u[i] == 0x5D { i += 1; return out }
            throw Bad()
        }
    }

    private mutating func parseString() throws -> String {
        i += 1 // "
        var buf: [UInt16] = []
        while i < u.count {
            let c = u[i]
            i += 1
            if c == 0x22 { return String(decoding: buf, as: UTF16.self) }
            if c != 0x5C { buf.append(c); continue }
            guard i < u.count else { throw Bad() }
            let e = u[i]
            i += 1
            switch e {
            case 0x22, 0x5C, 0x2F: buf.append(e)
            case 0x62: buf.append(0x08) // \b
            case 0x66: buf.append(0x0C) // \f
            case 0x6E: buf.append(0x0A) // \n
            case 0x72: buf.append(0x0D) // \r
            case 0x74: buf.append(0x09) // \t
            case 0x75: // \uXXXX (cặp thay thế UTF-16 ghép tự nhiên khi giải mã cả mảng)
                guard i + 4 <= u.count else { throw Bad() }
                var v: UInt16 = 0
                for k in 0..<4 {
                    let h = u[i + k]
                    let d: UInt16
                    switch h {
                    case 0x30...0x39: d = h - 0x30
                    case 0x41...0x46: d = h - 0x41 + 10
                    case 0x61...0x66: d = h - 0x61 + 10
                    default: throw Bad()
                    }
                    v = v << 4 | d
                }
                i += 4
                buf.append(v)
            default: buf.append(e)
            }
        }
        throw Bad()
    }

    private mutating func parseNumber() throws -> Any {
        let start = i
        var isDouble = false
        while i < u.count {
            let c = u[i]
            if (c >= 0x30 && c <= 0x39) || c == 0x2D || c == 0x2B { i += 1; continue }
            if c == 0x2E || c == 0x65 || c == 0x45 { isDouble = true; i += 1; continue }
            break
        }
        guard i > start else { throw Bad() }
        let s = String(decoding: u[start..<i], as: UTF16.self)
        if !isDouble, let n = Int(s) { return n }
        guard let d = Double(s) else { throw Bad() }
        return d
    }
}

// Đọc khoá của đối tượng JSON an toàn (thiếu / null / sai kiểu → mặc định), như optString / optInt của org.json.
enum J {
    /// Chuỗi của một khoá (null / thiếu / không phải chuỗi → "").
    static func str(_ o: [String: Any]?, _ key: String) -> String { ((o?[key] as? String) ?? "").trimmed }

    static func int(_ o: [String: Any]?, _ key: String, _ def: Int = 0) -> Int {
        switch o?[key] {
        case let n as Int: return n
        case let d as Double: return Int(d)
        case let s as String: return Int(s) ?? def
        default: return def
        }
    }

    static func bool(_ o: [String: Any]?, _ key: String, _ def: Bool = false) -> Bool {
        switch o?[key] {
        case let b as Bool: return b
        case let s as String: return s == "true" ? true : (s == "false" ? false : def)
        default: return def
        }
    }

    static func obj(_ o: [String: Any]?, _ key: String) -> [String: Any]? { o?[key] as? [String: Any] }

    static func arr(_ o: [String: Any]?, _ key: String) -> [Any]? { o?[key] as? [Any] }

    static func objects(_ a: [Any]?) -> [[String: Any]] { (a ?? []).compactMap { $0 as? [String: Any] } }
}
