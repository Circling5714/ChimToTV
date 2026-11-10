import Foundation

/// Điểm IMDb của một phim — lấy thẳng từ dữ liệu KKPhim (phimapi.com có sẵn imdb.vote_average / vote_count), như bản TV
/// (ImdbTop.kt). Không gọi IMDb. Ít hơn minVotes lượt chấm thì coi như chưa có điểm (10 điểm từ 1 lượt chấm).
struct Imdb: Hashable {
    let score: Double
    let votes: Int

    static let minVotes = 100

    /// Nhãn trên thẻ phim, vd "IMDb 8.5".
    var label: String { "IMDb " + String(format: "%.1f", locale: Locale(identifier: "en_US_POSIX"), score) }

    /// Dòng thông tin ở trang phim, vd "IMDb 8.5 (378 nghìn lượt chấm)".
    var detail: String { "\(label) (\(Imdb.votesText(votes)) lượt chấm)" }

    /// Đối tượng "imdb" của KKPhim → Imdb (nil = chưa có điểm / quá ít lượt).
    static func of(_ o: [String: Any]?) -> Imdb? {
        guard let o = o else { return nil }
        let s = dbl(o["vote_average"]) ?? 0
        let v = Int(num64(o["vote_count"]))
        return s > 0 && s <= 10 && v >= minVotes ? Imdb(score: s, votes: v) : nil
    }

    static func votesText(_ v: Int) -> String {
        if v >= 1_000_000 {
            var s = String(format: "%.1f", locale: Locale(identifier: "en_US_POSIX"), Double(v) / 1_000_000).replacingOccurrences(of: ".", with: ",")
            if s.hasSuffix(",0") { s.removeLast(2) }
            return s + " triệu"
        }
        if v >= 1000 { return "\(v / 1000) nghìn" }
        return "\(v)"
    }
}

/// Danh mục «Điểm IMDb cao» (như bản TV): vài trang đầu (phim mới cập nhật) của các danh sách KKPhim, xếp theo điểm IMDb có
/// trọng số (như Top 250 của IMDb: WR = v/(v+m)·R + m/(v+m)·C; dưới số lượt tối thiểu thì bỏ, m = 5 lần số đó). API KKPhim không
/// sắp theo điểm, kho quá lớn để máy tự quét hết → chỉ xếp trong vài trăm phim mới nhất mỗi nhóm. Lưu kết quả 12 giờ.
enum ImdbTop {
    struct Group {
        let id: String
        let title: String
        let path: String
        let pages: Int
        let minVotes: Int
        var kids = false
    }

    /// Đường dẫn API kiểu KKPhim (www.kkphim.com/api/movies/… — ApiSite gọi thẳng phimapi.com).
    static let groups: [Group] = [
        Group(id: "le", title: "Phim lẻ", path: "category/phim-le", pages: 8, minVotes: 10_000),
        Group(id: "bo", title: "Phim bộ", path: "category/phim-bo", pages: 8, minVotes: 2_000),
        Group(id: "han", title: "Phim Hàn Quốc", path: "quoc-gia/han-quoc", pages: 5, minVotes: 1_000),
        Group(id: "aumy", title: "Phim Âu Mỹ", path: "quoc-gia/au-my", pages: 5, minVotes: 5_000),
        Group(id: "tq", title: "Phim Trung Quốc", path: "quoc-gia/trung-quoc", pages: 5, minVotes: 500),
        Group(id: "hh", title: "Hoạt hình", path: "category/hoat-hinh", pages: 6, minVotes: 1_000, kids: true),
    ]

    private static let origin = "https://www.kkphim.com"
    private static let pageSize = 64
    private static let top = 60
    private static let ttl: Int64 = 12 * 3600_000

    /// Danh mục (của trang) xếp được theo IMDb: danh sách KKPhim (dữ liệu có điểm IMDb).
    static func canRank(_ url: String) -> Bool {
        guard let h = Site.hostOf(url) else { return false }
        return (h.hasSuffix(ApiSite.kkHost) || h.hasSuffix("phimapi.com")) && url.contains("/api/")
    }

    /// Trang «Điểm IMDb cao»: mỗi nhóm một hàng (hồ sơ trẻ em: chỉ hoạt hình).
    static func page(kids: Bool) async throws -> FilmPage {
        let gs = groups.filter { !kids || $0.kids }
        let lists = await withTaskGroup(of: (Int, [Film]?).self) { tg -> [Int: [Film]] in
            for (i, g) in gs.enumerated() { tg.addTask { (i, try? await group(g)) } }
            var out: [Int: [Film]] = [:]
            for await (i, f) in tg { if let f = f { out[i] = f } }
            return out
        }
        let sections = gs.enumerated().compactMap { i, g -> FilmSection? in
            guard let films = lists[i], !films.isEmpty else { return nil }
            return FilmSection(title: g.title, films: films)
        }
        if sections.isEmpty { throw AppError("Không tải được điểm IMDb (danh sách KKPhim)") }
        return FilmPage(films: [], nextUrl: nil, sections: sections)
    }

