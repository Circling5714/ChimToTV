import Foundation

// Trang chủ tổng hợp (từ iPad 1.8) — bản Android: HomeFeed.kt (giữ cùng tên hàm để sửa hai bên song song).

/// Thẻ phim của trang chủ tổng hợp. links: các nguồn có phim này, nguồn mở trước đứng đầu.
struct Card: Hashable {
    let film: Film
    let chip: String?
    let links: [FeedItem]
}

/// Một hàng. more: địa chỉ danh sách đầy đủ (nil = «Xem thêm» hiện mọi phim đã có trong cards).
struct FeedRow: Hashable {
    let id: String
    let title: String
    let cards: [Card]
    var more: String? = nil
}

/// Phim đang xem dở vừa có tập mới (key: tên đã chuẩn hoá; ep: số tập mới nhất).
struct FreshCard: Hashable {
    let card: Card
    let key: String
    let ep: Double
}

enum HomeFeed {
    private static let day: Int64 = 86_400_000
    /// Phim xem trong 30 ngày qua mới được tự báo tập mới; tập mới phải ra trong 7 ngày qua.
    private static let freshWatched = 30 * day
    private static let freshNew = 7 * day
    /// Số phim tối đa mỗi hàng (giữ cho «Xem thêm»).
    private static let rowMax = 40

    enum Role: Int { case web, new, similar, interest, dominant, dub, kids, movies, topic, tags }

    /// Một phần cần tải: vai trò + tiêu đề hàng / địa chỉ «Xem thêm» (nếu có).
    struct Part {
        let role: Role
        let req: Feed.Req
        var title: String? = nil
        var more: String? = nil
        var id: String = ""
        var key: String { req.key }
        var rowId: String { id.isEmpty ? req.key : id }
    }

    /// Chủ đề cho người xem chưa có lịch sử (id lưu trong Prefs.homeTopics).
    static let topics: [(id: String, label: String)] = [
        ("cn", "Hoạt hình Trung Quốc"), ("kh", "Kiếm hiệp"), ("kr", "Phim Hàn"), ("mv", "Phim lẻ Âu Mỹ"),
        ("hk", "Phim xưa Hồng Kông"), ("kid", "Hoạt hình cho bé"), ("jp", "Anime Nhật"), ("tm", "Thuyết minh / lồng tiếng"),
    ]

    /// Nguồn trong danh sách trang của người xem (gốc địa chỉ); web: trang web đang dùng + trang hay xem gần đây.
    struct Sources {
        let kk: String?, nc: String?, vs: String?, k20: String?
        let web: [String]
        var count: Int { [kk, nc, vs, k20].compactMap { $0 }.count + web.count }
    }

    static func sources() -> Sources {
        let sites = Prefs.sites
        let base = Prefs.baseUrl
        let kk = sites.first { ApiSite.isKk($0) }.map(ApiSite.origin)
        let nc = sites.first { NguonC.isSite($0) }.map(ApiSite.origin)
        let vs = sites.first { ApiSite.isVs($0) }.map(ApiSite.origin)
        let k20 = sites.first { Stremio.isSite($0) }.map(ApiSite.origin)
        let api = Set([kk, nc, vs, k20].compactMap { $0 }.compactMap { Site.hostOf($0) })
        let now = nowMs()
        let recent = Prefs.history.filter { now - $0.time < freshWatched }.compactMap { Site.hostOf($0.filmUrl) }.distinct { $0 }
        // Trang web đọc HTML: tối đa 2 trang (trang đang dùng + trang xem gần nhất)
        let web = ([base] + recent.compactMap { h in sites.first { Site.hostOf($0) == h } })
            .filter { Site.hostOf($0) != nil && !api.contains(Site.hostOf($0)!) }.distinct { Site.hostOf($0)! }.prefix(2)
        return Sources(kk: kk, nc: nc, vs: vs, k20: k20, web: Array(web))
    }

    // MARK: - Sở thích

