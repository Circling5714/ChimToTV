import Foundation

/// Server dự phòng KKPhim (API công khai phimapi.com) cho các trang tự có trình phát — bản Android: KkPhim.kt (giữ cùng
/// tên hàm để sửa song song). yanhh3d, hoathinh3d.ee, hhkungfu nhúng chung streamfree.vip — hỏng là hỏng cả ba. Mở một tập
/// thì tìm đúng phim đó trên KKPhim (slug của trang, không khớp thì tìm theo tên; tên phim hoặc tên gốc phải trùng), lấy
/// tập cùng số rồi thêm server "KKPhim Vietsub / Thuyết Minh" (luồng HLS — app tự phát, bỏ quảng cáo chèn) vào Menu. Trang
/// chia bản (Việt Sub / Thuyết Minh) thì chỉ thêm đúng ngôn ngữ của bản đang xem. Trang lỗi hẳn mà KKPhim có tập này thì
/// vẫn xem được bằng KKPhim. Trang đã có sẵn nguồn KKPhim (mephim / kkphim — ApiSite, phimmoi — NextSite) không thêm.
enum KkPhim {
    static let api = "https://phimapi.com"

    /// Đầu tên loại server KKPhim, vd "kk-vs", "kk-tm" (điểm server / server quen lưu theo tên này).
    static let prefix = "kk-"

    private static let ttl: Int64 = 10 * 60_000 // chi tiết phim (danh sách tập) giữ 10 phút
    private static let missTtl: Int64 = 30 * 60_000 // phim không có trên KKPhim: 30 phút sau mới hỏi lại

    private static let lock = NSLock()
    private static var cache: [String: (at: Int64, detail: [String: Any]?)] = [:]
    /// Phim của trang → slug trên KKPhim đã tìm thấy (khỏi tìm lại theo tên).
    private static var slugs: [String: String] = [:]

    /// Trang dùng nguồn dự phòng: bật trong Cài đặt và không phải trang đã có sẵn KKPhim.
    static func appliesTo(_ url: String) -> Bool {
        guard Prefs.kkBackup, !ApiSite.isKnown(url), !NextSite.isKnown(url), let h = Site.hostOf(url) else { return false }
        return !h.hasSuffix("phimapi.com") && !h.hasSuffix("kkphim.com")
    }

    /// Server dự phòng (KKPhim "kk-…", NguonC "nc-…"): link có sẵn trong EpisodeSource.links.
    static func isBackup(_ type: String) -> Bool { type.hasPrefix(prefix) || type.hasPrefix(NguonC.prefix) }

    /// Trang tập + server dự phòng KKPhim và NguonC, tải song song. Nguồn dự phòng chậm / lỗi thì thôi; trang tập lỗi mà
    /// nguồn dự phòng có tập này → chỉ phát bằng nguồn dự phòng.
    static func source(_ d: FilmDetail, _ group: EpisodeGroup?, _ ep: Episode) async throws -> EpisodeSource {
        var backups: [() async throws -> [(Server, Video)]] = []
        if appliesTo(d.url) { backups.append { try await servers(d, group, ep) } }
        if NguonC.appliesTo(d.url) { backups.append { try await NguonC.servers(d, group, ep) } }
        if backups.isEmpty { return try await Site.source(ep.url) }
        return try await combineAll(ep.url, site: { try await Site.source(ep.url) }, backups: backups)
    }

    static func combine(_ epUrl: String, site: @escaping () async throws -> EpisodeSource,
                        kk: @escaping () async throws -> [(Server, Video)],
                        waitMs: Int64 = 6_000, failWaitMs: Int64 = 15_000) async throws -> EpisodeSource {
        try await combineAll(epUrl, site: site, backups: [kk], waitMs: waitMs, failWaitMs: failWaitMs)
    }

    /// Chạy song song trang tập (site) và các nguồn dự phòng (mỗi nguồn một task). Trang xong thì chờ nguồn dự phòng thêm
    /// tới waitMs tính từ lúc bắt đầu; trang lỗi thì chờ tới failWaitMs (lúc đó dự phòng là đường duy nhất). Nguồn nào chưa
    /// xong / lỗi thì bỏ, nguồn đã xong vẫn dùng.
    static func combineAll(_ epUrl: String, site: @escaping () async throws -> EpisodeSource,
                           backups: [() async throws -> [(Server, Video)]],
                           waitMs: Int64 = 6_000, failWaitMs: Int64 = 15_000) async throws -> EpisodeSource {
        let start = nowMs()
        let tasks = backups.map { b in Task<[(Server, Video)], Never> { (try? await b()) ?? [] } }
        var src: EpisodeSource?
        var siteError: Error?
        do { src = try await site() } catch { siteError = error }
        let deadline = start + (src != nil ? waitMs : failWaitMs)
        var extra: [(Server, Video)] = []
        for t in tasks { extra += await waitFor(t, ms: max(1, deadline - nowMs())) }
        if let s = src { return attach(s, extra) }
        if extra.isEmpty { throw siteError ?? AppError("Không mở được tập này") }
        var links: [String: Video] = [:]
        for (s, v) in extra { links[s.type] = v }
        return EpisodeSource(epUrl: epUrl, servers: extra.map { $0.0 }, postId: "", ep: "", sv: "", playerUrl: nil,
                             directEmbed: nil, pageMode: false, links: links)
    }

