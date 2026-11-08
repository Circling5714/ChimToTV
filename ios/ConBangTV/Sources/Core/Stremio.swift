import Foundation

/// CLB Phim Xưa qua add-on Stremio «K20 Phim Tổng Hợp» (sc.k-20.xyz, từ 1.7) — bản Android: Stremio.kt (giữ cùng tên
/// hàm). Giao thức Stremio: /catalog/movie/clbpx-movie[/genre=…&search=…&skip=N].json (30 phim mỗi lần),
/// /meta/movie/clbpx:<slug>.json (videos = các tập; phim lẻ không có), /stream/movie/<mã tập>.json (link MP4 qua máy chủ
/// trung gian của k-20: mở chậm, tua xa có khi không được). Chỉ trả dữ liệu cho User-Agent kiểu trình duyệt.
///
/// Địa chỉ trong app: danh sách = địa chỉ catalog; phim = https://sc.k-20.xyz/clbpx/<slug>; tập = …/clbpx/<slug>/<mã tập>
/// (phim lẻ: …/clbpx/<slug>/full).
enum Stremio {
    static let host = "sc.k-20.xyz"

    /// Tên hiện trong app (tên miền của add-on không nói lên gì).
    static let name = "CLB Phim Xưa"

    private static let type = "movie"
    private static let catalogId = "clbpx-movie"
    private static let idPrefix = "clbpx"
    private static let full = "full"

    /// Thể loại của catalog (manifest: extra genre). Mục đầu = mới cập nhật.
    private static let genres = [
        "Mới Cập Nhật", "Kiếm Hiệp", "Tiên Hiệp", "Tâm Lý", "Ma Kinh Dị", "Điện Ảnh Châu Á", "Điện Ảnh Âu Mỹ", "Hàn Quốc", "Anime",
        "TV Series", "Thập Niên 60", "Thập Niên 70", "Thập Niên 80", "Thập Niên 90", "Thập Niên 2000",
    ]

    static func isSite(_ url: String) -> Bool { Site.hostOf(url) == host }

    static func home(_ base: String) async throws -> Home {
        let o = ApiSite.origin(base)
        let blocks = ["Mới cập nhật", "Kiếm Hiệp", "TV Series", "Thập Niên 90", "Thập Niên 80"]
            .map { ($0, catalog(o, genre: $0 == "Mới cập nhật" ? nil : $0)) }
        let sections = try await ApiSite.sections(blocks) { parseList(try await getData($0), $0).films }
        return Home(sections: sections, categories: genres.dropFirst().map { Category(title: $0, url: catalog(o, genre: $0)) },
                    movedTo: nil)
    }

    static func newestUrl(_ base: String) -> String { catalog(ApiSite.origin(base)) }

    static func searchUrl(_ base: String, _ query: String) -> String { catalog(ApiSite.origin(base), search: query.trimmed) }

    static func filters(_ base: String) -> (genres: [Category], countries: [Category]) {
        let o = ApiSite.origin(base)
        return (genres.map { Category(title: $0, url: catalog(o, genre: $0)) }, [])
    }

    /// Địa chỉ catalog; tham số kiểu Stremio: /catalog/movie/clbpx-movie/genre=…&skip=30.json.
    static func catalog(_ origin: String, genre: String? = nil, search: String? = nil, skip: Int = 0) -> String {
        var extra: [String] = []
        if let g = genre { extra.append("genre=" + enc("Thể loại: \(g)")) }
        if let s = search { extra.append("search=" + enc(s)) }
        if skip > 0 { extra.append("skip=\(skip)") }
        return "\(origin)/catalog/\(type)/\(catalogId)" + (extra.isEmpty ? "" : "/" + extra.joined(separator: "&")) + ".json"
    }

    static func list(_ url: String) async throws -> FilmPage { parseList(try await getData(url), url) }