    /// Thể loại quá chung (không dùng để gợi ý); chinh-kich / tam-ly tính nửa điểm.
    private static let generic: Set<String> = ["hoat-hinh", "phim-18", "tv-shows", "phim-bo", "phim-le"]
    private static let half: Set<String> = ["chinh-kich", "tam-ly"]
    /// Không hợp trẻ em (hồ sơ trẻ em bỏ các phim này và mọi phim không phải hoạt hình).
    private static let adult: Set<String> = ["phim-18", "kinh-di", "hinh-su", "chien-tranh"]
    /// Tên phim ma / kinh dị mà nguồn không gắn thể loại.
    private static let scary = Rx(#"(?i)horror|kinh dị|xác sống|zombie|ma quỷ|quỷ dữ|ác quỷ|kyoufu"#)

    struct Combo: Hashable {
        let type: String, country: String, genre: String
    }

    struct Pair: Hashable {
        let type: String, country: String
    }

    /// Điểm sở thích của một người xem.
    struct Taste {
        let genre: [String: Double]
        let combo: [Combo: Double]
        let pair: [Pair: Double]
        let total: Double

        func affinity(_ t: Tags?) -> Double {
            guard let t = t else { return 0 }
            let p = pair[Pair(type: t.type ?? "", country: t.countries.first ?? "")] ?? 0
            return t.genres.reduce(0) { $0 + (genre[$1] ?? 0) } + p
        }
    }

    /// Điểm của mỗi phim = số tập đã xem (1–20) × ½ mũ (số ngày từ lần xem cuối ÷ 14), cộng vào thể loại, cặp (loại, nước)
    /// và bộ ba (loại, nước, thể loại). Phim chưa biết thể loại thì bỏ qua.
    static func taste(_ history: [HistoryItem], tagsOf: (String) -> Tags?, watched: (String) -> Int, now: Int64) -> Taste {
        var genre: [String: Double] = [:], combo: [Combo: Double] = [:], pair: [Pair: Double] = [:]
        var total = 0.0
        for h in history {
            let days = Double(max(0, now - h.time)) / Double(day)
            guard days <= 120, let t = tagsOf(h.title) else { continue }
            let w = Double(min(20, max(1, watched(h.filmUrl)))) * pow(0.5, days / 14)
            total += w
            let type = t.type ?? "", country = t.countries.first ?? ""
            pair[Pair(type: type, country: country), default: 0] += w
            for g in t.genres where !generic.contains(g) {
                let gw = half.contains(g) ? w / 2 : w
                genre[g, default: 0] += gw
                if !type.isEmpty && !country.isEmpty { combo[Combo(type: type, country: country, genre: g), default: 0] += gw }
            }
        }
        return Taste(genre: genre, combo: combo, pair: pair, total: total)
    }

    private static func typeSlug(_ type: String) -> String? {
        switch type {
        case "hoathinh": return "hoat-hinh"
        case "series": return "phim-bo"
        case "single": return "phim-le"
        case "tvshows": return "tv-shows"
        default: return nil
        }
    }

    static func typeName(_ type: String) -> String {
        switch type {
        case "hoathinh": return "hoạt hình"
        case "series": return "phim bộ"
        case "single": return "phim lẻ"
        case "tvshows": return "TV show"
        default: return "phim"
        }
    }

    // MARK: - Lập danh sách cần tải

    /// pid: người xem lúc lập kế hoạch; kids: hồ sơ trẻ em.
    struct Plan {
        let parts: [Part]
        let src: Sources
        var similar: HistoryItem? = nil
        var similarTags: Tags? = nil
        var taste: Taste? = nil
        var topics: Set<String> = []
        var pid = ""
        var kids = false
    }

    private static let json = ["Accept": "application/json"]

    private static func getRoot(_ url: String) async throws -> [String: Any] {
        let r = try await Http.get(url, headers: json)
        guard let o = try JSONSerialization.jsonObject(with: r.data) as? [String: Any] else { throw AppError("Nguồn trả dữ liệu lạ") }
        return o
    }

    /// Lập kế hoạch tải cho người xem đang dùng (đọc Prefs, không chạm mạng).
    static func plan(tagsKnownOnly: Bool = false) -> Plan {
        let s = sources()
        let kids = Prefs.kids
        var parts: [Part] = []
        let now = nowMs()
        let history = Prefs.history
        func host(_ o: String) -> String { Site.hostOf(o) ?? "" }

        // Hồ sơ trẻ em: không lấy danh sách «mới cập nhật» chung, chỉ các danh sách hoạt hình
        if !kids {
            if let o = s.kk {
                parts.append(Part(role: .new, req: Feed.Req(key: "new|\(o)", host: host(o)) {
                    Feed.Loaded(items: Feed.parseKk(try await ApiSite.json("\(o)/api/movies/newest?page=1&limit=40"), o))
                }))
            }
            if let o = s.nc {
                parts.append(Part(role: .new, req: Feed.Req(key: "new|\(o)", host: host(o)) {
                    Feed.Loaded(items: Feed.parseNc(try await getRoot(NguonC.newestUrl(o)), o))
                }))
            }
            if let o = s.vs {
                parts.append(Part(role: .new, req: Feed.Req(key: "new|\(o)", host: host(o)) {
                    Feed.Loaded(items: Feed.parseKk(try await ApiSite.json("\(o)/api/danh-sach/phim-moi-cap-nhat?page=1"), o))
                }))
            }
            // Trang web (trang đang dùng trước): phim mới cập nhật + menu mục của trang
            for w in s.web {
                parts.append(Part(role: .web, req: Feed.Req(key: "web|\(w)", host: host(w)) {
                    let h = try await Site.home(w)
                    return Feed.Loaded(items: Array(Feed.fromFilms(newestOf(h)).prefix(rowMax)), categories: h.categories, movedTo: h.movedTo)
                }))
            }
            if let o = s.k20 {
                parts.append(Part(role: .new, req: Feed.Req(key: "new|\(o)", host: host(o)) {
                    Feed.Loaded(items: Feed.fromFilms(try await Stremio.list(Stremio.newestUrl(o)).films))
                }))
            }
        }

        // Thể loại của phim đã xem chưa biết: tra KKPhim (mỗi lượt tối đa 6 phim; không thấy thì 3 ngày sau mới tra lại)
        if !tagsKnownOnly && (s.kk != nil || Prefs.kkBackup) {
            let todo = history.filter { now - $0.time < 120 * day }.distinct { KkPhim.norm($0.title) }
                .filter { FilmTags.needsLookup($0.title) }.prefix(6)
            for h in todo {
                parts.append(Part(role: .tags, req: Feed.Req(key: "tags|\(KkPhim.norm(h.title))", host: "phimapi.com", keep: false) {
                    try await FilmTags.lookup(h.title)
                    return Feed.Loaded(items: [])
                }))
            }
        }

        let counts = Prefs.watchedCounts()
        let taste: Taste? = history.isEmpty ? nil : self.taste(history, tagsOf: FilmTags.of, watched: { counts[$0] ?? 0 }, now: now)
        let topics = Prefs.homeTopics
        var similar: HistoryItem?
        var similarTags: Tags?
        var top: Pair?
        if let taste = taste, taste.total > 0 {
            // Giống «phim vừa xem»: cùng loại phim, nước và thể loại riêng nhất (trẻ em: chỉ phim hoạt hình)
            similar = history.first { h in
                guard let t = FilmTags.of(h.title) else { return false }
                return t.type != nil && !t.countries.isEmpty && (!kids || t.type == "hoathinh")
            }
            similarTags = similar.flatMap { FilmTags.of($0.title) }
            var used = Set<String>()
            if let sim = similar, let st = similarTags {
                let g = st.genres.filter { !generic.contains($0) && !(kids && adult.contains($0)) }
                    .enumerated().sorted { ((half.contains($0.element) ? 1 : 0), $0.offset) < ((half.contains($1.element) ? 1 : 0), $1.offset) }
                    .first?.element
                if let url = filterUrl(s, st.type!, st.countries[0], g) {
                    if let g = g { used.insert(g) }
                    parts.append(Part(role: .similar, req: req(url), title: "Giống «\(sim.title)»", more: url, id: "similar"))
                }
            }
            // Vì … hay xem <thể loại>: hai bộ ba điểm cao nhất, khác thể loại với hàng trên
            var n = 0
            for cb in taste.combo.sorted(by: { $0.value > $1.value }).map({ $0.key }) where n < 2 {
                if used.contains(cb.genre) || half.contains(cb.genre) { continue }
                if kids && (cb.type != "hoathinh" || adult.contains(cb.genre)) { continue }
                guard let url = filterUrl(s, cb.type, cb.country, cb.genre) else { continue }
                used.insert(cb.genre)
                n += 1
                let title = "Vì \(Prefs.profile.name) hay xem \(ApiSite.genreName(cb.genre)) · \(typeName(cb.type)) \(ApiSite.countryName(cb.country))"
                parts.append(Part(role: .interest, req: req(url), title: title, more: url, id: "interest-\(cb.genre)"))
            }
            // Loại phim + nước xem nhiều nhất (≥ 40%): hàng mới cập nhật riêng, gộp mọi nguồn
            if let best = taste.pair.filter({ !$0.key.type.isEmpty && !$0.key.country.isEmpty }).max(by: { $0.value < $1.value }),
               best.value >= taste.total * 0.4 {
                top = best.key
            }
            if let top = top, !kids {
                let t = typeName(top.type)
                let title = "\(t.prefix(1).uppercased() + t.dropFirst()) \(ApiSite.countryName(top.country)) mới cập nhật · mọi nguồn"
                if let url = filterUrl(s, top.type, top.country, nil) {
                    parts.append(Part(role: .dominant, req: req(url), title: title, more: nil, id: "dominant"))
                }
            }
        } else if history.isEmpty && !kids {
            for (id, label) in Self.topics where topics.contains(id) {
                if let tr = topicReq(s, id) {
                    parts.append(Part(role: .topic, req: tr.0, title: "Vì bạn chọn: \(label)", more: tr.1, id: "topic-\(id)"))
                }
            }
        }
        if kids {
            parts += kidsParts(s)
        } else {
            if let d = dubPart(s, top) { parts.append(d) }
            // Phim lẻ mới (xếp theo thể loại người xem hay xem)
            if let o = s.kk {
                let u = "\(o)/api/movies/category/phim-le?page=1&limit=30"
                parts.append(Part(role: .movies, req: req(u), title: "Phim lẻ mới", more: u, id: "movies"))
            } else if let o = s.vs {
                let u = "\(o)/api/danh-sach/phim-le?page=1"
                parts.append(Part(role: .movies, req: req(u), title: "Phim lẻ mới", more: u, id: "movies"))
            }
        }
        return Plan(parts: parts, src: s, similar: similar, similarTags: similarTags, taste: taste, topics: topics,
                    pid: Prefs.profileId, kids: kids)
    }

    /// Người hay xem bản thuyết minh / lồng tiếng (từ nửa số lần mở tập gần đây, ít nhất 5 lần): hàng riêng.
    private static func dubPart(_ s: Sources, _ top: Pair?) -> Part? {
        let langs = Prefs.watchLangs
        let tm = langs.filter { $0 == "tm" }.count, lt = langs.filter { $0 == "lt" }.count
        guard langs.count >= 5, (tm + lt) * 2 >= langs.count else { return nil }
        let slug = lt > tm ? "long-tieng" : "thuyet-minh"
        let name = lt > tm ? "Lồng tiếng" : "Thuyết minh"
        if let o = s.kk {
            if let top = top, let t = typeSlug(top.type) {
                let url = "\(o)/api/movies/category/\(t)?country=\(top.country)&sort_lang=\(slug)&page=1&limit=30"
                return Part(role: .dub, req: req(url), title: "\(name) mới · \(typeName(top.type)) \(ApiSite.countryName(top.country))",
                            more: url, id: "dub")
            }
            let url = "\(o)/api/movies/category/phim-\(slug)?page=1&limit=30"
            return Part(role: .dub, req: req(url), title: "Phim \(name.lowercased()) mới", more: url, id: "dub")
        }
        if let o = s.nc {
            let url = "\(o)/api/films/ngon-ngu/\(slug)?page=1"
            return Part(role: .dub, req: Feed.Req(key: "list|\(url)", host: Site.hostOf(o) ?? "") {
                Feed.Loaded(items: Feed.parseNc(try await getRoot(url), o))
            }, title: "Phim \(name.lowercased()) mới", more: url, id: "dub")
        }
        return nil
    }

    /// Hồ sơ trẻ em: hoạt hình thiếu nhi (KKPhim «trẻ em» + VSMOV «thiếu nhi»), gia đình, lồng tiếng.
    private static func kidsParts(_ s: Sources) -> [Part] {
        var out: [Part] = []
        if let o = s.kk {
            let u = "\(o)/api/movies/category/hoat-hinh?category=tre-em&page=1&limit=30"
            out.append(Part(role: .kids, req: req(u), title: "Hoạt hình thiếu nhi", more: u, id: "kids-young"))
        }
        if let o = s.vs {
            let u = "\(o)/api/the-loai/thieu-nhi?page=1"
            out.append(Part(role: .kids, req: req(u), title: "Hoạt hình thiếu nhi", more: u, id: "kids-young"))
        }
        if let o = s.kk {
            let fam = "\(o)/api/movies/category/hoat-hinh?category=gia-dinh&page=1&limit=30"
            out.append(Part(role: .kids, req: req(fam), title: "Hoạt hình gia đình", more: fam, id: "kids-family"))
            let dub = "\(o)/api/movies/category/hoat-hinh?sort_lang=long-tieng&page=1&limit=30"
            out.append(Part(role: .kids, req: req(dub), title: "Hoạt hình lồng tiếng", more: dub, id: "kids-dub"))
        }
        return out
    }

    /// Danh sách lọc theo loại phim + nước (+ thể loại): KKPhim lọc được cả ba; không có KKPhim thì VSMOV.
    private static func filterUrl(_ s: Sources, _ type: String, _ country: String, _ genre: String?) -> String? {
        if let o = s.kk, let t = typeSlug(type) {
            return "\(o)/api/movies/category/\(t)?country=\(country)" + (genre.map { "&category=\($0)" } ?? "") + "&page=1&limit=30"
        }
        if let o = s.vs {
            let p = genre.map { "the-loai/\($0)?country=\(country)" } ?? (type == "hoathinh" ? "the-loai/hoat-hinh?country=\(country)" : "quoc-gia/\(country)")
            return page("\(o)/api/\(p)")
        }
        return nil
    }

    private static func page(_ url: String) -> String { url + (url.contains("?") ? "&" : "?") + "page=1" }

    /// Đọc một danh sách KKPhim / VSMOV theo địa chỉ (định dạng KKPhim).
    private static func req(_ url: String) -> Feed.Req {
        let o = ApiSite.origin(url)
        return Feed.Req(key: "list|\(url)", host: Site.hostOf(o) ?? "") { Feed.Loaded(items: Feed.parseKk(try await ApiSite.json(url), o)) }
    }

    /// Danh sách cho một chủ đề: nguồn đầu tiên có trong danh sách trang. (yêu cầu, địa chỉ «Xem thêm»).
    private static func topicReq(_ s: Sources, _ id: String) -> (Feed.Req, String?)? {
        func kk(_ p: String) -> String? { s.kk.map { page("\($0)/api/movies/\(p)") + "&limit=30" } }
        func vs(_ p: String) -> String? { s.vs.map { page("\($0)/api/\(p)") } }
        let url: String?
        switch id {
        case "cn": url = kk("category/hoat-hinh?country=trung-quoc") ?? vs("the-loai/hoat-hinh?country=trung-quoc")
        case "kh": url = vs("the-loai/kiem-hiep") ?? s.k20.map { Stremio.catalog($0, genre: "Kiếm Hiệp") }
        case "kr": url = kk("category/phim-bo?country=han-quoc") ?? vs("quoc-gia/han-quoc")
        case "mv": url = kk("category/phim-le?country=au-my") ?? vs("danh-sach/phim-le")
        case "hk": url = s.k20.map { Stremio.newestUrl($0) } ?? kk("quoc-gia/hong-kong")
        case "kid": url = vs("the-loai/thieu-nhi") ?? kk("category/hoat-hinh?category=gia-dinh")
        case "jp": url = kk("category/hoat-hinh?country=nhat-ban") ?? vs("the-loai/hoat-hinh?country=nhat-ban")
        case "tm": url = kk("category/phim-thuyet-minh") ?? s.nc.map { "\($0)/api/films/ngon-ngu/thuyet-minh?page=1" }
        default: url = nil
        }
        guard let u = url else { return nil }
        let o = ApiSite.origin(u)
        let r = Feed.Req(key: "list|\(u)", host: Site.hostOf(u) ?? "") {
            if Stremio.isSite(u) { return Feed.Loaded(items: Feed.fromFilms(try await Stremio.list(u).films)) }
            if NguonC.isSite(u) { return Feed.Loaded(items: Feed.parseNc(try await getRoot(u), o)) }
            return Feed.Loaded(items: Feed.parseKk(try await ApiSite.json(u), o))
        }
        return (r, u)
    }

    private static let newRx = Rx(#"(?i)mới|cập nhật|update|new"#)

    /// Khối «mới cập nhật» của trang web (không có thì khối lớn nhất / mọi khối).
    private static func newestOf(_ h: Home) -> [Film] {
        var pick = h.sections.filter { newRx.matches($0.title) }
        if pick.isEmpty { pick = Array(h.sections.filter { $0.films.count >= 6 }.prefix(1)) }
        if pick.isEmpty { pick = h.sections }
        return pick.flatMap { $0.films }.distinct { $0.url }
    }

    // MARK: - Dựng trang

    /// Một lần dựng trang chủ. note: dòng trạng thái; noteGaveUp: khi thôi chờ nguồn chậm; pending: còn nguồn chậm có thể
    /// về muộn — late() dựng lại khi chúng về.
    struct Snapshot {
        let rows: [FeedRow]
        let fresh: [FreshCard]
        let categories: [Category]?
        var movedTo: (old: String, new: String)? = nil
        let note: String
        let noteGaveUp: String
        let empty: Bool
        let fromCache: Bool
        var pending = false
        var results: [(Part, Feed.Result)] = []
        var plan: Plan? = nil
        var startedAt: Int64 = 0
    }

    /// Nguồn chậm (quá hạn / chưa tới lượt) — khác nguồn lỗi hẳn: có thể về muộn.
    private static func slow(_ r: Feed.Result) -> Bool {
        r.state != .ok && !r.resting && (r.error == nil || r.error!.hasPrefix("quá"))
    }

    /// Nguồn chậm của lần dựng trước đã về (bản lưu mới hơn lúc bắt đầu tải): dựng lại; chưa có gì mới thì nil.
    static func late(_ s: Snapshot) -> Snapshot? {
        guard let plan = s.plan else { return nil }
        var changed = false
        let res = s.results.map { (p, r) -> (Part, Feed.Result) in
            guard p.role != .tags, slow(r), p.req.keep, let c = Feed.readCache(p.key), c.at >= s.startedAt else { return (p, r) }
            changed = true
            return (p, Feed.Result(req: p.req, state: .ok, items: c.loaded.items, categories: c.loaded.categories, error: nil,
                                   resting: false, ms: r.ms, savedAt: c.at))
        }
        guard changed else { return nil }
        let snap = build(plan, res, fromCache: false, startedAt: s.startedAt)
        saveLast(plan.pid, snap)
        return snap
    }

    /// Bản lưu của mọi phần; nil nếu chưa có bản lưu nào.
    static func cached(_ plan: Plan) -> Snapshot? {
        let results = plan.parts.map { p -> (Part, Feed.Result) in
            if let c = Feed.readCache(p.key) {
                return (p, Feed.Result(req: p.req, state: .cached, items: c.loaded.items, categories: c.loaded.categories, error: nil,
                                       resting: false, ms: 0, savedAt: c.at))
            }
            return (p, Feed.Result(req: p.req, state: .noData, items: [], categories: nil, error: nil, resting: false, ms: 0, savedAt: 0))
        }
        guard results.contains(where: { $0.0.role != .tags && !$0.1.items.isEmpty }) else { return nil }
        return build(plan, results, fromCache: true)
    }

    /// Tải mọi phần rồi dựng trang. Lượt đầu chưa biết thể loại phim đã xem: tra xong thì lập lại kế hoạch, tải thêm hàng gợi ý.
    static func load(_ plan: Plan) async -> Snapshot {
        let started = nowMs()
        var p = plan
        var results = Array(zip(p.parts, await Feed.load(p.parts.map { $0.req })))
        if p.parts.contains(where: { $0.role == .tags }) {
            let again = self.plan(tagsKnownOnly: true)
            let have = Set(results.map { $0.0.key })
            let extra = again.parts.filter { !have.contains($0.key) }
            if !extra.isEmpty { results += Array(zip(extra, await Feed.load(extra.map { $0.req }))) }
            var byKey: [String: Feed.Result] = [:]
            for (q, r) in results { byKey[q.key] = r }
            results = again.parts.compactMap { a in byKey[a.key].map { (a, $0) } } + results.filter { $0.0.role == .tags }
            p = again
        }
        let snap = build(p, results, fromCache: false, startedAt: started)
        saveLast(p.pid, snap)
        return snap
    }

    /// Dữ liệu người xem dùng khi dựng trang (tách khỏi Prefs để test).
    struct Ctx {
        let history: [HistoryItem]
        let follows: Set<String>
        let seen: (String) -> Double?
        let base: String
        let sites: [String]
        let now: Int64
    }

    private static let timeFmt: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm"
        return f
    }()

    static func build(_ plan: Plan, _ results: [(Part, Feed.Result)], fromCache: Bool, startedAt: Int64 = 0) -> Snapshot {
        let now = nowMs()
        let ctx = Ctx(history: Prefs.history, follows: Set(Prefs.follows.map { $0.filmUrl }), seen: Prefs.seenEp,
                      base: Site.hostOf(Prefs.baseUrl) ?? "", sites: Prefs.sites, now: now)
        let built = buildRows(plan, results, ctx)
        let webBase = results.first { $0.0.role == .web && $0.0.key == "web|\(Prefs.baseUrl)" }?.1
        let cats = webBase?.categories ?? Site.menu(Prefs.baseUrl)
        let moved = results.lazy.compactMap { (p, r) -> (String, String)? in
            guard p.role == .web, r.state == .ok, let m = r.movedTo else { return nil }
            return (String(p.key.dropFirst("web|".count)), m)
        }.first
        let content = fromCache ? [] : results.filter { $0.0.role != .tags }
        func names(_ l: [(Part, Feed.Result)]) -> String { l.map { $0.0.req.host }.distinct { $0 }.map { Site.tag($0, ctx.sites) }.joined(separator: ", ") }
        let slowParts = content.filter { slow($0.1) }
        let failedParts = content.filter { $0.1.state != .ok && !slow($0.1) && !$0.1.resting }
        let restParts = content.filter { $0.1.resting }
        let time = timeFmt.string(from: Date(timeIntervalSince1970: Double(results.map { $0.1.savedAt }.max() ?? now) / 1000))
        let tail = (failedParts.isEmpty ? "" : " · lỗi: \(names(failedParts))") + (restParts.isEmpty ? "" : " · tạm nghỉ: \(names(restParts))")
        let note: String
        if fromCache { note = "Bản lưu lúc \(time) · đang làm mới…" }
        else if !slowParts.isEmpty { note = "Cập nhật lúc \(time) · đang chờ \(names(slowParts))…\(tail)" }
        else { note = "Cập nhật lúc \(time)\(tail)" }
        let gaveUp = "Cập nhật lúc \(time)" + (slowParts.isEmpty ? "" : " · chậm, dùng bản lưu: \(names(slowParts))") + tail
        return Snapshot(rows: built.rows, fresh: built.fresh, categories: cats, movedTo: moved.map { (old: $0.0, new: $0.1) },
                        note: note, noteGaveUp: gaveUp, empty: built.rows.isEmpty && built.fresh.isEmpty, fromCache: fromCache,
                        pending: !slowParts.isEmpty, results: results, plan: plan, startedAt: startedAt)
    }

    static func buildRows(_ plan: Plan, _ results: [(Part, Feed.Result)], _ ctx: Ctx) -> (rows: [FeedRow], fresh: [FreshCard]) {
        let content = results.filter { $0.0.role != .tags }
        // Gộp mọi danh sách (trang web đang dùng trước, rồi các nguồn API) — một phim chỉ thuộc một nhóm
        let ordered = content.enumerated().sorted { ($0.element.0.role.rawValue, $0.offset) < ($1.element.0.role.rawValue, $1.offset) }.map { $0.element }
        let merged = Feed.merge(ordered.map { $0.1.items }, now: ctx.now)
        var groupOf: [String: FeedFilm] = [:]
        for f in merged { for i in f.items { groupOf[i.url] = f } }
        let down = Set(content.filter { $0.1.state != .ok }.map { $0.0.req.host })

        // Thứ tự nguồn khi mở phim: trang đang dùng, trang hay xem gần đây, rồi KKPhim / NguonC / VSMOV / CLB Phim Xưa
        let prefer = ([ctx.base] + ctx.history.compactMap { Site.hostOf($0.filmUrl) } +
            [plan.src.kk, plan.src.nc, plan.src.vs, plan.src.k20].compactMap { $0 }.compactMap { Site.hostOf($0) }).distinct { $0 }
        var histUrl: [String: HistoryItem] = [:], histName: [String: HistoryItem] = [:]
        for h in ctx.history {
            if histUrl[h.filmUrl] == nil { histUrl[h.filmUrl] = h }
            for n in KkPhim.names(["name": h.title]) where histName[n] == nil { histName[n] = h }
        }
        var memo: [ObjectIdentifier: HistoryItem?] = [:]
        func historyOf(_ f: FeedFilm) -> HistoryItem? {
            if let m = memo[ObjectIdentifier(f)] { return m }
            let h = f.items.lazy.compactMap { histUrl[$0.url] }.first ?? f.names.lazy.compactMap { histName[$0] }.first
            memo[ObjectIdentifier(f)] = .some(h)
            return h
        }
        func watched(_ f: FeedFilm) -> Bool { historyOf(f) != nil }
        var tags: [String: String] = [:]
        func tag(_ host: String) -> String {
            if let t = tags[host] { return t }
            let t = Site.tag(host, ctx.sites)
            tags[host] = t
            return t
        }

        func card(_ f: FeedFilm) -> Card {
            let own = historyOf(f).flatMap { Site.hostOf($0.filmUrl) }
            let o = f.ordered([own].compactMap { $0 } + prefer)
            let live = o.items.filter { !down.contains($0.host) }
            let first = live.first ?? o.items[0]
            let links = [first] + o.items.filter { $0.url != first.url }
            let badge = f.latest?.ep.map { "Tập \(num($0))" } ?? first.badge
            let chip = live.isEmpty ? "bản lưu" : (live.count > 1 ? "\(live.count) nguồn" : tag(first.host))
            return Card(film: Film(url: first.url, title: first.title, poster: links.lazy.compactMap { $0.poster }.first, badge: badge),
                        chip: chip, links: links)
        }

        // Hồ sơ trẻ em: chỉ phim hoạt hình, không có thể loại không hợp trẻ em (phim chưa rõ thể loại — vd danh sách «thiếu
        // nhi» của VSMOV — giữ lại vì đã đến từ danh sách dành cho trẻ em)
        func kidsOk(_ f: FeedFilm) -> Bool {
            guard plan.kids else { return true }
            if f.items.contains(where: { scary.matches($0.title) }) { return false }
            guard let t = f.tags else { return true }
            return (t.type == nil || t.type == "hoathinh") && !t.genres.contains { adult.contains($0) }
        }

        var rows: [FeedRow] = []
        var claimed = Set<FeedFilm>()
        func films(_ r: Feed.Result) -> [FeedFilm] { r.items.compactMap { groupOf[$0.url] }.distinct { ObjectIdentifier($0) } }
        func add(_ id: String, _ title: String, _ list: [FeedFilm], _ more: String?, claim: Bool, min: Int = 3) {
            let l = Array(list.filter { !claimed.contains($0) && kidsOk($0) }.prefix(rowMax))
            guard l.count >= min else { return }
            if claim { l.prefix(6).forEach { claimed.insert($0) } }
            rows.append(FeedRow(id: id, title: title, cards: l.map(card), more: more))
        }
        func byRole(_ r: Role) -> [(Part, Feed.Result)] { content.filter { $0.0.role == r } }
        func newestFirst(_ l: [FeedFilm]) -> [FeedFilm] {
            l.enumerated().sorted { ($0.element.updated, -$0.offset) > ($1.element.updated, -$1.offset) }.map { $0.element }
        }
        func byAffinity(_ l: [FeedFilm]) -> [FeedFilm] {
            l.enumerated().sorted {
                (plan.taste?.affinity($0.element.tags) ?? 0, $0.element.updated, -$0.offset) >
                    (plan.taste?.affinity($1.element.tags) ?? 0, $1.element.updated, -$1.offset)
            }.map { $0.element }
        }

        // Có tập mới: phim đang xem dở (xem trong 30 ngày, đã xem kịp tập mới nhất lúc đó) vừa có tập mới ở bất kỳ nguồn nào
        var fresh: [FreshCard] = []
        var keys: [String] = []
        var byKey: [String: [HistoryItem]] = [:]
        for h in ctx.history {
            let k = KkPhim.norm(h.title)
            if byKey[k] == nil { keys.append(k) }
            byKey[k, default: []].append(h)
        }
        for key in keys {
            let same = byKey[key]!
            let h = same[0] // xem gần nhất
            guard ctx.now - h.time <= freshWatched, !ctx.follows.contains(h.filmUrl),
                  let watchedEp = same.compactMap({ Feed.epOf($0.epTitle) }).max(),
                  let f = merged.first(where: { g in g.items.contains { $0.url == h.filmUrl } || g.names.contains(key) }),
                  let latest = f.latest, let ep = latest.ep, latest.updated >= ctx.now - freshNew else { continue }
            let seen = ctx.seen(key)
            if let s = seen, watchedEp < s - 1 { continue } // chưa xem kịp — không phải «tập mới»
            let known = max(watchedEp, seen ?? watchedEp)
            guard ep > known else { continue }
            // Xem từ trước bản này (chưa biết lúc đó phim có bao nhiêu tập): số tập ra thêm phải hợp với thời gian từ lần xem
            // cuối (≤ 2 tập + 3 tập mỗi tuần) — đang xem tập 31 mà phim đã tới 108 là đang xem dở
            if seen == nil && ep - watchedEp > Double(2 + Int(Double(ctx.now - h.time) / Double(day) / 7 * 3)) { continue }
            let ownHost = Site.hostOf(h.filmUrl) ?? ""
            let badge = ep - known >= 2 ? "+\(num(ep - known)) · Tập \(num(ep))" : "Tập \(num(ep)) mới"
            let chip: String? = down.contains(latest.host) ? "lần kiểm trước" : (latest.host != ownHost ? "có ở \(Site.tag(latest.host, ctx.sites))" : nil)
            let open = latest.host == ownHost ? (f.items.first { $0.url == h.filmUrl } ?? latest) : latest
            let links = [open] + f.items.filter { $0.url != open.url }
            fresh.append(FreshCard(card: Card(film: Film(url: open.url, title: h.title, poster: h.poster ?? open.poster, badge: badge),
                                              chip: chip, links: links), key: key, ep: ep))
        }

        // Người xem chưa có lịch sử: hàng theo chủ đề đã chọn
        for (p, r) in byRole(.topic) { add(p.rowId, p.title ?? "", newestFirst(films(r)), p.more, claim: true) }

        // Giống «phim vừa xem»: xếp theo số thể loại chung
        let simGenres = Set(plan.similarTags?.genres ?? [])
        let me = plan.similar.map { KkPhim.names(["name": $0.title]) } ?? []
        for (p, r) in byRole(.similar) {
            let list = films(r).filter { !watched($0) && !$0.matches(me) }.enumerated().sorted {
                ($0.element.tags?.genres.filter { simGenres.contains($0) }.count ?? 0, $0.element.updated, -$0.offset) >
                    ($1.element.tags?.genres.filter { simGenres.contains($0) }.count ?? 0, $1.element.updated, -$1.offset)
            }.map { $0.element }
            add(p.rowId, p.title ?? "", list, p.more, claim: true, min: 4)
        }
        let interest = byRole(.interest)
        func interestRow(_ i: Int) {
            guard i < interest.count else { return }
            let (p, r) = interest[i]
            add(p.rowId, p.title ?? "", byAffinity(films(r).filter { !watched($0) }), p.more, claim: true)
        }
        interestRow(0)

        // Loại phim hay xem, mới cập nhật · mọi nguồn
        let dominant = byRole(.dominant)
        if !dominant.isEmpty {
            var set: [FeedFilm] = []
            for (_, r) in dominant { set += films(r) }
            add("dominant", dominant[0].0.title ?? "", newestFirst(set.distinct { ObjectIdentifier($0) }.filter { !watched($0) }), nil, claim: true)
        }
        // Thuyết minh / lồng tiếng mới
        for (p, r) in byRole(.dub) { add(p.rowId, p.title ?? "", newestFirst(films(r).filter { !watched($0) }), p.more, claim: true) }
        // Hồ sơ trẻ em: hoạt hình thiếu nhi (KKPhim + VSMOV gộp một hàng), gia đình, lồng tiếng
        var kidIds: [String] = []
        var kidParts: [String: [(Part, Feed.Result)]] = [:]
        for pr in byRole(.kids) {
            if kidParts[pr.0.rowId] == nil { kidIds.append(pr.0.rowId) }
            kidParts[pr.0.rowId, default: []].append(pr)
        }
        for id in kidIds {
            let ps = kidParts[id]!
            let set = ps.flatMap { films($0.1) }.distinct { ObjectIdentifier($0) }
            add(id, ps[0].0.title ?? "", newestFirst(set.filter { !watched($0) }), ps.lazy.compactMap { $0.0.more }.first, claim: true)
        }
        interestRow(1)

        // Mới cập nhật · mọi nguồn
        let newest = (byRole(.web) + byRole(.new)).flatMap { films($0.1) }.distinct { ObjectIdentifier($0) }
        add("newest", "Mới cập nhật · mọi nguồn", newestFirst(newest.filter { !watched($0) }), nil, claim: false)

        for (p, r) in byRole(.movies) { add(p.rowId, p.title ?? "", byAffinity(films(r).filter { !watched($0) }), p.more, claim: false) }
        return (rows, fresh)
    }

    /// Người xem vừa mở trang phim / một tập: ghi tập mới nhất đã thấy (theo tên — chung mọi nguồn). Xem riêng thì không ghi.
    static func markSeen(_ d: FilmDetail) {
        guard !Prefs.privateMode,
              let n = d.groups.flatMap({ $0.episodes }).compactMap({ Feed.epOf($0.title) }).max() else { return }
        Prefs.setSeenEp(KkPhim.norm(d.title), n)
    }

    /// 108.0 → "108", 10.5 → "10.5".
    static func num(_ d: Double) -> String { d == d.rounded() ? String(Int64(d)) : String(d) }

    // MARK: - Trang chủ lần trước (mở app là hiện ngay)

    private static func lastKey(_ pid: String) -> String { "last|\(pid)" }

    static func saveLast(_ pid: String, _ s: Snapshot) {
        guard !s.fromCache, !s.empty else { return }
        var o: [String: Any] = [
            "at": nowMs(),
            "rows": s.rows.map { r in ["id": r.id, "t": r.title, "m": r.more ?? "", "c": r.cards.map(cardJson)] as [String: Any] },
            "fresh": s.fresh.map { ["c": cardJson($0.card), "k": $0.key, "e": $0.ep] as [String: Any] },
        ]
        if let cats = s.categories { o["cats"] = cats.map { ["t": $0.title, "u": $0.url] } }
        if let d = try? JSONSerialization.data(withJSONObject: o), let text = String(data: d, encoding: .utf8) {
            Feed.writeText(lastKey(pid), text)
        }
    }

    /// Trang chủ lần trước của người xem pid (dưới 24 giờ); nil nếu chưa có.
    static func last(_ pid: String) -> Snapshot? {
        guard let text = Feed.readText(lastKey(pid)), let data = text.data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let at = num64(o["at"])
        guard nowMs() - at <= Feed.cacheMax else { return nil }
        let rows = J.objects(J.arr(o, "rows")).map { r in
            FeedRow(id: J.str(r, "id"), title: J.str(r, "t"), cards: J.objects(J.arr(r, "c")).compactMap(cardOf), more: J.str(r, "m").nonEmpty)
        }
        let fresh = J.objects(J.arr(o, "fresh")).compactMap { f -> FreshCard? in
            guard let c = J.obj(f, "c").flatMap(cardOf) else { return nil }
            return FreshCard(card: c, key: J.str(f, "k"), ep: dbl(f["e"]) ?? 0)
        }
        let cats = (o["cats"] as? [Any]).map { J.objects($0).map { Category(title: J.str($0, "t"), url: J.str($0, "u")) } }
        let note = "Bản lưu lúc \(timeFmt.string(from: Date(timeIntervalSince1970: Double(at) / 1000))) · đang làm mới…"
        return Snapshot(rows: rows, fresh: fresh, categories: cats, note: note, noteGaveUp: note, empty: rows.isEmpty && fresh.isEmpty,
                        fromCache: true)
    }

    private static func cardJson(_ c: Card) -> [String: Any] {
        ["u": c.film.url, "n": c.film.title, "p": c.film.poster ?? "", "b": c.film.badge ?? "", "x": c.chip ?? "",
         "l": c.links.map { FeedItem(url: $0.url, title: $0.title, poster: $0.poster, badge: $0.badge, names: [], year: $0.year,
                                     updated: $0.updated, ep: $0.ep, tags: nil).json() }]
    }

    private static func cardOf(_ o: [String: Any]) -> Card? {
        guard let url = J.str(o, "u").nonEmpty else { return nil }
        return Card(film: Film(url: url, title: J.str(o, "n"), poster: J.str(o, "p").nonEmpty, badge: J.str(o, "b").nonEmpty),
                    chip: J.str(o, "x").nonEmpty, links: J.objects(J.arr(o, "l")).compactMap(FeedItem.of))
    }
}

/// Thể loại / quốc gia / loại phim của phim đã xem (tra KKPhim theo tên, lưu chung cả nhà). Khoá = tên đã chuẩn hoá;
/// {g, c, t, at} hoặc {miss: lúc tra không thấy}.
enum FilmTags {
    private static let maxCount = 400
    private static let missRetry: Int64 = 3 * 86_400_000
    private static let lock = NSLock()
    private static var mem: [String: [String: Any]]?
    private static var memOf: String?

