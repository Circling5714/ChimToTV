import Foundation
import SwiftSoup

/// NguonC (phim.nguonc.com, từ 1.7) — bản Android: NguonC.kt (giữ cùng tên hàm để sửa song song). API công khai
/// /api/films/… (định dạng riêng, không phải KKPhim). Dùng hai cách:
/// - Trang riêng: trang chủ / danh sách / tìm kiếm / chi tiết / tập đọc từ API. Địa chỉ phim = /phim/<slug>, tập =
///   /phim/<slug>/<tập>?sv=<thứ tự server>; danh sách = địa chỉ API.
/// - Server dự phòng thứ hai (cạnh KKPhim) cho yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw.
/// Video chỉ có trình phát nhúng (embedN.streamc.xyz) → phát qua iframe, agent tự bỏ quảng cáo của JW.
enum NguonC {
    static let host = "phim.nguonc.com"
    static let api = "https://phim.nguonc.com/api"

    /// Đầu tên loại server dự phòng NguonC, vd "nc-vs", "nc-tm".
    static let prefix = "nc-"

    private static let ttl: Int64 = 10 * 60_000
    private static let missTtl: Int64 = 30 * 60_000

    private static let menu: [(String, String)] = [
        ("Hoạt hình", "films/danh-sach/hoat-hinh"),
        ("Phim bộ", "films/danh-sach/phim-bo"),
        ("Phim lẻ", "films/danh-sach/phim-le"),
        ("TV Shows", "films/danh-sach/tv-shows"),
        ("Phim thuyết minh", "films/ngon-ngu/thuyet-minh"),
        ("Phim lồng tiếng", "films/ngon-ngu/long-tieng"),
        ("Phim Trung Quốc", "films/quoc-gia/trung-quoc"),
        ("Phim Hàn Quốc", "films/quoc-gia/han-quoc"),
        ("Phim Mỹ", "films/quoc-gia/my"),
        ("Phim Nhật Bản", "films/quoc-gia/nhat-ban"),
        ("Phim Thái Lan", "films/quoc-gia/thai-lan"),
    ]
    private static let genres: [(String, String)] = [
        ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Hoạt Hình", "hoat-hinh"), ("Giả Tưởng", "gia-tuong"),
        ("Khoa Học Viễn Tưởng", "khoa-hoc-vien-tuong"), ("Kinh Dị", "kinh-di"), ("Tình Cảm", "tinh-cam"), ("Lãng Mạn", "lang-man"),
        ("Hài", "phim-hai"), ("Tâm Lý", "tam-ly"), ("Phiêu Lưu", "phieu-luu"), ("Hình Sự", "hinh-su"), ("Gây Cấn", "gay-can"),
        ("Bí Ẩn", "bi-an"), ("Chiến Tranh", "chien-tranh"), ("Chính Kịch", "chinh-kich"), ("Gia Đình", "gia-dinh"),
        ("Lịch Sử", "lich-su"), ("Tài Liệu", "tai-lieu"), ("Nhạc", "phim-nhac"), ("Miền Tây", "mien-tay"), ("Phim 18+", "phim-18"),
    ]
    private static let countries: [(String, String)] = [
        ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Mỹ", "my"), ("Anh", "anh"),
        ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
        ("Việt Nam", "viet-nam"), ("Pháp", "phap"), ("Đức", "duc"), ("Tây Ban Nha", "tay-ban-nha"), ("Nga", "nga"),
        ("Canada", "canada"), ("Úc", "uc"), ("Italy", "italy"), ("Philippines", "philippines"), ("Brazil", "brazil"), ("Bỉ", "bi"),
    ]

    static func isSite(_ url: String) -> Bool { Site.hostOf(url) == host }

    // MARK: - Trang riêng

