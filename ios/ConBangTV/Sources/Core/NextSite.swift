import Foundation
import SwiftSoup

/// Dữ liệu Next.js của một trang (các chuỗi self.__next_f.push nối lại), kèm bản UTF-16 để bóc JSON tại chỗ.
final class NextData {
    let text: String
    private let ns: NSString
    private let units: [UInt16]

    init(_ text: String) {
        self.text = text
        ns = text as NSString
        units = Array(text.utf16)
    }

    var length: Int { ns.length }

    func indexOf(_ needle: String, from: Int = 0) -> Int? {
        guard from >= 0, from < ns.length else { return nil }
        let r = ns.range(of: needle, options: .literal, range: NSRange(location: from, length: ns.length - from))
        return r.location == NSNotFound ? nil : r.location
    }

    func value(at i: Int) -> Any? { JSONScan.value(units, at: i) }
}

/// Trang phim dựng bằng Next.js, giao diện kiểu "PhimMoi" (vd phimmoir.fun) — bản Android: NextSite.kt.
/// Không đọc HTML: mọi dữ liệu (phim, đủ danh sách tập, server, link video) nằm sẵn trong các đoạn push của trang.
enum NextSite {
    private static let push = "self.__next_f.push([1,"
    private static let episodeKey = "\"episode\":{\"episodes\""

