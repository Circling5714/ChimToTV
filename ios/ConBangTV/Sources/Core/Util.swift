import Foundation

// Tiện ích nhỏ dùng chung: regex, chuỗi, thời gian, chạy có giới hạn thời gian.

/// Biểu thức chính quy (NSRegularExpression, vị trí theo UTF-16 như NSString).
struct Rx {
    let re: NSRegularExpression

    init(_ pattern: String, _ options: NSRegularExpression.Options = []) {
        // Mẫu viết sẵn trong code — sai là lỗi lập trình
        re = try! NSRegularExpression(pattern: pattern, options: options)
    }

    /// Nhóm của lần khớp đầu tiên (nhóm 0 = cả chuỗi khớp); nhóm không khớp → "".
    func groups(in s: String, from: Int = 0) -> [String]? {
        let ns = s as NSString
        guard from <= ns.length,
              let m = re.firstMatch(in: s, range: NSRange(location: from, length: ns.length - from)) else { return nil }
        return (0..<m.numberOfRanges).map { i in
            let r = m.range(at: i)
            return r.location == NSNotFound ? "" : ns.substring(with: r)
        }
    }

    func group(_ i: Int, in s: String) -> String? { groups(in: s).map { $0[i] } }

    func matches(_ s: String) -> Bool {
        re.firstMatch(in: s, range: NSRange(location: 0, length: (s as NSString).length)) != nil
    }

    func all(in s: String, from: Int = 0) -> [NSTextCheckingResult] {
        let ns = s as NSString
        guard from <= ns.length else { return [] }
        return re.matches(in: s, range: NSRange(location: from, length: ns.length - from))
    }

    func replace(in s: String, with template: String) -> String {
        re.stringByReplacingMatches(in: s, range: NSRange(location: 0, length: (s as NSString).length), withTemplate: template)
    }
}

extension String {
    var trimmed: String { trimmingCharacters(in: .whitespacesAndNewlines) }

    /// nil khi rỗng (sau khi bỏ khoảng trắng).
    var nonEmpty: String? { let t = trimmed; return t.isEmpty ? nil : t }

    func trimmingSuffix(_ ch: Character) -> String {
        var s = self
        while s.last == ch { s.removeLast() }
        return s
    }

    func trimmingChars(_ set: String) -> String { trimmingCharacters(in: CharacterSet(charactersIn: set)) }

    /// Phần sau lần xuất hiện cuối của ký tự (không có thì cả chuỗi), như substringAfterLast của Kotlin.
    func afterLast(_ ch: Character) -> String {
        guard let i = lastIndex(of: ch) else { return self }
        return String(self[index(after: i)...])
    }

    func before(_ sep: String) -> String {
        guard let r = range(of: sep) else { return self }
        return String(self[..<r.lowerBound])
    }

    /// Mã hoá cho tham số trong địa chỉ (giống URLEncoder của Java: dấu cách → +).
    var formEncoded: String {
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._*")
        return (addingPercentEncoding(withAllowedCharacters: allowed) ?? self).replacingOccurrences(of: "%20", with: "+")
    }
}

extension Array {
    subscript(safe i: Int) -> Element? { i >= 0 && i < count ? self[i] : nil }
}

extension Sequence {
    /// Bỏ phần tử trùng khoá, giữ phần tử đầu tiên (distinctBy của Kotlin).
    func distinct<K: Hashable>(by key: (Element) -> K) -> [Element] {
        var seen = Set<K>()
        return filter { seen.insert(key($0)).inserted }
    }
}

/// "1:05", "1:02:03"
func formatTime(_ seconds: Double) -> String {
    let s = max(0, Int(seconds.isFinite ? seconds : 0))
    let h = s / 3600, m = (s % 3600) / 60, sec = s % 60
    return h > 0 ? String(format: "%d:%02d:%02d", h, m, sec) : String(format: "%d:%02d", m, sec)
}

/// Thời gian hiện tại (ms kể từ 1970), như System.currentTimeMillis().
func nowMs() -> Int64 { Int64(Date().timeIntervalSince1970 * 1000) }

/// Chạy việc với giới hạn thời gian; quá hạn thì báo lỗi (việc dở dang bị huỷ).
func withTimeout<T>(_ seconds: Double, _ work: @escaping () async throws -> T) async throws -> T {
    try await withThrowingTaskGroup(of: T.self) { group in
        group.addTask { try await work() }
        group.addTask {
            try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
            throw AppError("Quá \(Int(seconds)) giây không trả lời")
        }
        let first = try await group.next()!
        group.cancelAll()
        return first
    }
}

/// Câu báo lỗi dễ hiểu cho người xem.
func friendly(_ error: Error) -> String {
    if let e = error as? AppError { return e.message }
    if let e = error as? URLError {
        switch e.code {
        case .notConnectedToInternet: return "Không có mạng."
        case .timedOut: return "Trang không trả lời (quá thời gian chờ)."
        case .cannotFindHost, .dnsLookupFailed: return "Không tìm thấy tên miền (trang đổi tên miền hoặc bị chặn?)."
        case .cannotConnectToHost: return "Không kết nối được tới trang."
        case .networkConnectionLost: return "Mất kết nối giữa chừng."
        case .secureConnectionFailed, .serverCertificateUntrusted, .serverCertificateHasBadDate:
            return "Lỗi kết nối bảo mật (chứng chỉ của trang)."
        default: return "Lỗi mạng (\(e.code.rawValue))."
        }
    }
    return error.localizedDescription
}
