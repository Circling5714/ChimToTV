package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONObject
import java.net.URLEncoder
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.pow

/** Thẻ phim của trang chủ tổng hợp. links: các nguồn có phim này, nguồn mở trước đứng đầu. */
data class Card(val film: Film, val chip: String?, val links: List<FeedItem>)

/** Một hàng. more: địa chỉ danh sách đầy đủ (null = «Xem thêm» hiện mọi phim đã có trong cards). */
/** slot = loại hàng trong cài đặt của người xem (HomeFeed.ROWS: "similar", "interest", "newest"…; "topics" = theo chủ đề). */
data class FeedRow(val id: String, val title: String, val cards: List<Card>, val more: String? = null, val slot: String = "")

/** Phim đang xem dở vừa có tập mới (key: tên đã chuẩn hoá; ep: số tập mới nhất). */
data class FreshCard(val card: Card, val key: String, val ep: Double)

/**
 * Trang chủ tổng hợp: lập danh sách nguồn cần đọc theo người xem đang dùng, tải (Feed.load), gộp phim trùng rồi dựng các
 * hàng — Có tập mới (phim đang xem dở, mọi nguồn), Giống «phim vừa xem», Vì … hay xem <thể loại>, <loại phim hay xem> mới
 * cập nhật, Mới cập nhật · mọi nguồn, Phim lẻ mới. Người xem chưa có lịch sử: chọn chủ đề (Prefs.homeTopics).
 */
object HomeFeed {
    private const val DAY = 86_400_000L

    /** Phim xem trong 30 ngày qua mới được tự báo tập mới; tập mới phải ra trong 7 ngày qua. */
    private const val FRESH_WATCHED = 30 * DAY
    private const val FRESH_NEW = 7 * DAY

    /** Số phim tối đa mỗi hàng (giữ cho «Xem thêm»). */
    private const val ROW_MAX = 40

    enum class Role { WEB, NEW, SIMILAR, INTEREST, DOMINANT, DUB, KIDS, MOVIES, TOPIC, TAGS }

    /** Một phần cần tải: vai trò trong trang chủ + tiêu đề hàng / địa chỉ «Xem thêm» (nếu có). */
    class Part(
        val role: Role, val req: Feed.Req, val title: String? = null, val more: String? = null, val id: String = req.key,
        val slot: String = "",
    )

    /**
     * Các hàng người xem bật / tắt, sắp thứ tự được (Cài đặt → Trang chủ → Các hàng), theo thứ tự mặc định. «Có tập mới» /
     * «Xem tiếp» (HEAD_ROWS) chỉ bật / tắt — luôn ở đầu trang; hàng trẻ em (KIDS_ROWS) chỉ có ở hồ sơ trẻ em.
     */
    val ROWS = listOf(
        "new-eps" to "Có tập mới",
        "continue" to "Xem tiếp",
        "similar" to "Giống «phim vừa xem»",
        "interest" to "Vì … hay xem <thể loại> (sở thích 1)",
        "dominant" to "Loại phim hay xem · mới cập nhật",
        "dub" to "Thuyết minh / lồng tiếng mới",
        "kids-young" to "Hoạt hình thiếu nhi",
        "kids-family" to "Hoạt hình gia đình",
        "kids-dub" to "Hoạt hình lồng tiếng",
        "interest2" to "Vì … hay xem <thể loại> (sở thích 2)",
        "newest" to "Mới cập nhật · mọi nguồn",
        "movies" to "Phim lẻ mới",
    )
    val HEAD_ROWS = setOf("new-eps", "continue")
    val KIDS_ROWS = setOf("kids-young", "kids-family", "kids-dub")

    /** Hàng hồ sơ trẻ em không có (chỉ hoạt hình hợp trẻ em: không lấy danh sách chung / thuyết minh / phim lẻ). */
    val ADULT_ROWS = setOf("dominant", "dub", "newest", "movies")

    /**
     * Ghép thứ tự đã lưu ("id" bật, "-id" tắt) với các hàng mặc định: bỏ id lạ / trùng, hàng mặc định chưa có (hàng mới ở bản
     * sau) chèn ngay sau hàng đứng trước nó trong thứ tự mặc định.
     */
    fun mergeRows(saved: List<String>, defaults: List<String> = ROWS.map { it.first }): List<Pair<String, Boolean>> {
        val out = ArrayList<Pair<String, Boolean>>()
        for (s in saved) {
            val on = !s.startsWith("-")
            val id = s.removePrefix("-")
            if (id in defaults && out.none { it.first == id }) out += id to on
        }
        defaults.forEachIndexed { i, id ->
            if (out.any { it.first == id }) return@forEachIndexed
            val prev = defaults.subList(0, i).lastOrNull { p -> out.any { it.first == p } }
            val at = prev?.let { p -> out.indexOfFirst { it.first == p } + 1 } ?: 0
            out.add(at, id to true)
        }
        return out
    }

    /** Các hàng của người xem đang dùng (thứ tự, bật / tắt). */
    fun rowPrefs(): List<Pair<String, Boolean>> = mergeRows(Prefs.homeRows)

    fun rowOn(id: String): Boolean = rowPrefs().firstOrNull { it.first == id }?.second ?: true

    /** Loại hàng của một hàng lưu từ bản cũ (chưa ghi slot): theo id; hai hàng «Vì … hay xem» theo thứ tự xuất hiện. */
    private fun legacySlot(id: String, interestBefore: Int): String = when {
        id.startsWith("interest-") -> if (interestBefore == 0) "interest" else "interest2"
        id.startsWith("topic-") -> "topics"
        else -> id
    }

    /**
     * Xếp các hàng theo cài đặt của người xem: bỏ hàng đã tắt, theo thứ tự đã chọn; hàng theo chủ đề (người xem mới) luôn
     * đầu, hàng lạ (không có trong cài đặt) cuối.
     */
    fun arrange(rows: List<FeedRow>, prefs: List<Pair<String, Boolean>> = rowPrefs()): List<FeedRow> {
        val pos = HashMap<String, Int>()
        prefs.forEachIndexed { i, (id, on) -> pos[id] = if (on) i else -1 }
        var interest = 0
        val keyed = rows.map { r ->
            val slot = r.slot.ifEmpty { legacySlot(r.id, interest) }
            if (slot.startsWith("interest")) interest++
            r to slot
        }
        return keyed.filter { (_, s) -> pos[s] != -1 }
            .sortedBy { (_, s) -> if (s == "topics") -1 else pos[s] ?: Int.MAX_VALUE }
            .map { it.first }
    }

