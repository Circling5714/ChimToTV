package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.util.Locale
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * Điểm IMDb của một phim — lấy thẳng từ dữ liệu KKPhim (phimapi.com có sẵn imdb.vote_average / vote_count, khoảng 60% phim
 * lẻ có điểm; hoạt hình Trung Quốc ít). Không gọi IMDb. votes < MIN_VOTES thì coi như chưa có điểm (10 điểm từ 1 lượt chấm).
 */
data class Imdb(val score: Double, val votes: Int) {
    /** Nhãn trên thẻ phim, vd "IMDb 8.5". */
    val label: String get() = "IMDb " + String.format(Locale.US, "%.1f", score)

    /** Dòng thông tin ở trang phim, vd "IMDb 8.5 (378 nghìn lượt chấm)". */
    val detail: String get() = "$label (${votesText(votes)} lượt chấm)"

    companion object {
        const val MIN_VOTES = 100

        /** Đối tượng "imdb" của KKPhim → Imdb (null = chưa có điểm / quá ít lượt). */
        fun of(o: JSONObject?): Imdb? {
            o ?: return null
            val s = o.optDouble("vote_average", 0.0)
            val v = o.optInt("vote_count", 0)
            return if (s > 0 && s <= 10 && v >= MIN_VOTES) Imdb(s, v) else null
        }

        fun votesText(v: Int): String = when {
            v >= 1_000_000 -> String.format(Locale.US, "%.1f", v / 1_000_000.0).replace('.', ',').removeSuffix(",0") + " triệu"
            v >= 1000 -> "${v / 1000} nghìn"
            else -> "$v"
        }
    }
}

/**
 * Danh mục «Điểm IMDb cao»: lấy vài trang đầu (phim mới cập nhật) của các danh sách KKPhim, xếp theo điểm IMDb có trọng số
 * (như bảng Top 250 của IMDb: WR = v/(v+m)·R + m/(v+m)·C — phim ít lượt chấm bị kéo về điểm trung bình C của nhóm; dưới số lượt
 * tối thiểu của nhóm thì bỏ, m = 5 lần số đó). API KKPhim không sắp theo điểm, kho phim quá lớn (≈ 26 nghìn) để TV tự quét hết →
 * chỉ xếp trong vài trăm phim mới nhất mỗi nhóm. Lưu kết quả 12 giờ (bộ nhớ + tệp trong cache).
 */
object ImdbTop {
    data class Group(val id: String, val title: String, val path: String, val pages: Int, val minVotes: Int, val kids: Boolean = false)

    /** Đường dẫn API kiểu KKPhim (www.kkphim.com/api/movies/… — ApiSite gọi thẳng phimapi.com). */
    val GROUPS = listOf(
        Group("le", "Phim lẻ", "category/phim-le", 8, 10_000),
        Group("bo", "Phim bộ", "category/phim-bo", 8, 2_000),
        Group("han", "Phim Hàn Quốc", "quoc-gia/han-quoc", 5, 1_000),
        Group("aumy", "Phim Âu Mỹ", "quoc-gia/au-my", 5, 5_000),
        Group("tq", "Phim Trung Quốc", "quoc-gia/trung-quoc", 5, 500),
        Group("hh", "Hoạt hình", "category/hoat-hinh", 6, 1_000, kids = true),
    )

    private const val ORIGIN = "https://www.kkphim.com"
    private const val PAGE_SIZE = 64
    private const val TOP = 60
    private const val TTL = 12 * 3600_000L

    /** Danh mục (của trang) xếp được theo IMDb: danh sách KKPhim (dữ liệu có điểm IMDb). */
    fun canRank(url: String): Boolean = Site.hostOf(url)?.let { it.endsWith(ApiSite.KK_HOST) || it.endsWith("phimapi.com") } == true &&
        "/api/" in url

    private val lock = Any()
    private val memory = HashMap<String, Pair<Long, List<Film>>>()

    @Volatile var cacheDir: File? = null

    /** Trang «Điểm IMDb cao»: mỗi nhóm một hàng (hồ sơ trẻ em: chỉ hoạt hình). */
    fun page(kids: Boolean): FilmPage {
        val groups = GROUPS.filter { !kids || it.kids }
        val pool = Executors.newFixedThreadPool(4)
        try {
            val lists = pool.invokeAll(groups.map { g -> Callable { runCatching { group(g) }.getOrNull() } }, 120, TimeUnit.SECONDS)
                .map { f -> runCatching { f.get() }.getOrNull() }
            val sections = groups.zip(lists).mapNotNull { (g, films) -> films?.takeIf { it.isNotEmpty() }?.let { Section(g.title, it) } }
            if (sections.isEmpty()) throw IOException("Không tải được điểm IMDb (danh sách KKPhim)")
            return FilmPage(emptyList(), null, sections)
        } finally {
            pool.shutdownNow()
        }
    }

