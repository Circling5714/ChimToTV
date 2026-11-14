import CryptoKit
import Foundation

// Trang chủ tổng hợp (từ iPad 1.8) — bản Android: Feed.kt (giữ cùng tên hàm để sửa hai bên song song).

/// Thể loại / quốc gia / loại phim theo slug của KKPhim (hanh-dong, trung-quoc; loại: hoathinh, series, single, tvshows).
struct Tags: Hashable {
    let genres: [String]
    let countries: [String]
    let type: String?

    func json() -> [String: Any] { ["g": genres, "c": countries, "t": type ?? ""] }

    static func of(_ o: [String: Any]?) -> Tags? {
        guard let o = o else { return nil }
        return Tags(genres: (o["g"] as? [String]) ?? [], countries: (o["c"] as? [String]) ?? [], type: J.str(o, "t").nonEmpty)
    }

    /// category / country [{name, slug}] + type của KKPhim / VSMOV.
    static func fromKk(_ m: [String: Any]) -> Tags? {
        let g = slugs(J.arr(m, "category")), c = slugs(J.arr(m, "country"))
        let t = J.str(m, "type").nonEmpty
        if g.isEmpty && c.isEmpty && t == nil { return nil }
        return Tags(genres: g, countries: c, type: t)
    }

    private static func slugs(_ a: [Any]?) -> [String] {
        J.objects(a).compactMap { o in J.str(o, "slug").nonEmpty ?? Feed.slug(J.str(o, "name")).nonEmpty }
    }
}

/// Một phim trong danh sách của một nguồn. names: tên đã chuẩn hoá (KkPhim.norm) để gộp phim trùng; updated: lúc nguồn cập
/// nhật (ms; 0 = không ghi); ep: số tập mới nhất.
struct FeedItem: Hashable {
    let url: String
    let title: String
    let poster: String?
    let badge: String?
    let names: Set<String>
    let year: Int?
    var updated: Int64
    let ep: Double?
    let tags: Tags?
    /// Điểm IMDb (KKPhim).
    let imdb: Imdb?
    /// Tên miền nguồn (tính một lần).
    let host: String

    init(url: String, title: String, poster: String?, badge: String?, names: Set<String>, year: Int?, updated: Int64,
         ep: Double?, tags: Tags?, imdb: Imdb? = nil) {
        self.url = url
        self.title = title
        self.poster = poster
        self.badge = badge
        self.names = names
        self.year = year
        self.updated = updated
        self.ep = ep
        self.tags = tags
        self.imdb = imdb
        host = Site.hostOf(url) ?? ""
    }

    func json() -> [String: Any] {
        var o: [String: Any] = ["u": url, "n": title, "p": poster ?? "", "b": badge ?? "", "k": Array(names), "y": year ?? 0,
                                "t": updated, "e": ep ?? -1.0]
        if let t = tags { o["g"] = t.json() }
        if let i = imdb { o["is"] = i.score; o["iv"] = i.votes }
        return o
    }

    static func of(_ o: [String: Any]) -> FeedItem? {
        guard let url = J.str(o, "u").nonEmpty, let title = J.str(o, "n").nonEmpty else { return nil }
        let e = dbl(o["e"]) ?? -1
        return FeedItem(url: url, title: title, poster: J.str(o, "p").nonEmpty, badge: J.str(o, "b").nonEmpty,
                        names: Set(((o["k"] as? [String]) ?? []).filter { !$0.isEmpty }), year: Int(num64(o["y"])).nonZero,
                        updated: num64(o["t"]), ep: e >= 0 ? e : nil, tags: Tags.of(J.obj(o, "g")),
                        imdb: (dbl(o["is"]) ?? 0) > 0 ? Imdb(score: dbl(o["is"])!, votes: Int(num64(o["iv"]))) : nil)
    }
}

private extension Int {
    var nonZero: Int? { self > 0 ? self : nil }
}