    static func home(_ base: String) async throws -> Home {
        let o = ApiSite.origin(base)
        let blocks: [(String, String)] = [
            ("Hoạt hình mới cập nhật", apiUrl(o, "films/danh-sach/hoat-hinh")),
            ("Phim mới cập nhật", newestUrl(o)),
            ("Phim bộ mới", apiUrl(o, "films/danh-sach/phim-bo")),
            ("Phim lẻ mới", apiUrl(o, "films/danh-sach/phim-le")),
            ("Phim thuyết minh", apiUrl(o, "films/ngon-ngu/thuyet-minh")),
        ]
        let sections = try await ApiSite.sections(blocks) { parseList(try await getJSON($0), $0).films }
        return Home(sections: sections, categories: menuOf(base), movedTo: nil)
    }

    static func menuOf(_ base: String) -> [Category] {
        let o = ApiSite.origin(base)
        return menu.map { Category(title: $0.0, url: apiUrl(o, $0.1)) }
    }

    static func newestUrl(_ base: String) -> String { apiUrl(ApiSite.origin(base), "films/phim-moi-cap-nhat") }

    static func searchUrl(_ base: String, _ query: String) -> String {
        "\(ApiSite.origin(base))/api/films/search?keyword=" + query.trimmed.formEncoded + "&page=1"
    }

    static func filters(_ base: String) -> (genres: [Category], countries: [Category]) {
        let o = ApiSite.origin(base)
        return (genres.map { Category(title: $0.0, url: apiUrl(o, "films/the-loai/\($0.1)")) },
                countries.map { Category(title: $0.0, url: apiUrl(o, "films/quoc-gia/\($0.1)")) })
    }

    private static func apiUrl(_ origin: String, _ path: String) -> String { "\(origin)/api/\(path)?page=1" }

    static func list(_ url: String) async throws -> FilmPage { parseList(try await getJSON(url), url) }

    /// {status, paginate:{current_page, total_page}, items:[{name, slug, thumb_url (dọc), poster_url (ngang), current_episode}]}.
    static func parseList(_ root: [String: Any], _ url: String) -> FilmPage {
        let o = ApiSite.origin(url)
        let films = J.objects(J.arr(root, "items")).compactMap { m -> Film? in
            guard let slug = J.str(m, "slug").nonEmpty, let name = J.str(m, "name").nonEmpty else { return nil }
            let poster = (J.str(m, "thumb_url").nonEmpty ?? J.str(m, "poster_url")).hasPrefix("http")
                ? (J.str(m, "thumb_url").nonEmpty ?? J.str(m, "poster_url")) : nil
            return Film(url: "\(o)/phim/\(slug)", title: name, poster: poster, badge: NextSite.badge(J.str(m, "current_episode")))
        }.distinct { $0.url }
        let p = J.obj(root, "paginate")
        let page = J.int(p, "current_page"), pages = J.int(p, "total_page")
        let next = (!films.isEmpty && page >= 1 && page < pages) ? ApiSite.withPage(url, page + 1) : nil
        return FilmPage(films: films, nextUrl: next)
    }

    static func detail(_ url: String) async throws -> FilmDetail {
        let film = try filmUrl(url)
        return try parseDetail(try await getJSON(detailApi(film)), film)
    }

    static func parseDetail(_ root: [String: Any], _ url: String) throws -> FilmDetail {
        guard let m = J.obj(root, "movie") else { throw AppError("Không đọc được thông tin phim") }
        let desc = J.str(m, "description").nonEmpty.flatMap { (try? SwiftSoup.parse($0).text())?.nonEmpty }
        let cats = categories(m)
        var year = ""
        if let y = m["year"] { year = "\(y)" }
        if year == "0" || year == "<null>" { year = "" }
        let info = [J.str(m, "current_episode"), J.str(m, "time"), J.str(m, "language"), cats["Quốc gia"] ?? "", year,
                    cats["Thể loại"] ?? ""].filter { !$0.isEmpty }.joined(separator: "  •  ").nonEmpty
        var groups: [EpisodeGroup] = []
        for (i, s) in J.objects(J.arr(m, "episodes")).enumerated() {
            let eps = J.objects(J.arr(s, "items")).compactMap { e -> Episode? in
                guard let slug = J.str(e, "slug").nonEmpty else { return nil }
                if !J.str(e, "embed").hasPrefix("http") && !J.str(e, "m3u8").hasPrefix("http") { return nil }
                return Episode(title: ApiSite.title(J.str(e, "name").nonEmpty ?? slug), url: "\(url)/\(slug)?sv=\(i)")
            }.distinct { $0.url }
            if !eps.isEmpty { groups.append(EpisodeGroup(name: J.str(s, "server_name").nonEmpty ?? "Server \(i + 1)", episodes: eps)) }
        }
        let p = J.str(m, "thumb_url").nonEmpty ?? J.str(m, "poster_url")
        return FilmDetail(url: url, title: J.str(m, "name").nonEmpty ?? url, otherTitle: J.str(m, "original_name").nonEmpty,
                          poster: p.hasPrefix("http") ? p : nil, description: desc, info: info, groups: groups)
    }