    /** Chủ đề cho người xem chưa có lịch sử (id lưu trong Prefs.homeTopics). */
    val TOPICS = listOf(
        "cn" to "Hoạt hình Trung Quốc", "kh" to "Kiếm hiệp", "kr" to "Phim Hàn", "mv" to "Phim lẻ Âu Mỹ",
        "hk" to "Phim xưa Hồng Kông", "kid" to "Hoạt hình cho bé", "jp" to "Anime Nhật", "tm" to "Thuyết minh / lồng tiếng",
    )

    /** Nguồn trong danh sách trang của người xem (gốc địa chỉ); web: trang web đang dùng + trang hay xem gần đây. */
    class Sources(val kk: String?, val nc: String?, val vs: String?, val k20: String?, val web: List<String>) {
        val count get() = listOfNotNull(kk, nc, vs, k20).size + web.size
    }

    fun sources(): Sources {
        val sites = Prefs.sites
        val base = Prefs.baseUrl
        val kk = sites.firstOrNull { ApiSite.isKk(it) }?.let(ApiSite::origin)
        val nc = sites.firstOrNull { NguonC.isSite(it) }?.let(ApiSite::origin)
        val vs = sites.firstOrNull { ApiSite.isVs(it) }?.let(ApiSite::origin)
        val k20 = sites.firstOrNull { Stremio.isSite(it) }?.let(ApiSite::origin)
        val api = listOfNotNull(kk, nc, vs, k20).mapNotNull { Site.hostOf(it) }.toSet()
        val now = System.currentTimeMillis()
        val recent = Prefs.history.filter { now - it.time < FRESH_WATCHED }.mapNotNull { Site.hostOf(it.filmUrl) }.distinct()
        // Trang web đọc HTML nặng với TV yếu: tối đa 2 trang (trang đang dùng + trang xem gần nhất)
        val web = (listOf(base) + recent.mapNotNull { h -> sites.firstOrNull { Site.hostOf(it) == h } })
            .filter { Site.hostOf(it) !in api && Site.hostOf(it) != null }.distinctBy { Site.hostOf(it) }.take(2)
        return Sources(kk, nc, vs, k20, web)
    }

    // ---- Sở thích ----

    /** Thể loại quá chung (không dùng để gợi ý); chinh-kich / tam-ly tính nửa điểm. */
    private val GENERIC = setOf("hoat-hinh", "phim-18", "tv-shows", "phim-bo", "phim-le")
    private val HALF = setOf("chinh-kich", "tam-ly")

    data class Combo(val type: String, val country: String, val genre: String)

    /** Điểm sở thích của một người xem (cộng dồn theo phim đã xem, phim xem gần đây tính nhiều hơn). */
    class Taste(
        val genre: Map<String, Double>, val combo: Map<Combo, Double>, val pair: Map<Pair<String, String>, Double>,
        val total: Double,
    ) {
        fun affinity(t: Tags?): Double {
            if (t == null) return 0.0
            val p = pair[(t.type ?: "") to (t.countries.firstOrNull() ?: "")] ?: 0.0
            return t.genres.sumOf { genre[it] ?: 0.0 } + p
        }
    }

    /**
     * Điểm của mỗi phim = số tập đã xem (1–20) × ½ mũ (số ngày từ lần xem cuối ÷ 14), cộng vào từng thể loại, cặp
     * (loại phim, nước) và bộ ba (loại, nước, thể loại). Phim chưa biết thể loại thì bỏ qua.
     */
    fun taste(history: List<HistoryItem>, tagsOf: (String) -> Tags?, watched: (String) -> Int, now: Long): Taste {
        val genre = HashMap<String, Double>()
        val combo = HashMap<Combo, Double>()
        val pair = HashMap<Pair<String, String>, Double>()
        var total = 0.0
        for (h in history) {
            val days = (now - h.time).coerceAtLeast(0) / DAY.toDouble()
            if (days > 120) continue
            val t = tagsOf(h.title) ?: continue
            val w = watched(h.filmUrl).coerceIn(1, 20) * 0.5.pow(days / 14)
            total += w
            val type = t.type.orEmpty()
            val country = t.countries.firstOrNull().orEmpty()
            pair.merge(type to country, w, Double::plus)
            for (g in t.genres) {
                if (g in GENERIC) continue
                val gw = if (g in HALF) w / 2 else w
                genre.merge(g, gw, Double::plus)
                if (type.isNotEmpty() && country.isNotEmpty()) combo.merge(Combo(type, country, g), gw, Double::plus)
            }
        }
        return Taste(genre, combo, pair, total)
    }

    /** Loại phim KKPhim → đường dẫn danh sách (category/…) / tên hiện. */
    private fun typeSlug(type: String) = when (type) {
        "hoathinh" -> "hoat-hinh"
        "series" -> "phim-bo"
        "single" -> "phim-le"
        "tvshows" -> "tv-shows"
        else -> null
    }

    fun typeName(type: String) = when (type) {
        "hoathinh" -> "hoạt hình"
        "series" -> "phim bộ"
        "single" -> "phim lẻ"
        "tvshows" -> "TV show"
        else -> "phim"
    }

    // ---- Lập danh sách cần tải ----

    /** Thể loại không hợp trẻ em: hồ sơ trẻ em bỏ các phim này (và mọi phim không phải hoạt hình). */
    private val ADULT = setOf("phim-18", "kinh-di", "hinh-su", "chien-tranh")

    /** Tên phim ma / kinh dị mà nguồn không gắn thể loại (vd «Horror Collector» trong mục trẻ em của KKPhim). */
    private val SCARY = Regex("(?i)horror|kinh dị|xác sống|zombie|ma quỷ|quỷ dữ|ác quỷ|kyoufu")

    /** pid: người xem lúc lập kế hoạch (lưu «trang chủ lần trước» đúng người); kids: hồ sơ trẻ em. */
    class Plan(
        val parts: List<Part>, val src: Sources, val similar: HistoryItem?, val similarTags: Tags?,
        val taste: Taste?, val topics: Set<String>, val pid: String = "", val kids: Boolean = false,
    )