/// Số trong dữ liệu JSON / từ điển (NSNumber, Int, Int64, Double, chuỗi số) → Int64 / Double.
func num64(_ v: Any?) -> Int64 {
    if let n = v as? NSNumber { return n.int64Value }
    if let s = v as? String { return Int64(s) ?? 0 }
    return 0
}

func dbl(_ v: Any?) -> Double? {
    if let n = v as? NSNumber { return n.doubleValue }
    if let s = v as? String { return Double(s) }
    return nil
}

/// Phim đã gộp từ các nguồn (items: mỗi nguồn một mục). So sánh theo chính đối tượng (một phim chỉ thuộc một nhóm).
final class FeedFilm: Hashable {
    let items: [FeedItem]
    let names: Set<String>
    let updated: Int64

    init(_ items: [FeedItem]) {
        self.items = items
        names = Set(items.flatMap { $0.names })
        updated = items.map { $0.updated }.max() ?? 0
    }

    var tags: Tags? { items.lazy.compactMap { $0.tags }.first }
    var year: Int? { items.lazy.compactMap { $0.year }.first }

    /// Nguồn có tập mới nhất (nil = không nguồn nào ghi số tập).
    var latest: FeedItem? {
        items.filter { $0.ep != nil }.enumerated().max { a, b in a.element.ep! < b.element.ep! || (a.element.ep! == b.element.ep! && a.offset > b.offset) }?.element
    }

    /// Xếp lại các nguồn theo thứ tự ưu tiên mở (giữ thứ tự cũ khi bằng nhau).
    func ordered(_ prefer: [String]) -> FeedFilm {
        func rank(_ i: FeedItem) -> Int { prefer.firstIndex(of: i.host) ?? prefer.count }
        return FeedFilm(items.enumerated().sorted { (rank($0.element), $0.offset) < (rank($1.element), $1.offset) }.map { $0.element })
    }

    func matches(_ other: Set<String>) -> Bool { !names.isDisjoint(with: other) }

    static func == (a: FeedFilm, b: FeedFilm) -> Bool { a === b }
    func hash(into h: inout Hasher) { h.combine(ObjectIdentifier(self)) }
}

/// Đọc danh sách phim mới của các nguồn, gộp phim trùng, giữ bản lưu từng nguồn. Nguồn lỗi / quá hạn: dùng bản lưu (dưới
/// 24 giờ), lỗi 3 lượt liên tiếp thì tạm nghỉ 30 phút (rồi 2 giờ, 6 giờ).
enum Feed {
    /// Mỗi nguồn chờ tối đa 6 giây; quá hạn thì dùng bản lưu, về muộn vẫn lưu cho lần sau.
    static let timeout: Int64 = 6_000
    /// Cả lượt tải không quá 15 giây.
    private static let total: Int64 = 15_000
    static let cacheMax: Int64 = 24 * 3_600_000
    /// Nguồn không ghi giờ cập nhật: coi như cứ 15 phút một phim, theo thứ tự trên trang.
    private static let pseudoGap: Int64 = 15 * 60_000
    private static let failsToRest = 3
    private static let rest: [Int64] = [30 * 60_000, 2 * 3_600_000, 6 * 3_600_000]

    /// Thư mục bản lưu (Application Support/feed); test đặt thư mục tạm.
    static var dir: URL? = {
        guard let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else { return nil }
        let d = base.appendingPathComponent("feed", isDirectory: true)
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d
    }()

    // MARK: - Đọc danh sách của từng nguồn

