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
    static let presetSites = [defaultBase, "https://hoathinh3d.pw", "https://hoathinh3d.ee", "https://hhkungfu.ee", "https://phimmois.fun",
                              "https://mephim.ddns.net", kkSite, ncSite, vsSite, k20Site]

    /// Trang KKPhim (từ 1.6) — dữ liệu lấy thẳng từ API công khai phimapi.com (xem ApiSite).
    static let kkSite = "https://www.kkphim.com"

    /// Từ 1.7: NguonC (API phim.nguonc.com), VSMOV (API vsmov.com), CLB Phim Xưa (add-on Stremio k-20).
    static let ncSite = "https://phim.nguonc.com"
    static let vsSite = "https://vsmov.com"
    static let k20Site = "https://sc.k-20.xyz"

    /// Trang mẫu thêm ở bản sau: thêm một lần vào danh sách người xem đã lưu (xoá đi thì thôi, không thêm lại).
    static let addedSites = ["https://mephim.ddns.net", kkSite, ncSite, vsSite, k20Site]

    /// Gọi lúc mở app (sau migrateMovedHosts).
    static func addNewPresets() {
        for url in addedSites {
            let key = "preset_added@\(url)"
            if store.bool(forKey: key) { continue }
            if let list = store.stringArray(forKey: "sites"), !list.contains(url) { sites = list + [url] }
            store.set(true, forKey: key)
        }
    }

    /// Trang đổi hẳn tên miền, đường dẫn giữ nguyên (tên cũ chuyển hướng 301 sang tên mới). Tên cũ hay bị nhà mạng chặn nên
    /// app không tự thấy trang đã chuyển → đổi tên cũ → mới trong mọi dữ liệu đã lưu (migrateMovedHosts).
    /// phimmoir.fun → phimmois.fun: 11/2026.
    static let movedHosts = ["phimmoir.fun": "phimmois.fun"]

    /// Đổi tên miền cũ → mới trong mọi khoá / giá trị đã lưu: danh sách trang, trang đang dùng của từng người xem, lịch sử,
    /// Xem tiếp, phim theo dõi, chỗ đang xem, server ưu tiên, điểm server… Khoá mới đã có thì giữ khoá mới. Gọi lúc mở app.
    static func migrateMovedHosts() {
        func rename(_ s: String) -> String { movedHosts.reduce(s) { $0.replacingOccurrences(of: $1.key, with: $1.value) } }
        func moved(_ s: String) -> Bool { movedHosts.keys.contains { s.contains($0) } }
        var changed = false
        let all = store.dictionaryRepresentation()
        for (k, v) in all {
            var nv: Any?
            if let s = v as? String, moved(s) {
                nv = rename(s)
            } else if let a = v as? [String], a.contains(where: moved) {
                nv = a.map(rename)
            } else if let d = v as? Data, let s = String(data: d, encoding: .utf8), moved(s) {
                nv = Data(rename(s).utf8)
            }
            guard moved(k) || nv != nil else { continue }
            let nk = rename(k)
            if nk != k {
                store.removeObject(forKey: k)
                if all[nk] != nil { continue }
            }
            store.set(nv ?? v, forKey: nk)
            changed = true
        }
        guard changed else { return }
        let s = sites
        sites = s // bỏ trang trùng (tên cũ đổi thành tên mới đã có sẵn)
        reload()
    }

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
        // Đồng bộ trong nhà: lúc xoá phim khỏi lịch sử / bỏ theo dõi
        "hist_del", "follow_del",
        "base_url", "seek_step", "auto_next", "video_fill",
        // Giới hạn giờ xem (WatchLimit)
        "limit_min", "bedtime", "watch_day", "watch_sec", "watch_extra", "unblock_until",
        // Trang chủ tổng hợp (HomeFeed): kiểu trang chủ, chủ đề đã chọn, tập mới nhất đã thấy, hồ sơ trẻ em, ngôn ngữ bản hay xem
        "home_mix", "home_topics", "seen_eps", "kids", "watch_langs",
    ]
    private static let profilePrefixes = ["server_type@", "server_label@"]
    /// Người thêm sau bắt đầu với trang phim / server ưu tiên / cách xem của người đang dùng.
    private static let copiedKeys: Set<String> = ["base_url", "seek_step", "auto_next", "video_fill", "home_mix"]

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

    // MARK: - Giới hạn giờ xem (xem WatchLimit) — cùng tên khoá bản Android

    /// Khoá của người xem id (người đầu tiên — id "" — giữ khoá cũ không hậu tố).
    static func pk(_ id: String, _ key: String) -> String { id.isEmpty ? key : "\(key)#\(id)" }

    /// Số phút xem mỗi ngày của người xem id (mặc định người đang dùng; 0 = không giới hạn).
    static func dailyLimit(_ id: String? = nil) -> Int { store.integer(forKey: pk(id ?? profileId, "limit_min")) }

    static func setDailyLimit(_ id: String, _ minutes: Int) { store.set(minutes, forKey: pk(id, "limit_min")) }

    /// Giờ đi ngủ (phút trong ngày, vd 21:00 = 1260; -1 = không): từ giờ đó tới 6 giờ sáng không xem được.
    static func bedtime(_ id: String? = nil) -> Int { store.object(forKey: pk(id ?? profileId, "bedtime")) as? Int ?? -1 }

    static func setBedtime(_ id: String, _ minute: Int) { store.set(minute, forKey: pk(id, "bedtime")) }

    /// Người đang xem: ngày đang đếm ("yyyy-MM-dd"), số giây đã xem, số phút được cho thêm trong ngày đó.
    static var watchDay: String {
        get { store.string(forKey: k("watch_day")) ?? "" }
        set { store.set(newValue, forKey: k("watch_day")) }
    }

    static var watchSeconds: Int {
        get { store.integer(forKey: k("watch_sec")) }
        set { store.set(newValue, forKey: k("watch_sec")) }
    }

    static var watchExtra: Int {
        get { store.integer(forKey: k("watch_extra")) }
        set { store.set(newValue, forKey: k("watch_extra")) }
    }

    /// Người đang xem được xem không giới hạn tới thời điểm này (ms) — phụ huynh cho thêm lúc giờ đi ngủ.
    static var unblockUntil: Int64 {
        get { (store.object(forKey: k("unblock_until")) as? NSNumber)?.int64Value ?? 0 }
        set { store.set(NSNumber(value: newValue), forKey: k("unblock_until")) }
    }

    /// Có ai đang bị giới hạn giờ xem (khi đó thêm / xoá người xem cũng cần mã PIN phụ huynh).
    static var anyLimits: Bool { profiles.contains { dailyLimit($0.id) > 0 || bedtime($0.id) >= 0 } }

    /// Mã PIN phụ huynh (cả nhà): đặt / đổi giới hạn giờ xem, cho xem thêm khi hết giờ.
    static var hasParentPin: Bool { store.string(forKey: "parent_pin") != nil }

    /// Đặt / đổi mã PIN phụ huynh; nil = xoá.
    static func setParentPin(_ pin: String?) {
        if let p = pin { store.set(pinHash("parent", p), forKey: "parent_pin") } else { store.removeObject(forKey: "parent_pin") }
    }

    static func checkParentPin(_ pin: String) -> Bool { store.string(forKey: "parent_pin") == pinHash("parent", pin) }

    /// Hỏi "Ai đang xem?" khi mở app (từ 2 người xem).
    static var askProfile: Bool {
        get { store.object(forKey: "ask_profile") as? Bool ?? true }
        set { store.set(newValue, forKey: "ask_profile") }
    }

    /// Về màn hình chính của iPad lúc đang xem: thu nhỏ phim (hình trong hình) thay vì dừng.
    /// Thêm server dự phòng KKPhim cho các trang tự có trình phát (xem KkPhim). Dùng chung cả máy.
    static var kkBackup: Bool {
        get { store.object(forKey: "kk_backup") as? Bool ?? true }
        set { store.set(newValue, forKey: "kk_backup") }
    }

    /// Server dự phòng NguonC (từ 1.7) cho yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw.
    static var ncBackup: Bool {
        get { store.object(forKey: "nc_backup") as? Bool ?? true }
        set { store.set(newValue, forKey: "nc_backup") }
    }

    static var pipOnHome: Bool {
        get { store.object(forKey: "pip_on_home") as? Bool ?? true }
        set { store.set(newValue, forKey: "pip_on_home") }
    }

    /// Cỡ chữ của app (1 = vừa; 0,9 – 1,5). Dùng chung cả máy.
    static var textScale: Double {
        get {
            let v = store.double(forKey: "text_scale")
            return v == 0 ? 1 : min(1.5, max(0.9, v))
        }
        set { store.set(newValue, forKey: "text_scale") }
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
            locked {
                // Đồng bộ trong nhà: ghi lúc thêm / bỏ từng trang
                let old = Set(sites)
                var seen = Set<String>()
                let list = newValue.filter { seen.insert($0).inserted }
                let now = nowMs()
                var add = stamps("sites_add"), del = stamps("sites_del")
                for s in list where !old.contains(s) { add[s] = now }
                for s in old where !seen.contains(s) { del[s] = now }
                setStamps("sites_add", add)
                setStamps("sites_del", del)
                store.set(list, forKey: "sites")
            }
        }
    }

    /// Trang tự chuyển sang tên miền mới: thay tên cũ trong danh sách (giữ chỗ), rồi dùng tên mới.
    static func replaceSite(_ old: String, with new: String) {
        sites = sites.map { $0 == old ? new : $0 }
        baseUrl = new
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
        // Nhà mạng chặn tên miền trần kkphim.com: gõ «kkphim.com» là trang KKPhim mẫu (www)
        if host.lowercased() == ApiSite.kkHost { return kkSite }
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
            var del = stamps(k("hist_del"))
            let now = nowMs()
            for f in Set(history.map { $0.filmUrl } + positions().map { $0.film }) where !f.isEmpty { del[f] = now }
            setStamps(k("hist_del"), del)
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
            var del = stamps(k("hist_del"))
            del[filmUrl] = nowMs()
            setStamps(k("hist_del"), del)
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
            follows = [Follow(filmUrl: d.url, title: d.title, poster: d.poster, count: n, latest: d.latestEpisode?.title ?? "", seen: n,
                              t: nowMs())] +
                follows.filter { $0.filmUrl != d.url }
        }
    }

    static func unfollow(_ filmUrl: String) {
        locked {
            var del = stamps(k("follow_del"))
            del[filmUrl] = nowMs()
            setStamps(k("follow_del"), del)
            follows = follows.filter { $0.filmUrl != filmUrl }
        }
    }

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

    /// Vượt chặn nhà mạng: chia nhỏ gói bắt tay HTTPS để nhà mạng không đọc được tên trang (xem Socks5Bridge.writeHello).
    static var unblock: Bool {
        get { store.bool(forKey: "unblock") }
        set { store.set(newValue, forKey: "unblock") }
    }

    /// Mức vượt chặn khi bật: 1 = chia nhỏ gói bắt tay, 2 = chia nhỏ + đảo thứ tự (byte đầu gửi với TTL 1). Như bản TV.
    static var unblockLevel: Int {
        get { min(2, max(1, store.object(forKey: "unblock_level") as? Int ?? 1)) }
        set { store.set(newValue, forKey: "unblock_level") }
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
        /// Lúc lưu (ms; nil = bản cũ) — đồng bộ trong nhà lấy lần lưu mới hơn.
        var t: Int64?
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
            list.append(Position(ep: epUrl, sec: seconds, film: filmUrl, t: nowMs()))
            if list.count > maxPositions { list.removeFirst(list.count - maxPositions) }
            savePositions(list)
        }
    }

    // MARK: - Đồng bộ trong nhà (SyncData.swift)

    /// Mốc thời gian theo khoá (lúc xoá / lúc thêm), lưu JSON {"khoá": ms}.
    static func stamps(_ key: String) -> [String: Int64] { decode([String: Int64].self, key) ?? [:] }

    static func setStamps(_ key: String, _ m: [String: Int64]) { encode(SyncMerge.tombstones(m, [:]), key) }

    /// Mã máy (ngẫu nhiên, tạo lần đầu) — máy kia nhận ra máy này qua mã này.
    static var deviceId: String {
        locked {
            if let id = store.string(forKey: "device_id") { return id }
            let id = UUID().uuidString.lowercased()
            store.set(id, forKey: "device_id")
            return id
        }
    }

    /// Dữ liệu đồng bộ của người xem id.
    static func syncProfile(_ p: Profile) -> SyncProfile {
        locked {
            let id = p.id
            let hist = decode([HistoryItem].self, pk(id, "history")) ?? []
            let pos = decode([Position].self, pk(id, "positions_v2")) ?? []
            let fol = decode([Follow].self, pk(id, "follows")) ?? []
            return SyncProfile(
                id: id, name: p.name,
                history: hist.map { SyncHistory(film: $0.filmUrl, title: $0.title, poster: $0.poster ?? "", ep: $0.epTitle,
                                                epUrl: $0.epUrl, time: $0.time) },
                histDel: stamps(pk(id, "hist_del")),
                pos: pos.map { SyncPos(ep: $0.ep, sec: $0.sec, film: $0.film, t: $0.t ?? 0) },
                follows: fol.map { SyncFollow(film: $0.filmUrl, title: $0.title, poster: $0.poster ?? "", count: $0.count,
                                              latest: $0.latest, seen: $0.seen, t: $0.t ?? 0) },
                followDel: stamps(pk(id, "follow_del")))
        }
    }

    /// Ghi dữ liệu đã gộp cho người xem id. Trả về true nếu có gì thay đổi.
    @discardableResult
    static func applySync(_ s: SyncProfile) -> Bool {
        locked {
            guard profiles.contains(where: { $0.id == s.id }) else { return false }
            let p = profiles.first { $0.id == s.id }!
            let old = syncProfile(p)
            guard old != s else { return false }
            let id = s.id
            encode(s.history.map { HistoryItem(filmUrl: $0.film, title: $0.title, poster: $0.poster.nonEmpty, epTitle: $0.ep,
                                               epUrl: $0.epUrl, time: $0.time) }, pk(id, "history"))
            encode(s.pos.map { Position(ep: $0.ep, sec: $0.sec, film: $0.film, t: $0.t == 0 ? nil : $0.t) }, pk(id, "positions_v2"))
            encode(s.follows.map { Follow(filmUrl: $0.film, title: $0.title, poster: $0.poster.nonEmpty, count: $0.count,
                                          latest: $0.latest, seen: $0.seen, t: $0.t == 0 ? nil : $0.t) }, pk(id, "follows"))
            setStamps(pk(id, "hist_del"), s.histDel)
            setStamps(pk(id, "follow_del"), s.followDel)
            historyCache = nil
            positionsCache = nil
            return true
        }
    }

    static var syncSites: SyncSites {
        SyncSites(list: sites, add: stamps("sites_add"), del: stamps("sites_del"))
    }

    /// Ghi danh sách trang đã gộp (không ghi lại mốc thêm / bỏ như khi người xem tự sửa). true = có thay đổi.
    @discardableResult
    static func applySites(_ s: SyncSites) -> Bool {
        locked {
            guard s != syncSites, !s.list.isEmpty else { return false }
            setStamps("sites_add", s.add)
            setStamps("sites_del", s.del)
            store.set(s.list, forKey: "sites")
            return true
        }
    }

    // MARK: - Trang chủ tổng hợp (HomeFeed, từ 1.8)

    /// true: trang chủ gộp phim từ mọi nguồn, xếp theo lịch sử xem; false: trang chủ của trang đang dùng.
    static var homeMix: Bool {
        get { store.object(forKey: k("home_mix")) as? Bool ?? true }
        set { store.set(newValue, forKey: k("home_mix")) }
    }

    /// Hồ sơ trẻ em: trang chủ (luôn kiểu tổng hợp) chỉ có phim hoạt hình hợp trẻ em. Tắt cần mã PIN phụ huynh (nếu có).
    static var kids: Bool {
        get { store.bool(forKey: k("kids")) }
        set { store.set(newValue, forKey: k("kids")) }
    }

    /// Trang chủ tổng hợp đang dùng (người xem chọn, hoặc hồ sơ trẻ em).
    static var homeMixOn: Bool { homeMix || kids }

    /// Chủ đề người xem chưa có lịch sử đã chọn (id trong HomeFeed.topics).
    static var homeTopics: Set<String> {
        get { Set(store.stringArray(forKey: k("home_topics")) ?? []) }
        set { store.set(Array(newValue).sorted(), forKey: k("home_topics")) }
    }

    /// Tập mới nhất người xem đã thấy của từng phim (khoá = tên đã chuẩn hoá — chung mọi nguồn).
    static func seenEp(_ key: String) -> Double? {
        (store.dictionary(forKey: k("seen_eps")) as? [String: Double])?[key]
    }

    static func setSeenEp(_ key: String, _ ep: Double) {
        guard !key.isEmpty else { return }
        locked {
            var d = (store.dictionary(forKey: k("seen_eps")) as? [String: Double]) ?? [:]
            if let old = d[key], old >= ep { return }
            d[key] = ep
            if d.count > 300 { for old in d.keys.prefix(d.count - 300) { d[old] = nil } }
            store.set(d, forKey: k("seen_eps"))
        }
    }

    /// Số tập đã xem của mọi phim (theo chỗ đang xem đã lưu) — dùng tính sở thích.
    static func watchedCounts() -> [String: Int] {
        var out: [String: Int] = [:]
        for p in positions() where !p.film.isEmpty { out[p.film, default: 0] += 1 }
        return out
    }

    /// Ghi ngôn ngữ của bản vừa mở ("vs" / "tm" / "lt"), giữ 40 lần gần nhất.
    static func recordWatchLang(_ lang: String) {
        locked {
            let list = (store.stringArray(forKey: k("watch_langs")) ?? []) + [lang]
            store.set(Array(list.suffix(40)), forKey: k("watch_langs"))
        }
    }

    static var watchLangs: [String] { store.stringArray(forKey: k("watch_langs")) ?? [] }

    /// Tình trạng nguồn của trang chủ tổng hợp (Feed) — chung cả máy.
    static var feedHealth: String {
        get { store.string(forKey: "feed_health") ?? "{}" }
        set { store.set(newValue, forKey: "feed_health") }
    }

    /// Thể loại / quốc gia của phim đã xem (FilmTags) — chung cả máy.
    static var filmTags: String {
        get { store.string(forKey: "film_tags") ?? "{}" }
        set { store.set(newValue, forKey: "film_tags") }
    }

    // MARK: - JSON trong UserDefaults

    static func decode<T: Decodable>(_ type: T.Type, _ key: String) -> T? {
        guard let d = store.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(type, from: d)
    }

    static func encode<T: Encodable>(_ value: T, _ key: String) {
        if let d = try? JSONEncoder().encode(value) { store.set(d, forKey: key) }
    }

    /// Toàn bộ khoá (cho cổng gỡ lỗi /prefs).
    static func dump() -> [String: Any] {
        store.dictionaryRepresentation().filter { k, _ in
            !k.hasPrefix("Apple") && !k.hasPrefix("NS") && !k.hasPrefix("com.apple") && k != "proxy_pass"
        }
    }
}
