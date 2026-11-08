import Foundation
import SwiftSoup

/// Trang phim dựng bằng React (trang trắng + JS, vd mephim.ddns.net) có API JSON riêng ở /api/movies/… (nguồn KKPhim,
/// định dạng của phimapi.com) — bản Android: ApiSite.kt (giữ cùng tên hàm để sửa song song). Không đọc HTML: danh sách,
/// chi tiết, tập và link video (.m3u8 — app tự phát, bỏ quảng cáo; hoặc trình phát nhúng của KKPhim) đều lấy từ API.
///
/// Địa chỉ trong app theo đường dẫn của chính trang: phim = /phim/<slug>, tập = /phim/<slug>/<tập>?sv=<số thứ tự bản>
/// (Vietsub / Thuyết Minh cùng tên tập nên phải ghi bản). Danh sách = địa chỉ API.
enum ApiSite {
    /// Trang KKPhim (www.kkphim.com, từ 1.6): không có API /api/movies riêng — mọi dữ liệu gọi thẳng API công khai
    /// phimapi.com (cùng định dạng, qua fallback). Địa chỉ phim / tập trong app vẫn theo tên miền kkphim.com.
    static let kkHost = "kkphim.com"

    /// VSMOV (vsmov.com, từ 1.7): API riêng cùng định dạng KKPhim nhưng khác đường dẫn (/api/danh-sach/…, /api/phim/…,
    /// /api/tim-kiem). Tập chỉ có trình phát nhúng; app đọc luồng HLS trong trang trình phát (link có chữ ký) để tự phát.
    static let vsHost = "vsmov.com"

    private static let lock = NSLock()
    private static var known: Set<String> = ["mephim.ddns.net", kkHost, vsHost, NguonC.host, Stremio.host]
    private static var notApi: Set<String> = []
    private static let cdn = "https://phimimg.com"
    private static let limit = 24

    /// Menu cố định (API không có danh sách mục): hoạt hình Trung Quốc lên đầu như các trang khác của app.
    private static let menu: [(String, String)] = [
        ("Hoạt hình Trung Quốc", "category/hoat-hinh?country=trung-quoc"),
        ("Hoạt hình", "category/hoat-hinh"),
        ("Phim bộ", "category/phim-bo"),
        ("Phim lẻ", "category/phim-le"),
        ("TV Shows", "category/tv-shows"),
        ("Phim thuyết minh", "category/phim-thuyet-minh"),
        ("Phim lồng tiếng", "category/phim-long-tieng"),
        ("Phim Trung Quốc", "quoc-gia/trung-quoc"),
        ("Phim Hàn Quốc", "quoc-gia/han-quoc"),
        ("Phim Âu Mỹ", "quoc-gia/au-my"),
        ("Phim Nhật Bản", "quoc-gia/nhat-ban"),
        ("Phim Thái Lan", "quoc-gia/thai-lan"),
    ]

    private static let genres: [(String, String)] = [
        ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Viễn Tưởng", "vien-tuong"), ("Kinh Dị", "kinh-di"),
        ("Tình Cảm", "tinh-cam"), ("Hài Hước", "hai-huoc"), ("Võ Thuật", "vo-thuat"), ("Tâm Lý", "tam-ly"),
        ("Phiêu Lưu", "phieu-luu"), ("Hình Sự", "hinh-su"), ("Chiến Tranh", "chien-tranh"), ("Thần Thoại", "than-thoai"),
        ("Bí Ẩn", "bi-an"), ("Học Đường", "hoc-duong"), ("Gia Đình", "gia-dinh"), ("Thể Thao", "the-thao"),
        ("Âm Nhạc", "am-nhac"), ("Chính Kịch", "chinh-kich"), ("Khoa Học", "khoa-hoc"), ("Tài Liệu", "tai-lieu"),
        ("Lịch Sử", "lich-su"), ("Kinh Điển", "kinh-dien"), ("Phim 18+", "phim-18"),
    ]
    private static let countries: [(String, String)] = [
        ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Âu Mỹ", "au-my"),
        ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
        ("Việt Nam", "viet-nam"), ("Anh", "anh"), ("Pháp", "phap"), ("Canada", "canada"), ("Philippines", "philippines"),
        ("Indonesia", "indonesia"), ("Malaysia", "malaysia"), ("Nga", "nga"), ("Đức", "duc"), ("Tây Ban Nha", "tay-ban-nha"),
        ("Thổ Nhĩ Kỳ", "tho-nhi-ky"), ("Úc", "uc"), ("Quốc Gia Khác", "quoc-gia-khac"),
    ]