    /// KKPhim (phimapi.com v1) / VSMOV: có thể loại, quốc gia, loại phim, giờ cập nhật (VSMOV không có thể loại).
    static func parseKk(_ root: [String: Any], _ origin: String) -> [FeedItem] {
        let data = J.obj(root, "data") ?? root
        let cdn = J.str(data, "APP_DOMAIN_CDN_IMAGE").nonEmpty ?? J.str(root, "pathImage").nonEmpty ?? "https://phimimg.com"
        return J.objects(J.arr(data, "items")).compactMap { o -> FeedItem? in
            guard let slug = J.str(o, "slug").nonEmpty, let name = J.str(o, "name").nonEmpty else { return nil }
            let ep = J.str(o, "episode_current")
            return FeedItem(url: "\(origin)/phim/\(slug)", title: name,
                            poster: ApiSite.image(J.str(o, "poster_url").nonEmpty ?? J.str(o, "thumb_url"), cdn),
                            badge: NextSite.badge(ep), names: KkPhim.names(o), year: year(o),
                            updated: time(J.str(J.obj(o, "modified"), "time")), ep: epOf(ep), tags: Tags.fromKk(o),
                            imdb: Imdb.of(J.obj(o, "imdb")))
        }.distinct { $0.url }
    }

    /// NguonC /api/films/…: tên gốc (có thể nhiều tên), năm, tập hiện tại, giờ cập nhật; không có thể loại.
    static func parseNc(_ root: [String: Any], _ origin: String) -> [FeedItem] {
        J.objects(J.arr(root, "items")).compactMap { o -> FeedItem? in
            guard let slug = J.str(o, "slug").nonEmpty, let name = J.str(o, "name").nonEmpty else { return nil }
            let ep = J.str(o, "current_episode")
            let poster = J.str(o, "thumb_url").nonEmpty ?? J.str(o, "poster_url")
            return FeedItem(url: "\(origin)/phim/\(slug)", title: name, poster: poster.hasPrefix("http") ? poster : nil,
                            badge: NextSite.badge(ep), names: KkPhim.names(o), year: year(o), updated: time(J.str(o, "modified")),
                            ep: epOf(ep), tags: nil)
        }.distinct { $0.url }
    }

    /// Phim từ trang web / CLB Phim Xưa (chỉ có tên, ảnh, nhãn tập).
    static func fromFilms(_ films: [Film]) -> [FeedItem] {
        films.map { f in
            FeedItem(url: f.url, title: f.title, poster: f.poster, badge: f.badge, names: KkPhim.names(["name": f.title]),
                     year: nil, updated: 0, ep: epOf(f.badge), tags: nil)
        }.distinct { $0.url }
    }

