import Foundation

/// Phim theo dõi: hỏi lại trang từng phim (tối đa 30 phút một lần) để biết phim nào có tập mới (bản Android: Follows.kt).
enum Follows {
    static let interval: Int64 = 30 * 60_000
    private static let lock = NSLock()
    private static var running = false

    /// Kiểm nếu đã tới hạn (force: kiểm ngay). Trả về các phim vừa có thêm tập so với lần kiểm trước (để báo một lần);
    /// nil = không kiểm (chưa tới hạn / đang kiểm / không theo dõi phim nào).
    static func check(force: Bool = false) async -> [Follow]? {
        let list = Prefs.follows
        let now = nowMs()
        lock.lock()
        if list.isEmpty || running || (!force && now - Prefs.lastFollowCheck < interval) {
            lock.unlock()
            return nil
        }
        running = true
        lock.unlock()
        Prefs.lastFollowCheck = now
        defer {
            lock.lock()
            running = false
            lock.unlock()
        }
        // Lần lượt từng phim (không chiếm hết mạng của trang chủ / ảnh)
        var fresh = Set<String>()
        for f in list {
            guard let d = try? await Site.detail(f.filmUrl), let ep = d.latestEpisode else { continue }
            if d.episodeCount > f.count { fresh.insert(f.filmUrl) }
            Prefs.updateFollow(f.filmUrl, count: d.episodeCount, latest: ep.title, seen: false)
        }
        return Prefs.follows.filter { fresh.contains($0.filmUrl) && $0.newCount > 0 }
    }
}