    /// Kết quả của task nếu xong trong ms; quá hạn → rỗng (task chạy tiếp, kết quả bỏ).
    private static func waitFor(_ task: Task<[(Server, Video)], Never>, ms: Int64) async -> [(Server, Video)] {
        await withCheckedContinuation { (c: CheckedContinuation<[(Server, Video)], Never>) in
            let once = Once()
            Task { let v = await task.value; if once.claim() { c.resume(returning: v) } }
            Task {
                try? await Task.sleep(nanoseconds: UInt64(ms) * 1_000_000)
                if once.claim() { c.resume(returning: []) }
            }
        }
    }

    private final class Once {
        private let lock = NSLock()
        private var done = false
        func claim() -> Bool {
            lock.lock()
            defer { lock.unlock() }
            if done { return false }
            done = true
            return true
        }
    }

    /// Thêm server KKPhim sau các server của trang. Trang mở thẳng trang tập (pageMode, vd hoathinh3d.pw) thì trình phát
    /// của trang thành một server ("Trình phát của trang") để chọn qua lại được.
    static func attach(_ src: EpisodeSource, _ extra: [(Server, Video)]) -> EpisodeSource {
        if extra.isEmpty || src.next { return src }
        let own = src.pageMode ? [Server(label: "Trình phát của trang", type: EpisodeSource.pageServer)] : src.servers
        var links = src.links
        for (s, v) in extra { links[s.type] = v }
        return EpisodeSource(epUrl: src.epUrl, servers: own + extra.map { $0.0 }, postId: src.postId, ep: src.ep, sv: src.sv,
                             playerUrl: src.playerUrl, directEmbed: src.directEmbed, pageMode: src.pageMode, links: links,
                             next: src.next)
    }

    /// Server KKPhim cho một tập (rỗng nếu KKPhim không có phim / tập này). Lỗi mạng → ném lỗi.
    static func servers(_ d: FilmDetail, _ group: EpisodeGroup?, _ ep: Episode) async throws -> [(Server, Video)] {
        guard let num = Site.epNumber(ep.title) ?? Site.epNumber(ep.url.trimmingSuffix("/").afterLast("/")),
              let detail = try await detail(d) else { return [] }
        return pick(detail, num, group.flatMap { PlayStats.lang($0.name) })
    }

    /// Server có tập số num trong chi tiết phim KKPhim ({movie, episodes:[{server_name, is_ai, server_data:[{name, slug,
    /// link_m3u8}]}]}). lang: ngôn ngữ của bản đang xem ("vs" / "tm" / "lt"; nil = trang không chia bản → mọi ngôn ngữ).
    static func pick(_ root: [String: Any], _ num: Double, _ lang: String?) -> [(Server, Video)] {
        var out: [(Server, Video)] = []
        var used = Set<String>()
        for s in J.objects(J.arr(root, "episodes")) {
            let l = PlayStats.lang(J.str(s, "server_name")) ?? "vs"
            if let lang = lang, l != lang { continue }
            guard let e = J.objects(J.arr(s, "server_data"))
                .first(where: { (Site.epNumber(J.str($0, "name")) ?? Site.epNumber(J.str($0, "slug"))) == num }) else { continue }
            let m3u8 = J.str(e, "link_m3u8")
            guard m3u8.hasPrefix("http") else { continue }
            let ai = J.bool(s, "is_ai")
            let base = l + (ai ? "-ai" : "")
            var type = prefix + base
            var n = 1
            while !used.insert(type).inserted {
                n += 1
                type = prefix + base + "-\(n)"
            }
            let name = l == "tm" ? "Thuyết Minh" : (l == "lt" ? "Lồng Tiếng" : "Vietsub")
            out.append((Server(label: "KKPhim \(name)" + (ai ? " (AI)" : "") + (n > 1 ? " \(n)" : ""), type: type), Video(url: m3u8)))
        }
        return out
    }

    // MARK: - Tìm phim trên KKPhim

    private static func detail(_ d: FilmDetail) async throws -> [String: Any]? {
        let now = nowMs()
        lock.lock()
        let hit = cache[d.url]
        lock.unlock()
        if let h = hit, now - h.at < (h.detail == nil ? missTtl : ttl) { return h.detail }
        let found = try await find(d)
        lock.lock()
        cache[d.url] = (now, found)
        if cache.count > 40, let oldest = cache.min(by: { $0.value.at < $1.value.at })?.key { cache[oldest] = nil }
        lock.unlock()
        return found
    }