    private static let tapRx = Rx(#"(?i)t[aậ]p\s*(\d+(?:\.\d+)?)"#)
    private static let ofRx = Rx(#"(\d+)\s*/\s*(\d+|\?+)"#)
    private static let numRx = Rx(#"^(\d+(?:\.\d+)?)$"#)

    /// Số tập trong nhãn: "Tập 108", "Hoàn Tất (16/16)", "108/120", "108"; "Full", "Top 1", "HD" → nil.
    static func epOf(_ label: String?) -> Double? {
        guard let s = label?.trimmed, !s.isEmpty else { return nil }
        if let g = tapRx.group(1, in: s) { return Double(g) }
        if let g = ofRx.group(1, in: s) { return Double(g) }
        if let g = numRx.group(1, in: s) { return Double(g) }
        return nil
    }

    private static let isoRx = Rx(#"(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2}))?(?:\.\d+)?\s*(Z|[+-]\d{2}:?\d{2})?"#)

    /// "2026-11-08T18:55:29.000Z", "2026-11-08T21:12:36+07:00" → ms; không ghi múi giờ thì coi là giờ Việt Nam.
    static func time(_ s: String?) -> Int64 {
        guard let s = s, let g = isoRx.groups(in: s) else { return 0 }
        var c = DateComponents()
        c.year = Int(g[1]); c.month = Int(g[2]); c.day = Int(g[3]); c.hour = Int(g[4]); c.minute = Int(g[5]); c.second = Int(g[6]) ?? 0
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        guard let d = cal.date(from: c) else { return 0 }
        let zone = g[7]
        var offset = 0
        if zone.isEmpty {
            offset = 7 * 60
        } else if zone != "Z" {
            let digits = zone.dropFirst().replacingOccurrences(of: ":", with: "")
            let m = (Int(digits.prefix(2)) ?? 0) * 60 + (Int(digits.dropFirst(2)) ?? 0)
            offset = zone.hasPrefix("-") ? -m : m
        }
        return Int64(d.timeIntervalSince1970 * 1000) - Int64(offset) * 60_000
    }

    private static func year(_ o: [String: Any]) -> Int? {
        let y = J.int(o, "year")
        return (1900...2100).contains(y) ? y : nil
    }

    /// "Hành Động" → "hanh-dong" (NguonC ghi tên thể loại, không có slug).
    static func slug(_ name: String) -> String { KkPhim.norm(name).replacingOccurrences(of: " ", with: "-") }

    // MARK: - Gộp phim trùng

    /// Gộp các danh sách (thứ tự = ưu tiên): có tên chung, năm lệch ≤ 1, khác nguồn. Cùng địa chỉ = cùng phim. Mục không ghi
    /// giờ cập nhật tính giờ giả theo thứ tự trên trang. Kết quả xếp mới cập nhật trước.
    static func merge(_ lists: [[FeedItem]], now: Int64 = nowMs()) -> [FeedFilm] {
        var groups: [[FeedItem]] = []
        var byName: [String: [Int]] = [:]
        var byUrl = Set<String>()
        for list in lists {
            for (rank, raw) in list.enumerated() {
                guard byUrl.insert(raw.url).inserted else { continue }
                var item = raw
                if item.updated <= 0 { item.updated = now - Int64(rank + 1) * pseudoGap }
                var hit: Int?
                for n in item.names {
                    for gi in byName[n] ?? [] where hit == nil {
                        let g = groups[gi]
                        let y = g.lazy.compactMap { $0.year }.first
                        if !g.contains(where: { $0.host == item.host }) && (y == nil || item.year == nil || abs(y! - item.year!) <= 1) {
                            hit = gi
                        }
                    }
                    if hit != nil { break }
                }
                let gi = hit ?? groups.count
                if hit == nil { groups.append([]) }
                groups[gi].append(item)
                for n in item.names where !(byName[n] ?? []).contains(gi) { byName[n, default: []].append(gi) }
            }
        }
        return groups.map { FeedFilm($0) }.enumerated().sorted { ($0.element.updated, -$0.offset) > ($1.element.updated, -$1.offset) }
            .map { $0.element }
    }

    // MARK: - Tải các nguồn: song song, hạn 6 giây, bản lưu, tạm nghỉ

    /// Một lần đọc: key = tên bản lưu, host = nguồn (tính lỗi / tạm nghỉ); keep = false: không lưu bản.
    struct Req {
        let key: String
        let host: String
        var keep = true
        let load: () async throws -> Loaded
    }

    struct Loaded {
        let items: [FeedItem]
        var categories: [Category]? = nil
        var movedTo: String? = nil
    }

    enum State { case ok, cached, noData }

    struct Result {
        let req: Req
        let state: State
        let items: [FeedItem]
        let categories: [Category]?
        let error: String?
        let resting: Bool
        let ms: Int64
        let savedAt: Int64
        var movedTo: String? = nil
    }

    private final class Job: @unchecked Sendable {
        let lock = NSLock()
        var startedAt: Int64 = 0
        var done: Loaded?
        var failed: Error?
        var finishedAt: Int64 = 0
        /// Đã tính một lần lỗi (quá hạn) — về muộn mà lỗi thì không tính thêm.
        var counted = false

        func read<T>(_ f: (Job) -> T) -> T { lock.lock(); defer { lock.unlock() }; return f(self) }
    }

    private final class HostSet: @unchecked Sendable {
        private let lock = NSLock()
        private var set = Set<String>()
        func insert(_ h: String) -> Bool { lock.lock(); defer { lock.unlock() }; return set.insert(h).inserted }
    }

    /// Tải các nguồn. Trả về khi mọi nguồn xong / quá hạn — nguồn quá hạn vẫn chạy tiếp để lưu bản mới.
    static func load(_ reqs: [Req]) async -> [Result] {
        let t0 = nowMs()
        var results = [Result?](repeating: nil, count: reqs.count)
        var jobs: [Job?] = []
        // Một lượt hỏi một nguồn nhiều lần (KKPhim: mới cập nhật + các hàng gợi ý) — chỉ tính một lần lỗi mỗi lượt
        let failedHosts = HostSet()
        for (i, r) in reqs.enumerated() {
            if resting(r.host) {
                results[i] = cached(r, nil, resting: true, ms: 0)
                jobs.append(nil)
                continue
            }
            let job = Job()
            jobs.append(job)
            Task.detached(priority: .utility) {
                job.read { $0.startedAt = nowMs() }
                do {
                    let got = try await r.load()
                    job.read { $0.done = got }
                    if r.keep { save(r.key, got) }
                    healthy(r.host)
                } catch {
                    let counted = job.read { j -> Bool in j.failed = error; return j.counted }
                    if !counted && failedHosts.insert(r.host) { failed(r.host) }
                }
                job.read { $0.finishedAt = nowMs() }
            }
        }
        while true {
            let now = nowMs()
            var waiting = false
            for (i, j) in jobs.enumerated() {
                guard let j = j, results[i] == nil else { continue }
                let (started, done, err, finished) = j.read { ($0.startedAt, $0.done, $0.failed, $0.finishedAt) }
                let ms = (finished > 0 ? finished : now) - (started > 0 ? started : now)
                if let d = done {
                    results[i] = Result(req: reqs[i], state: .ok, items: d.items, categories: d.categories, error: nil, resting: false,
                                        ms: ms, savedAt: now, movedTo: d.movedTo)
                } else if let e = err {
                    results[i] = cached(reqs[i], short(e), resting: false, ms: ms)
                } else if started > 0 && now - started > timeout {
                    j.read { $0.counted = true }
                    if failedHosts.insert(reqs[i].host) { failed(reqs[i].host) }
                    results[i] = cached(reqs[i], "quá \(timeout / 1000) giây", resting: false, ms: ms)
                } else if now - t0 > total {
                    results[i] = cached(reqs[i], nil, resting: false, ms: ms)
                } else {
                    waiting = true
                }
            }
            if !waiting { break }
            try? await Task.sleep(nanoseconds: 80_000_000)
        }
        return results.map { $0! }
    }

    private static func short(_ e: Error) -> String {
        if let h = e as? Http.HttpError { return "lỗi \(h.code)" }
        if let u = e as? URLError {
            switch u.code {
            case .cannotFindHost, .dnsLookupFailed: return "không tìm thấy máy chủ"
            case .timedOut: return "không trả lời"
            default: return "lỗi mạng"
            }
        }
        return String(e.localizedDescription.prefix(60))
    }

    private static func cached(_ r: Req, _ error: String?, resting: Bool, ms: Int64) -> Result {
        if r.keep, let c = readCache(r.key) {
            return Result(req: r, state: .cached, items: c.loaded.items, categories: c.loaded.categories, error: error, resting: resting,
                          ms: ms, savedAt: c.at)
        }
        return Result(req: r, state: .noData, items: [], categories: nil, error: error, resting: resting, ms: ms, savedAt: 0)
    }

    /// Bản lưu của một nguồn (dưới 24 giờ).
    static func readCache(_ key: String) -> (loaded: Loaded, at: Int64)? {
        guard let text = readText(key), let data = text.data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let at = num64(o["at"])
        guard nowMs() - at <= cacheMax else { return nil }
        let items = J.objects(J.arr(o, "items")).compactMap(FeedItem.of)
        let cats = (o["cats"] as? [Any]).map { J.objects($0).map { Category(title: J.str($0, "t"), url: J.str($0, "u")) } }
        return (Loaded(items: items, categories: cats), at)
    }

    private static func save(_ key: String, _ l: Loaded) {
        var o: [String: Any] = ["at": nowMs(), "items": l.items.map { $0.json() }]
        if let cats = l.categories { o["cats"] = cats.map { ["t": $0.title, "u": $0.url] } }
        if let d = try? JSONSerialization.data(withJSONObject: o), let s = String(data: d, encoding: .utf8) { writeText(key, s) }
    }

    /// Tệp lưu tự do theo khoá (vd trang chủ lần trước của từng người xem).
    static func writeText(_ key: String, _ text: String) {
        guard let f = fileOf(key) else { return }
        try? text.write(to: f, atomically: true, encoding: .utf8)
    }

    static func readText(_ key: String) -> String? {
        guard let f = fileOf(key) else { return nil }
        return try? String(contentsOf: f, encoding: .utf8)
    }

    private static func fileOf(_ key: String) -> URL? {
        guard let d = dir else { return nil }
        let h = SHA256.hash(data: Data(key.utf8)).prefix(16).map { String(format: "%02x", $0) }.joined()
        return d.appendingPathComponent("\(h).json")
    }

    // MARK: - Tình trạng nguồn (host → {f: lỗi liên tiếp, r: tạm nghỉ tới, l: số lần đã tạm nghỉ})

    private static let healthLock = NSLock()

    private static func health() -> [String: [String: Any]] {
        guard let d = Prefs.feedHealth.data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: [String: Any]] else { return [:] }
        return o
    }

    private static func saveHealth(_ h: [String: [String: Any]]) {
        if let d = try? JSONSerialization.data(withJSONObject: h), let s = String(data: d, encoding: .utf8) { Prefs.feedHealth = s }
    }

    private static func restUntil(_ o: [String: Any]?) -> Int64 { num64(o?["r"]) }

    static func resting(_ host: String) -> Bool {
        healthLock.lock(); defer { healthLock.unlock() }
        return restUntil(health()[host]) > nowMs()
    }

    /// Nguồn đang tạm nghỉ: host → tạm nghỉ tới (ms).
    static func restingHosts() -> [String: Int64] {
        healthLock.lock(); defer { healthLock.unlock() }
        let now = nowMs()
        return health().compactMapValues { o in restUntil(o) > now ? restUntil(o) : nil }
    }

    private static func healthy(_ host: String) {
        healthLock.lock(); defer { healthLock.unlock() }
        var h = health()
        guard h[host] != nil else { return }
        h[host] = nil
        saveHealth(h)
    }

    private static func failed(_ host: String) {
        healthLock.lock(); defer { healthLock.unlock() }
        var h = health()
        var o = h[host] ?? [:]
        let f = J.int(o, "f") + 1
        o["f"] = f
        if f >= failsToRest {
            let level = J.int(o, "l")
            o["r"] = nowMs() + rest[min(level, rest.count - 1)]
            o["l"] = level + 1
            o["f"] = 0
        }
        h[host] = o
        saveHealth(h)
    }

    /// «Thử lại ngay» trong Cài đặt: bỏ tạm nghỉ của mọi nguồn.
    static func clearRest() {
        healthLock.lock(); defer { healthLock.unlock() }
        Prefs.feedHealth = "{}"
    }

    /// App vừa lên bản mới (hoặc mới cài): bỏ tạm nghỉ của mọi nguồn một lần — bản mới có thể đọc được trang mà bản cũ lỗi liên
    /// tục (vd trang bật chống bot của Cloudflare, đọc hộ từ iPad / Mac 1.18), khỏi chờ tới 6 giờ. true = vừa bỏ. Như lõi Rust.
    @discardableResult
    static func clearRestOnUpgrade(_ version: String) -> Bool {
        guard Prefs.feedRestVersion != version else { return false }
        clearRest()
        Prefs.feedRestVersion = version
        return true
    }
}