    /// category: {"1": {group:{name:"Định dạng"}, list:[{name}]}, …} → tên nhóm → "a, b".
    private static func categories(_ m: [String: Any]) -> [String: String] {
        var out: [String: String] = [:]
        for (_, v) in J.obj(m, "category") ?? [:] {
            guard let g = v as? [String: Any], let name = J.str(J.obj(g, "group"), "name").nonEmpty else { continue }
            out[name] = J.objects(J.arr(g, "list")).map { J.str($0, "name") }.filter { !$0.isEmpty }.joined(separator: ", ")
        }
        return out
    }

    static func source(_ epUrl: String) async throws -> EpisodeSource {
        let root = try await getJSON(detailApi(try filmUrl(epUrl)))
        guard let movie = J.obj(root, "movie") else { throw AppError("Không đọc được thông tin phim") }
        let sv = Int(URLComponents(string: epUrl)?.queryItems?.first { $0.name == "sv" }?.value ?? "") ?? 0
        let tap = (URL(string: epUrl)?.path ?? "").trimmingSuffix("/").afterLast("/")
        let servers = J.objects(J.arr(movie, "episodes"))
        let order = (sv < servers.count ? [servers[sv]] : []) + servers
        guard let e = order.lazy.compactMap({ s in J.objects(J.arr(s, "items")).first { J.str($0, "slug") == tap } }).first
        else { throw AppError("Không tìm thấy tập này trên trang") }
        var links: [String: Video] = [:]
        var list: [Server] = []
        if J.str(e, "m3u8").hasPrefix("http") {
            links["m3u8"] = Video(url: J.str(e, "m3u8"))
            list.append(Server(label: "HLS", type: "m3u8"))
        }
        if J.str(e, "embed").hasPrefix("http") {
            links["embed"] = Video(url: J.str(e, "embed"))
            list.append(Server(label: "Trình phát NguonC", type: "embed"))
        }
        if links.isEmpty { throw AppError("Tập này chưa có video") }
        return EpisodeSource(epUrl: epUrl, servers: list, postId: "", ep: tap, sv: String(sv), playerUrl: nil, directEmbed: nil,
                             pageMode: false, links: links, next: true)
    }

    /// ".../phim/muc-than-ky/tap-1?sv=0" → ".../phim/muc-than-ky".
    static func filmUrl(_ url: String) throws -> String {
        let parts = (URL(string: url)?.path ?? "").split(separator: "/").map(String.init)
        guard let i = parts.firstIndex(of: "phim"), i + 1 < parts.count else { throw AppError("Địa chỉ phim lạ: \(url)") }
        return "\(ApiSite.origin(url))/phim/\(parts[i + 1])"
    }

    private static func detailApi(_ film: String) -> String { "\(ApiSite.origin(film))/api/film/" + film.afterLast("/") }

    // MARK: - Server dự phòng

    private static let lock = NSLock()
    private static var cache: [String: (at: Int64, movie: [String: Any]?)] = [:]
    private static var slugs: [String: String] = [:]

    /// Trang dùng server dự phòng NguonC: bật trong Cài đặt, không phải trang đã có NguonC (NguonC, phimmoi) / trang API.
    static func appliesTo(_ url: String) -> Bool { Prefs.ncBackup && !ApiSite.isKnown(url) && !NextSite.isKnown(url) }

