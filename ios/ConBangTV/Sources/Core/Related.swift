import Foundation

/// Phim liên quan ở trang phim (từ 1.12; bản TV: Related.kt, Windows: Related.cs):
/// - «Cùng bộ»: các phần / mùa khác của cùng bộ — tìm theo tên gốc (bỏ «Phần N / Season N / Mùa N», năm, «– 20 Tập», phần
///   sau dấu «:») trên trang của phim + KKPhim, xếp theo số phần.
/// - «Cùng thể loại»: KKPhim / VSMOV lọc theo loại + nước + thể loại đặc trưng nhất (như hàng «Giống …» của trang chủ); bỏ
///   phim đang xem và các phần cùng bộ, phim chưa xem lên trước.
/// Không nguồn nào (KKPhim, VSMOV, NguonC, ophim) tìm được theo diễn viên / đạo diễn nên chưa có hàng đó.
enum Related {
    static let max = 12

    struct Row: Identifiable {
        let id: String
        let title: String
        let films: [Film]
    }

    private static let partRe = try! NSRegularExpression(pattern: "(?:phần|phan|season|mùa|mua|part|ss)\\s*0*(\\d{1,3})", options: [.caseInsensitive])

    private static func replace(_ s: String, _ pattern: String, _ with: String = " ", _ opts: NSRegularExpression.Options = [.caseInsensitive]) -> String {
        guard let re = try? NSRegularExpression(pattern: pattern, options: opts) else { return s }
        return re.stringByReplacingMatches(in: s, range: NSRange(s.startIndex..., in: s), withTemplate: with)
    }

    private static func strip(_ title: String, keepPart: Bool) -> String {
        var t = replace(title, "\\s[–\\-]\\s*\\d+\\s*tập.*$")
        if !keepPart { t = partRe.stringByReplacingMatches(in: t, range: NSRange(t.startIndex..., in: t), withTemplate: " ") }
        t = replace(t, "\\(\\s*(?:19|20)\\d{2}\\s*\\)|\\b(?:19|20)\\d{2}\\b")
        return t
    }

    /// Tên để tìm (còn dấu): «Tiên Nghịch (Phần 2) 2024 – 20 Tập» → «Tiên Nghịch».
    static func searchName(_ title: String) -> String {
        var t = replace(strip(title, keepPart: false), "\\([^)]*\\)|\\[[^\\]]*\\]")
        t = String(t.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false).first ?? "")
        if let r = t.range(of: " - ") { t = String(t[..<r.lowerBound]) }
        let out = replace(t, "\\s+", " ").trimmingCharacters(in: CharacterSet(charactersIn: " -–"))
        return out.isEmpty ? title.trimmingCharacters(in: .whitespaces) : out
    }

    /// Tên gốc để so cùng bộ (chuẩn hoá, bỏ số phần đứng cuối): «Đấu Phá Thương Khung 5» → «dau pha thuong khung».
    static func base(_ title: String) -> String {
        let n = replace(KkPhim.norm(searchName(title)), "\\s\\d{1,2}$", "").trimmingCharacters(in: .whitespaces)
        return n.isEmpty ? KkPhim.norm(title) : n
    }

    /// Số phần: «Phần 5», «Season 2», «(Phần 01)», số cuối phần tên trước «:» («Đấu Phá Thương Khung 4: Đào Vong»); không có → nil.
    static func part(_ title: String) -> Int? {
        if let m = partRe.firstMatch(in: title, range: NSRange(title.startIndex..., in: title)),
           let r = Range(m.range(at: 1), in: title) { return Int(title[r]) }
        let head = String(title.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false).first ?? "")
        let n = KkPhim.norm(strip(head, keepPart: true))
        guard let re = try? NSRegularExpression(pattern: "\\s(\\d{1,2})$"),
              let m = re.firstMatch(in: n, range: NSRange(n.startIndex..., in: n)), let r = Range(m.range(at: 1), in: n) else { return nil }
        return Int(n[r])
    }

    /// Cùng bộ với phim đang xem: tên gốc trùng (đủ dài để khỏi trùng nhầm), không phải chính phim đó (cùng tên đầy đủ).
    static func sameSeries(_ mine: String, _ other: String) -> Bool {
        let b = base(mine)
        return b.count >= 4 && b == base(other) && KkPhim.norm(mine) != KkPhim.norm(other)
    }

    /// Các phần khác của cùng bộ (đã gộp các nguồn, nguồn trước được ưu tiên): bỏ phim trùng tên, xếp theo số phần (không ghi
    /// số = phần 1; cùng số giữ thứ tự tìm được).
    static func pickSeries(_ title: String, url: String, _ found: [Film]) -> [Film] {
        var seen = Set<String>()
        let films = found.filter { $0.url != url && sameSeries(title, $0.title) && seen.insert(KkPhim.norm($0.title)).inserted }
        return Array(films.enumerated().sorted { (part($0.element.title) ?? 1, $0.offset) < (part($1.element.title) ?? 1, $1.offset) }
            .map { $0.element }.prefix(max))
    }

    /// Hàng «Cùng thể loại»: bỏ phim đang xem và các phần cùng bộ; phim chưa xem lên trước (giữ thứ tự của nguồn).
    static func pickSimilar(_ title: String, url: String, _ found: [Film], watched: Set<String>) -> [Film] {
        var seen = Set<String>()
        let mine = KkPhim.norm(title)
        let films = found.filter { $0.url != url && KkPhim.norm($0.title) != mine && !sameSeries(title, $0.title) && seen.insert(KkPhim.norm($0.title)).inserted }
        let fresh = films.filter { !watched.contains(KkPhim.norm($0.title)) }
        let old = films.filter { watched.contains(KkPhim.norm($0.title)) }
        return Array((fresh + old).prefix(max))
    }

    /// Tải các hàng (lỗi một nguồn thì bỏ nguồn đó).
    static func load(title: String, url: String) async -> [Row] {
        var rows: [Row] = []
        let name = searchName(title)
        // Cùng bộ: trang của phim trước (cùng nguồn), rồi KKPhim
        var origins: [String] = []
        for o in [Site.originOf(url), HomeFeed.sources().kk].compactMap({ $0 }) where !origins.contains(o) { origins.append(o) }
        var found: [Film] = []
        for o in origins { found += (try? await Site.search(name, base: o).films) ?? [] }
        let series = pickSeries(title, url: url, found)
        if !series.isEmpty { rows.append(Row(id: "series", title: "Cùng bộ «\(name)»", films: series)) }
        // Cùng thể loại (tra thể loại qua KKPhim theo tên, nhớ chung cả nhà)
        if FilmTags.needsLookup(title) { try? await FilmTags.lookup(title) }
        if let tags = FilmTags.of(title), let sim = HomeFeed.relatedList(tags, kids: Prefs.kids) {
            let list = (try? await Site.list(sim.url).films) ?? []
            let watched = Set(Prefs.history.map { KkPhim.norm($0.title) })
            let films = pickSimilar(title, url: url, list, watched: watched).filter { f in !series.contains { $0.url == f.url } }
            if !films.isEmpty { rows.append(Row(id: "similar", title: "Cùng thể loại · \(sim.title)", films: films)) }
        }
        return rows
    }
}