    /** Lập kế hoạch tải cho người xem đang dùng (đọc Prefs — chạy ở luồng nền, không chạm mạng). */
    fun plan(tagsKnownOnly: Boolean = false): Plan {
        val s = sources()
        val kids = Prefs.kids
        // Hàng người xem đã tắt: khỏi tải (trang chủ nhanh hơn). «Mới cập nhật» vẫn tải danh sách mới của các nguồn — cần để
        // gộp phim và tìm «Có tập mới»
        val off = rowPrefs().filter { !it.second }.map { it.first }.toSet()
        val parts = ArrayList<Part>()
        val now = System.currentTimeMillis()
        val history = Prefs.history
        fun host(o: String) = Site.hostOf(o).orEmpty()
        val json = mapOf("Accept" to "application/json")

        // Hồ sơ trẻ em: không lấy danh sách «mới cập nhật» chung (đủ loại phim), chỉ các danh sách hoạt hình bên dưới
        if (!kids) {
            // Trang web (trang đang dùng trước): phim mới cập nhật + menu mục của trang
            val webParts = ArrayList<Part>()
            for (w in s.web) {
                webParts += Part(Role.WEB, Feed.Req("web|$w", host(w)) {
                    val h = Site.home(w)
                    Feed.Loaded(Feed.fromFilms(newestOf(h)).take(ROW_MAX), h.categories, h.movedTo)
                })
            }
            // Nguồn API (JSON nhẹ) chạy trước, trang web (đọc HTML nặng) sau, CLB Phim Xưa (máy chủ chậm) cuối
            s.kk?.let { o ->
                parts += Part(Role.NEW, Feed.Req("new|$o", host(o)) { Feed.Loaded(Feed.parseKk(ApiSite.json("$o/api/movies/newest?page=1&limit=40"), o)) })
            }
            s.nc?.let { o ->
                parts += Part(Role.NEW, Feed.Req("new|$o", host(o)) { Feed.Loaded(Feed.parseNc(Http.get(NguonC.newestUrl(o), json).body, o)) })
            }
            s.vs?.let { o ->
                parts += Part(Role.NEW, Feed.Req("new|$o", host(o)) { Feed.Loaded(Feed.parseKk(ApiSite.json("$o/api/danh-sach/phim-moi-cap-nhat?page=1"), o)) })
            }
            parts += webParts
            s.k20?.let { o ->
                parts += Part(Role.NEW, Feed.Req("new|$o", host(o)) { Feed.Loaded(Feed.fromFilms(Stremio.list(Stremio.newestUrl(o)).films)) })
            }
        }

        // Thể loại của phim đã xem chưa biết: tra KKPhim (mỗi lượt tối đa 6 phim, phim không có trên KKPhim 3 ngày sau mới tra lại)
        if (!tagsKnownOnly && (s.kk != null || Prefs.kkBackup)) {
            history.filter { now - it.time < 120 * DAY }.distinctBy { KkPhim.norm(it.title) }
                .filter { FilmTags.needsLookup(it.title) }.take(6).forEach { h ->
                    parts += Part(Role.TAGS, Feed.Req("tags|${KkPhim.norm(h.title)}", "phimapi.com", keep = false) {
                        FilmTags.lookup(h.title)
                        Feed.Loaded(emptyList())
                    })
                }
        }

        val counts = Prefs.watchedCounts()
        val taste = if (history.isEmpty()) null else taste(history, FilmTags::of, { counts[it] ?: 0 }, now)
        val topics = Prefs.homeTopics
        var similar: HistoryItem? = null
        var similarTags: Tags? = null
        var top: Pair<String, String>? = null
        if (taste != null && taste.total > 0) {
            // Giống «phim vừa xem»: cùng loại phim, nước và thể loại riêng nhất của phim đó (trẻ em: chỉ phim hoạt hình)
            similar = history.firstOrNull {
                FilmTags.of(it.title)?.let { t -> t.type != null && t.countries.isNotEmpty() && (!kids || t.type == "hoathinh") } == true
            }
            similarTags = similar?.let { FilmTags.of(it.title) }
            val used = HashSet<String>()
            if (similar != null && similarTags != null) {
                val g = similarTags.genres.filter { it !in GENERIC && !(kids && it in ADULT) }.sortedBy { if (it in HALF) 1 else 0 }.firstOrNull()
                val c = similarTags.countries.first()
                val url = filterUrl(s, similarTags.type!!, c, g)
                if (url != null) {
                    g?.let { used += it }
                    if ("similar" !in off) parts += Part(Role.SIMILAR, req(s, url), "Giống «${similar.title}»", url, "similar", "similar")
                }
            }
            // Vì … hay xem <thể loại>: hai bộ ba điểm cao nhất, khác thể loại với hàng trên
            val combos = taste.combo.entries.sortedByDescending { it.value }.map { it.key }
            var n = 0
            for (cb in combos) {
                if (n == 2) break
                // Thể loại quá chung (chính kịch, tâm lý) chỉ để tính điểm, không thành hàng riêng
                if (cb.genre in used || cb.genre in HALF) continue
                if (kids && (cb.type != "hoathinh" || cb.genre in ADULT)) continue
                val url = filterUrl(s, cb.type, cb.country, cb.genre) ?: continue
                used += cb.genre
                n++
                val slot = if (n == 1) "interest" else "interest2"
                if (slot in off) continue
                val title = "Vì ${Prefs.profile.name} hay xem ${ApiSite.genreName(cb.genre)} · ${typeName(cb.type)} ${ApiSite.countryName(cb.country)}"
                parts += Part(Role.INTEREST, req(s, url), title, url, "interest-${cb.genre}", slot)
            }
            // Loại phim + nước xem nhiều nhất (≥ 40%): hàng mới cập nhật riêng, gộp mọi nguồn
            val best = taste.pair.entries.filter { it.key.first.isNotEmpty() && it.key.second.isNotEmpty() }.maxByOrNull { it.value }
            if (best != null && best.value >= taste.total * 0.4) top = best.key
            if (top != null && !kids && "dominant" !in off) {
                val (type, country) = top
                val title = "${typeName(type).replaceFirstChar { it.uppercase() }} ${ApiSite.countryName(country)} mới cập nhật · mọi nguồn"
                // Một danh sách đã lọc (KKPhim / VSMOV); phim cùng tên ở trang web / NguonC / VSMOV (danh sách mới cập nhật)
                // tự gộp vào nên vẫn ghi đủ nguồn
                filterUrl(s, type, country, null)?.let { url -> parts += Part(Role.DOMINANT, req(s, url), title, null, "dominant", "dominant") }
            }
        } else if (history.isEmpty() && !kids) {
            for ((id, label) in TOPICS) {
                if (id !in topics) continue
                topicReq(s, id)?.let { (req, more) -> parts += Part(Role.TOPIC, req, "Vì bạn chọn: $label", more, "topic-$id", "topics") }
            }
        }
        if (kids) {
            parts += kidsParts(s).filter { it.slot !in off }
        } else {
            if ("dub" !in off) dubPart(s, top)?.let { parts += it }
            // Phim lẻ mới (xếp theo thể loại người xem hay xem)
            if ("movies" in off) {
                // người xem đã tắt hàng này
            } else if (s.kk != null) {
                val u = "${s.kk}/api/movies/category/phim-le?page=1&limit=30"
                parts += Part(Role.MOVIES, req(s, u), "Phim lẻ mới", u, "movies", "movies")
            } else if (s.vs != null) {
                val u = "${s.vs}/api/danh-sach/phim-le?page=1"
                parts += Part(Role.MOVIES, Feed.Req("list|$u", host(s.vs)) { Feed.Loaded(Feed.parseKk(ApiSite.json(u), s.vs)) }, "Phim lẻ mới", u, "movies", "movies")
            }
        }
        return Plan(parts, s, similar, similarTags, taste, topics, Prefs.profileId, kids)
    }

