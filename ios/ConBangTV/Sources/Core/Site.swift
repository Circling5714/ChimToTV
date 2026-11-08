import Foundation
import SwiftSoup

// Đọc dữ liệu từ trang phim (bản Android: Site.kt — giữ cùng tên hàm, cùng bộ chọn CSS để sửa song song).
// Không viết riêng cho từng trang: nhận dạng theo mẫu giao diện ("flw-item" kiểu yanhh3d, "halim-item" của theme HaLim).
// Trang dựng bằng Next.js (kiểu PhimMoi) thì chuyển cho NextSite; trang ứng dụng JS có API JSON (mephim) cho ApiSite.

extension Element {
    func q(_ css: String) -> [Element] { (try? select(css).array()) ?? [] }
    func q1(_ css: String) -> Element? { try? select(css).first() }
    var txt: String { ((try? text()) ?? "").trimmed }
    func at(_ key: String) -> String { (try? attr(key)) ?? "" }
    func abs(_ key: String) -> String { (try? absUrl(key)) ?? "" }
    var scriptText: String { data() }
}

enum Site {
    /// Thẻ phim ở lưới / trượt / bảng xếp hạng.
    static let card = ".flw-item, li.item-top, .halim-item, a.halim-trending-link, a.t10-card"

    /// Tiêu đề khối trên trang chủ.
    static let heading = ".cat-heading, .section-title"

    // MARK: - Bộ nhớ chi tiết phim (30 phim gần nhất)

    private static let cacheLock = NSLock()
    private static var detailCache: [String: FilmDetail] = [:]
    private static var cacheOrder: [String] = []

    static func cachedDetail(_ url: String) -> FilmDetail? {
        cacheLock.lock()
        defer { cacheLock.unlock() }
        return detailCache[url]
    }

    private static func cache(_ d: FilmDetail, _ url: String) {
        cacheLock.lock()
        defer { cacheLock.unlock() }
        detailCache[url] = d
        cacheOrder.removeAll { $0 == url }
        cacheOrder.append(url)
        if cacheOrder.count > 30 { detailCache[cacheOrder.removeFirst()] = nil }
    }

    static func fetch(_ url: String, headers: [String: String] = [:]) async throws -> Document {
        let r = try await Http.get(url, headers: headers)
        return try parse(r.body, r.url)
    }

    static func parse(_ html: String, _ baseUri: String) throws -> Document {
        try SwiftSoup.parse(html, baseUri)
    }

    // MARK: - Trang chủ

    static func home(_ base: String = Prefs.baseUrl) async throws -> Home {
        if ApiSite.isKnown(base) { return try await ApiSite.home(base) }
        let doc = try await fetch(base + "/")
        let moved = originOf(doc.location()).flatMap { hostOf($0) != hostOf(base) ? $0 : nil }
        var home: Home
        if let data = NextSite.data(doc) {
            home = await NextSite.home(doc, data)
        } else {
            home = parseHome(doc)
        }
        // Trang trắng của ứng dụng JS (React…): thử API kiểu mephim
        if home.sections.isEmpty && isAppShell(doc), await ApiSite.probe(base) { return try await ApiSite.home(base) }
        home.movedTo = moved
        return home
    }

    /// Trang chỉ có khung rỗng cho ứng dụng JS (vd <div id="root"></div>) — dữ liệu không nằm trong HTML.
    static func isAppShell(_ doc: Document) -> Bool {
        guard let root = doc.q1("#root, #app, #__nuxt") else { return false }
        return root.children().isEmpty() && doc.q("a[href]").count < 5
    }

    /// Thời gian tính phần tải qua mạng, không tính lúc đọc HTML.
    static func check(_ base: String) async throws -> SiteCheck {
        let t0 = Date()
        if ApiSite.isKnown(base) {
            let films = try await ApiSite.list(ApiSite.newestUrl(base)).films.count
            return SiteCheck(ms: Int(Date().timeIntervalSince(t0) * 1000), films: films)
        }
        let r = try await Http.get(base + "/")
        let ms = Int(Date().timeIntervalSince(t0) * 1000)
        let doc = try parse(r.body, r.url)
        return SiteCheck(ms: ms, films: parseHome(doc).sections.reduce(0) { $0 + $1.films.count })
    }

