import Foundation

/// Dữ liệu app ghi cho widget «Xem tiếp» (dùng chung giữa app và widget): feed.json + ảnh bìa nhỏ trong thư mục
/// App Group. Widget không tự tải gì từ mạng — app ghi lại mỗi lần lịch sử xem / người xem đổi.
struct WidgetFeedData: Codable, Equatable {
    struct Item: Codable, Equatable, Hashable {
        let title: String
        let episode: String
        /// Tên ngắn của trang (vd "yanhh3d").
        let site: String
        let filmUrl: String
        let epUrl: String
        /// Tên tệp ảnh bìa trong thư mục posters/ (nil = chưa có ảnh).
        let poster: String?
    }

    var profileId: String
    var profileName: String
    var items: [Item]
    /// Xem riêng / chưa có lịch sử: widget ghi lời nhắn thay cho danh sách.
    var note: String?
    var updated: Double

    /// Mã nhóm App Group và lược đồ URL mở app (ghi trong Info.plist của cả app lẫn widget, theo bản thử / bản chính).
    static var groupId: String { Bundle.main.object(forInfoDictionaryKey: "ConBangAppGroup") as? String ?? "group.vn.chimto.tv" }
    static var scheme: String { Bundle.main.object(forInfoDictionaryKey: "ConBangURLScheme") as? String ?? "conbangtv" }

    static var folder: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: groupId)?
            .appendingPathComponent("Widget", isDirectory: true)
    }

    static var feedFile: URL? { folder?.appendingPathComponent("feed.json") }
    static var posterFolder: URL? { folder?.appendingPathComponent("posters", isDirectory: true) }

    static func load() -> WidgetFeedData? {
        guard let f = feedFile, let d = try? Data(contentsOf: f) else { return nil }
        return try? JSONDecoder().decode(WidgetFeedData.self, from: d)
    }

    /// Link mở app phát tiếp một phim: conbangtv://play?p=<người xem>&film=<link phim>&ep=<link tập>.
    static func playURL(_ item: Item, profile: String) -> URL? {
        var c = URLComponents()
        c.scheme = scheme
        c.host = "play"
        c.queryItems = [
            URLQueryItem(name: "p", value: profile),
            URLQueryItem(name: "film", value: item.filmUrl),
            URLQueryItem(name: "ep", value: item.epUrl),
        ]
        return c.url
    }

    static var openURL: URL? { URL(string: "\(scheme)://open") }
}