    /** Xếp một danh mục KKPhim theo IMDb: 5 trang đầu (≈ 320 phim mới cập nhật), phim từ 1000 lượt chấm. */
    fun forList(url: String): FilmPage {
        val key = "list|" + url.replace(Regex("[?&]page=\\d+"), "")
        cached(key)?.let { return FilmPage(it, null) }
        val urls = (1..5).map { withPage(url, it) }
        val films = fetch(urls)
        val ranked = rank(films, 1000, 120)
        if (ranked.isNotEmpty()) store(key, ranked)
        return FilmPage(ranked, null)
    }

    private fun group(g: Group): List<Film> {
        val key = "group|" + g.id
        cached(key)?.let { return it }
        val films = fetch((1..g.pages).map { withPage("$ORIGIN/api/movies/${g.path}", it) })
        val ranked = rank(films, g.minVotes, TOP)
        if (ranked.isNotEmpty()) store(key, ranked)
        return ranked
    }

    /** Tải các trang song song (tối đa 4 cùng lúc); trang lỗi thì bỏ, lỗi hết thì báo. */
    private fun fetch(urls: List<String>): List<Film> {
        val pool = Executors.newFixedThreadPool(4)
        try {
            val pages = pool.invokeAll(urls.map { u -> Callable { runCatching { Site.list(u).films }.getOrNull() } }, 90, TimeUnit.SECONDS)
                .map { f -> runCatching { f.get() }.getOrNull() }
            if (pages.all { it == null }) throw IOException("Không tải được danh sách KKPhim")
            return pages.filterNotNull().flatten().distinctBy { it.url }
        } finally {
            pool.shutdownNow()
        }
    }

    /** Trang thứ n, mỗi trang PAGE_SIZE phim (thay page= / limit= có sẵn). */
    internal fun withPage(url: String, page: Int): String {
        var u = url.replace(Regex("([?&])page=\\d+"), "$1page=$page").replace(Regex("([?&])limit=\\d+"), "$1limit=$PAGE_SIZE")
        if (!Regex("[?&]page=").containsMatchIn(u)) u += (if ('?' in u) "&" else "?") + "page=$page"
        if (!Regex("[?&]limit=").containsMatchIn(u)) u += "&limit=$PAGE_SIZE"
        return u
    }

    /**
     * Xếp theo điểm có trọng số: chỉ phim từ minVotes lượt chấm; C = điểm trung bình của các phim đó; phim ít lượt bị kéo về C.
     * Bằng điểm thì phim nhiều lượt hơn trước.
     */
    internal fun rank(films: List<Film>, minVotes: Int, top: Int): List<Film> {
        val ok = films.filter { (it.imdb?.votes ?: 0) >= minVotes }
        if (ok.isEmpty()) return emptyList()
        val c = ok.sumOf { it.imdb!!.score } / ok.size
        // m = 5 lần số lượt tối thiểu: phim 9,4 điểm từ 1.200 lượt không vượt phim 8,6 điểm từ 2 triệu lượt
        val m = minVotes * 5.0
        fun wr(f: Film): Double {
            val v = f.imdb!!.votes.toDouble()
            return v / (v + m) * f.imdb.score + m / (v + m) * c
        }
        return ok.sortedWith(compareByDescending<Film> { wr(it) }.thenByDescending { it.imdb!!.votes }).take(top)
    }

    // ---- Lưu kết quả 12 giờ ----

    private fun cached(key: String): List<Film>? {
        synchronized(lock) {
            memory[key]?.let { (t, films) -> if (System.currentTimeMillis() - t < TTL) return films }
        }
        val f = file() ?: return null
        val o = runCatching { JSONObject(f.readText()) }.getOrNull()?.optJSONObject(key) ?: return null
        val t = o.optLong("t")
        if (System.currentTimeMillis() - t >= TTL) return null
        val films = filmsOf(o.optJSONArray("f") ?: return null)
        synchronized(lock) { memory[key] = t to films }
        return films
    }

    private fun store(key: String, films: List<Film>) {
        val t = System.currentTimeMillis()
        synchronized(lock) {
            memory[key] = t to films
            val f = file() ?: return
            val root = runCatching { JSONObject(f.readText()) }.getOrDefault(JSONObject())
            root.put(key, JSONObject().put("t", t).put("f", JSONArray(films.map { fl ->
                JSONObject().put("u", fl.url).put("n", fl.title).put("p", fl.poster ?: "").put("b", fl.badge ?: "")
                    .put("s", fl.imdb?.score ?: 0.0).put("v", fl.imdb?.votes ?: 0)
            })))
            runCatching { f.writeText(root.toString()) }
        }
    }

    private fun filmsOf(a: JSONArray): List<Film> = (0 until a.length()).mapNotNull { i ->
        val o = a.optJSONObject(i) ?: return@mapNotNull null
        val url = o.optString("u").ifEmpty { return@mapNotNull null }
        Film(url, o.optString("n"), o.optString("p").ifEmpty { null }, o.optString("b").ifEmpty { null },
            Imdb(o.optDouble("s"), o.optInt("v")).takeIf { it.score > 0 })
    }

    private fun file(): File? = cacheDir?.let { File(it, "imdb_top.json") }

    /** Xoá kết quả đã lưu (Cài đặt → xoá bộ nhớ đệm / test). */
    fun clear() {
        synchronized(lock) {
            memory.clear()
            file()?.delete()
        }
    }
}