    /// Tập (theo định danh) các phần tử khớp bộ chọn — thay cho Element.is / closest của jsoup.
    static func idSet(_ root: Element, _ css: String) -> Set<ObjectIdentifier> { Set(root.q(css).map { ObjectIdentifier($0) }) }

    /// Phần tử cha (parent() của SwiftSoup có thể trả Node).
    static func up(_ e: Element) -> Element? { e.parent() as? Element }

    static func hasAncestor(_ el: Element, in set: Set<ObjectIdentifier>, includeSelf: Bool = false) -> Bool {
        var n: Element? = includeSelf ? el : up(el)
        while let e = n {
            if set.contains(ObjectIdentifier(e)) { return true }
            n = up(e)
        }
        return false
    }

    static func closest(_ el: Element, _ set: Set<ObjectIdentifier>) -> Element? {
        var n: Element? = el
        while let e = n {
            if set.contains(ObjectIdentifier(e)) { return e }
            n = up(e)
        }
        return nil
    }

    /// Gom thẻ phim theo tiêu đề khối đứng trước nó (theo thứ tự trong trang).
    static func parseHome(_ doc: Document) -> Home {
        if let data = NextSite.data(doc) { return NextSite.parseHome(doc, data) }
        let cards = idSet(doc, card)
        let realtime = idSet(doc, ".block_area-realtime")
        let schedule = idSet(doc, ".block_area-schedule")
        var order: [Element?] = []
        var groups: [ObjectIdentifier?: [Element]] = [:]
        var heading: Element?
        for el in doc.q("\(Self.heading), \(card)") {
            if hasAncestor(el, in: cards) { continue }
            if cards.contains(ObjectIdentifier(el)) {
                let k = heading.map { ObjectIdentifier($0) }
                if groups[k] == nil {
                    groups[k] = []
                    order.append(heading)
                }
                groups[k]!.append(el)
            } else {
                heading = el
            }
        }
        var sections: [FilmSection] = []
        for h in order {
            let films = (groups[h.map { ObjectIdentifier($0) }] ?? []).compactMap { parseCard($0) }.distinct { $0.url }
            if films.isEmpty { continue }
            let text = h?.txt ?? ""
            let title: String
            if h == nil {
                title = "Nổi bật"
            } else if text.isEmpty {
                title = hasAncestor(h!, in: realtime, includeSelf: true) ? "Xem nhiều" : "Phim"
            } else if hasAncestor(h!, in: schedule, includeSelf: true) {
                title = "\(text) hôm nay"
            } else {
                title = text
            }
            sections.append(FilmSection(title: title, films: films))
        }
        return Home(sections: sections, categories: parseCategories(doc), movedTo: nil)
    }

    private static let skipCategory = Rx("account|login|dang-nhap|lich-su|history|bookmark|theo-doi|sitemap|feed")

    private static func parseCategories(_ doc: Document) -> [Category] {
        let home = originOf(doc.location()).flatMap { hostOf($0) }
        var out: [Category] = []
        var seen = Set<String>()
        var links = doc.q("a.nav-link[href], li.menu-item > a[href], #header nav a[href], .navbar a[href], #menu a[href]")
        if links.isEmpty { links = doc.q(".block_area-genres a[href]") }
        for a in links {
            // Bỏ mục cha của menu thả xuống (href="#")
            let href = a.at("href").trimmed
            if href.hasPrefix("#") || href.lowercased().hasPrefix("javascript") { continue }
            let url = a.abs("href").trimmingSuffix("/")
            let title = a.txt.isEmpty ? a.at("title").trimmed : a.txt
            if url.isEmpty || title.isEmpty || seen.contains(url) { continue }
            guard hostOf(url) == home, let path = URL(string: url)?.path, !path.trimmingChars("/").isEmpty else { continue }
            if skipCategory.matches(url) { continue }
            seen.insert(url)
            out.append(Category(title: title, url: url))
        }
        return out
    }

