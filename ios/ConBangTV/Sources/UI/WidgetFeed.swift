import UIKit
import WidgetKit

/// Ghi dữ liệu cho widget «Xem tiếp» (WidgetFeedData): 6 phim gần nhất trong lịch sử của người xem đang dùng, kèm ảnh
/// bìa nhỏ. Gọi sau mỗi lần lịch sử / người xem đổi; gộp các lần gọi sát nhau, chỉ báo widget vẽ lại khi dữ liệu khác.
@MainActor
enum WidgetFeed {
    static let maxItems = 6
    private static var pending: Task<Void, Never>?

    static func update() {
        #if targetEnvironment(macCatalyst)
        // Bản Mac không có widget; ghi vào thư mục App Group của app khác mã gói làm macOS 15 hỏi quyền
        return
        #endif
        pending?.cancel()
        pending = Task {
            try? await Task.sleep(nanoseconds: 400_000_000)
            guard !Task.isCancelled else { return }
            await write()
        }
    }

    private static func write() async {
        guard let folder = WidgetFeedData.folder, let posters = WidgetFeedData.posterFolder else {
            Log.d("Widget", "không mở được thư mục App Group \(WidgetFeedData.groupId)")
            return
        }
        let fm = FileManager.default
        try? fm.createDirectory(at: posters, withIntermediateDirectories: true)
        let profile = Prefs.profile
        let history = Array(Prefs.history.prefix(maxItems))
        var items: [WidgetFeedData.Item] = []
        for h in history {
            var file: String?
            if let url = h.poster {
                let name = ImageCache.hash(url) + ".jpg"
                let dest = posters.appendingPathComponent(name)
                if fm.fileExists(atPath: dest.path) {
                    file = name
                } else if let img = await ImageCache.shared.image(url, maxPixel: 360),
                          let jpg = img.jpegData(compressionQuality: 0.8) {
                    try? jpg.write(to: dest, options: .atomic)
                    file = name
                }
            }
            let host = Site.hostOf(h.filmUrl) ?? ""
            items.append(.init(title: h.title, episode: h.epTitle, site: host.before("."), filmUrl: h.filmUrl,
                               epUrl: h.epUrl, poster: file))
        }
        // Bỏ ảnh không còn dùng
        let keep = Set(items.compactMap { $0.poster })
        for f in (try? fm.contentsOfDirectory(atPath: posters.path)) ?? [] where !keep.contains(f) {
            try? fm.removeItem(at: posters.appendingPathComponent(f))
        }
        let note: String? = items.isEmpty
            ? (Prefs.privateMode ? "\(profile.name) đang bật Xem riêng." : "Chưa có phim đang xem.")
            : nil
        var feed = WidgetFeedData(profileId: profile.id, profileName: profile.name, items: items, note: note, updated: 0)
        if var old = WidgetFeedData.load() {
            old.updated = 0
            if old == feed { return } // không đổi gì: khỏi bắt widget vẽ lại
        }
        feed.updated = Date().timeIntervalSince1970
        guard let file = WidgetFeedData.feedFile, let data = try? JSONEncoder().encode(feed) else { return }
        try? fm.createDirectory(at: folder, withIntermediateDirectories: true)
        do {
            try data.write(to: file, options: .atomic)
            WidgetCenter.shared.reloadAllTimelines()
            Log.d("Widget", "đã ghi \(items.count) phim cho widget (\(profile.name))")
        } catch {
            Log.d("Widget", "không ghi được feed.json: \(error.localizedDescription)")
        }
    }

    /// Link widget: conbangtv://play?p=…&film=…&ep=… → (người xem, phim, tập).
    static func parse(_ url: URL) -> (profile: String, film: String, ep: String)? {
        guard url.scheme == WidgetFeedData.scheme, url.host == "play",
              let q = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems else { return nil }
        func v(_ n: String) -> String? { q.first { $0.name == n }?.value }
        guard let film = v("film"), let ep = v("ep"), film.hasPrefix("http"), ep.hasPrefix("http") else { return nil }
        return (v("p") ?? "", film, ep)
    }
}