    /**
     * Người hay xem bản thuyết minh / lồng tiếng (từ nửa số lần mở tập gần đây, ít nhất 5 lần): hàng «Thuyết minh mới» theo
     * loại phim + nước hay xem (KKPhim lọc được ngôn ngữ), không có KKPhim thì danh sách ngôn ngữ của NguonC.
     */
    private fun dubPart(s: Sources, top: Pair<String, String>?): Part? {
        val langs = Prefs.watchLangs
        val tm = langs.count { it == "tm" }
        val lt = langs.count { it == "lt" }
        if (langs.size < 5 || (tm + lt) * 2 < langs.size) return null
        val slug = if (lt > tm) "long-tieng" else "thuyet-minh"
        val name = if (lt > tm) "Lồng tiếng" else "Thuyết minh"
        s.kk?.let { o ->
            val t = top?.first?.let(::typeSlug)
            val url = if (t != null) "$o/api/movies/category/$t?country=${top.second}&sort_lang=$slug&page=1&limit=30"
            else "$o/api/movies/category/phim-$slug?page=1&limit=30"
            val title = if (t != null) "$name mới · ${typeName(top.first)} ${ApiSite.countryName(top.second)}" else "Phim ${name.lowercase()} mới"
            return Part(Role.DUB, req(s, url), title, url, "dub")
        }
        s.nc?.let { o ->
            val url = "$o/api/films/ngon-ngu/$slug?page=1"
            return Part(Role.DUB, Feed.Req("list|$url", Site.hostOf(o).orEmpty()) {
                Feed.Loaded(Feed.parseNc(Http.get(url, mapOf("Accept" to "application/json")).body, o))
            }, "Phim ${name.lowercase()} mới", url, "dub")
        }
        return null
    }

    /** Hồ sơ trẻ em: hoạt hình thiếu nhi (KKPhim «trẻ em» + VSMOV «thiếu nhi»), hoạt hình gia đình, hoạt hình lồng tiếng. */
    private fun kidsParts(s: Sources): List<Part> {
        val out = ArrayList<Part>()
        s.kk?.let { o ->
            val u = "$o/api/movies/category/hoat-hinh?category=tre-em&page=1&limit=30"
            out += Part(Role.KIDS, req(s, u), "Hoạt hình thiếu nhi", u, "kids-young", "kids-young")
        }
        s.vs?.let { o ->
            val u = "$o/api/the-loai/thieu-nhi?page=1"
            out += Part(Role.KIDS, Feed.Req("list|$u", Site.hostOf(o).orEmpty()) { Feed.Loaded(Feed.parseKk(ApiSite.json(u), o)) },
                "Hoạt hình thiếu nhi", u, "kids-young", "kids-young")
        }
        s.kk?.let { o ->
            val fam = "$o/api/movies/category/hoat-hinh?category=gia-dinh&page=1&limit=30"
            out += Part(Role.KIDS, req(s, fam), "Hoạt hình gia đình", fam, "kids-family", "kids-family")
            val dub = "$o/api/movies/category/hoat-hinh?sort_lang=long-tieng&page=1&limit=30"
            out += Part(Role.KIDS, req(s, dub), "Hoạt hình lồng tiếng", dub, "kids-dub", "kids-dub")
        }
        return out
    }

    /** Danh sách lọc theo loại phim + nước (+ thể loại) — KKPhim lọc được cả ba; không có KKPhim thì VSMOV (thể loại + nước). */
    private fun filterUrl(s: Sources, type: String, country: String, genre: String?): String? {
        s.kk?.let { o ->
            val t = typeSlug(type) ?: return@let
            return "$o/api/movies/category/$t?country=$country" + (genre?.let { "&category=$it" } ?: "") + "&page=1&limit=30"
        }
        s.vs?.let { o ->
            val p = when {
                genre != null -> "the-loai/$genre?country=$country"
                type == "hoathinh" -> "the-loai/hoat-hinh?country=$country"
                else -> "quoc-gia/$country"
            }
            return page("$o/api/$p")
        }
        return null
    }

    /** Thêm page=1 (địa chỉ đã có / chưa có dấu ?). */
    private fun page(url: String) = url + (if ('?' in url) "&" else "?") + "page=1"

    /** Đọc một danh sách KKPhim / VSMOV theo địa chỉ (định dạng KKPhim). */
    private fun req(s: Sources, url: String): Feed.Req {
        val o = ApiSite.origin(url)
        return Feed.Req("list|$url", Site.hostOf(o).orEmpty()) { Feed.Loaded(Feed.parseKk(ApiSite.json(url), o)) }
    }

    /** Danh sách cho một chủ đề: nguồn đầu tiên có trong danh sách trang. Trả về (yêu cầu, địa chỉ «Xem thêm»). */
    private fun topicReq(s: Sources, id: String): Pair<Feed.Req, String?>? {
        fun kk(p: String) = s.kk?.let { page("$it/api/movies/$p") + "&limit=30" }
        fun vs(p: String) = s.vs?.let { page("$it/api/$p") }
        val url = when (id) {
            "cn" -> kk("category/hoat-hinh?country=trung-quoc") ?: vs("the-loai/hoat-hinh?country=trung-quoc")
            "kh" -> vs("the-loai/kiem-hiep") ?: s.k20?.let { Stremio.catalog(it, genre = "Kiếm Hiệp") }
            "kr" -> kk("category/phim-bo?country=han-quoc") ?: vs("quoc-gia/han-quoc")
            "mv" -> kk("category/phim-le?country=au-my") ?: vs("danh-sach/phim-le")
            "hk" -> s.k20?.let { Stremio.newestUrl(it) } ?: kk("quoc-gia/hong-kong")
            "kid" -> vs("the-loai/thieu-nhi") ?: kk("category/hoat-hinh?category=gia-dinh")
            "jp" -> kk("category/hoat-hinh?country=nhat-ban") ?: vs("the-loai/hoat-hinh?country=nhat-ban")
            "tm" -> kk("category/phim-thuyet-minh") ?: s.nc?.let { "$it/api/films/ngon-ngu/thuyet-minh?page=1" }
            else -> null
        } ?: return null
        val host = Site.hostOf(url).orEmpty()
        val o = ApiSite.origin(url)
        val req = Feed.Req("list|$url", host) {
            Feed.Loaded(
                when {
                    Stremio.isSite(url) -> Feed.fromFilms(Stremio.list(url).films)
                    NguonC.isSite(url) -> Feed.parseNc(Http.get(url, mapOf("Accept" to "application/json")).body, o)
                    else -> Feed.parseKk(ApiSite.json(url), o)
                }
            )
        }
        return req to url
    }

