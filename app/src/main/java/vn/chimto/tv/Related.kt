package vn.chimto.tv

/**
 * Phim liên quan ở trang phim (từ 2.12; iPad: Related.swift, Windows: Related.cs):
 * - «Cùng bộ»: các phần / mùa khác của cùng bộ — tìm theo tên gốc (bỏ «Phần N / Season N / Mùa N», năm, «– 20 Tập», phần sau
 *   dấu «:») trên trang của phim + KKPhim, xếp theo số phần. Hoạt hình Trung Quốc rất nhiều phần nên đây là hàng đáng giá nhất.
 * - «Cùng thể loại»: KKPhim / VSMOV lọc theo loại + nước + thể loại đặc trưng nhất (như hàng «Giống …» của trang chủ); bỏ
 *   phim đang xem và các phần cùng bộ, phim chưa xem lên trước.
 * Không nguồn nào (KKPhim, VSMOV, NguonC, ophim) tìm được theo diễn viên / đạo diễn nên chưa có hàng đó.
 */
object Related {
    const val MAX = 12

    class Row(val id: String, val title: String, val films: List<Film>)

    private val PART = Regex("""(?i)(?:phần|phan|season|mùa|mua|part|ss)\s*0*(\d{1,3})""")
    private val EPISODES = Regex("""(?i)\s[–\-]\s*\d+\s*tập.*$""")
    private val YEAR = Regex("""\(\s*(?:19|20)\d{2}\s*\)|\b(?:19|20)\d{2}\b""")
    private val BRACKETS = Regex("""\([^)]*\)|\[[^\]]*]""")

    /** Tên để tìm (còn dấu): «Tiên Nghịch (Phần 2) 2024 – 20 Tập» → «Tiên Nghịch». */
    fun searchName(title: String): String {
        var t = EPISODES.replace(title, " ")
        t = PART.replace(t, " ")
        t = YEAR.replace(t, " ")
        t = BRACKETS.replace(t, " ")
        t = t.substringBefore(':').substringBefore(" - ")
        val out = t.replace(Regex("""\s+"""), " ").trim().trimEnd('-', '–', ' ')
        return out.ifEmpty { title.trim() }
    }

    /** Tên gốc để so cùng bộ (chuẩn hoá, bỏ số phần đứng cuối): «Đấu Phá Thương Khung 5» → «dau pha thuong khung». */
    fun base(title: String): String {
        val n = KkPhim.norm(searchName(title)).replace(Regex("""\s\d{1,2}$"""), "").trim()
        return n.ifEmpty { KkPhim.norm(title) }
    }

    /** Số phần: «Phần 5», «Season 2», «(Phần 01)», số cuối tên («Đấu Phá Thương Khung 5»); không có → null (thường là phần 1). */
    fun part(title: String): Int? {
        PART.find(title)?.let { return it.groupValues[1].toIntOrNull() }
        // «Đấu Phá Thương Khung 4: Đào Vong», «Thế Giới Hoàn Mỹ 3»: số đứng cuối phần tên trước dấu «:»
        val n = KkPhim.norm(EPISODES.replace(YEAR.replace(title.substringBefore(':'), " "), " "))
        return Regex("""\s(\d{1,2})$""").find(n)?.groupValues?.get(1)?.toIntOrNull()
    }

    /** Cùng bộ với phim đang xem: tên gốc trùng (đủ dài để khỏi trùng nhầm), không phải chính phim đó (cùng tên đầy đủ). */
    fun sameSeries(mine: String, other: String): Boolean {
        val b = base(mine)
        return b.length >= 4 && b == base(other) && KkPhim.norm(mine) != KkPhim.norm(other)
    }

    /**
     * Các phần khác của cùng bộ trong kết quả tìm (đã gộp các nguồn, nguồn trước được ưu tiên): bỏ phim trùng tên (cùng phim ở
     * nguồn khác), xếp theo số phần (không ghi số = phần 1; cùng số giữ thứ tự tìm được — vd phim lẻ ngoại truyện).
     */
    fun pickSeries(title: String, url: String, found: List<Film>): List<Film> {
        val seen = HashSet<String>()
        return found.filter { it.url != url && sameSeries(title, it.title) && seen.add(KkPhim.norm(it.title)) }
            .sortedBy { part(it.title) ?: 1 }
            .take(MAX)
    }

    /** Hàng «Cùng thể loại»: bỏ phim đang xem và các phần cùng bộ; phim chưa xem lên trước (giữ thứ tự của nguồn). */
    fun pickSimilar(title: String, url: String, found: List<Film>, watched: Set<String>): List<Film> {
        val seen = HashSet<String>()
        val films = found.filter { it.url != url && KkPhim.norm(it.title) != KkPhim.norm(title) && !sameSeries(title, it.title) }
            .filter { seen.add(KkPhim.norm(it.title)) }
        val (old, fresh) = films.partition { KkPhim.norm(it.title) in watched }
        return (fresh + old).take(MAX)
    }

    /** Gốc (https://tên-miền) của trang chứa phim. */
    private fun originOf(url: String) = Site.originOf(url) ?: url.substringBefore("/", url)

    /** Tải các hàng (chạy ở luồng nền; lỗi một nguồn thì bỏ nguồn đó). */
    fun load(title: String, url: String): List<Row> {
        val rows = ArrayList<Row>()
        val name = searchName(title)
        // Cùng bộ: trang của phim trước (cùng nguồn, mở nhanh), rồi KKPhim
        val src = HomeFeed.sources()
        val origins = listOfNotNull(originOf(url), src.kk).distinct()
        val found = origins.flatMap { o -> runCatching { Site.search(name, o).films }.getOrDefault(emptyList()) }
        val series = pickSeries(title, url, found)
        if (series.isNotEmpty()) rows += Row("series", "Cùng bộ «$name»", series)
        // Cùng thể loại (tra thể loại qua KKPhim theo tên, nhớ chung cả nhà)
        if (FilmTags.needsLookup(title)) runCatching { FilmTags.lookup(title) }
        val tags = FilmTags.of(title)
        val similar = tags?.let { HomeFeed.relatedList(it, Prefs.kids) }
        if (similar != null) {
            val list = runCatching { Site.list(similar.first).films }.getOrDefault(emptyList())
            val watched = Prefs.history.map { KkPhim.norm(it.title) }.toSet()
            val films = pickSimilar(title, url, list, watched).filterNot { f -> series.any { it.url == f.url } }
            if (films.isNotEmpty()) rows += Row("similar", "Cùng thể loại · ${similar.second}", films)
        }
        return rows
    }
}