    /// Trang KKPhim (gọi thẳng phimapi.com) thêm vài mục theo các danh sách riêng của KKPhim.
    private static let kkMenu: [(String, String)] = Array(menu.prefix(1)) +
        [("Hoạt hình Trung Quốc thuyết minh", "category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh")] +
        Array(menu.dropFirst()) + [("Phim chiếu rạp", "category/phim-chieu-rap")]

    static func isKk(_ url: String) -> Bool { Site.hostOf(url) == kkHost }

    static func isVs(_ url: String) -> Bool { Site.hostOf(url) == vsHost }

    /// Menu VSMOV: lọc hoạt hình theo nước có trên /api/the-loai/…?country=.
    private static let vsMenu: [(String, String)] = [
        ("Hoạt hình Trung Quốc", "the-loai/hoat-hinh?country=trung-quoc"),
        ("Hoạt hình", "the-loai/hoat-hinh"),
        ("Phim bộ", "danh-sach/phim-bo"),
        ("Phim lẻ", "danh-sach/phim-le"),
        ("Phim nhóm sub VSMOV", "danh-sach/subteam"),
        ("Phim Trung Quốc", "quoc-gia/trung-quoc"),
        ("Phim Hàn Quốc", "quoc-gia/han-quoc"),
        ("Phim Âu Mỹ", "quoc-gia/au-my"),
        ("Phim Nhật Bản", "quoc-gia/nhat-ban"),
        ("Phim Thái Lan", "quoc-gia/thai-lan"),
    ]
    private static let vsGenres: [(String, String)] = [
        ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Kiếm Hiệp", "kiem-hiep"), ("Tiên Hiệp", "tien-hiep"),
        ("Võ Thuật", "vo-thuat"), ("Viễn Tưởng", "vien-tuong"), ("Giả Tưởng", "gia-tuong"), ("Kinh Dị", "kinh-di"),
        ("Lãng Mạn", "lang-man"), ("Hài", "hai"), ("Chính Kịch", "chinh-kich"), ("Phiêu Lưu", "phieu-luu"),
        ("Hình Sự", "hinh-su"), ("Tội Phạm", "toi-pham"), ("Chiến Tranh", "chien-tranh"), ("Bí Ẩn", "bi-an"),
        ("Học Đường", "hoc-duong"), ("Gia Đình", "gia-dinh"), ("Thiếu Nhi", "thieu-nhi"), ("Thanh Xuân", "thanh-xuan"),
        ("Thương Trường", "thuong-truong"), ("Xã Hội Đen", "xa-hoi-den"), ("Truyền Hình Thực Tế", "truyen-hinh-thuc-te"),
        ("Phim Nhạc", "phim-nhac"),
    ]
    private static let vsCountries: [(String, String)] = [
        ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Âu Mỹ", "au-my"), ("Mỹ", "my"),
        ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
        ("Việt Nam", "viet-nam"), ("Anh", "anh"), ("Pháp", "phap"), ("Canada", "canada"), ("Úc", "uc"), ("Đức", "duc"),
        ("Tây Ban Nha", "tay-ban-nha"), ("Thổ Nhĩ Kỳ", "tho-nhi-ky"), ("Philippines", "philippines"), ("Indonesia", "indonesia"),
    ]

    /// Địa chỉ API của VSMOV: "danh-sach/phim-bo" → https://vsmov.com/api/danh-sach/phim-bo?page=1.
    private static func vsApi(_ origin: String, _ path: String, _ page: Int) -> String {
        let parts = path.split(separator: "?", maxSplits: 1).map(String.init)
        return "\(origin)/api/\(parts[0])?" + (parts.count > 1 ? parts[1] + "&" : "") + "page=\(page)"
    }