    /** Khối «mới cập nhật» của trang web (không có thì khối lớn nhất / mọi khối). */
    private fun newestOf(h: Home): List<Film> {
        val fresh = h.sections.filter { Regex("(?i)mới|cập nhật|update|new").containsMatchIn(it.title) }
        val pick = fresh.ifEmpty { h.sections.filter { it.films.size >= 6 }.take(1) }.ifEmpty { h.sections }
        return pick.flatMap { it.films }.distinctBy { it.url }
    }

    // ---- Dựng trang ----

    /**
     * Một lần dựng trang chủ. note: dòng trạng thái; noteGaveUp: dòng trạng thái khi thôi chờ nguồn chậm; pending: còn nguồn
     * chậm (quá 6 giây / chưa tới lượt) có thể về muộn — late() dựng lại khi chúng về. results / plan / startedAt: để dựng lại.
     */
    class Snapshot(
        val rows: List<FeedRow>, val fresh: List<FreshCard>, val categories: List<Category>?, val movedTo: Pair<String, String>?,
        val note: String, val noteGaveUp: String, val failed: List<String>, val empty: Boolean, val fromCache: Boolean,
        val pending: Boolean = false, val results: List<Pair<Part, Feed.Result>> = emptyList(), val plan: Plan? = null,
        val startedAt: Long = 0,
    )

    // ---- Trang chủ lần trước (mở app là hiện ngay) ----

    private fun lastKey(pid: String) = "last|$pid"

    /** Lưu lần dựng vừa xong của người xem pid (một tệp nhỏ: các hàng, phim có tập mới, menu). */
    fun saveLast(pid: String, s: Snapshot) {
        if (s.fromCache || s.empty) return
        val o = JSONObject().put("at", System.currentTimeMillis())
            .put("rows", JSONArray(s.rows.map { r ->
                JSONObject().put("id", r.id).put("t", r.title).put("m", r.more ?: "").put("s", r.slot)
                    .put("c", JSONArray(r.cards.map(::cardJson)))
            }))
            .put("fresh", JSONArray(s.fresh.map { JSONObject().put("c", cardJson(it.card)).put("k", it.key).put("e", it.ep) }))
        s.categories?.let { cats -> o.put("cats", JSONArray(cats.map { JSONObject().put("t", it.title).put("u", it.url) })) }
        Feed.writeText(lastKey(pid), o.toString())
    }

    /** Trang chủ lần trước của người xem pid (dưới 24 giờ); null nếu chưa có. */
    fun last(pid: String): Snapshot? {
        val o = runCatching { JSONObject(Feed.readText(lastKey(pid)) ?: return null) }.getOrNull() ?: return null
        val at = o.optLong("at")
        if (System.currentTimeMillis() - at > Feed.CACHE_MAX) return null
        fun arr(k: String) = o.optJSONArray(k) ?: JSONArray()
        val rows = arr("rows").let { a ->
            (0 until a.length()).mapNotNull { i ->
                val r = a.optJSONObject(i) ?: return@mapNotNull null
                val c = r.optJSONArray("c") ?: JSONArray()
                FeedRow(r.optString("id"), r.optString("t"), (0 until c.length()).mapNotNull { j -> c.optJSONObject(j)?.let(::cardOf) },
                    r.optString("m").ifEmpty { null }, r.optString("s"))
            }
        }
        val fresh = arr("fresh").let { a ->
            (0 until a.length()).mapNotNull { i ->
                val f = a.optJSONObject(i) ?: return@mapNotNull null
                FreshCard(cardOf(f.optJSONObject("c") ?: return@mapNotNull null) ?: return@mapNotNull null, f.optString("k"), f.optDouble("e"))
            }
        }
        val cats = o.optJSONArray("cats")?.let { c ->
            (0 until c.length()).mapNotNull { i -> c.optJSONObject(i)?.let { Category(it.optString("t"), it.optString("u")) } }
        }
        val time = SimpleDateFormat("HH:mm", Locale.US).format(Date(at))
        val note = "Bản lưu lúc $time · đang làm mới…"
        return Snapshot(rows, fresh, cats, null, note, note, emptyList(), rows.isEmpty() && fresh.isEmpty(), fromCache = true)
    }

    private fun cardJson(c: Card) = JSONObject().put("u", c.film.url).put("n", c.film.title).put("p", c.film.poster ?: "")
        .put("b", c.film.badge ?: "").put("x", c.chip ?: "")
        .put("l", JSONArray(c.links.map { it.copy(names = emptySet()).json() }))

    private fun cardOf(o: JSONObject): Card? {
        val url = o.optString("u").ifEmpty { return null }
        val l = o.optJSONArray("l") ?: JSONArray()
        return Card(
            Film(url, o.optString("n"), o.optString("p").ifEmpty { null }, o.optString("b").ifEmpty { null }),
            o.optString("x").ifEmpty { null }, (0 until l.length()).mapNotNull { l.optJSONObject(it)?.let(FeedItem::of) },
        )
    }

    /** Nguồn chậm (quá hạn / chưa tới lượt) — khác nguồn lỗi hẳn (lỗi mạng, lỗi HTTP): có thể về muộn. */
    private fun slow(r: Feed.Result) = r.state != Feed.State.OK && !r.resting && (r.error == null || r.error.startsWith("quá"))

    /**
     * Nguồn chậm của lần dựng trước đã về (bản lưu mới hơn lúc bắt đầu tải): dựng lại với dữ liệu mới; chưa có gì mới thì null.
     */
    fun late(s: Snapshot): Snapshot? {
        val plan = s.plan ?: return null
        var changed = false
        val res = s.results.map { (p, r) ->
            if (p.role == Role.TAGS || !slow(r) || !p.req.keep) return@map p to r
            val c = Feed.readCache(p.req.key)
            if (c == null || c.second < s.startedAt) return@map p to r
            changed = true
            p to Feed.Result(p.req, Feed.State.OK, c.first.items, c.first.categories, null, false, r.ms, c.second)
        }
        if (!changed) return null
        return build(plan, res, fromCache = false, startedAt = s.startedAt).also { saveLast(plan.pid, it) }
    }