    /// Xếp một danh mục KKPhim theo IMDb: 5 trang đầu (≈ 320 phim mới cập nhật), phim từ 1000 lượt chấm.
    static func forList(_ url: String) async throws -> FilmPage {
        let key = "list|" + url.replacingOccurrences(of: #"[?&]page=\d+"#, with: "", options: .regularExpression)
        if let c = cached(key) { return FilmPage(films: c, nextUrl: nil) }
        let films = try await fetch((1...5).map { withPage(url, $0) })
        let ranked = rank(films, minVotes: 1000, top: 120)
        if !ranked.isEmpty { store(key, ranked) }
        return FilmPage(films: ranked, nextUrl: nil)
    }

    private static func group(_ g: Group) async throws -> [Film] {
        let key = "group|" + g.id
        if let c = cached(key) { return c }
        let films = try await fetch((1...g.pages).map { withPage("\(origin)/api/movies/\(g.path)", $0) })
        let ranked = rank(films, minVotes: g.minVotes, top: top)
        if !ranked.isEmpty { store(key, ranked) }
        return ranked
    }

    /// Tải các trang song song; trang lỗi thì bỏ, lỗi hết thì báo.
    private static func fetch(_ urls: [String]) async throws -> [Film] {
        let pages = await withTaskGroup(of: (Int, [Film]?).self) { tg -> [Int: [Film]] in
            for (i, u) in urls.enumerated() { tg.addTask { (i, try? await Site.list(u).films) } }
            var out: [Int: [Film]] = [:]
            for await (i, f) in tg { if let f = f { out[i] = f } }
            return out
        }
        if pages.isEmpty { throw AppError("Không tải được danh sách KKPhim") }
        return (0..<urls.count).compactMap { pages[$0] }.flatMap { $0 }.distinct { $0.url }
    }

    /// Trang thứ n, mỗi trang pageSize phim (thay page= / limit= có sẵn).
    static func withPage(_ url: String, _ page: Int) -> String {
        var u = url.replacingOccurrences(of: #"([?&])page=\d+"#, with: "$1page=\(page)", options: .regularExpression)
            .replacingOccurrences(of: #"([?&])limit=\d+"#, with: "$1limit=\(pageSize)", options: .regularExpression)
        if u.range(of: #"[?&]page="#, options: .regularExpression) == nil { u += (u.contains("?") ? "&" : "?") + "page=\(page)" }
        if u.range(of: #"[?&]limit="#, options: .regularExpression) == nil { u += "&limit=\(pageSize)" }
        return u
    }

    /// Xếp theo điểm có trọng số: chỉ phim từ minVotes lượt chấm; C = điểm trung bình của các phim đó; m = 5 lần minVotes (phim
    /// 9,4 điểm từ 1.200 lượt không vượt phim 8,6 điểm từ 2 triệu lượt). Bằng điểm thì phim nhiều lượt hơn trước.
    static func rank(_ films: [Film], minVotes: Int, top: Int) -> [Film] {
        let ok = films.filter { ($0.imdb?.votes ?? 0) >= minVotes }
        guard !ok.isEmpty else { return [] }
        let c = ok.reduce(0.0) { $0 + $1.imdb!.score } / Double(ok.count)
        let m = Double(minVotes) * 5
        func wr(_ f: Film) -> Double {
            let v = Double(f.imdb!.votes)
            return v / (v + m) * f.imdb!.score + m / (v + m) * c
        }
        return Array(ok.sorted { a, b in
            let x = wr(a), y = wr(b)
            return x != y ? x > y : a.imdb!.votes > b.imdb!.votes
        }.prefix(top))
    }

    // MARK: Lưu kết quả 12 giờ

    private static let lock = NSLock()
    private static var memory: [String: (Int64, [Film])] = [:]

    private static var file: URL {
        FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("imdb_top.json")
    }

    private static func nowMs() -> Int64 { Int64(Date().timeIntervalSince1970 * 1000) }

    private static func cached(_ key: String) -> [Film]? {
        lock.lock()
        defer { lock.unlock() }
        if let (t, f) = memory[key], nowMs() - t < ttl { return f }
        guard let d = try? Data(contentsOf: file), let root = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any],
              let o = root[key] as? [String: Any] else { return nil }
        let t = num64(o["t"])
        guard nowMs() - t < ttl else { return nil }
        let films = ((o["f"] as? [[String: Any]]) ?? []).compactMap { x -> Film? in
            guard let u = J.str(x, "u").nonEmpty else { return nil }
            let s = dbl(x["s"]) ?? 0
            return Film(url: u, title: J.str(x, "n"), poster: J.str(x, "p").nonEmpty, badge: J.str(x, "b").nonEmpty,
                        imdb: s > 0 ? Imdb(score: s, votes: Int(num64(x["v"]))) : nil)
        }
        memory[key] = (t, films)
        return films
    }

    private static func store(_ key: String, _ films: [Film]) {
        lock.lock()
        defer { lock.unlock() }
        let t = nowMs()
        memory[key] = (t, films)
        var root = ((try? Data(contentsOf: file)).flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]) ?? [:]
        root[key] = ["t": t, "f": films.map { f -> [String: Any] in
            ["u": f.url, "n": f.title, "p": f.poster ?? "", "b": f.badge ?? "", "s": f.imdb?.score ?? 0, "v": f.imdb?.votes ?? 0]
        }]
        if let d = try? JSONSerialization.data(withJSONObject: root) { try? d.write(to: file, options: .atomic) }
    }

    /// Xoá kết quả đã lưu (test).
    static func clear() {
        lock.lock()
        defer { lock.unlock() }
        memory = [:]
        try? FileManager.default.removeItem(at: file)
    }
}