    static func isKnown(_ url: String) -> Bool {
        guard let h = Site.hostOf(url) else { return false }
        lock.lock()
        defer { lock.unlock() }
        return known.contains(h)
    }

    /// Trang chưa biết mà HTML là trang trắng của ứng dụng JS: thử API (một lần mỗi trang).
    static func probe(_ base: String) async -> Bool {
        guard let host = Site.hostOf(base) else { return false }
        lock.lock()
        let k = known.contains(host), n = notApi.contains(host)
        lock.unlock()
        if k { return true }
        if n { return false }
        let ok = ((try? await getJSON("\(origin(base))/api/movies/newest?page=1&limit=1", fallback: false))?["items"] as? [Any]) != nil
        lock.lock()
        if ok { known.insert(host) } else { notApi.insert(host) }
        lock.unlock()
        return ok
    }

    // MARK: - Trang chủ / danh sách

    /// Trang chủ: các khối tải song song; khối nào lỗi thì bỏ.
    static func home(_ base: String) async throws -> Home {
        if NguonC.isSite(base) { return try await NguonC.home(base) }
        if Stremio.isSite(base) { return try await Stremio.home(base) }
        let o = origin(base)
        let kk = isKk(base), vs = isVs(base)
        // KKPhim không có khối thịnh hành / song ngữ: thêm hoạt hình thuyết minh, phim bộ, phim lẻ mới
        let blocks: [(String, String)] = vs ? [
            ("Hoạt hình Trung Quốc", vsApi(o, "the-loai/hoat-hinh?country=trung-quoc", 1)),
            ("Phim mới cập nhật", newestUrl(o)),
            ("Phim bộ mới", vsApi(o, "danh-sach/phim-bo", 1)),
            ("Phim lẻ mới", vsApi(o, "danh-sach/phim-le", 1)),
            ("Phim nhóm sub VSMOV", vsApi(o, "danh-sach/subteam", 1)),
        ] : kk ? [
            ("Hoạt hình Trung Quốc mới cập nhật", api(o, "category/hoat-hinh?country=trung-quoc", 1)),
            ("Hoạt hình Trung Quốc thuyết minh", api(o, "category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh", 1)),
            ("Phim mới cập nhật", newestUrl(o)),
            ("Phim bộ mới", api(o, "category/phim-bo", 1)),
            ("Phim lẻ mới", api(o, "category/phim-le", 1)),
        ] : [
            ("Hoạt hình Trung Quốc mới cập nhật", api(o, "category/hoat-hinh?country=trung-quoc", 1)),
            ("Phim mới cập nhật", newestUrl(o)),
            ("Thịnh hành", "\(o)/api/movies/trending"),
            ("Song ngữ / Thuyết minh", "\(o)/api/movies/song-ngu"),
        ]
        let sections = try await sections(blocks) { parseItems(try await getJSON($0), o) }
        return Home(sections: sections, categories: menuOf(base), movedTo: nil)
    }

    /// Mục menu của trang (cố định — không cần tải trang chủ; trang chủ tổng hợp dùng).
    static func menuOf(_ base: String) -> [Category] {
        if NguonC.isSite(base) { return NguonC.menuOf(base) }
        if Stremio.isSite(base) { return Stremio.menuOf(base) }
        let o = origin(base)
        return isVs(base) ? vsMenu.map { Category(title: $0.0, url: vsApi(o, $0.1, 1)) }
            : (isKk(base) ? kkMenu : menu).map { Category(title: $0.0, url: api(o, $0.1, 1)) }
    }

    /// Tên thể loại theo slug ("vo-thuat" → "võ thuật") — tiêu đề hàng gợi ý của trang chủ tổng hợp.
    static func genreName(_ slug: String) -> String {
        (genres + vsGenres).first { $0.1 == slug }?.0.lowercased() ?? slug.replacingOccurrences(of: "-", with: " ")
    }

    /// Tên nước theo slug ("trung-quoc" → "Trung Quốc").
    static func countryName(_ slug: String) -> String {
        (countries + vsCountries).first { $0.1 == slug }?.0 ?? slug.replacingOccurrences(of: "-", with: " ")
    }

