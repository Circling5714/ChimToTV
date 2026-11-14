import Foundation

/// Tìm bản iPad / Mac mới trên GitHub Releases (bản Android: Updater.kt). Bản iPad phát hành bằng tag riêng "ios-v…", bản Mac
/// "mac-v…", không bao giờ là bản "latest" (app TV đọc releases/latest và lấy tệp .apk) → ở đây duyệt danh sách release, lấy
/// tag của nền tảng đang chạy. iPad / iPhone cài bằng TrollStore (apple-magnifier://install?url=<link .ipa>); Mac tự thay
/// chính nó (MacUpdate.swift).
enum UpdateCheck {
    static let releasesApi = "https://api.github.com/repos/Circling5714/ChimToTV/releases?per_page=30"
    static let checkEvery: Int64 = 2 * 60 * 60 * 1000
    static let snooze: Int64 = 24 * 60 * 60 * 1000

    /// Tag và loại tệp của từng nền tảng.
    struct Platform {
        let tagPrefix: String
        let fileSuffix: String
        static let ios = Platform(tagPrefix: "ios-v", fileSuffix: ".ipa")
        static let mac = Platform(tagPrefix: "mac-v", fileSuffix: ".zip")
        static var current: Platform {
            #if targetEnvironment(macCatalyst)
            return .mac
            #else
            return .ios
            #endif
        }
    }

    struct Release {
        let version: String
        /// Link tải tệp cài: .ipa (iPad / iPhone) hoặc .zip chứa ConBangTV.app (Mac)
        let fileUrl: String
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

    /// Bản mới nhất của nền tảng (bỏ bản nháp / thử nghiệm); nil = chưa có bản nào.
    static func latest(api: String = releasesApi, platform: Platform = .current) async throws -> Release? {
        let r = try await Http.get(api, headers: ["Accept": "application/vnd.github+json"])
        return pick(r.data, platform: platform)
    }

    /// Đọc danh sách release (JSON của GitHub) → bản mới nhất của nền tảng.
    static func pick(_ json: Data, platform: Platform) -> Release? {
        guard let list = (try? JSONSerialization.jsonObject(with: json)) as? [[String: Any]] else { return nil }
        let releases: [Release] = list.compactMap { o in
            let tag = (o["tag_name"] as? String) ?? ""
            guard tag.hasPrefix(platform.tagPrefix), o["draft"] as? Bool != true, o["prerelease"] as? Bool != true,
                  let assets = o["assets"] as? [[String: Any]],
                  let file = assets.first(where: { (($0["name"] as? String) ?? "").lowercased().hasSuffix(platform.fileSuffix) }),
                  let url = file["browser_download_url"] as? String else { return nil }
            return Release(version: String(tag.dropFirst(platform.tagPrefix.count)), fileUrl: url,
                           notes: (o["body"] as? String) ?? "", tag: tag)
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
        // Ghi chú lỡ lưu kèm BOM (Windows 1.16, 14/11/2026) thì dòng «## Có gì mới» không bắt đầu bằng "#"
        let clean = notes.hasPrefix("\u{FEFF}") ? String(notes.dropFirst()) : notes
        let lines = clean.components(separatedBy: .newlines).map { $0.trimmed }
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
