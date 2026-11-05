import Foundation

/// Phim theo dõi: hỏi lại trang từng phim (tối đa 30 phút một lần) để biết phim nào có tập mới (bản Android: Follows.kt).
enum Follows {
    static let interval: Int64 = 30 * 60_000
    private static let lock = NSLock()
    /// Người xem đang được kiểm (mỗi người một danh sách theo dõi).
    private static var running = Set<String>()

    /// Kiểm nếu đã tới hạn (force: kiểm ngay). Trả về các phim vừa có thêm tập so với lần kiểm trước (để báo một lần);
    /// nil = không kiểm (chưa tới hạn / đang kiểm / không theo dõi phim nào) hoặc đã đổi người xem trong lúc kiểm.
    static func check(force: Bool = false) async -> [Follow]? {
        let owner = Prefs.profileId
        let list = Prefs.follows
        let now = nowMs()
        lock.lock()
        if list.isEmpty || running.contains(owner) || (!force && now - Prefs.lastFollowCheck < interval) {
            lock.unlock()
            return nil
        }
        running.insert(owner)
        lock.unlock()
        Prefs.lastFollowCheck = now
        defer {
            lock.lock()
            running.remove(owner)
            lock.unlock()
        }
        // Lần lượt từng phim (không chiếm hết mạng của trang chủ / ảnh)
        var fresh = Set<String>()
        for f in list {
            guard let d = try? await Site.detail(f.filmUrl), let ep = d.latestEpisode else { continue }
            // Đổi người xem trong lúc kiểm: danh sách theo dõi là của người cũ
            guard Prefs.profileId == owner else { return nil }
            if d.episodeCount > f.count { fresh.insert(f.filmUrl) }
            Prefs.updateFollow(f.filmUrl, count: d.episodeCount, latest: ep.title, seen: false)
        }
        guard Prefs.profileId == owner else { return nil }
        return Prefs.follows.filter { fresh.contains($0.filmUrl) && $0.newCount > 0 }
    }
}