    private static func store() -> [String: [String: Any]] {
        let raw = Prefs.filmTags
        if raw != memOf || mem == nil {
            mem = (raw.data(using: .utf8).flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: [String: Any]] }) ?? [:]
            memOf = raw
        }
        return mem!
    }

    static func of(_ title: String) -> Tags? {
        lock.lock(); defer { lock.unlock() }
        guard let e = store()[KkPhim.norm(title)], e["miss"] == nil else { return nil }
        return Tags.of(e)
    }

    static func needsLookup(_ title: String) -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard let e = store()[KkPhim.norm(title)] else { return true }
        guard let miss = e["miss"] else { return false }
        return nowMs() - num64(miss) > missRetry
    }

    static func remember(_ key: String, _ tags: Tags?) {
        guard !key.isEmpty else { return }
        lock.lock(); defer { lock.unlock() }
        var s = store()
        if let t = tags {
            var o = t.json()
            o["at"] = nowMs()
            s[key] = o
        } else {
            s[key] = ["miss": nowMs()]
        }
        if s.count > maxCount {
            func at(_ o: [String: Any]) -> Int64 { o["at"] != nil ? num64(o["at"]) : num64(o["miss"]) }
            for k in s.sorted(by: { at($0.value) < at($1.value) }).prefix(s.count - maxCount).map({ $0.key }) { s[k] = nil }
        }
        if let d = try? JSONSerialization.data(withJSONObject: s), let raw = String(data: d, encoding: .utf8) {
            Prefs.filmTags = raw
            mem = s
            memOf = raw
        }
    }

    /// Tra KKPhim theo tên (lỗi mạng → ném lỗi, không ghi; không thấy → ghi «không có»).
    static func lookup(_ title: String) async throws {
        let q = KkPhim.api + "/v1/api/tim-kiem?keyword=" + String(title.prefix(80)).formEncoded + "&limit=10"
        let r = try await Http.get(q, headers: ["Accept": "application/json"])
        let root = (try? JSONSerialization.jsonObject(with: r.data) as? [String: Any]) ?? [:]
        let items = J.objects(J.arr(J.obj(root, "data"), "items"))
        let hit = KkPhim.choose(items, KkPhim.names(["name": title]))
        remember(KkPhim.norm(title), hit.flatMap(Tags.fromKk))
    }
}