    /// Chi tiết phim KKPhim khớp với phim của trang; nil = KKPhim không có.
    private static func find(_ d: FilmDetail) async throws -> [String: Any]? {
        let mine = Set([d.title, d.otherTitle].compactMap { $0 }.map(norm).filter { !$0.isEmpty })
        if mine.isEmpty { return nil }
        lock.lock()
        let known = slugs[d.url]
        lock.unlock()
        if let s = known, let m = try await movie(s) { return m }
        // Slug của trang thường trùng KKPhim (tien-nghich, dau-pha-thuong-khung-phan-5…); chỉ nhận phim hoạt hình ở bước này
        if let own = slugOf(d.url), let m = try await movie(own),
           let mv = J.obj(m, "movie"), J.str(mv, "type") == "hoathinh", matches(mv, mine) {
            remember(d.url, own)
            return m
        }
        let q = "\(api)/v1/api/tim-kiem?keyword=" + String(d.title.prefix(80)).formEncoded + "&limit=20"
        let items = J.objects(J.arr(J.obj(try await getJSON(q), "data"), "items"))
        guard let hit = choose(items, mine), let slug = J.str(hit, "slug").nonEmpty else { return nil }
        let m = try await movie(slug)
        if m != nil { remember(d.url, slug) }
        return m
    }

    /// Kết quả tìm khớp tên (phim hoạt hình trước — tìm kiếm của KKPhim có thể ra phim người đóng trùng tên).
    static func choose(_ items: [[String: Any]], _ mine: Set<String>) -> [String: Any]? {
        let hits = items.filter { matches($0, mine) }
        return hits.first { J.str($0, "type") == "hoathinh" } ?? hits.first
    }

    private static func remember(_ film: String, _ slug: String) {
        lock.lock()
        slugs[film] = slug
        lock.unlock()
    }

    /// Chi tiết phim trên KKPhim; nil nếu không có slug này (KKPhim trả 404).
    private static func movie(_ slug: String) async throws -> [String: Any]? {
        do {
            let o = try await getJSON("\(api)/phim/" + slug.formEncoded)
            return J.bool(o, "status") && J.obj(o, "movie") != nil ? o : nil
        } catch let e as Http.HttpError where e.code == 404 {
            return nil
        }
    }

    private static func getJSON(_ url: String) async throws -> [String: Any] {
        let r = try await Http.get(url, headers: ["Accept": "application/json"])
        guard let o = try JSONSerialization.jsonObject(with: r.data) as? [String: Any] else { throw AppError("KKPhim trả dữ liệu lạ") }
        return o
    }

    private static let slugRx = Rx(#"^[a-z0-9-]+$"#)

    /// Slug của phim trong địa chỉ trang: ".../tu-tien/tien-nghich" → "tien-nghich".
    static func slugOf(_ url: String) -> String? {
        guard let path = URL(string: url)?.path else { return nil }
        var s = path.trimmingSuffix("/").afterLast("/")
        if s.hasSuffix(".html") { s = String(s.dropLast(5)) }
        return slugRx.matches(s) ? s : nil
    }

    static func matches(_ m: [String: Any], _ mine: Set<String>) -> Bool { !names(m).isDisjoint(with: mine) }

    private static let paren = Rx(#"^(.+?)\s*\((.+)\)\s*$"#)
    private static let digits = Rx(#"^\d+$"#)

    /// Các tên của một phim KKPhim (đã chuẩn hoá): tên, tên gốc; tên có phần trong ngoặc thì thêm phần ngoài / trong ngoặc
    /// ("Thần Y Cổ Đại Ở Đô Thị (Đô Thị Cổ Tiên Y)") — trừ khi trong ngoặc chỉ là số phần ("Đấu Phá Thương Khung (Phần 5)").
    static func names(_ m: [String: Any]) -> Set<String> {
        var out = Set<String>()
        // NguonC: original_name có thể gồm nhiều tên cách nhau dấu phẩy ("Mu Shen Ji, Tales of Herding Gods")
        let others = J.str(m, "original_name").split(separator: ",").map { String($0).trimmed }.filter { !$0.isEmpty }
        for raw in [J.str(m, "name"), J.str(m, "origin_name")] + others {
            out.insert(norm(raw))
            if let g = paren.groups(in: raw) {
                let inner = norm(g[2])
                if !digits.matches(inner) {
                    out.insert(norm(g[1]))
                    out.insert(inner)
                }
            }
        }
        out.remove("")
        return out
    }

    private static let nonAlnum = Rx(#"[^a-z0-9]+"#)
    private static let season = Rx(#"\b(phan|season|mua|part|ss)\s*(\d+)"#)
    private static let spaces = Rx(#"\s+"#)

    /// Tên để so: bỏ dấu, chữ thường, chỉ chữ + số; "Phần 5" / "Season 5" / "SS5" → "5".
    static func norm(_ s: String) -> String {
        var t = s.replacingOccurrences(of: "đ", with: "d").replacingOccurrences(of: "Đ", with: "D")
            .folding(options: [.diacriticInsensitive, .caseInsensitive], locale: nil).lowercased()
        t = nonAlnum.replace(in: t, with: " ")
        t = season.replace(in: t, with: "$2")
        return spaces.replace(in: t, with: " ").trimmed
    }
}