    /// Thẻ phim → Film. Hỗ trợ flw-item (yanhh3d), halim-item (HaLim), item-top (bảng xếp hạng).
    static func parseCard(_ el: Element) -> Film? {
        guard let a = el.q1("a.film-poster-ahref[href], a.halim-thumb[href], .film-name a[href]") ?? el.q1("a[href]")
            ?? ((el.tagName() == "a" && el.hasAttr("href")) ? el : nil) else { return nil }
        let url = a.abs("href")
        if url.isEmpty { return nil }
        let img = el.q1("img")
        let candidates = [el.q1(".film-name a, .entry-title, .halim-trending-title-text, h4, .t10-movie-title")?.txt, a.at("title"),
                          img?.at("alt")]
        guard let title = candidates.compactMap({ $0?.nonEmpty }).first else { return nil }
        let badge = el.q1(".tick-rate, .fdi-item, .episode")?.txt.nonEmpty
            ?? el.q1(".t10-rank-num")?.txt.nonEmpty.map { "Top \($0)" }
        return Film(url: url, title: title, poster: img.flatMap { imageUrl($0) }, badge: badge)
    }

    static func parseCards(_ root: Element) -> [Film] {
        let all = root.q(card)
        let set = Set(all.map { ObjectIdentifier($0) })
        return all.filter { !hasAncestor($0, in: set) }.compactMap { parseCard($0) }.distinct { $0.url }
    }

    private static func imageUrl(_ img: Element) -> String? {
        ["data-src", "data-lazy-src", "data-original", "src"].map { img.abs($0) }.first { $0.hasPrefix("http") }
    }

