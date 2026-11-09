import Foundation

/// Tìm bản iPad mới trên GitHub Releases (bản Android: Updater.kt). Bản iPad phát hành bằng tag riêng "ios-v…" và không
/// bao giờ là bản "latest" (app TV đọc releases/latest và lấy tệp .apk) → ở đây duyệt danh sách release, lấy tag ios-v*.
/// Cài bằng TrollStore: mở apple-magnifier://install?url=<link .ipa>.
enum UpdateCheck {
    static let releasesApi = "https://api.github.com/repos/Circling5714/ChimToTV/releases?per_page=30"
    static let checkEvery: Int64 = 2 * 60 * 60 * 1000
    static let snooze: Int64 = 24 * 60 * 60 * 1000

    struct Release {
        let version: String
        let ipaUrl: String
        let notes: String
        let tag: String
    }

    static var currentVersion: String {
        #if DEVTOOLS
        // Bản thử: giả bản đang dùng (khoá debug_fake_version, đặt qua /prefs) để thử hộp thoại «Có bản mới»
        if let v = UserDefaults.standard.string(forKey: "debug_fake_version"), !v.isEmpty { return v }
        #endif
        return (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String) ?? "0"
    }

    /// Bản iPad mới nhất (bỏ bản nháp / thử nghiệm); nil = chưa có bản nào.
    static func latest(api: String = releasesApi) async throws -> Release? {
        let r = try await Http.get(api, headers: ["Accept": "application/vnd.github+json"])
        guard let list = try JSONSerialization.jsonObject(with: r.data) as? [[String: Any]] else { return nil }
        let releases: [Release] = list.compactMap { o in
            let tag = (o["tag_name"] as? String) ?? ""
            guard tag.hasPrefix("ios-v"), o["draft"] as? Bool != true, o["prerelease"] as? Bool != true,
                  let assets = o["assets"] as? [[String: Any]],
                  let ipa = assets.first(where: { (($0["name"] as? String) ?? "").lowercased().hasSuffix(".ipa") }),
                  let url = ipa["browser_download_url"] as? String else { return nil }
            return Release(version: String(tag.dropFirst("ios-v".count)), ipaUrl: url, notes: (o["body"] as? String) ?? "", tag: tag)
        }
        return releases.max { isNewer($1.version, than: $0.version) }
    }

    static func isNewer(_ remote: String, than local: String = currentVersion) -> Bool {
        let a = parts(remote), b = parts(local)
        for i in 0..<max(a.count, b.count) {
            let x = a[safe: i] ?? 0, y = b[safe: i] ?? 0
            if x != y { return x > y }
        }
        return false
    }

    private static func parts(_ v: String) -> [Int] {
        v.before("-").split(separator: ".").map { Int($0.trimmingCharacters(in: .whitespaces)) ?? 0 }
    }

    /// Ghi chú phát hành (markdown) → vài dòng chữ thường cho hộp thoại; có mục "Có gì mới" thì chỉ lấy mục đó.
    static func summary(_ notes: String) -> String {
        let lines = notes.components(separatedBy: .newlines).map { $0.trimmed }
        let start = lines.firstIndex { $0.hasPrefix("#") && $0.lowercased().contains("có gì mới") }
        var section = lines
        if let s = start {
            section = Array(lines[(s + 1)...])
            if let end = section.firstIndex(where: { $0.hasPrefix("## ") }) { section = Array(section[..<end]) }
        }
        let text = section
            .filter { !$0.isEmpty && !$0.hasPrefix("![") && !$0.hasPrefix("|") && !$0.contains("SHA-256") }
            .map { $0.trimmingChars("#-* ").replacingOccurrences(of: "**", with: "").replacingOccurrences(of: "`", with: "") }
            .filter { !$0.isEmpty }
            .joined(separator: "\n")
        return String(text.prefix(600))
    }

    /// Đường dẫn mở TrollStore để cài .ipa từ link.
    static func trollStoreURL(_ ipa: String) -> URL? {
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._~")
        guard let enc = ipa.addingPercentEncoding(withAllowedCharacters: allowed) else { return nil }
        return URL(string: "apple-magnifier://install?url=" + enc)
    }
}
