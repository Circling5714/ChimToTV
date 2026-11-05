import Foundation
import CryptoKit

/// Người xem (hồ sơ) — bản Android: Profile trong Prefs.kt. id "" = người xem đầu tiên: dữ liệu nằm ở các khoá cũ (như
/// trước khi có hồ sơ); người thêm sau dùng khoá có hậu tố "#<id>". color = chỉ số trong Prefs.profileColors; pin = mã băm
/// của mã PIN (nil = không khoá).
struct Profile: Codable, Hashable, Identifiable {
    var id: String
    var name: String
    var color: Int
    var pin: String?

    var locked: Bool { pin != nil }

    /// Chữ cái đầu tên (trên vòng tròn màu).
    var initial: String { name.trimmed.first.map { String($0).uppercased() } ?? "?" }
}

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

    // MARK: - Người xem (hồ sơ)

    static let maxProfiles = 8
    static let pinLength = 4
    private static let firstProfileName = "Cả nhà"

    /// Màu vòng tròn của người xem (RGB, cùng thứ tự bản Android); người thêm sau lấy màu chưa ai dùng.
    static let profileColors: [UInt32] = [0x2BAF85, 0xE5533D, 0x3B82F6, 0xF59E0B, 0x8B5CF6, 0x06B6D4, 0xEC4899, 0x84CC16]
    static let profileColorNames = ["Xanh lá", "Đỏ cam", "Xanh dương", "Vàng", "Tím", "Xanh ngọc", "Hồng", "Xanh nõn chuối"]

    /// Khoá riêng từng người xem (còn lại — danh sách trang, điểm server, mốc nhạc mở đầu, cập nhật… — dùng chung).
    private static let profileKeys: Set<String> = [
        "history", "positions_v2", "follows", "last_follow_check", "private_mode",
        "base_url", "seek_step", "auto_next", "video_fill",
    ]
    private static let profilePrefixes = ["server_type@", "server_label@"]
    /// Người thêm sau bắt đầu với trang phim / server ưu tiên / cách xem của người đang dùng.
    private static let copiedKeys: Set<String> = ["base_url", "seek_step", "auto_next", "video_fill"]

    private static var pidCache: String?

    /// Người xem đang dùng ("" = người đầu tiên).
    static var profileId: String {
        locked {
            if let p = pidCache { return p }
            let saved = store.string(forKey: "profile") ?? ""
            let list = profiles
            let p = list.contains { $0.id == saved } ? saved : list[0].id
            pidCache = p
            return p
        }
    }

    /// Khoá của người xem đang dùng.
    private static func k(_ key: String) -> String {
        let p = profileId
        return p.isEmpty ? key : "\(key)#\(p)"
    }

    private static func isProfileKey(_ base: String) -> Bool {
        profileKeys.contains(base) || profilePrefixes.contains { base.hasPrefix($0) }
    }

    /// Các khoá riêng đang có của một người xem: (khoá thật, khoá gốc không hậu tố).
    private static func ownKeys(_ id: String) -> [(key: String, base: String)] {
        store.dictionaryRepresentation().keys.compactMap { key in
            let base: String?
            if id.isEmpty {
                base = key.contains("#") ? nil : key
            } else {
                base = key.hasSuffix("#\(id)") ? String(key.dropLast(id.count + 1)) : nil
            }
            guard let b = base, isProfileKey(b) else { return nil }
            return (key, b)
        }
    }

    static var profiles: [Profile] {
        let list = decode([Profile].self, "profiles") ?? []
        return list.isEmpty ? [Profile(id: "", name: firstProfileName, color: 0, pin: nil)] : list
    }

    private static func saveProfiles(_ list: [Profile]) { encode(list, "profiles") }

    /// Người xem đang dùng.
    static var profile: Profile {
        let id = profileId
        let list = profiles
        return list.first { $0.id == id } ?? list[0]
    }

    static func switchProfile(_ id: String) {
        locked {
            guard profiles.contains(where: { $0.id == id }) else { return }
            pidCache = id
            store.set(id, forKey: "profile")
            historyCache = nil
            positionsCache = nil
            privatePositions.removeAll()
        }
    }

    /// Quên mọi thứ đang giữ trong bộ nhớ (đổi kho — test).
    static func reload() {
        locked {
            pidCache = nil
            historyCache = nil
            positionsCache = nil
            privatePositions.removeAll()
        }
    }

    /// Thêm người xem (chưa chuyển sang): lịch sử trống, trang phim / server ưu tiên / cách xem chép từ người đang dùng.
    @discardableResult
    static func addProfile(_ name: String) -> Profile {
        locked {
            let list = profiles
            var n = 1
            while list.contains(where: { $0.id == "p\(n)" }) { n += 1 }
            let id = "p\(n)"
            let used = Set(list.map { $0.color })
            let color = profileColors.indices.first { !used.contains($0) } ?? (list.count % profileColors.count)
            for (key, base) in ownKeys(profileId) where copiedKeys.contains(base) || profilePrefixes.contains(where: { base.hasPrefix($0) }) {
                store.set(store.object(forKey: key), forKey: "\(base)#\(id)")
            }
            store.set(baseUrl, forKey: "base_url#\(id)")
            let p = Profile(id: id, name: name.trimmed, color: color, pin: nil)
            saveProfiles(list + [p])
            return p
        }
    }

    static func renameProfile(_ id: String, _ name: String) {
        locked { saveProfiles(profiles.map { var p = $0; if p.id == id { p.name = name.trimmed }; return p }) }
    }

    static func setProfileColor(_ id: String, _ color: Int) {
        locked { saveProfiles(profiles.map { var p = $0; if p.id == id { p.color = color }; return p }) }
    }

    /// Đặt mã PIN (4 số); nil = tắt khoá.
    static func setProfilePin(_ id: String, _ pin: String?) {
        locked { saveProfiles(profiles.map { var p = $0; if p.id == id { p.pin = pin.map { pinHash(id, $0) } }; return p }) }
    }

    static func checkPin(_ p: Profile, _ pin: String) -> Bool { p.pin == nil || p.pin == pinHash(p.id, pin) }

    /// Không lưu mã PIN thật, chỉ lưu mã băm (khoá cho trẻ em khỏi vào nhầm, không phải mật khẩu bảo mật).
    private static func pinHash(_ id: String, _ pin: String) -> String {
        SHA256.hash(data: Data("chimto-pin:\(id):\(pin)".utf8)).map { String(format: "%02x", $0) }.joined()
    }

    /// Xoá người xem cùng toàn bộ dữ liệu riêng; còn một người thì không xoá được.
    static func deleteProfile(_ id: String) {
        locked {
            let list = profiles
            guard list.count > 1, list.contains(where: { $0.id == id }) else { return }
            for (key, _) in ownKeys(id) { store.removeObject(forKey: key) }
            let rest = list.filter { $0.id != id }
            saveProfiles(rest)
            if profileId == id { switchProfile(rest[0].id) }
        }
    }

    /// Hỏi "Ai đang xem?" khi mở app (từ 2 người xem).
    static var askProfile: Bool {
        get { store.object(forKey: "ask_profile") as? Bool ?? true }
        set { store.set(newValue, forKey: "ask_profile") }
    }

    /// Về màn hình chính của iPad lúc đang xem: thu nhỏ phim (hình trong hình) thay vì dừng.
    static var pipOnHome: Bool {
        get { store.object(forKey: "pip_on_home") as? Bool ?? true }
        set { store.set(newValue, forKey: "pip_on_home") }
    }

    // MARK: - Trang phim

    /// Trang phim đang dùng (địa chỉ gốc, không có dấu / ở cuối). Đặt trang mới thì tự thêm vào danh sách. Riêng từng
    /// người xem.
    static var baseUrl: String {
        get { store.string(forKey: k("base_url")) ?? store.string(forKey: "base_url") ?? defaultBase }
        set {
            store.set(newValue, forKey: k("base_url"))
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

    /// Thêm trang vào danh sách (không đổi trang đang dùng); đã có thì thôi.
    static func addSite(_ url: String) {
        if !sites.contains(url) { sites = sites + [url] }
    }

    /// Bỏ trang khỏi danh sách (còn ít nhất một trang); đang dùng trang đó thì sang trang đầu danh sách.
    @discardableResult
    static func removeSite(_ url: String) -> Bool {
        let rest = sites.filter { $0 != url }
        guard !rest.isEmpty else { return false }
        sites = rest
        if baseUrl == url { baseUrl = rest[0] }
        return true
    }

    /// Trang mẫu không còn trong danh sách (người xem đã xoá) — để thêm lại nhanh.
    static var missingPresets: [String] {
        let list = sites
        return presetSites.filter { !list.contains($0) }
    }

    /// Server người xem chọn lần gần nhất trên từng trang (vd "tiktik").
    static func serverType(_ host: String?) -> String? { store.string(forKey: k("server_type@\(host ?? "")")) }
    static func serverLabel(_ host: String?) -> String? { store.string(forKey: k("server_label@\(host ?? "")")) }

    static func setServer(_ host: String?, type: String?, label: String?) {
        store.set(type, forKey: k("server_type@\(host ?? "")"))
        store.set(label, forKey: k("server_label@\(host ?? "")"))
    }

    static var seekStep: Int {
        get { store.object(forKey: k("seek_step")) as? Int ?? 10 }
        set { store.set(newValue, forKey: k("seek_step")) }
    }

    static var autoNext: Bool {
        get { store.object(forKey: k("auto_next")) as? Bool ?? true }
        set { store.set(newValue, forKey: k("auto_next")) }
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
            let h = decode([HistoryItem].self, k("history")) ?? []
            historyCache = h
            return h
        }
    }

    private static func saveHistory(_ list: [HistoryItem]) {
        locked {
            historyCache = list
            encode(list, k("history"))
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
        get { decode([Follow].self, k("follows")) ?? [] }
        set { encode(newValue, k("follows")) }
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
        get { Int64(store.double(forKey: k("last_follow_check"))) }
        set { store.set(Double(newValue), forKey: k("last_follow_check")) }
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
        get { store.bool(forKey: k("video_fill")) }
        set { store.set(newValue, forKey: k("video_fill")) }
    }

    // MARK: - Riêng tư

    /// Xem riêng: không ghi lịch sử xem và chỗ đang xem (vẫn đọc được chỗ đã lưu từ trước).
    static var privateMode: Bool {
        get { store.bool(forKey: k("private_mode")) }
        set { store.set(newValue, forKey: k("private_mode")) }
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

    // MARK: - Proxy SOCKS5 (xem AppProxy) — dùng chung cả nhà

    static var proxyOn: Bool {
        get { store.bool(forKey: "proxy_on") }
        set { store.set(newValue, forKey: "proxy_on") }
    }

    static var proxyHost: String {
        get { store.string(forKey: "proxy_host") ?? "" }
        set { store.set(newValue, forKey: "proxy_host") }
    }

    static var proxyPort: Int {
        get { store.object(forKey: "proxy_port") as? Int ?? 1080 }
        set { store.set(newValue, forKey: "proxy_port") }
    }

    static var proxyUser: String {
        get { store.string(forKey: "proxy_user") ?? "" }
        set { store.set(newValue, forKey: "proxy_user") }
    }

    private static var proxyPassCache: String?

    /// Mật khẩu proxy: lưu trong Keychain của iPad (bản Android: Android Keystore). Keychain không dùng được (app thiếu
    /// quyền) thì lưu trong cài đặt của app như bản Android cũ.
    static var proxyPass: String {
        get {
            locked {
                if let c = proxyPassCache { return c }
                let v = Secret.get("proxy_pass") ?? store.string(forKey: "proxy_pass") ?? ""
                proxyPassCache = v
                return v
            }
        }
        set {
            locked {
                proxyPassCache = newValue
                if Secret.set("proxy_pass", newValue) { store.removeObject(forKey: "proxy_pass") } else { store.set(newValue, forKey: "proxy_pass") }
            }
        }
    }

    /// true: DNS của proxy; false: DNS của iPad.
    static var proxyRemoteDns: Bool {
        get { store.object(forKey: "proxy_remote_dns") as? Bool ?? true }
        set { store.set(newValue, forKey: "proxy_remote_dns") }
    }

    /// true: chỉ trang phim đi qua proxy; false: toàn bộ app.
    static var proxySitesOnly: Bool {
        get { store.bool(forKey: "proxy_sites_only") }
        set { store.set(newValue, forKey: "proxy_sites_only") }
    }

    /// Proxy đang dùng; nil = tắt (hoặc chưa nhập máy chủ / cổng hợp lệ).
    static func proxyConfig() -> SocksConfig? {
        guard proxyOn else { return nil }
        let host = proxyHost.trimmed
        guard !host.isEmpty, (1...65535).contains(proxyPort) else { return nil }
        return SocksConfig(host: host, port: proxyPort, user: proxyUser, pass: proxyPass, remoteDns: proxyRemoteDns, sitesOnly: proxySitesOnly)
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
            let p = decode([Position].self, k("positions_v2")) ?? []
            positionsCache = p
            return p
        }
    }

    private static func savePositions(_ list: [Position]) {
        locked {
            positionsCache = list
            encode(list, k("positions_v2"))
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
        store.dictionaryRepresentation().filter { k, _ in
            !k.hasPrefix("Apple") && !k.hasPrefix("NS") && !k.hasPrefix("com.apple") && k != "proxy_pass"
        }
    }
}