    /// JSON của một API (KKPhim: gọi thẳng phimapi.com; trang lỗi thì phimapi.com) — trang chủ tổng hợp dùng.
    static func json(_ url: String) async throws -> [String: Any] { try await getJSON(url) }

    /// Các khối trang chủ tải song song; khối nào lỗi / rỗng thì bỏ, lỗi hết thì báo lỗi.
    static func sections(_ blocks: [(String, String)], _ load: @escaping (String) async throws -> [Film]) async throws -> [FilmSection] {
        var found = [FilmSection?](repeating: nil, count: blocks.count)
        var firstError: Error?
        await withTaskGroup(of: (Int, Result<[Film], Error>).self) { group in
            for (i, b) in blocks.enumerated() {
                group.addTask {
                    do { return (i, .success(try await load(b.1))) } catch { return (i, .failure(error)) }
                }
            }
            for await (i, r) in group {
                switch r {
                case .success(let films): if !films.isEmpty { found[i] = FilmSection(title: blocks[i].0, films: films) }
                case .failure(let e): if firstError == nil { firstError = e }
                }
            }
        }
        let sections = found.compactMap { $0 }
        if sections.isEmpty { throw firstError ?? AppError("Không tải được danh sách phim") }
        return sections
    }

    static func list(_ url: String) async throws -> FilmPage {
        if NguonC.isSite(url) { return try await NguonC.list(url) }
        if Stremio.isSite(url) { return try await Stremio.list(url) }
        return parseList(try await getJSON(url), url)
    }

    /// Danh sách có phân trang (data.params.pagination; VSMOV: pagination ở gốc) → trang sau = page + 1.
    static func parseList(_ root: [String: Any], _ url: String) -> FilmPage {
        let films = parseItems(root, origin(url))
        let data = J.obj(root, "data") ?? root
        let p = J.obj(J.obj(data, "params"), "pagination") ?? J.obj(root, "pagination")
        let page = J.int(p, "currentPage"), pages = J.int(p, "totalPages")
        let next = (!films.isEmpty && page >= 1 && page < pages) ? withPage(url, page + 1) : nil
        return FilmPage(films: films, nextUrl: next)
    }

    static func parseList(_ data: Data, _ url: String) -> FilmPage {
        parseList((try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:], url)
    }

    /// Phim mới cập nhật (đo nhanh trang trong Cài đặt).
    static func newestUrl(_ base: String) -> String {
        if NguonC.isSite(base) { return NguonC.newestUrl(base) }
        if Stremio.isSite(base) { return Stremio.newestUrl(base) }
        if isVs(base) { return "\(origin(base))/api/danh-sach/phim-moi-cap-nhat?page=1" }
        return "\(origin(base))/api/movies/newest?page=1&limit=\(limit)"
    }

    static func searchUrl(_ base: String, _ query: String) -> String {
        if NguonC.isSite(base) { return NguonC.searchUrl(base, query) }
        if Stremio.isSite(base) { return Stremio.searchUrl(base, query) }
        if isVs(base) { return "\(origin(base))/api/tim-kiem?keyword=" + query.trimmed.formEncoded + "&page=1&limit=\(limit)" }
        return "\(origin(base))/api/movies/search?keyword=" + query.trimmed.formEncoded + "&page=1&limit=\(limit)"
    }

    static func filters(_ base: String) -> (genres: [Category], countries: [Category]) {
        if NguonC.isSite(base) { return NguonC.filters(base) }
        if Stremio.isSite(base) { return Stremio.filters(base) }
        let o = origin(base)
        if isVs(base) {
            return (vsGenres.map { Category(title: $0.0, url: vsApi(o, "the-loai/\($0.1)", 1)) },
                    vsCountries.map { Category(title: $0.0, url: vsApi(o, "quoc-gia/\($0.1)", 1)) })
        }
        return (genres.map { Category(title: $0.0, url: api(o, "the-loai/\($0.1)", 1)) },
                countries.map { Category(title: $0.0, url: api(o, "quoc-gia/\($0.1)", 1)) })
    }