    /// Trang danh sách kiểu mới của hoathinh3d.pw (11/2026): HTML chỉ có khung trống, phim nạp bằng API JSON —
    /// `/wp-json/halim/v1/{movies|ongoing|completed|top-rated}?page=N&per_page=20` (Phim Lẻ thêm sortby) và lịch chiếu
    /// `/wp-json/movie-schedule/v1/daily?day=thu-hai…`. Đọc địa chỉ API ngay trong mã JS của trang (API_URL). Bản Android:
    /// Site.HalimApi.
    enum HalimApi {
        static let path = "/wp-json/halim/v1/"
        static let perPage = 20
        private static let api = Rx(#"API_URL\s*=\s*["']([^"']*wp-json\\?/halim\\?/v1\\?/[a-z0-9-]+)["']"#)
        private static let sort = Rx(#"SORTBY\s*=\s*["']([a-z0-9_-]+)["']"#)
        private static let scheduleRx = Rx(#"API_URL\s*=\s*["']([^"']*wp-json\\?/movie-schedule\\?/v1\\?/daily)["']"#)
        private static let pageRx = Rx(#"[?&]page=(\d+)"#)

        /// Trang 1 của API trong HTML (nil = không phải trang kiểu này).
        static func firstPage(_ html: String) -> String? {
            guard let a = api.group(1, in: html)?.replacingOccurrences(of: "\\/", with: "/") else { return nil }
            let s = sort.group(1, in: html).map { "sortby=\($0)&" } ?? ""
            return "\(a)?\(s)page=1&per_page=\(perPage)"
        }

        static func scheduleApi(_ html: String) -> String? {
            scheduleRx.group(1, in: html)?.replacingOccurrences(of: "\\/", with: "/")
        }

        /// {success, has_more, items:[{title, permalink, poster_url, latest_episode?, rank?}]}
        static func parse(_ data: Data, _ url: String) -> FilmPage {
            guard let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
                  let items = o["items"] as? [[String: Any]] else { return FilmPage(films: [], nextUrl: nil) }
            let films = items.compactMap { film($0) }.distinct { $0.url }
            let page = pageRx.group(1, in: url).flatMap { Int($0) } ?? 1
            var next: String?
            if (o["has_more"] as? Bool) == true, !films.isEmpty, let r = url.range(of: #"([?&]page=)\d+"#, options: .regularExpression) {
                let prefix = url[r].hasPrefix("&") ? "&page=" : "?page="
                next = url.replacingCharacters(in: r, with: "\(prefix)\(page + 1)")
            }
            return FilmPage(films: films, nextUrl: next)
        }

        private static func film(_ o: [String: Any]) -> Film? {
            guard let url = (o["permalink"] as? String)?.nonEmpty, let title = (o["title"] as? String)?.nonEmpty else { return nil }
            let poster = (o["poster_url"] as? String)?.nonEmpty ?? (o["thumbnail"] as? String)?.nonEmpty
            var badge: String?
            if let ep = (o["latest_episode"] as? String)?.nonEmpty {
                badge = ep.allSatisfy(\.isNumber) ? "Tập \(ep)" : ep
            } else if let rank = o["rank"] as? Int, rank > 0 {
                badge = "Top \(rank)"
            }
            let early = (o["is_early_screening"] as? Bool) == true ? (o["early_screening_time"] as? String)?.nonEmpty : nil
            let b = [early, badge].compactMap { $0 }.joined(separator: " · ")
            return Film(url: url, title: title, poster: poster, badge: b.isEmpty ? nil : b)
        }

        private static let days = [("chu-nhat", "Chủ nhật"), ("thu-hai", "Thứ 2"), ("thu-ba", "Thứ 3"), ("thu-tu", "Thứ 4"),
                                   ("thu-nam", "Thứ 5"), ("thu-sau", "Thứ 6"), ("thu-bay", "Thứ 7")]

        /// Một ngày của lịch chiếu: phim chiếu sớm (kèm giờ) trước, rồi phim thường.
        static func parseDay(_ data: Data) -> [Film] {
            guard let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return [] }
            let all = ["early_movies", "regular_movies"].flatMap { (o[$0] as? [[String: Any]]) ?? [] }
            return all.compactMap { film($0) }.distinct { $0.url }
        }

        /// Cả tuần, bắt đầu từ hôm nay (giờ Việt Nam).
        static func schedule(_ api: String) async -> [FilmSection] {
            var cal = Calendar(identifier: .gregorian)
            cal.timeZone = TimeZone(identifier: "Asia/Ho_Chi_Minh") ?? .current
            let today = cal.component(.weekday, from: Date()) - 1 // Chủ nhật = 0
            var out: [FilmSection] = []
            for i in 0..<7 {
                let (slug, name) = days[(today + i) % 7]
                guard let r = try? await Http.get("\(api)?day=\(slug)") else { continue }
                let films = parseDay(r.data)
                if !films.isEmpty { out.append(FilmSection(title: name + (i == 0 ? " (hôm nay)" : ""), films: films)) }
            }
            return out
        }
    }

    // MARK: - Danh sách / tìm kiếm

    static func list(_ url: String) async throws -> FilmPage {
        if ApiSite.isKnown(url) { return try await ApiSite.list(url) }
        // hoathinh3d.pw (11/2026): Phim Lẻ / Đang Chiếu / Hoàn Thành / Đánh Giá Cao nạp phim bằng API JSON
        if url.contains(HalimApi.path) {
            return HalimApi.parse(try await Http.get(url, headers: ["Accept": "application/json"]).data, url)
        }
        let doc = try await fetch(url)
        let html = (try? doc.html()) ?? ""
        if let first = HalimApi.firstPage(html) { return try await list(first) }
        if let api = HalimApi.scheduleApi(html) { return FilmPage(films: [], nextUrl: nil, sections: await HalimApi.schedule(api)) }
        let page = parseList(doc, url)
        if page.films.isEmpty {
            let days = doc.q(".day-tabs .day-tab[data-id]")
            if !days.isEmpty { return FilmPage(films: [], nextUrl: nil, sections: await schedule(doc, days)) }
        }
        return page
    }

    static func parseList(_ doc: Document, _ url: String) -> FilmPage {
        if let data = NextSite.data(doc) { return NextSite.parseList(doc, data, url) }
        let main = doc.q("#main-content, #main-contents, main").first { !parseCards($0).isEmpty }
        // Trang lịch chiếu yanhh3d: khung chính trống, chỉ có bảng xếp hạng ở cột phụ → để list() nạp theo ngày
        let films: [Film]
        if let main = main {
            films = parseCards(main)
        } else if !doc.q(".day-tabs .day-tab[data-id]").isEmpty {
            films = []
        } else {
            films = parseCards(doc)
        }
        let next = doc.q1(".pagination a.next[href], a.next.page-numbers[href], a[rel=next][href]")?.abs("href")
        return FilmPage(films: films, nextUrl: (next?.isEmpty == false && next != url) ? next : nil)
    }

    private static let ajaxUrl = Rx(#"ajax_url\s*=\s*['"]([^'"]+)['"]"#)

    /// Trang lịch chiếu yanhh3d: mỗi ngày nạp riêng qua admin-ajax (action yan_ajax_schedule), bắt đầu từ hôm nay.
    private static func schedule(_ doc: Document, _ days: [Element]) async -> [FilmSection] {
        let scripts = doc.q("script").map { $0.scriptText }.joined(separator: "\n")
        let ajax = ajaxUrl.group(1, in: scripts) ?? resolve(doc.location(), "/wp-admin/admin-ajax.php")
        let today = max(0, days.firstIndex { $0.hasClass("active") } ?? 0)
        let ordered = Array(days[today...]) + Array(days[..<today])
        var out: [FilmSection] = []
        for (i, day) in ordered.enumerated() {
            guard let r = try? await Http.post(ajax, form: ["action": "yan_ajax_schedule", "day": day.at("data-id")],
                                               headers: ["Referer": doc.location(), "X-Requested-With": "XMLHttpRequest"]),
                  let o = try? JSONSerialization.jsonObject(with: r.data) as? [String: Any],
                  let html = o["html"] as? String,
                  let part = try? parse(html, doc.location()) else { continue }
            let films = parseCards(part)
            if !films.isEmpty { out.append(FilmSection(title: day.txt + (i == 0 ? " (hôm nay)" : ""), films: films)) }
        }
        return out
    }

    /// Trang WordPress nhận ?s=; trang Next.js tìm ở /tim-kiem (chưa biết loại trang thì thử ?s= trước).
    static func search(_ query: String, base: String = Prefs.baseUrl) async throws -> FilmPage {
        if ApiSite.isKnown(base) { return try await ApiSite.list(ApiSite.searchUrl(base, query)) }
        if !NextSite.isKnown(base) {
            let page = try await list("\(base)/?s=" + query.trimmed.formEncoded)
            if !NextSite.isKnown(base) { return page }
        }
        return try await list(NextSite.searchUrl(base, query))
    }

    /// Tìm cùng lúc trên mọi trang trong danh sách: mỗi trang một nhóm, trang đang dùng đứng đầu; lỗi / quá 20 giây thì bỏ.
    static func searchAll(_ query: String) async throws -> FilmPage {
        let sites = ([Prefs.baseUrl] + Prefs.sites).distinct { $0 }
        var results = [Result<FilmPage, Error>?](repeating: nil, count: sites.count)
        await withTaskGroup(of: (Int, Result<FilmPage, Error>).self) { group in
            for (i, base) in sites.enumerated() {
                group.addTask {
                    do { return (i, .success(try await withTimeout(20) { try await search(query, base: base) })) }
                    catch { return (i, .failure(error)) }
                }
            }
            for await (i, r) in group { results[i] = r }
        }
        var firstError: Error?
        var sections: [FilmSection] = []
        for (i, r) in results.enumerated() {
            switch r {
            case .success(let page):
                let films = (page.films + page.sections.flatMap { $0.films }).distinct { $0.url }
                if !films.isEmpty { sections.append(FilmSection(title: "\(hostOf(sites[i]) ?? sites[i]) — \(films.count) phim", films: films)) }
            case .failure(let e):
                if firstError == nil { firstError = e }
            case .none: break
            }
        }
        if let e = firstError, results.allSatisfy({ if case .failure? = $0 { return true } else { return false } }) { throw e }
        return FilmPage(films: [], nextUrl: nil, sections: sections)
    }

    // MARK: - Chi tiết phim

    static func detail(_ url: String) async throws -> FilmDetail {
        if ApiSite.isKnown(url) {
            let d = try await ApiSite.detail(url)
            cache(d, url)
            return d
        }
        let doc = try await fetch(url)
        if isAppShell(doc), await ApiSite.probe(url) { return try await detail(url) }
        let d: FilmDetail
        if let data = NextSite.data(doc) {
            d = try await NextSite.detail(doc, data, url)
        } else {
            d = parseDetail(doc, url)
        }
        cache(d, url)
        return d
    }

    static func parseDetail(_ doc: Document, _ url: String) -> FilmDetail {
        if let data = NextSite.data(doc), let d = try? NextSite.parseDetail(doc, data, url) { return d }
        let title = doc.q1(".anisc-detail .film-name, h1.film-name, h1")?.txt.nonEmpty ?? meta(doc, "og:title") ?? url
        let other = doc.q1(".film-name-real, .info-hero__alt, .movie-detail .original_title, h2.org_title")?.txt.nonEmpty
            .flatMap { $0 != title ? $0 : nil }
        let poster = doc.q1(".anisc-poster img, .info-hero__poster img, .movie-thumb, .halim-movie-wrapper .movie-poster img")
            .flatMap { imageUrl($0) } ?? meta(doc, "og:image")
        let descEl = doc.q1(".film-description .text, .film-description, .short-content, .info-hero__desc, #film-content, .film-content, .entry-content .video-item")
        let desc = descEl.flatMap { $0.txt.count > 20 ? $0.txt : nil } ?? meta(doc, "og:description")
        var infoParts: [String] = []
        if let s = doc.q1(".movie-status, .info-hero__status, .status-badge")?.txt { infoParts.append(s) }
        if let dur = doc.q(".anisc-info .item").first(where: { $0.txt.hasPrefix("Thời lượng") })?.q1(".name")?.txt { infoParts.append(dur) }
        infoParts.append(doc.q(".item-cate a, .info-hero__tags a, .category a[rel~=tag]").map { $0.txt }.joined(separator: ", "))
        let info = infoParts.filter { !$0.isEmpty }.distinct { $0 }.joined(separator: "  •  ").nonEmpty

        var groups = parseEpisodeGroups(doc)
        if groups.isEmpty, let play = doc.q1("a.btn-play[href], a.watch-btn[href], a.watch-movie[href]")?.abs("href").nonEmpty {
            groups = [EpisodeGroup(name: "Tập phim", episodes: [Episode(title: "Xem phim", url: play)])]
        }
        return FilmDetail(url: url, title: title, otherTitle: other, poster: poster, description: desc, info: info, groups: groups)
    }

    /// Danh sách tập theo nhóm, mỗi nhóm sắp tăng dần (trang web thường xếp tập mới nhất lên đầu).
    private static func parseEpisodeGroups(_ doc: Document) -> [EpisodeGroup] {
        // HaLim: mỗi <ul class="halim-list-eps"> là danh sách tập của một server (Việt Sub, Thuyết Minh…)
        let lists = doc.q("ul.halim-list-eps")
        if !lists.isEmpty {
            let tabLabels = doc.q(".hh3d-server-tab__label, .halim-server-tab .label")
            let servers = idSet(doc, ".halim-server")
            let clean: (Element?) -> String? = { $0?.txt.trimmingChars("#: ").nonEmpty }
            var groups: [EpisodeGroup] = []
            for (i, ul) in lists.enumerated() {
                let eps = episodesFrom(ul.q("a[href]"))
                if eps.isEmpty { continue }
                let name = clean(closest(ul, servers)?.q1(".halim-server-name")) ?? clean(tabLabels[safe: i]) ?? "Server \(i + 1)"
                groups.append(EpisodeGroup(name: name, episodes: eps))
            }
            return groups.distinct { $0.episodes.map { $0.url } }
        }
        var links = doc.q("a.ep-item[href]")
        if links.isEmpty { links = doc.q(".ss-list a[href], .episodes a[href]") }
        if links.isEmpty { links = doc.q("a[href*=/tap-]") }
        let eps = episodesFrom(links)
        return eps.isEmpty ? [] : [EpisodeGroup(name: "Tập phim", episodes: eps)]
    }

    private static let plainNumber = Rx(#"^\d+(\.\d+)?$"#)

    private static func episodesFrom(_ links: [Element]) -> [Episode] {
        var eps: [Episode] = []
        var seen = Set<String>()
        for a in links {
            let u = a.abs("href")
            if u.isEmpty || seen.contains(u) { continue }
            seen.insert(u)
            var title = a.txt.isEmpty ? a.at("title").trimmed : a.txt
            if plainNumber.matches(title) { title = "Tập \(title)" }
            eps.append(Episode(title: title, url: u))
        }
        let nums = eps.map { epNumber($0.title) ?? epNumber($0.url.afterLast("/")) }
        if Double(nums.compactMap { $0 }.count) >= Double(eps.count) * 0.8 {
            // Sắp theo số tập (tập không có số xuống cuối), giữ thứ tự gốc khi bằng nhau
            return eps.indices.sorted { a, b in
                switch (nums[a], nums[b]) {
                case let (x?, y?): return x != y ? x < y : a < b
                case (.some, nil): return true
                case (nil, .some): return false
                default: return a < b
                }
            }.map { eps[$0] }
        }
        return eps.reversed()
    }

    private static let epWord = Rx(#"(?i)t[aậ]p[\s\-_]*(\d+(?:\.\d+)?)"#)
    private static let anyNumber = Rx(#"(\d+(?:\.\d+)?)"#)

    /// Số tập trong tên / đường dẫn tập: "Tập 165" → 165, "tap-08" → 8 (dùng cả để ghép tập với KKPhim).
    static func epNumber(_ s: String) -> Double? {
        (epWord.group(1, in: s) ?? anyNumber.group(1, in: s)).flatMap { Double($0) }
    }

    // MARK: - Trang tập phim

    static func source(_ epUrl: String) async throws -> EpisodeSource {
        if ApiSite.isKnown(epUrl) { return try await ApiSite.source(epUrl) }
        let doc = try await fetch(epUrl)
        if isAppShell(doc), await ApiSite.probe(epUrl) { return try await ApiSite.source(epUrl) }
        return parseSource(doc, epUrl)
    }

    private static let movieInfo = Rx(#"MovieInfo\s*=\s*\{\s*id:\s*(\d+)"#)
    private static let playerPhp = Rx(#"url:\s*['"]([^'"]*player\.php)['"]"#)

    static func parseSource(_ doc: Document, _ epUrl: String) -> EpisodeSource {
        if let data = NextSite.data(doc), let s = try? NextSite.parseSource(doc, data, epUrl) { return s }
        let scripts = doc.q("script").map { $0.scriptText }.joined(separator: "\n")
        let servers = doc.q("#list_sv [data-type], .get-eps[data-type], .btn3dsv[data-type]")
            .map { Server(label: $0.txt, type: $0.at("data-type")) }
            .filter { !$0.type.isEmpty }.distinct { $0.type }
            .map { $0.label.isEmpty ? Server(label: $0.type, type: $0.type) : $0 }

        let active = doc.q1("a.ep-item.active[data-post-id], .halim-episode.active a[data-post-id], .active > a[data-post-id]")
            ?? doc.q1("[data-post-id][data-ep]")
        let postId = active?.at("data-post-id").nonEmpty ?? movieInfo.group(1, in: scripts) ?? ""
        var tail = epUrl.trimmingSuffix("/").afterLast("/")
        if tail.hasSuffix(".html") { tail = String(tail.dropLast(5)) }
        let ep = active?.at("data-ep").nonEmpty ?? tail
        let sv = active?.at("data-sv").nonEmpty ?? "1"

        // player.php kiểu "dox_ajax_player" (yanhh3d và các trang cùng nhóm)
        var playerUrl: String?
        if scripts.contains("dox_ajax_player") {
            playerUrl = playerPhp.group(1, in: scripts).map { resolve(epUrl, $0) } ?? resolve(epUrl, "/player/player.php")
        }
        // Bỏ qua iframe giữ chỗ src="" (absUrl của chuỗi rỗng ra chính địa chỉ trang)
        let direct = doc.q("#video-player iframe, .player-frame iframe, #halim-player-wrapper iframe")
            .first { let s = $0.at("src"); return s.hasPrefix("http") || s.hasPrefix("//") }?.abs("src").nonEmpty
        let pageMode = (playerUrl == nil || servers.isEmpty || postId.isEmpty) && direct == nil
        return EpisodeSource(epUrl: epUrl, servers: servers, postId: postId, ep: ep, sv: sv, playerUrl: playerUrl,
                             directEmbed: direct, pageMode: pageMode)
    }

    /// Hỏi player.php địa chỉ trình phát nhúng (iframe) của một server (trang Next.js: link .m3u8 / iframe).
    static func video(_ src: EpisodeSource, _ server: Server?) async throws -> Video {
        // Server dự phòng KKPhim / NguonC: link có sẵn (KkPhim.attach)
        if let s = server, KkPhim.isBackup(s.type), let v = src.links[s.type] { return v }
        if src.next {
            guard let server = server else { throw AppError("Không có server") }
            return try await NextSite.video(src, server)
        }
        guard let server = server, let playerUrl = src.playerUrl else {
            guard let direct = src.directEmbed else { throw AppError("Không có server") }
            return Video(url: direct)
        }
        let url = playerUrl + "?action=dox_ajax_player&post_id=" + src.postId.formEncoded + "&chapter_st=" + src.ep.formEncoded +
            "&type=" + server.type.formEncoded + "&sv=" + src.sv.formEncoded
        let r = try await Http.get(url, headers: ["Referer": src.epUrl, "X-Requested-With": "XMLHttpRequest"])
        let doc = try parse(r.body, src.epUrl)
        if let f = doc.q1("iframe[src]")?.abs("src"), f.hasPrefix("http") { return Video(url: f) }
        throw AppError("Server \"\(server.label)\" chưa có video cho tập này")
    }

    static func meta(_ doc: Document, _ property: String) -> String? {
        doc.q1("meta[property=\"\(property)\"], meta[name=\"\(property)\"]")?.at("content").nonEmpty
    }

    static func resolve(_ base: String, _ ref: String) -> String {
        URL(string: ref, relativeTo: URL(string: base))?.absoluteString ?? ref
    }

    /// Tên trang hiện trong app: tên miền, riêng add-on k-20 ghi «CLB Phim Xưa (sc.k-20.xyz)».
    static func displayName(_ host: String) -> String { host == Stremio.host ? "\(Stremio.name) (\(host))" : host }

    /// Tên ngắn ghi sau tên phim của trang khác: "yanhh3d", "nguonc" (bỏ nhãn chung "phim." / "www."), "CLB Phim Xưa".
    static func shortName(_ host: String) -> String {
        if host == Stremio.host { return Stremio.name }
        let p = host.split(separator: ".").map(String.init)
        return p.count > 2 && ["phim", "www", "m"].contains(p[0]) ? p[1] : (p.first ?? host)
    }

    /// Tên ngắn của trang trong danh sách; hai trang cùng tên ngắn (hoathinh3d.ee / hoathinh3d.pw) thì ghi đủ tên miền.
    static func tag(_ host: String, _ sites: [String]) -> String {
        let short = shortName(host)
        let clash = sites.contains { s in
            guard let o = hostOf(s) else { return false }
            return o != host && shortName(o) == short
        }
        return clash ? host : short
    }

    /// Menu cố định của trang API (không cần tải trang chủ); trang web thì nil (menu đọc từ trang chủ).
    static func menu(_ base: String) -> [Category]? { ApiSite.isKnown(base) ? ApiSite.menuOf(base) : nil }

    static func hostOf(_ url: String) -> String? {
        guard let h = URL(string: url)?.host?.lowercased() else { return nil }
        return h.hasPrefix("www.") ? String(h.dropFirst(4)) : h
    }

    static func originOf(_ url: String) -> String? {
        guard let u = URL(string: url), let scheme = u.scheme, let host = u.host else { return nil }
        return "\(scheme)://\(host)" + (u.port.map { ":\($0)" } ?? "")
    }
}