    private static let tapCount = Rx(#"(\d+)\s*Tập\s*$"#, [.caseInsensitive])

    /// {metas:[{id:"clbpx:<slug>", name, poster}]}; trang sau = skip + số phim (hết khi rỗng; tìm kiếm chỉ một trang).
    static func parseList(_ data: Data, _ url: String) -> FilmPage {
        let o = ApiSite.origin(url)
        let root = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
        let films = J.objects(J.arr(root, "metas")).compactMap { m -> Film? in
            let id = J.str(m, "id")
            guard id.hasPrefix("\(idPrefix):"), let name = J.str(m, "name").nonEmpty else { return nil }
            let slug = String(id.dropFirst(idPrefix.count + 1))
            guard !slug.isEmpty else { return nil }
            let poster = J.str(m, "poster").hasPrefix("http") ? J.str(m, "poster") : nil
            return Film(url: "\(o)/\(idPrefix)/\(slug)", title: name, poster: poster,
                        badge: tapCount.groups(in: name).map { "\($0[1]) tập" })
        }.distinct { $0.url }
        let extra = extraOf(url)
        var next: String?
        if !films.isEmpty && extra["search"] == nil {
            let skip = (Int(extra["skip"] ?? "") ?? 0) + films.count
            let g = extra["genre"].map { $0.hasPrefix("Thể loại: ") ? String($0.dropFirst("Thể loại: ".count)) : $0 }
            next = catalog(o, genre: g, skip: skip)
        }
        return FilmPage(films: films, nextUrl: next)
    }

    /// Tham số trong đoạn cuối địa chỉ catalog: "genre=Th%E1…&skip=30.json" → {genre: "Thể loại: …", skip: "30"}.
    private static func extraOf(_ url: String) -> [String: String] {
        var last = (URLComponents(string: url)?.percentEncodedPath ?? "").afterLast("/")
        if last.hasSuffix(".json") { last = String(last.dropLast(5)) }
        guard last.contains("=") else { return [:] }
        var out: [String: String] = [:]
        for p in last.split(separator: "&") {
            let kv = p.split(separator: "=", maxSplits: 1).map(String.init)
            if kv.count == 2 { out[kv[0]] = kv[1].removingPercentEncoding ?? kv[1] }
        }
        return out
    }

    static func detail(_ url: String) async throws -> FilmDetail {
        let film = try filmUrl(url)
        return try parseDetail(try await getData(metaUrl(film)), film)
    }

    private static let leadingZero = Rx(#"^(Tập\s*)0+(\d)"#, [.caseInsensitive])

    /// {meta:{name, poster, description, releaseInfo, genres, videos:[{id, title, season, episode}]}}.
    static func parseDetail(_ data: Data, _ url: String) throws -> FilmDetail {
        let root = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
        guard let m = J.obj(root, "meta") else { throw AppError("Không đọc được thông tin phim") }
        let prefix = J.str(m, "id") + ":"
        let videos = J.objects(J.arr(m, "videos")).sorted {
            (J.int($0, "season", 1), J.int($0, "episode")) < (J.int($1, "season", 1), J.int($1, "episode"))
        }
        var eps: [Episode] = []
        if videos.isEmpty {
            eps = [Episode(title: "Xem phim", url: "\(url)/\(full)")]
        } else {
            eps = videos.compactMap { v -> Episode? in
                let id = J.str(v, "id")
                guard id.hasPrefix(prefix), id.count > prefix.count else { return nil }
                let code = String(id.dropFirst(prefix.count))
                let t = fixZero(J.str(v, "title").nonEmpty ?? "Tập \(J.int(v, "episode"))")
                return Episode(title: t, url: "\(url)/\(enc(code))")
            }.distinct { $0.url }
        }
        let gs = (J.arr(m, "genres") as? [String]) ?? []
        let info = [J.str(m, "releaseInfo"), gs.joined(separator: ", ")].filter { !$0.isEmpty }.joined(separator: "  •  ").nonEmpty
        let poster = J.str(m, "poster").hasPrefix("http") ? J.str(m, "poster") : nil
        return FilmDetail(url: url, title: J.str(m, "name").nonEmpty ?? url, otherTitle: nil, poster: poster,
                          description: J.str(m, "description").nonEmpty, info: info,
                          groups: [EpisodeGroup(name: "Tập phim", episodes: eps)])
    }

    /// "Tập 01" → "Tập 1".
    private static func fixZero(_ s: String) -> String {
        let ns = s as NSString
        guard let m = leadingZero.all(in: s).first else { return s }
        return ns.replacingCharacters(in: m.range, with: ns.substring(with: m.range(at: 1)) + ns.substring(with: m.range(at: 2)))
    }

    /// Mỗi link MP4 là một server: "720p", "1080p · 3,1 GB"… (cùng chất lượng có nhiều bản thì ghi dung lượng).
    static func source(_ epUrl: String) async throws -> EpisodeSource {
        let film = try filmUrl(epUrl)
        let raw = (URLComponents(string: epUrl)?.percentEncodedPath ?? "").trimmingSuffix("/").afterLast("/")
        let code = raw.removingPercentEncoding ?? raw
        let slug = film.afterLast("/")
        let id = code == full ? "\(idPrefix):\(slug)" : "\(idPrefix):\(slug):\(code)"
        return try parseSource(try await getData("\(ApiSite.origin(film))/stream/\(type)/\(enc(id)).json"), epUrl)
    }

    private static let sizeRx = Rx(#"[?&]size=(\d+)"#)

    static func parseSource(_ data: Data, _ epUrl: String) throws -> EpisodeSource {
        let root = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
        let streams = J.objects(J.arr(root, "streams")).filter { J.str($0, "url").hasPrefix("http") }
        if streams.isEmpty { throw AppError("Tập này chưa có video") }
        let quality = streams.map { (J.str($0, "title").components(separatedBy: "\n").first ?? "").trimmed.nonEmpty ?? "MP4" }
        var servers: [Server] = []
        var links: [String: Video] = [:]
        for (i, s) in streams.enumerated() {
            let url = J.str(s, "url")
            let size = sizeRx.groups(in: url).flatMap { Double($0[1]) }
            let dup = quality.filter { $0 == quality[i] }.count > 1
            let label = quality[i] + (dup && size != nil ? " · " + String(format: "%.1f GB", size! / 1e9).replacingOccurrences(of: ".", with: ",") : "")
            let t = "mp4-\(i)"
            servers.append(Server(label: label, type: t))
            links[t] = Video(url: url, subtitle: nil, file: true)
        }
        return EpisodeSource(epUrl: epUrl, servers: servers, postId: "", ep: "", sv: "", playerUrl: nil, directEmbed: nil,
                             pageMode: false, links: links, next: true)
    }

    /// ".../clbpx/nch1993/LM4bPc-3a" → ".../clbpx/nch1993".
    static func filmUrl(_ url: String) throws -> String {
        let parts = (URL(string: url)?.path ?? "").split(separator: "/").map(String.init)
        guard let i = parts.firstIndex(of: idPrefix), i + 1 < parts.count else { throw AppError("Địa chỉ phim lạ: \(url)") }
        return "\(ApiSite.origin(url))/\(idPrefix)/\(parts[i + 1])"
    }

    private static func metaUrl(_ film: String) -> String {
        "\(ApiSite.origin(film))/meta/\(type)/" + enc("\(idPrefix):" + film.afterLast("/")) + ".json"
    }

    private static func getData(_ url: String) async throws -> Data {
        do {
            return try await Http.get(url, headers: ["Accept": "application/json"]).data
        } catch let e as Http.HttpError where e.code == 404 {
            throw AppError("CLB Phim Xưa không có phim / tập này")
        }
    }

    /// Mã hoá chặt (chỉ giữ chữ số / chữ cái ASCII và "-._~"), dấu cách = %20 — như URLEncoder của bản Android + "%20".
    static func enc(_ s: String) -> String {
        var allowed = CharacterSet()
        allowed.insert(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._*")
        return s.addingPercentEncoding(withAllowedCharacters: allowed) ?? s
    }
}