    /// Server NguonC cho một tập (rỗng nếu NguonC không có phim / tập này). Lỗi mạng → ném lỗi.
    static func servers(_ d: FilmDetail, _ group: EpisodeGroup?, _ ep: Episode) async throws -> [(Server, Video)] {
        guard let num = Site.epNumber(ep.title) ?? Site.epNumber(ep.url.trimmingSuffix("/").afterLast("/")),
              let movie = try await movieFor(d) else { return [] }
        return pick(movie, num, group.flatMap { PlayStats.lang($0.name) })
    }

    /// Server có tập số num trong phim NguonC ({name, episodes:[{server_name, items:[{name, slug, embed}]}]}).
    static func pick(_ movie: [String: Any], _ num: Double, _ lang: String?) -> [(Server, Video)] {
        var out: [(Server, Video)] = []
        var used = Set<String>()
        for s in J.objects(J.arr(movie, "episodes")) {
            let l = PlayStats.lang(J.str(s, "server_name")) ?? "vs"
            if let lang = lang, l != lang { continue }
            guard let e = J.objects(J.arr(s, "items"))
                .first(where: { (Site.epNumber(J.str($0, "name")) ?? Site.epNumber(J.str($0, "slug"))) == num }) else { continue }
            let embed = J.str(e, "embed")
            guard embed.hasPrefix("http") else { continue }
            var type = prefix + l
            var n = 1
            while !used.insert(type).inserted {
                n += 1
                type = prefix + l + "-\(n)"
            }
            let name = l == "tm" ? "Thuyết Minh" : (l == "lt" ? "Lồng Tiếng" : "Vietsub")
            out.append((Server(label: "NguonC \(name)" + (n > 1 ? " \(n)" : ""), type: type), Video(url: embed)))
        }
        return out
    }

    private static func movieFor(_ d: FilmDetail) async throws -> [String: Any]? {
        let now = nowMs()
        lock.lock()
        let hit = cache[d.url]
        lock.unlock()
        if let h = hit, now - h.at < (h.movie == nil ? missTtl : ttl) { return h.movie }
        let found = try await find(d)
        lock.lock()
        cache[d.url] = (now, found)
        if cache.count > 40, let oldest = cache.min(by: { $0.value.at < $1.value.at })?.key { cache[oldest] = nil }
        lock.unlock()
        return found
    }

    /// Phim NguonC khớp tên với phim của trang (slug của trang trước, rồi tìm theo tên); nil = NguonC không có.
    private static func find(_ d: FilmDetail) async throws -> [String: Any]? {
        let mine = Set([d.title, d.otherTitle].compactMap { $0 }.map(KkPhim.norm).filter { !$0.isEmpty })
        if mine.isEmpty { return nil }
        lock.lock()
        let known = slugs[d.url]
        lock.unlock()
        if let s = known, let m = try await movie(s) { return m }
        if let own = KkPhim.slugOf(d.url), let m = try await movie(own), KkPhim.matches(m, mine) {
            remember(d.url, own)
            return m
        }
        let q = "\(api)/films/search?keyword=" + String(d.title.prefix(80)).formEncoded
        guard let hit = J.objects(J.arr(try await getJSON(q), "items")).first(where: { KkPhim.matches($0, mine) }),
              let slug = J.str(hit, "slug").nonEmpty else { return nil }
        let m = try await movie(slug)
        if m != nil { remember(d.url, slug) }
        return m
    }

    private static func remember(_ film: String, _ slug: String) {
        lock.lock()
        slugs[film] = slug
        lock.unlock()
    }

    /// Phim trên NguonC (movie); nil nếu không có slug này (404).
    private static func movie(_ slug: String) async throws -> [String: Any]? {
        do {
            return J.obj(try await getJSON("\(api)/film/" + slug.formEncoded), "movie")
        } catch let e as Http.HttpError where e.code == 404 {
            return nil
        }
    }

    private static func getJSON(_ url: String) async throws -> [String: Any] {
        let r = try await Http.get(url, headers: ["Accept": "application/json"])
        guard let o = try JSONSerialization.jsonObject(with: r.data) as? [String: Any] else { throw AppError("NguonC trả dữ liệu lạ") }
        return o
    }
}
