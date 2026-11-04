import Foundation

/// Cài đặt + lịch sử xem (bản Android: Prefs.kt, cùng tên khoá). Lưu trong UserDefaults; danh sách giữ sẵn trong bộ nhớ.
enum Prefs {
    static let defaultBase = "https://yanhh3d.ee"

    /// Các trang mẫu có sẵn trong danh sách lần đầu cài.
    static let presetSites = [defaultBase, "https://hoathinh3d.pw", "https://hoathinh3d.ee", "https://hhkungfu.ee", "https://phimmoir.fun"]

    private static let maxHistory = 60
    private static let maxPositions = 400

    /// Bản thử (Beta) có thể dùng kho riêng khi chạy test trên máy tính.
    static var store: UserDefaults = .standard
    private static let lock = NSRecursiveLock()

    private static func locked<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }

    // MARK: - Trang phim

    /// Trang phim đang dùng (địa chỉ gốc, không có dấu / ở cuối). Đặt trang mới thì tự thêm vào danh sách.
    static var baseUrl: String {
        get { store.string(forKey: "base_url") ?? defaultBase }
        set {
            store.set(newValue, forKey: "base_url")
            if !sites.contains(newValue) { sites = sites + [newValue] }
        }
    }

    /// Danh sách trang phim để chọn nhanh (mặc định: các trang mẫu).
    static var sites: [String] {
        get { store.stringArray(forKey: "sites") ?? presetSites }
        set {
            var seen = Set<String>()
            store.set(newValue.filter { seen.insert($0).inserted }, forKey: "sites")
        }
    }

    /// Server người xem chọn lần gần nhất trên từng trang (vd "tiktik").
    static func serverType(_ host: String?) -> String? { store.string(forKey: "server_type@\(host ?? "")") }
    static func serverLabel(_ host: String?) -> String? { store.string(forKey: "server_label@\(host ?? "")") }

    static func setServer(_ host: String?, type: String?, label: String?) {
        store.set(type, forKey: "server_type@\(host ?? "")")
        store.set(label, forKey: "server_label@\(host ?? "")")
    }

    static var seekStep: Int {
        get { store.object(forKey: "seek_step") as? Int ?? 10 }
        set { store.set(newValue, forKey: "seek_step") }
    }

    static var autoNext: Bool {
        get { store.object(forKey: "auto_next") as? Bool ?? true }
        set { store.set(newValue, forKey: "auto_next") }
    }

    /// Chuẩn hoá chuỗi người dùng gõ ("yanhh3d.ee", "https://yanhh3d.ee/") → "https://yanhh3d.ee"; sai thì nil.
    static func normalizeUrl(_ input: String) -> String? {
        var s = input.trimmed.trimmingSuffix("/")
        if s.isEmpty || s.contains(" ") { return nil }
        if !s.lowercased().hasPrefix("http://") && !s.lowercased().hasPrefix("https://") { s = "https://" + s }
        guard let host = URL(string: s)?.host, host.contains(".") else { return nil }
        return s
    }

    // MARK: - Chấm điểm server (xem PlayStats)

    struct Stat: Codable {
        /// ms từ lúc mở tới khi phát (-1 = không đo), giây chạy, giây đứng hình, số lần giật, lỗi
        var s: Int64, p: Int, w: Int, n: Int, x: Bool
    }

    static var playStats: [String: [Stat]] {
        get { decode([String: [Stat]].self, "play_stats") ?? [:] }
        set { encode(newValue, "play_stats") }
    }

    // MARK: - Cập nhật

    static var autoUpdate: Bool {
        get { store.object(forKey: "auto_update") as? Bool ?? true }
        set { store.set(newValue, forKey: "auto_update") }
    }

    static var lastUpdateCheck: Int64 {
        get { Int64(store.double(forKey: "last_update_check")) }
        set { store.set(Double(newValue), forKey: "last_update_check") }
    }

    static func snoozeUpdate(_ version: String, until: Int64) {
        store.set(version, forKey: "snooze_version")
        store.set(Double(until), forKey: "snooze_until")
    }

    static func isUpdateSnoozed(_ version: String) -> Bool {
        store.string(forKey: "snooze_version") == version && nowMs() < Int64(store.double(forKey: "snooze_until"))
    }

    // MARK: - Lịch sử xem

    private static var historyCache: [HistoryItem]?

    static var history: [HistoryItem] {
        locked {
            if let h = historyCache { return h }
            let h = decode([HistoryItem].self, "history") ?? []
            historyCache = h
            return h
        }
    }

    private static func saveHistory(_ list: [HistoryItem]) {
        locked {
            historyCache = list
            encode(list, "history")
        }
    }

    static func historyFor(_ filmUrl: String) -> HistoryItem? { history.first { $0.filmUrl == filmUrl } }

    static func addHistory(_ item: HistoryItem) {
        locked { saveHistory(Array(([item] + history.filter { $0.filmUrl != item.filmUrl }).prefix(maxHistory))) }
    }

    static func clearHistory() {
        locked {
            privatePositions.removeAll()
            saveHistory([])
            savePositions([])
        }
    }

    /// Xoá một phim khỏi lịch sử, kèm chỗ đang xem / đánh dấu đã xem của các tập phim đó.
    static func removeHistory(_ filmUrl: String) {
        locked {
            let item = historyFor(filmUrl)
            let eps = Set(positions().filter { $0.film == filmUrl }.map { $0.ep } + [item?.epUrl].compactMap { $0 })
            for e in eps { privatePositions[e] = nil }
            savePositions(positions().filter { !eps.contains($0.ep) })
            saveHistory(history.filter { $0.filmUrl != filmUrl })
        }
    }

    // MARK: - Phim theo dõi

    static var follows: [Follow] {
        get { decode([Follow].self, "follows") ?? [] }
        set { encode(newValue, "follows") }
    }

    static func followOf(_ filmUrl: String) -> Follow? { follows.first { $0.filmUrl == filmUrl } }

    static func follow(_ d: FilmDetail) {
        locked {
            let n = d.episodeCount
            follows = [Follow(filmUrl: d.url, title: d.title, poster: d.poster, count: n, latest: d.latestEpisode?.title ?? "", seen: n)] +
                follows.filter { $0.filmUrl != d.url }
        }
    }

    static func unfollow(_ filmUrl: String) { locked { follows = follows.filter { $0.filmUrl != filmUrl } } }

    /// Cập nhật số tập mới nhất; seen = true: người xem vừa mở phim (hết "tập mới").
    static func updateFollow(_ filmUrl: String, count: Int, latest: String, seen: Bool) {
        locked {
            var list = follows
            guard let i = list.firstIndex(where: { $0.filmUrl == filmUrl }) else { return }
            // Trang bớt tập (gộp / sửa danh sách) thì không để "tập mới" kẹt mãi
            let wasSeen = list[i].seen
            list[i].count = count
            list[i].latest = latest
            list[i].seen = (seen || count < wasSeen) ? count : wasSeen
            follows = list
        }
    }

    static var lastFollowCheck: Int64 {
        get { Int64(store.double(forKey: "last_follow_check")) }
        set { store.set(Double(newValue), forKey: "last_follow_check") }
    }

    // MARK: - Nhạc mở đầu (mốc riêng từng phim, giây)

    /// (đầu, hết) nhạc mở đầu; hết = -1 là mới đánh dấu đầu.
    static func intro(_ filmUrl: String) -> (start: Int, end: Int)? {
        guard let p = store.string(forKey: "intro@\(filmUrl)")?.split(separator: ","), let s = p.first.flatMap({ Int($0) }) else { return nil }
        return (s, p.count > 1 ? Int(p[1]) ?? -1 : -1)
    }

    static func setIntro(_ filmUrl: String, start: Int, end: Int) { store.set("\(start),\(end)", forKey: "intro@\(filmUrl)") }

    static func clearIntro(_ filmUrl: String) { store.removeObject(forKey: "intro@\(filmUrl)") }

    /// Lấp đầy màn hình: video rộng (21:9) phóng cho hết viền đen, cắt bớt hai bên.
    static var videoFill: Bool {
        get { store.bool(forKey: "video_fill") }
        set { store.set(newValue, forKey: "video_fill") }
    }

    // MARK: - Riêng tư

    /// Xem riêng: không ghi lịch sử xem và chỗ đang xem (vẫn đọc được chỗ đã lưu từ trước).
    static var privateMode: Bool {
        get { store.bool(forKey: "private_mode") }
        set { store.set(newValue, forKey: "private_mode") }
    }

    /// Tự xoá dữ liệu duyệt web khi rời app.
    static var autoClearWeb: Bool {
        get { store.bool(forKey: "auto_clear_web") }
        set { store.set(newValue, forKey: "auto_clear_web") }
    }

    /// Trình duyệt trong app đã chạy từ lần xoá dữ liệu gần nhất (mặc định true: chưa xoá lần nào).
    static var webDirty: Bool {
        get { store.object(forKey: "web_dirty") as? Bool ?? true }
        set { store.set(newValue, forKey: "web_dirty") }
    }

    // MARK: - Vị trí đang xem từng tập (giây)

    struct Position: Codable {
        let ep: String
        var sec: Int
        var film: String
    }

    /// Thứ tự lưu: cũ nhất trước (quá maxPositions thì bỏ bớt đầu danh sách).
    private static var positionsCache: [Position]?

    private static func positions() -> [Position] {
        locked {
            if let p = positionsCache { return p }
            let p = decode([Position].self, "positions_v2") ?? []
            positionsCache = p
            return p
        }
    }

    private static func savePositions(_ list: [Position]) {
        locked {
            positionsCache = list
            encode(list, "positions_v2")
        }
    }

    /// Xem riêng: chỗ đang xem chỉ giữ trong bộ nhớ tới khi tắt app.
    private static var privatePositions: [String: Int] = [:]

    static func position(_ epUrl: String) -> Int {
        locked { privatePositions[epUrl] ?? positions().last { $0.ep == epUrl }?.sec ?? 0 }
    }

    /// Tập đã xem hết (lưu vị trí 0 khi xem tới ~90 giây cuối).
    static func finished(_ epUrl: String) -> Bool {
        locked {
            if let p = privatePositions[epUrl] { return p == 0 }
            return positions().last { $0.ep == epUrl }?.sec == 0
        }
    }

    static func watchedEpisodes() -> Set<String> { Set(positions().map { $0.ep }) }

    static func savePosition(_ epUrl: String, seconds: Int, filmUrl: String) {
        locked {
            if privateMode {
                privatePositions[epUrl] = seconds
                return
            }
            privatePositions[epUrl] = nil
            var list = positions().filter { $0.ep != epUrl }
            list.append(Position(ep: epUrl, sec: seconds, film: filmUrl))
            if list.count > maxPositions { list.removeFirst(list.count - maxPositions) }
            savePositions(list)
        }
    }

    // MARK: - JSON trong UserDefaults

    private static func decode<T: Decodable>(_ type: T.Type, _ key: String) -> T? {
        guard let d = store.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(type, from: d)
    }

    private static func encode<T: Encodable>(_ value: T, _ key: String) {
        if let d = try? JSONEncoder().encode(value) { store.set(d, forKey: key) }
    }

    /// Toàn bộ khoá (cho cổng gỡ lỗi /prefs).
    static func dump() -> [String: Any] {
        store.dictionaryRepresentation().filter { k, _ in !k.hasPrefix("Apple") && !k.hasPrefix("NS") && !k.hasPrefix("com.apple") }
    }
}