    /// Phim trong một phản hồi danh sách (items ở gốc hoặc trong data).
    static func parseItems(_ root: [String: Any], _ origin: String) -> [Film] {
        let data = J.obj(root, "data") ?? root
        let base = J.str(data, "APP_DOMAIN_CDN_IMAGE").nonEmpty ?? J.str(root, "cdnDomain").nonEmpty ?? cdn
        return J.objects(J.arr(data, "items")).compactMap { film($0, origin, base) }.distinct { $0.url }
    }

    private static func film(_ o: [String: Any], _ origin: String, _ base: String) -> Film? {
        guard let slug = J.str(o, "slug").nonEmpty, let name = J.str(o, "name").nonEmpty else { return nil }
        let poster = image(J.str(o, "poster_url").nonEmpty ?? J.str(o, "thumb_url"), J.str(o, "cdnDomain").nonEmpty ?? base)
        return Film(url: "\(origin)/phim/\(slug)", title: name, poster: poster, badge: NextSite.badge(J.str(o, "episode_current")))
    }

    // MARK: - Chi tiết phim

    static func detail(_ url: String) async throws -> FilmDetail {
        if NguonC.isSite(url) { return try await NguonC.detail(url) }
        if Stremio.isSite(url) { return try await Stremio.detail(url) }
        let film = try filmUrl(url)
        return try parseDetail(try await getJSON(try detailApi(film)), film)
    }

    static func parseDetail(_ root: [String: Any], _ url: String) throws -> FilmDetail {
        guard let m = J.obj(root, "movie") else { throw AppError("Không đọc được thông tin phim") }
        let desc = J.str(m, "content").nonEmpty.flatMap { (try? SwiftSoup.parse($0).text())?.nonEmpty }
        var year = ""
        if let y = m["year"] { year = "\(y)" }
        if year == "0" || year == "<null>" { year = "" }
        let info = [J.str(m, "episode_current"), J.str(m, "time"), J.str(m, "lang"), names(J.arr(m, "country")), year,
                    names(J.arr(m, "category"))].filter { !$0.isEmpty }.joined(separator: "  •  ").nonEmpty
        var groups: [EpisodeGroup] = []
        for (i, s) in J.objects(J.arr(root, "episodes")).enumerated() {
            let eps = J.objects(J.arr(s, "server_data")).compactMap { e -> Episode? in
                guard let slug = J.str(e, "slug").nonEmpty else { return nil }
                if J.str(e, "link_m3u8").isEmpty && J.str(e, "link_embed").isEmpty { return nil }
                return Episode(title: title(J.str(e, "name").nonEmpty ?? slug), url: "\(url)/\(slug)?sv=\(i)")
            }.distinct { $0.url }
            // VSMOV: "Vietsub\r\n      #1" → gộp khoảng trắng
            let name = J.str(s, "server_name").components(separatedBy: .whitespacesAndNewlines).filter { !$0.isEmpty }.joined(separator: " ")
            if !eps.isEmpty { groups.append(EpisodeGroup(name: name.nonEmpty ?? "Bản \(i + 1)", episodes: eps)) }
        }
        return FilmDetail(url: url, title: J.str(m, "name").nonEmpty ?? url, otherTitle: J.str(m, "origin_name").nonEmpty,
                          poster: image(J.str(m, "poster_url").nonEmpty ?? J.str(m, "thumb_url"), cdn),
                          description: desc, info: info, groups: groups)
    }