    /** Bản lưu của mọi phần (hiện ngay lúc mở app); null nếu chưa có bản lưu nào. */
    fun cached(plan: Plan): Snapshot? {
        val results = plan.parts.map { p ->
            val c = Feed.readCache(p.req.key)
            p to if (c != null) Feed.Result(p.req, Feed.State.CACHED, c.first.items, c.first.categories, null, false, 0, c.second)
            else Feed.Result(p.req, Feed.State.NO_DATA, emptyList(), null, null, false, 0, 0)
        }
        if (results.none { it.first.role != Role.TAGS && it.second.items.isNotEmpty() }) return null
        return build(plan, results, fromCache = true)
    }

    /**
     * Tải mọi phần rồi dựng trang (luồng nền). Lượt đầu chưa biết thể loại phim đã xem: tra xong thì lập lại kế hoạch và tải
     * thêm các hàng gợi ý.
     */
    fun load(plan: Plan): Snapshot {
        val started = System.currentTimeMillis()
        var p = plan
        var results = p.parts.zip(Feed.load(p.parts.map { it.req }))
        if (p.parts.any { it.role == Role.TAGS }) {
            val again = plan(tagsKnownOnly = true)
            val have = results.map { it.first.req.key }.toSet()
            val extra = again.parts.filter { it.req.key !in have }
            if (extra.isNotEmpty()) results = results + extra.zip(Feed.load(extra.map { it.req }))
            // Thứ tự / tiêu đề theo kế hoạch mới (có hàng gợi ý); phần đã tải thì dùng lại
            val byKey = results.associateBy { it.first.req.key }
            results = again.parts.mapNotNull { a -> byKey[a.req.key]?.let { a to it.second } } +
                results.filter { it.first.role == Role.TAGS }
            p = again
        }
        return build(p, results, fromCache = false, startedAt = started).also { saveLast(p.pid, it) }
    }

    /** Dựng các hàng từ kết quả đã tải (hàm thuần trừ phần đọc Prefs — xem buildRows). */
    fun build(plan: Plan, results: List<Pair<Part, Feed.Result>>, fromCache: Boolean, startedAt: Long = 0): Snapshot {
        val now = System.currentTimeMillis()
        val history = Prefs.history
        val ctx = Ctx(
            name = Prefs.profile.name, history = history, follows = Prefs.follows.map { it.filmUrl }.toSet(),
            seen = Prefs::seenEp, base = Site.hostOf(Prefs.baseUrl).orEmpty(), sites = Prefs.sites, now = now,
        )
        val rows = buildRows(plan, results, ctx)
        val web = results.firstOrNull { it.first.role == Role.WEB && it.first.req.key == "web|${Prefs.baseUrl}" }?.second
        val cats = web?.categories ?: Site.menu(Prefs.baseUrl)
        // Trang web đã chuyển tên miền (theo chuyển hướng): (gốc cũ, gốc mới)
        val moved = results.firstNotNullOfOrNull { (p, r) ->
            if (p.role == Role.WEB && r.state == Feed.State.OK) r.movedTo?.let { p.req.key.removePrefix("web|") to it } else null
        }
        val content = results.filter { it.first.role != Role.TAGS && !fromCache }
        fun names(l: List<Pair<Part, Feed.Result>>) = l.map { it.first.req.host }.distinct().joinToString(", ") { siteTag(it, ctx.sites) }
        // Nguồn chậm (có thể về muộn) / lỗi hẳn / đang tạm nghỉ (lỗi nhiều lần) — ghi riêng
        val slowParts = content.filter { slow(it.second) }
        val failedParts = content.filter { it.second.state != Feed.State.OK && !slow(it.second) && !it.second.resting }
        val restParts = content.filter { it.second.resting }
        val time = SimpleDateFormat("HH:mm", Locale.US).format(Date(results.maxOfOrNull { it.second.savedAt } ?: now))
        val tail = (if (failedParts.isNotEmpty()) " · lỗi: ${names(failedParts)}" else "") +
            (if (restParts.isNotEmpty()) " · tạm nghỉ: ${names(restParts)}" else "")
        val note = when {
            fromCache -> "Bản lưu lúc $time · đang làm mới…"
            slowParts.isNotEmpty() -> "Cập nhật lúc $time · đang chờ ${names(slowParts)}…$tail"
            else -> "Cập nhật lúc $time$tail"
        }
        val gaveUp = "Cập nhật lúc $time" + (if (slowParts.isNotEmpty()) " · chậm, dùng bản lưu: ${names(slowParts)}" else "") + tail
        val empty = rows.rows.isEmpty() && rows.fresh.isEmpty()
        return Snapshot(
            rows.rows, rows.fresh, cats, moved, note, gaveUp, (failedParts + restParts).map { it.first.req.host }.distinct(),
            empty, fromCache, pending = slowParts.isNotEmpty(), results = results, plan = plan, startedAt = startedAt,
        )
    }

    /** Dữ liệu người xem dùng khi dựng trang (tách khỏi Prefs để test). */
    class Ctx(
        val name: String, val history: List<HistoryItem>, val follows: Set<String>, val seen: (String) -> Double?,
        val base: String, val sites: List<String>, val now: Long,
    )

    class Built(val rows: List<FeedRow>, val fresh: List<FreshCard>)