    /// Khối phim trang chủ: ["$","$L26","Trung Quốc Hot Nhất",{"movies":[…]}]
    private static let sectionRx = Rx(#"\["\$","\$L\w+","([^"]+)",\{"movies":\["#)
    private static let movieRx = Rx(#"\{"id":\d+,"name":""#)
    private static let pagingRx = Rx(#""total":(\d+),"page":(\d+),"limit":(\d+)"#)

    /// Menu của trang chỉ có 10 thể loại → thêm mục cố định (đường dẫn bộ lọc của chính trang, mới cập nhật lên đầu).
    static let menu: [(String, String)] = [
        ("Hoạt hình Trung Quốc", "/loc-phim?genre=hoat-hinh&country=trung-quoc&sort=updated_at&order=desc"),
        ("Hoạt hình", "/loc-phim?genre=hoat-hinh&sort=updated_at&order=desc"),
        ("Phim bộ mới", "/loc-phim?type=phim-bo&sort=updated_at&order=desc"),
        ("Phim lẻ mới", "/loc-phim?type=phim-le&sort=updated_at&order=desc"),
        ("Phim Hàn Quốc", "/loc-phim?country=han-quoc&sort=updated_at&order=desc"),
        ("Phim Trung Quốc", "/loc-phim?country=trung-quoc&sort=updated_at&order=desc"),
        ("Phim Âu Mỹ", "/loc-phim?country=au-my&sort=updated_at&order=desc"),
        ("Phim Nhật Bản", "/loc-phim?country=nhat-ban&sort=updated_at&order=desc"),
        ("Phim Thái Lan", "/loc-phim?country=thai-lan&sort=updated_at&order=desc"),
    ]

    /// Trang đã gặp là Next.js (để tìm kiếm đi thẳng /tim-kiem thay cho ?s= của WordPress).
    private static let hostsLock = NSLock()
    private static var hosts = Set<String>()

    static func isKnown(_ base: String) -> Bool {
        hostsLock.lock()
        defer { hostsLock.unlock() }
        return Site.hostOf(base).map { hosts.contains($0) } ?? false
    }

    /// Dữ liệu Next.js của trang; nil nếu không phải trang Next.js.
    static func data(_ doc: Document) -> NextData? {
        var sb = ""
        for s in doc.q("script") {
            let js = s.scriptText
            let r = (js as NSString).range(of: push, options: .literal)
            if r.location == NSNotFound { continue }
            if let str = JSONScan.value(js, at: r.location + r.length) as? String { sb += str }
        }
        if sb.isEmpty { return nil }
        if let h = Site.hostOf(doc.location()) {
            hostsLock.lock()
            hosts.insert(h)
            hostsLock.unlock()
        }
        return NextData(sb)
    }

    // MARK: - Trang chủ / danh sách

    /// Trang chủ, thêm khối hoạt hình Trung Quốc mới cập nhật lên đầu.
    static func home(_ doc: Document, _ data: NextData) async -> Home {
        var h = parseHome(doc, data)
        let hh3d = (try? await list(origin(doc) + menu[0].1).films) ?? []
        if !hh3d.isEmpty { h.sections.insert(FilmSection(title: "Hoạt hình Trung Quốc mới cập nhật", films: hh3d), at: 0) }
        return h
    }

    static func parseHome(_ doc: Document, _ data: NextData) -> Home {
        let origin = origin(doc)
        var sections: [FilmSection] = []
        let matches = sectionRx.all(in: data.text)
        let first = matches.first?.range.location ?? data.length
        let top = movies(data, origin, from: 0, to: first)
        if !top.isEmpty { sections.append(FilmSection(title: "Nổi bật", films: top)) }
        for m in matches {
            let arrayAt = m.range.location + m.range.length - 1
            guard let arr = data.value(at: arrayAt) as? [Any] else { continue }
            let films = J.objects(arr).compactMap { film($0, origin) }.distinct { $0.url }
            if !films.isEmpty { sections.append(FilmSection(title: (data.text as NSString).substring(with: m.range(at: 1)), films: films)) }
        }
        return Home(sections: sections, categories: categories(doc, origin), movedTo: nil)
    }

    private static func categories(_ doc: Document, _ origin: String) -> [Category] {
        var out: [Category] = []
        var seen = Set<String>()
        for (title, path) in menu where seen.insert(origin + path).inserted {
            out.append(Category(title: title, url: origin + path))
        }
        for a in doc.q("a[href^=\"/the-loai/\"]") {
            let url = a.abs("href").trimmingSuffix("/")
            let title = a.txt
            if !title.isEmpty, seen.insert(url).inserted { out.append(Category(title: title, url: url)) }
        }
        return out
    }

    /// Bộ lọc của trang (/loc-phim): đủ thể loại và quốc gia. Bỏ mục rác (năm, mã hai chữ) và tên trùng.
    static func filters(_ base: String) async throws -> (genres: [Category], countries: [Category]) {
        let r = try await Http.get(base + "/loc-phim")
        let doc = try Site.parse(r.body, r.url)
        guard let d = data(doc) else { throw AppError("Trang không còn dạng Next.js") }
        return parseFilters(doc, d)
    }

    static func parseFilters(_ doc: Document, _ d: NextData) -> (genres: [Category], countries: [Category]) {
        let origin = origin(doc)
        func read(_ key: String, _ param: String) -> [Category] {
            // Phim trong trang cũng có "genres":[…] của riêng nó → lấy danh sách dài nhất (bộ lọc của trang)
            let tag = "\"\(key)\":[{"
            var best: [Any] = []
            var i = d.indexOf(tag)
            while let at = i {
                if let a = d.value(at: at + key.count + 3) as? [Any], a.count > best.count { best = a }
                i = d.indexOf(tag, from: at + tag.count)
            }
            return J.objects(best).filter { J.bool($0, "is_active", true) }
                .map { (J.str($0, "name"), J.str($0, "slug")) }
                .filter { name, slug in
                    !slug.isEmpty && name.contains { $0.isLetter } && !(name.count <= 2 && name.allSatisfy { $0.isUppercase })
                }
                .distinct { $0.0.lowercased() }
                .map { name, slug in
                    Category(title: name, url: "\(origin)/loc-phim?\(param)=\(slug.formEncoded)&sort=updated_at&order=desc")
                }
        }
        return (read("genres", "genre"), read("countries", "country"))
    }

    private static func list(_ url: String) async throws -> FilmPage {
        let r = try await Http.get(url)
        let doc = try Site.parse(r.body, r.url)
        guard let d = data(doc) else { throw AppError("Trang không còn dạng Next.js") }
        return parseList(doc, d, url)
    }

    /// Danh sách / tìm kiếm: 21 phim một trang, trang sau = tham số page tiếp theo (dữ liệu có total/page/limit).
    static func parseList(_ doc: Document, _ data: NextData, _ url: String) -> FilmPage {
        let films = movies(data, origin(doc))
        var next: String?
        if let g = pagingRx.groups(in: data.text), let total = Int(g[1]), let page = Int(g[2]), let limit = Int(g[3]),
           !films.isEmpty, page * limit < total {
            next = withPage(url, page + 1)
        }
        return FilmPage(films: films, nextUrl: next)
    }

    static func searchUrl(_ base: String, _ query: String) -> String { "\(base)/tim-kiem?keyword=" + query.trimmed.formEncoded }

    private static let pageParam = Rx(#"([?&]page=)\d+"#)

    private static func withPage(_ url: String, _ page: Int) -> String {
        if pageParam.matches(url) { return pageParam.replace(in: url, with: "$1\(page)") }
        return url + (url.contains("?") ? "&" : "?") + "page=\(page)"
    }

    // MARK: - Chi tiết phim

    /// Chi tiết + danh sách tập của các ngôn ngữ khác (tải song song).
    static func detail(_ doc: Document, _ data: NextData, _ url: String) async throws -> FilmDetail {
        var d = try parseDetail(doc, data, url)
        guard let ep = episodeData(data) else { return d }
        let origin = origin(doc)
        guard let slug = movie(data, url).map({ J.str($0, "slug") }), !slug.isEmpty else { return d }
        // Trang chỉ trả danh sách tập của một ngôn ngữ khi tập được hỏi có ở ngôn ngữ đó → hỏi tập đầu, rồi tập cuối
        let mainEps = d.groups.first?.episodes ?? []
        let found0 = [mainEps.first, mainEps.last].compactMap { $0 }.map { epSlug($0.url) }.filter { !$0.isEmpty }.distinct { $0 }
        let probes = found0.isEmpty ? ["tap-1"] : found0
        let extra = languages(ep).filter { lang in !d.groups.contains { $0.name == lang.name } }
        var found = [EpisodeGroup?](repeating: nil, count: extra.count)
        await withTaskGroup(of: (Int, EpisodeGroup?).self) { group in
            for (i, lang) in extra.enumerated() {
                guard let server = bestServer(ep, lang.slug) else { continue }
                group.addTask {
                    for probe in probes {
                        guard let r = try? await Http.get("\(origin)/xem-phim/\(slug)/\(probe)/\(lang.slug)?server=\(server)"),
                              let doc = try? Site.parse(r.body, r.url),
                              let dd = NextSite.data(doc), let e = episodeData(dd),
                              J.str(e, "current_language") == lang.slug else { continue }
                        let eps = episodes(e, origin, slug, lang.slug)
                        if !eps.isEmpty { return (i, EpisodeGroup(name: lang.name, episodes: eps)) }
                    }
                    return (i, nil)
                }
            }
            for await (i, g) in group { found[i] = g }
        }
        d.groups += found.compactMap { $0 }
        return d
    }

    static func parseDetail(_ doc: Document, _ data: NextData, _ url: String) throws -> FilmDetail {
        let origin = origin(doc)
        guard let m = movie(data, url) else { throw AppError("Không đọc được thông tin phim") }
        let slug = J.str(m, "slug")
        let rawDesc = J.str(m, "description")
        let desc = (rawDesc.count > 20 && !rawDesc.hasPrefix("$")) ? rawDesc
            : doc.q1("meta[property=\"og:description\"], meta[name=\"description\"]")?.at("content").nonEmpty
        var year = ""
        if let y = m["year"] as? Int { year = String(y) } else if let y = m["year"] as? Double { year = String(Int(y)) }
        let info = [
            J.str(m, "status_text").nonEmpty ?? J.str(m, "display_status"),
            J.str(J.obj(m, "country"), "name"),
            year,
            names(J.arr(m, "genres")),
        ].filter { !$0.isEmpty }.joined(separator: "  •  ").nonEmpty

        var groups: [EpisodeGroup] = []
        if let ep = episodeData(data) {
            let lang = J.str(ep, "current_language")
            let eps = episodes(ep, origin, slug, lang)
            if !eps.isEmpty {
                groups.append(EpisodeGroup(name: languages(ep).first { $0.slug == lang }?.name ?? "Tập phim", episodes: eps))
            }
        }
        let poster = (J.str(m, "poster_url").nonEmpty ?? J.str(m, "thumb_url").nonEmpty).flatMap { abs(origin, $0) }
        return FilmDetail(url: url, title: J.str(m, "name").nonEmpty ?? url, otherTitle: J.str(m, "origin_name").nonEmpty,
                          poster: poster, description: desc, info: info, groups: groups)
    }

    /// Đối tượng phim của trang (trang có thể nhắc tới phim khác → ưu tiên phim trùng slug trong địa chỉ).
    private static func movie(_ data: NextData, _ url: String) -> [String: Any]? {
        let slug = url.trimmingSuffix("/").afterLast("/")
        var first: [String: Any]?
        let tag = "\"movie\":{"
        var i = data.indexOf(tag)
        while let at = i {
            if let o = data.value(at: at + "\"movie\":".utf16.count) as? [String: Any] {
                if J.str(o, "slug") == slug { return o }
                if first == nil { first = o }
            }
            i = data.indexOf(tag, from: at + 1)
        }
        return first
    }

    private static func episodeData(_ data: NextData) -> [String: Any]? {
        guard let i = data.indexOf(episodeKey) else { return nil }
        return data.value(at: i + "\"episode\":".utf16.count) as? [String: Any]
    }

    /// Ngôn ngữ có tập (theo thứ tự trang đưa): slug → tên hiển thị, vd "thuyetminh" → "Thuyết Minh".
    private static func languages(_ ep: [String: Any]) -> [(slug: String, name: String)] {
        var out: [(slug: String, name: String)] = []
        for s in J.objects(J.arr(ep, "available_sources")) {
            let slug = J.str(s, "language_slug")
            if !slug.isEmpty, !out.contains(where: { $0.slug == slug }) {
                out.append((slug, J.str(s, "language_name").nonEmpty ?? slug))
            }
        }
        return out
    }

    /// Server có nhiều tập nhất của một ngôn ngữ (lấy danh sách tập đầy đủ nhất).
    private static func bestServer(_ ep: [String: Any], _ lang: String) -> String? {
        J.objects(J.arr(ep, "available_sources")).filter { J.str($0, "language_slug") == lang }
            .max { J.int($0, "total_episodes") < J.int($1, "total_episodes") }
            .flatMap { J.str($0, "server_slug").nonEmpty }
    }

    private static func episodes(_ ep: [String: Any], _ origin: String, _ slug: String, _ lang: String) -> [Episode] {
        J.objects(J.arr(ep, "episodes")).compactMap { e -> Episode? in
            guard let s = J.str(e, "slug").nonEmpty else { return nil }
            let l = J.str(e, "language").nonEmpty ?? lang
            let title = J.str(e, "episode_label").nonEmpty ?? J.str(e, "name").nonEmpty ?? s
            return Episode(title: title, url: "\(origin)/xem-phim/\(slug)/\(s)/\(l)")
        }.distinct { $0.url }
    }

    // MARK: - Trang tập phim

    /// Server của tập theo ngôn ngữ đang xem; link video của server trang mở sẵn đọc luôn (khỏi tải lại).
    static func parseSource(_ doc: Document, _ data: NextData, _ epUrl: String) throws -> EpisodeSource {
        guard let ep = episodeData(data) else { throw AppError("Không đọc được trang tập phim") }
        let lang = J.str(ep, "current_language")
        var servers: [Server] = J.objects(J.arr(ep, "episode_sources")).compactMap { s in
            guard let sv = J.obj(s, "server") else { return nil }
            let l = J.str(J.obj(s, "language"), "slug")
            if !lang.isEmpty && !l.isEmpty && l != lang { return nil }
            return Server(label: J.str(sv, "name"), type: J.str(sv, "slug"))
        }
        if servers.isEmpty {
            servers = J.objects(J.arr(ep, "available_sources")).filter { lang.isEmpty || J.str($0, "language_slug") == lang }
                .map { Server(label: J.str($0, "server_name"), type: J.str($0, "server_slug")) }
        }
        servers = servers.filter { !$0.type.isEmpty }.distinct { $0.type }
            .map { $0.label.isEmpty ? Server(label: $0.type, type: $0.type) : $0 }
        let cur = currentServer(ep)
        var links: [String: Video] = [:]
        if !cur.isEmpty, let v = videoOf(ep, origin(doc)) { links[cur] = v }
        return EpisodeSource(epUrl: epUrl, servers: servers, postId: "", ep: epSlug(epUrl), sv: "", playerUrl: nil,
                             directEmbed: nil, pageMode: false, links: links, next: true)
    }

    /// ".../xem-phim/tien-nghich/tap-165/vietsub" → "tap-165".
    static func epSlug(_ epUrl: String) -> String {
        let parts = epUrl.trimmingSuffix("/").split(separator: "/", omittingEmptySubsequences: false)
        return parts.count >= 2 ? String(parts[parts.count - 2]) : ""
    }

    /// Link video của một server: đã có sẵn thì dùng, không thì mở trang tập với ?server=… để lấy.
    static func video(_ src: EpisodeSource, _ server: Server) async throws -> Video {
        if let v = src.links[server.type] { return v }
        let r = try await Http.get(src.epUrl + "?server=" + server.type.formEncoded)
        let doc = try Site.parse(r.body, r.url)
        guard let d = data(doc), let ep = episodeData(d) else { throw AppError("Không đọc được trang tập phim") }
        // Tập chưa có trên server này thì trang tự mở server khác → coi như chưa có
        guard currentServer(ep) == server.type, let v = videoOf(ep, origin(doc)) else {
            throw AppError("Server \"\(server.label)\" chưa có video cho tập này")
        }
        return v
    }

    private static func currentServer(_ ep: [String: Any]) -> String {
        switch ep["current_server"] {
        case let o as [String: Any]: return J.str(o, "slug")
        case let s as String: return s
        default: return ""
        }
    }

    /// Server riêng của trang ("HD") cho link dạng /public/m3u8/… (cùng tên miền) và phụ đề rời.
    private static func videoOf(_ ep: [String: Any], _ origin: String) -> Video? {
        guard let v = J.obj(ep, "video"),
              let link = J.str(v, "link_m3u8").nonEmpty ?? J.str(v, "link_embed").nonEmpty,
              let url = abs(origin, link) else { return nil }
        return Video(url: url, subtitle: subtitle(J.arr(v, "subtitles"), origin))
    }

    /// Phụ đề rời: bản mặc định, không có thì tiếng Việt.
    private static func subtitle(_ a: [Any]?, _ origin: String) -> String? {
        let subs = J.objects(a).filter { !J.str($0, "url").isEmpty }
        let s = subs.first { J.bool($0, "isDefault") } ?? subs.first { J.str($0, "lang") == "vie" } ?? subs.first
        return s.flatMap { abs(origin, J.str($0, "url")) }
    }

    // MARK: - Tiện ích

    private static func movies(_ data: NextData, _ origin: String, from: Int = 0, to: Int? = nil) -> [Film] {
        let end = to ?? data.length
        var out: [Film] = []
        var seen = Set<String>()
        for m in movieRx.all(in: data.text, from: from) {
            if m.range.location >= end { break }
            guard let o = data.value(at: m.range.location) as? [String: Any], let f = film(o, origin) else { continue }
            if seen.insert(f.url).inserted { out.append(f) }
        }
        return out
    }

    /// Phim trong danh sách (đối tượng thể loại / quốc gia cũng có id + name + slug nhưng không có poster).
    private static func film(_ o: [String: Any], _ origin: String) -> Film? {
        guard o["poster_url"] != nil, let slug = J.str(o, "slug").nonEmpty, let name = J.str(o, "name").nonEmpty else { return nil }
        let poster = (J.str(o, "poster_url").nonEmpty ?? J.str(o, "thumb_url").nonEmpty).flatMap { abs(origin, $0) }
        return Film(url: "\(origin)/phim/\(slug)", title: name, poster: poster, badge: badge(J.str(o, "display_status")))
    }

    private static let numberRx = Rx(#"\d+"#)

    /// Nhãn trên thẻ phim (một dòng): "Thuyết Minh 107, Vietsub 165" → "Tập 165", "Vietsub Full" → "Full".
    static func badge(_ status: String) -> String? {
        let ns = status as NSString
        let n = numberRx.all(in: status).compactMap { Int(ns.substring(with: $0.range)) }.max()
        if let n = n { return "Tập \(n)" }
        if status.lowercased().contains("full") { return "Full" }
        return status.nonEmpty
    }

    private static func names(_ a: [Any]?) -> String {
        J.objects(a).map { J.str($0, "name") }.filter { !$0.isEmpty }.joined(separator: ", ")
    }

    private static func origin(_ doc: Document) -> String { Site.originOf(doc.location()) ?? Prefs.baseUrl }

    /// Đường dẫn tuyệt đối; dạng lạ (vd "data:…") thì nil — bỏ một ảnh / link chứ không hỏng cả trang.
    private static func abs(_ origin: String, _ ref: String) -> String? {
        guard let u = URL(string: ref, relativeTo: URL(string: origin + "/")) else { return nil }
        let s = u.absoluteString
        return s.hasPrefix("http") ? s : nil
    }
}