    private static let leadingZero = Rx(#"^(Tập\s*)0+(\d)"#, [.caseInsensitive])

    private static let bareNumber = Rx(#"^\d+(\.\d+)?$"#)

    /// "Tập 01" → "Tập 1", "164" → "Tập 164" (VSMOV ghi số trần); "Full" giữ nguyên.
    static func title(_ name: String) -> String {
        let s = name.trimmed
        if bareNumber.matches(s) {
            var n = s
            while n.count > 1 && n.hasPrefix("0") && !n.hasPrefix("0.") { n.removeFirst() }
            return "Tập " + n
        }
        let ns = s as NSString
        guard let m = leadingZero.all(in: s).first else { return s }
        return ns.replacingCharacters(in: m.range, with: ns.substring(with: m.range(at: 1)) + ns.substring(with: m.range(at: 2)))
    }

    // MARK: - Tập phim

    /// Hai server cho mỗi tập: luồng HLS (app tự phát, bỏ quảng cáo) và trình phát nhúng của KKPhim (dự phòng).
    static func source(_ epUrl: String) async throws -> EpisodeSource {
        if NguonC.isSite(epUrl) { return try await NguonC.source(epUrl) }
        if Stremio.isSite(epUrl) { return try await Stremio.source(epUrl) }
        let root = try await getJSON(try detailApi(try filmUrl(epUrl)))
        let sv = Int(URLComponents(string: epUrl)?.queryItems?.first { $0.name == "sv" }?.value ?? "") ?? 0
        let tap = (URL(string: epUrl)?.path ?? "").trimmingSuffix("/").afterLast("/")
        let servers = J.objects(J.arr(root, "episodes"))
        let order = (sv < servers.count ? [servers[sv]] : []) + servers
        guard let e = order.lazy.compactMap({ s in J.objects(J.arr(s, "server_data")).first { J.str($0, "slug") == tap } }).first
        else { throw AppError("Không tìm thấy tập này trên trang") }
        var links: [String: Video] = [:]
        var list: [Server] = []
        let vs = isVs(epUrl)
        let embed = J.str(e, "link_embed").hasPrefix("http") ? J.str(e, "link_embed") : nil
        var hls = J.str(e, "link_m3u8").hasPrefix("http") ? J.str(e, "link_m3u8") : nil
        if hls == nil, vs, let em = embed { hls = await vsHls(em) }
        if let h = hls {
            links["m3u8"] = Video(url: h)
            list.append(Server(label: vs ? "HLS" : "HLS (bỏ quảng cáo)", type: "m3u8"))
        }
        if let em = embed {
            links["embed"] = Video(url: em)
            list.append(Server(label: vs ? "Trình phát VSMOV" : "Trình phát KKPhim", type: "embed"))
        }
        if links.isEmpty { throw AppError("Tập này chưa có video") }
        return EpisodeSource(epUrl: epUrl, servers: list, postId: "", ep: tap, sv: String(sv), playerUrl: nil, directEmbed: nil,
                             pageMode: false, links: links, next: true)
    }

    /// Luồng HLS trong trang trình phát VSMOV (vN.streamvsmov.com/video/<mã>): link có chữ ký (signedMasterUrl, hết hạn sau
    /// khoảng một giờ) hoặc /stream/<mã>/master.m3u8 khi máy chủ đó không ký. Đoạn video ngụy trang ảnh PNG — hls.html cắt
    /// phần ảnh. Không đọc được thì nil (vẫn còn trình phát nhúng).
    private static func vsHls(_ embed: String) async -> String? {
        guard let r = try? await Http.get(embed, headers: ["Referer": "https://vsmov.com/"]) else { return nil }
        return vsHlsFrom(String(decoding: r.data, as: UTF8.self))
    }

    private static let signedRx = Rx(#"signedMasterUrl\s*:\s*"(https?://[^"]+)""#)
    private static let baseRx = Rx(#"baseUrl\s*=\s*"(https?://[^"]+)""#)
    private static let hashRx = Rx(#"videoHash\s*=\s*"([0-9a-zA-Z-]+)""#)

    static func vsHlsFrom(_ page: String) -> String? {
        if let g = signedRx.groups(in: page) { return g[1] }
        guard let b = baseRx.groups(in: page)?[1], let h = hashRx.groups(in: page)?[1] else { return nil }
        return "\(b)/stream/\(h)/master.m3u8"
    }

    // MARK: - Tiện ích

    /// Địa chỉ phim từ địa chỉ phim / tập: ".../phim/tien-nghich/tap-01?sv=1" → ".../phim/tien-nghich".
    static func filmUrl(_ url: String) throws -> String {
        let parts = (URL(string: url)?.path ?? "").split(separator: "/").map(String.init)
        guard let i = parts.firstIndex(of: "phim"), i + 1 < parts.count else { throw AppError("Địa chỉ phim lạ: \(url)") }
        return "\(origin(url))/phim/\(parts[i + 1])"
    }

    private static func detailApi(_ url: String) throws -> String {
        let slug = (try filmUrl(url)).afterLast("/")
        return isVs(url) ? "\(origin(url))/api/phim/\(slug)" : "\(origin(url))/api/movies/detail/\(slug)"
    }

    private static func api(_ origin: String, _ path: String, _ page: Int) -> String {
        let parts = path.split(separator: "?", maxSplits: 1).map(String.init)
        return "\(origin)/api/movies/\(parts[0])?" + (parts.count > 1 ? parts[1] + "&" : "") + "page=\(page)&limit=\(limit)"
    }

    private static let pageRx = Rx(#"([?&]page=)\d+"#)

    static func withPage(_ url: String, _ page: Int) -> String {
        let ns = url as NSString
        if let m = pageRx.all(in: url).first {
            return ns.replacingCharacters(in: m.range, with: ns.substring(with: m.range(at: 1)) + String(page))
        }
        return url + (url.contains("?") ? "&" : "?") + "page=\(page)"
    }

    /// Gọi API của trang; trang lỗi (máy chủ của trang hay báo 500 «Failed to fetch … from providers») thì gọi thẳng
    /// nguồn KKPhim (phimapi.com — cùng định dạng), như chính trang web làm.
    private static func getJSON(_ url: String, fallback useFallback: Bool = true) async throws -> [String: Any] {
        // Trang KKPhim: gọi thẳng phimapi.com (kkphim.com không có /api/movies)
        if isKk(url) { return try await fetchJSON(fallback(url) ?? url) }
        do {
            return try await fetchJSON(url)
        } catch {
            guard useFallback, let alt = fallback(url) else { throw error }
            if let r = try? await fetchJSON(alt) { return r }
            throw error
        }
    }

    private static func fetchJSON(_ url: String) async throws -> [String: Any] {
        let r = try await Http.get(url, headers: ["Accept": "application/json"])
        guard let o = try JSONSerialization.jsonObject(with: r.data) as? [String: Any] else { throw AppError("Trang trả dữ liệu lạ") }
        if let e = o["error"] as? String, o["items"] == nil, o["data"] == nil, o["movie"] == nil { throw AppError(e) }
        return o
    }

    /// Địa chỉ tương ứng trên phimapi.com của một API của trang (không có thì nil, vd thịnh hành / song ngữ).
    static func fallback(_ url: String) -> String? {
        guard let c = URLComponents(string: url), let r = c.path.range(of: "/api/movies/") else { return nil }
        let path = String(c.path[r.upperBound...])
        let q = c.percentEncodedQuery.map { "?" + $0 } ?? ""
        let kk = "https://phimapi.com"
        if path == "newest" { return "\(kk)/v1/api/danh-sach/phim-moi-cap-nhat\(q)" }
        if path.hasPrefix("category/") { return "\(kk)/v1/api/danh-sach/\(path.dropFirst("category/".count))\(q)" }
        if path.hasPrefix("the-loai/") || path.hasPrefix("quoc-gia/") { return "\(kk)/v1/api/\(path)\(q)" }
        if path == "search" { return "\(kk)/v1/api/tim-kiem\(q)" }
        if path.hasPrefix("detail/") { return "\(kk)/phim/\(path.dropFirst("detail/".count))" }
        return nil
    }

    static func origin(_ url: String) -> String { Site.originOf(url) ?? url.trimmingSuffix("/") }

    /// Ảnh: đường dẫn đầy đủ giữ nguyên; "uploads/movies/…" → nối vào máy chủ ảnh.
    static func image(_ ref: String, _ base: String) -> String? {
        guard !ref.isEmpty else { return nil }
        if ref.hasPrefix("http") { return ref }
        let host = Site.originOf(base) ?? cdn
        var r = ref
        while r.hasPrefix("/") { r.removeFirst() }
        return "\(host)/\(r)"
    }

    private static func names(_ a: [Any]?) -> String {
        J.objects(a).map { J.str($0, "name") }.filter { !$0.isEmpty }.joined(separator: ", ")
    }
}