    fun buildRows(plan: Plan, results: List<Pair<Part, Feed.Result>>, ctx: Ctx): Built {
        val content = results.filter { it.first.role != Role.TAGS }
        // Gộp mọi danh sách (trang web đang dùng trước, rồi các nguồn API) — một phim chỉ thuộc một nhóm
        val ordered = content.sortedBy { it.first.role.ordinal }
        val merged = Feed.merge(ordered.map { it.second.items }, ctx.now)
        val groupOf = HashMap<String, FeedFilm>()
        merged.forEach { f -> f.items.forEach { groupOf[it.url] = f } }
        val down = content.filter { it.second.state != Feed.State.OK }.map { it.first.req.host }.toSet()

        // Thứ tự nguồn khi mở phim: trang đang dùng, trang hay xem gần đây, rồi KKPhim / NguonC / VSMOV / CLB Phim Xưa
        val prefer = (listOf(ctx.base) + ctx.history.mapNotNull { Site.hostOf(it.filmUrl) } +
            listOfNotNull(plan.src.kk, plan.src.nc, plan.src.vs, plan.src.k20).mapNotNull { Site.hostOf(it) }).distinct()
        // Phim đã xem: theo địa chỉ phim hoặc tên (tra sẵn một lần cho mỗi phim)
        val histUrl = HashMap<String, HistoryItem>()
        val histName = HashMap<String, HistoryItem>()
        for (h in ctx.history) {
            histUrl.putIfAbsent(h.filmUrl, h)
            KkPhim.names(JSONObject().put("name", h.title)).forEach { histName.putIfAbsent(it, h) }
        }
        val historyMemo = java.util.IdentityHashMap<FeedFilm, Any>()
        fun historyOf(f: FeedFilm): HistoryItem? = historyMemo.getOrPut(f) {
            f.items.firstNotNullOfOrNull { histUrl[it.url] } ?: f.names.firstNotNullOfOrNull { histName[it] } ?: Unit
        } as? HistoryItem
        fun watched(f: FeedFilm) = historyOf(f) != null
        val tags = HashMap<String, String>()
        fun tag(host: String) = tags.getOrPut(host) { siteTag(host, ctx.sites) }

        fun card(f: FeedFilm): Card {
            val own = historyOf(f)?.let { Site.hostOf(it.filmUrl) }
            val o = f.ordered(listOfNotNull(own) + prefer)
            val live = o.items.filter { it.host !in down }
            val first = live.firstOrNull() ?: o.items.first()
            val links = listOf(first) + o.items.filter { it !== first }
            val badge = f.latest?.ep?.let { "Tập ${num(it)}" } ?: first.badge
            val chip = when {
                live.isEmpty() -> "bản lưu"
                live.size > 1 -> "${live.size} nguồn"
                else -> tag(first.host)
            }
            return Card(Film(first.url, first.title, links.firstNotNullOfOrNull { it.poster }, badge), chip, links)
        }

        val rows = ArrayList<FeedRow>()
        val claimed = HashSet<FeedFilm>()
        fun films(part: Part, r: Feed.Result) = r.items.mapNotNull { groupOf[it.url] }.distinct()
        // Hồ sơ trẻ em: chỉ phim hoạt hình, không có thể loại không hợp trẻ em (phim chưa rõ thể loại — vd danh sách «thiếu
        // nhi» của VSMOV — giữ lại vì đã đến từ danh sách dành cho trẻ em)
        fun kidsOk(f: FeedFilm): Boolean = !plan.kids || (f.tags.let { t ->
            t == null || ((t.type == null || t.type == "hoathinh") && t.genres.none { it in ADULT })
        } && f.items.none { SCARY.containsMatchIn(it.title) })
        fun add(slot: String, id: String, title: String, list: List<FeedFilm>, more: String?, claim: Boolean, min: Int = 3) {
            val l = list.filter { it !in claimed && kidsOk(it) }.take(ROW_MAX)
            if (l.size < min) return
            if (claim) claimed += l.take(6)
            rows += FeedRow(id, title, l.map(::card), more, slot)
        }
        val byRole = content.groupBy { it.first.role }

        // Có tập mới: phim đang xem dở (xem trong 30 ngày, đã xem kịp tập mới nhất lúc đó) vừa có tập mới ở bất kỳ nguồn nào
        val fresh = ArrayList<FreshCard>()
        val byKey = ctx.history.groupBy { KkPhim.norm(it.title) }
        for ((key, same) in byKey) {
            val h = same.first() // xem gần nhất
            if (ctx.now - h.time > FRESH_WATCHED || h.filmUrl in ctx.follows) continue
            val watchedEp = same.mapNotNull { Feed.epOf(it.epTitle) }.maxOrNull() ?: continue
            val f = merged.firstOrNull { g -> g.items.any { it.url == h.filmUrl } || key in g.names } ?: continue
            val latest = f.latest ?: continue
            val ep = latest.ep ?: continue
            if (latest.updated < ctx.now - FRESH_NEW) continue
            val seen = ctx.seen(key)
            if (seen != null && watchedEp < seen - 1) continue // chưa xem kịp (vd phim cũ đang xem dở) — không phải «tập mới»
            val known = maxOf(watchedEp, seen ?: watchedEp)
            if (ep <= known) continue
            // Xem từ trước 2.8 (chưa biết lúc đó phim có bao nhiêu tập): chỉ coi là «tập mới» khi số tập ra thêm hợp với thời
            // gian từ lần xem cuối (≤ 2 tập + 3 tập mỗi tuần) — đang xem tập 31 mà phim đã tới 108 là đang xem dở, không phải mới
            if (seen == null && ep - watchedEp > 2 + ((ctx.now - h.time) / DAY.toDouble() / 7 * 3).toInt()) continue
            val ownHost = Site.hostOf(h.filmUrl).orEmpty()
            val badge = if (ep - known >= 2) "+${num(ep - known)} · Tập ${num(ep)}" else "Tập ${num(ep)} mới"
            // Tập mới chỉ có ở nguồn khác: ghi ở góc trên ("có ở nguonc"), mở thẳng nguồn đó. Nguồn đó đang lỗi (đọc bản lưu):
            // vẫn báo, ghi «lần kiểm trước»
            val chip = when {
                latest.host in down -> "lần kiểm trước"
                latest.host != ownHost -> "có ở ${siteTag(latest.host, ctx.sites)}"
                else -> null
            }
            val open = if (latest.host == ownHost) f.items.firstOrNull { it.url == h.filmUrl } ?: latest else latest
            val links = listOf(open) + f.items.filter { it !== open }
            fresh += FreshCard(Card(Film(open.url, h.title, h.poster ?: open.poster, badge), chip, links), key, ep)
        }

        // Người xem chưa có lịch sử: hàng theo chủ đề đã chọn
        byRole[Role.TOPIC]?.forEach { (p, r) -> add("topics", p.id, p.title!!, films(p, r).sortedByDescending { it.updated }, p.more, claim = true) }

        // Giống «phim vừa xem»: xếp theo số thể loại chung
        val simTags = plan.similarTags
        byRole[Role.SIMILAR]?.forEach { (p, r) ->
            val me = plan.similar?.let { s -> KkPhim.names(JSONObject().put("name", s.title)) }.orEmpty()
            val list = films(p, r).filterNot { watched(it) || it.matches(me) }
                .sortedWith(compareByDescending<FeedFilm> { f -> f.tags?.genres?.count { it in simTags?.genres.orEmpty() } ?: 0 }.thenByDescending { it.updated })
            add("similar", p.id, p.title!!, list, p.more, claim = true, min = 4)
        }
        val interest = byRole[Role.INTEREST].orEmpty()
        fun interestRow(slot: String) = interest.firstOrNull { it.first.slot == slot }?.let { (p, r) ->
            val list = films(p, r).filterNot(::watched)
                .sortedWith(compareByDescending<FeedFilm> { plan.taste?.affinity(it.tags) ?: 0.0 }.thenByDescending { it.updated })
            add(slot, p.id, p.title!!, list, p.more, claim = true)
        }
        interestRow("interest")

        // Loại phim hay xem, mới cập nhật · mọi nguồn: phim của KKPhim / VSMOV đã lọc + nguồn khác gộp được với chúng
        val dominant = byRole[Role.DOMINANT].orEmpty()
        if (dominant.isNotEmpty()) {
            val set = LinkedHashSet<FeedFilm>()
            dominant.forEach { (p, r) -> set += films(p, r) }
            add("dominant", "dominant", dominant.first().first.title!!, set.filterNot(::watched).sortedByDescending { it.updated }, null, claim = true)
        }
        // Thuyết minh / lồng tiếng mới (người hay xem bản thuyết minh / lồng tiếng)
        byRole[Role.DUB]?.forEach { (p, r) ->
            add("dub", p.id, p.title!!, films(p, r).filterNot(::watched).sortedByDescending { it.updated }, p.more, claim = true)
        }
        // Hồ sơ trẻ em: hoạt hình thiếu nhi (KKPhim + VSMOV gộp một hàng), gia đình, lồng tiếng
        byRole[Role.KIDS]?.groupBy { it.first.id }?.forEach { (id, ps) ->
            val set = LinkedHashSet<FeedFilm>()
            ps.forEach { (p, r) -> set += films(p, r) }
            add(id, id, ps.first().first.title!!, set.filterNot(::watched).sortedByDescending { it.updated },
                ps.firstNotNullOfOrNull { it.first.more }, claim = true)
        }
        interestRow("interest2")

        // Mới cập nhật · mọi nguồn (trang web + KKPhim / NguonC / VSMOV / CLB Phim Xưa)
        val newest = LinkedHashSet<FeedFilm>()
        (byRole[Role.WEB].orEmpty() + byRole[Role.NEW].orEmpty()).forEach { (p, r) -> newest += films(p, r) }
        add("newest", "newest", "Mới cập nhật · mọi nguồn", newest.filterNot(::watched).sortedByDescending { it.updated }, null, claim = false)

        byRole[Role.MOVIES]?.forEach { (p, r) ->
            val list = films(p, r).filterNot(::watched)
                .sortedWith(compareByDescending<FeedFilm> { plan.taste?.affinity(it.tags) ?: 0.0 }.thenByDescending { it.updated })
            add("movies", p.id, p.title!!, list, p.more, claim = false)
        }
        // Thứ tự / bật tắt theo cài đặt của người xem (Cài đặt → Trang chủ → Các hàng)
        return Built(arrange(rows), fresh)
    }

    /**
     * Người xem vừa mở trang phim / một tập: ghi tập mới nhất đã thấy của phim (theo tên — chung mọi nguồn), hết báo «tập
     * mới» tới khi có tập sau. Xem riêng thì không ghi.
     */
    fun markSeen(d: FilmDetail) {
        if (Prefs.privateMode) return
        val n = d.groups.flatMap { it.episodes }.mapNotNull { Feed.epOf(it.title) }.maxOrNull() ?: return
        Prefs.setSeenEp(KkPhim.norm(d.title), n)
    }

    /** 108.0 → "108", 10.5 → "10.5". */
    private fun num(d: Double) = if (d == Math.floor(d)) d.toLong().toString() else d.toString()
}

/**
 * Thể loại / quốc gia / loại phim của các phim đã xem (tra KKPhim theo tên, lưu chung cả nhà): dùng để tính sở thích.
 * Khoá = tên đã chuẩn hoá; {g, c, t, at} hoặc {miss: lúc tra không thấy}.
 */
object FilmTags {
    private const val MAX = 400
    private const val MISS_RETRY = 3 * 86_400_000L

    /** Bản đọc sẵn trong bộ nhớ (đọc JSON mỗi lần tra thì TV yếu chậm); ghi thì cập nhật cả hai. */
    private var mem: JSONObject? = null
    private var memOf: String? = null

    @Synchronized
    private fun store(): JSONObject {
        val raw = Prefs.filmTags
        if (raw != memOf || mem == null) {
            mem = runCatching { JSONObject(raw) }.getOrDefault(JSONObject())
            memOf = raw
        }
        return mem!!
    }

    @Synchronized
    fun of(title: String): Tags? = store().optJSONObject(KkPhim.norm(title))?.takeIf { !it.has("miss") }?.let(Tags::of)

    @Synchronized
    fun needsLookup(title: String): Boolean {
        val e = store().optJSONObject(KkPhim.norm(title)) ?: return true
        return e.has("miss") && System.currentTimeMillis() - e.optLong("miss") > MISS_RETRY
    }

    @Synchronized
    fun remember(key: String, tags: Tags?) {
        if (key.isEmpty()) return
        val s = JSONObject(store().toString())
        s.put(key, tags?.json()?.put("at", System.currentTimeMillis()) ?: JSONObject().put("miss", System.currentTimeMillis()))
        val keys = s.keys().asSequence().toList()
        if (keys.size > MAX) {
            keys.sortedBy { k -> s.optJSONObject(k)?.let { it.optLong("at", it.optLong("miss")) } ?: 0 }
                .take(keys.size - MAX).forEach { s.remove(it) }
        }
        val raw = s.toString()
        Prefs.filmTags = raw
        mem = s
        memOf = raw
    }

    /** Tra KKPhim theo tên (lỗi mạng → ngoại lệ, không ghi gì; không thấy → ghi «không có»). */
    fun lookup(title: String) {
        val q = KkPhim.API + "/v1/api/tim-kiem?keyword=" + URLEncoder.encode(title.take(80), "UTF-8") + "&limit=10"
        val data = JSONObject(Http.get(q, mapOf("Accept" to "application/json")).body).optJSONObject("data")
        val a = data?.optJSONArray("items") ?: JSONArray()
        val items = (0 until a.length()).mapNotNull { a.optJSONObject(it) }
        val hit = KkPhim.choose(items, KkPhim.names(JSONObject().put("name", title)))
        remember(KkPhim.norm(title), hit?.let(Tags::fromKk))
    }
}
