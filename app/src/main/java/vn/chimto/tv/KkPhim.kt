package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONObject
import java.net.URI
import java.net.URLEncoder
import java.text.Normalizer
import java.util.concurrent.FutureTask
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/**
 * Server dự phòng KKPhim (API công khai phimapi.com) cho các trang tự có trình phát: yanhh3d, hoathinh3d.ee, hhkungfu
 * nhúng chung streamfree.vip — hỏng là hỏng cả ba. Mở một tập thì tìm đúng phim đó trên KKPhim (slug của trang, không
 * khớp thì tìm theo tên; tên phim hoặc tên gốc phải trùng), lấy tập cùng số rồi thêm server "KKPhim Vietsub / Thuyết
 * Minh" (luồng HLS — app tự phát bằng hls.html, bỏ quảng cáo chèn) vào menu ▼. Trang chia bản (Việt Sub / Thuyết Minh)
 * thì chỉ thêm đúng ngôn ngữ của bản đang xem. Trang lỗi hẳn mà KKPhim có tập này thì vẫn xem được bằng KKPhim.
 *
 * Trang đã có sẵn nguồn KKPhim (mephim — ApiSite, phimmoi — NextSite) không thêm.
 */
object KkPhim {
    const val API = "https://phimapi.com"

    /** Đầu tên loại server KKPhim, vd "kk-vs", "kk-tm" (điểm server / server quen lưu theo tên này). */
    const val PREFIX = "kk-"

    private const val TTL = 10 * 60_000L // chi tiết phim (danh sách tập) giữ 10 phút
    private const val MISS_TTL = 30 * 60_000L // phim không có trên KKPhim: 30 phút sau mới hỏi lại

    private class Entry(val at: Long, val detail: JSONObject?)

    private val cache = object : LinkedHashMap<String, Entry>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Entry>) = size > 40
    }

    /** Phim của trang → slug trên KKPhim đã tìm thấy (khỏi tìm lại theo tên). */
    private val slugs = HashMap<String, String>()

    /** Trang dùng nguồn dự phòng: bật trong Cài đặt và không phải trang đã có sẵn KKPhim. */
    fun appliesTo(url: String) = Prefs.kkBackup && !ApiSite.isKnown(url) && !NextSite.isKnown(url) &&
        Site.hostOf(url)?.let { it.endsWith("phimapi.com") || it.endsWith("kkphim.com") } != true

    /**
     * Trang tập + server KKPhim, tải song song. KKPhim chậm / lỗi thì thôi (chờ thêm tối đa vài giây, không báo lỗi).
     * Trang tập lỗi mà KKPhim có tập này → chỉ phát bằng KKPhim.
     */
    fun source(d: FilmDetail, group: EpisodeGroup?, ep: Episode): EpisodeSource =
        if (!appliesTo(d.url)) Site.source(ep.url) else combine(ep.url, { Site.source(ep.url) }, { servers(d, group, ep) })

    /**
     * Chạy song song trang tập (site) và tìm KKPhim (kk). Trang xong thì chờ KKPhim thêm tới [waitMs] tính từ lúc bắt
     * đầu; trang lỗi thì chờ tới [failWaitMs] (lúc đó KKPhim là đường duy nhất). KKPhim lỗi = không có server dự phòng.
     */
    fun combine(
        epUrl: String, site: () -> EpisodeSource, kk: () -> List<Pair<Server, Video>>,
        waitMs: Long = 6_000, failWaitMs: Long = 15_000,
    ): EpisodeSource {
        val start = System.currentTimeMillis()
        val task = FutureTask { runCatching(kk).getOrDefault(emptyList()) }
        thread(isDaemon = true, name = "kkphim") { task.run() }
        val src = runCatching(site)
        val wait = (if (src.isSuccess) waitMs else failWaitMs) - (System.currentTimeMillis() - start)
        val extra = runCatching { task.get(wait.coerceAtLeast(1), TimeUnit.MILLISECONDS) }.getOrDefault(emptyList())
        src.onSuccess { return attach(it, extra) }
        if (extra.isEmpty()) throw src.exceptionOrNull()!!
        return EpisodeSource(epUrl, extra.map { it.first }, "", "", "", null, null, false, extra.associate { it.first.type to it.second })
    }

    /**
     * Thêm server KKPhim sau các server của trang. Trang mở thẳng trang tập (pageMode, vd hoathinh3d.pw) thì trình phát của
     * trang thành một server ("Trình phát của trang") để chọn qua lại được.
     */
    fun attach(src: EpisodeSource, extra: List<Pair<Server, Video>>): EpisodeSource {
        if (extra.isEmpty() || src.next) return src
        val own = if (src.pageMode) listOf(Server("Trình phát của trang", PAGE_SERVER)) else src.servers
        return src.copy(servers = own + extra.map { it.first }, links = src.links + extra.associate { it.first.type to it.second })
    }

    /** Server KKPhim cho một tập (rỗng nếu KKPhim không có phim / tập này). Lỗi mạng → ngoại lệ. */
    fun servers(d: FilmDetail, group: EpisodeGroup?, ep: Episode): List<Pair<Server, Video>> {
        val num = Site.epNumber(ep.title) ?: Site.epNumber(ep.url.trimEnd('/').substringAfterLast('/')) ?: return emptyList()
        val detail = detail(d) ?: return emptyList()
        return pick(detail, num, group?.name?.let { PlayStats.lang(it) })
    }

    /**
     * Server có tập số [num] trong chi tiết phim KKPhim ({movie, episodes:[{server_name, is_ai, server_data:[{name, slug,
     * link_m3u8}]}]}). lang: ngôn ngữ của bản đang xem ("vs" / "tm" / "lt"; null = trang không chia bản → mọi ngôn ngữ).
     */
    fun pick(root: JSONObject, num: Double, lang: String?): List<Pair<Server, Video>> {
        val out = ArrayList<Pair<Server, Video>>()
        val used = HashSet<String>()
        for (s in objects(root.optJSONArray("episodes"))) {
            val l = PlayStats.lang(str(s, "server_name")) ?: "vs"
            if (lang != null && l != lang) continue
            val e = objects(s.optJSONArray("server_data"))
                .firstOrNull { (Site.epNumber(str(it, "name")) ?: Site.epNumber(str(it, "slug"))) == num } ?: continue
            val m3u8 = str(e, "link_m3u8").takeIf { it.startsWith("http") } ?: continue
            val ai = s.optBoolean("is_ai")
            val base = l + if (ai) "-ai" else ""
            var type = PREFIX + base
            var n = 1
            while (!used.add(type)) type = PREFIX + base + "-" + ++n
            val name = when (l) {
                "tm" -> "Thuyết Minh"
                "lt" -> "Lồng Tiếng"
                else -> "Vietsub"
            }
            out += Server("KKPhim $name" + (if (ai) " (AI)" else "") + (if (n > 1) " $n" else ""), type) to Video(m3u8)
        }
        return out
    }

    // ---- Tìm phim trên KKPhim ----

    private fun detail(d: FilmDetail): JSONObject? {
        val now = System.currentTimeMillis()
        synchronized(cache) { cache[d.url] }?.let { if (now - it.at < if (it.detail == null) MISS_TTL else TTL) return it.detail }
        val found = find(d)
        synchronized(cache) { cache[d.url] = Entry(now, found) }
        return found
    }

    /** Chi tiết phim KKPhim khớp với phim của trang; null = KKPhim không có. */
    private fun find(d: FilmDetail): JSONObject? {
        val mine = listOfNotNull(d.title, d.otherTitle).map(::norm).filter { it.isNotEmpty() }.toSet()
        if (mine.isEmpty()) return null
        synchronized(slugs) { slugs[d.url] }?.let { s -> movie(s)?.let { return it } }
        // Slug của trang thường trùng KKPhim (tien-nghich, dau-pha-thuong-khung-phan-5…); chỉ nhận phim hoạt hình ở bước này
        val own = slugOf(d.url)
        if (own != null) {
            movie(own)?.takeIf { r -> r.optJSONObject("movie")?.let { str(it, "type") == "hoathinh" && matches(it, mine) } == true }
                ?.let { remember(d.url, own); return it }
        }
        val q = "$API/v1/api/tim-kiem?keyword=" + URLEncoder.encode(d.title.take(80), "UTF-8") + "&limit=20"
        val items = objects(JSONObject(get(q)).optJSONObject("data")?.optJSONArray("items"))
        val hit = choose(items, mine) ?: return null
        val slug = str(hit, "slug")
        return movie(slug)?.also { remember(d.url, slug) }
    }

    /** Kết quả tìm khớp tên (phim hoạt hình trước, cùng trang tìm kiếm của KKPhim có thể ra phim người đóng trùng tên). */
    fun choose(items: List<JSONObject>, mine: Set<String>): JSONObject? =
        items.filter { matches(it, mine) }.minByOrNull { if (str(it, "type") == "hoathinh") 0 else 1 }

    private fun remember(film: String, slug: String) = synchronized(slugs) { slugs[film] = slug }

    /** Chi tiết phim trên KKPhim; null nếu không có slug này (KKPhim trả 404). */
    private fun movie(slug: String): JSONObject? {
        val o = try {
            JSONObject(get("$API/phim/" + URLEncoder.encode(slug, "UTF-8")))
        } catch (e: Http.HttpError) {
            if (e.code == 404) return null else throw e
        }
        return o.takeIf { it.optBoolean("status") && it.optJSONObject("movie") != null }
    }

    private fun get(url: String) = Http.get(url, mapOf("Accept" to "application/json")).body

    /** Slug của phim trong địa chỉ trang: ".../tu-tien/tien-nghich" → "tien-nghich". */
    fun slugOf(url: String): String? = runCatching { URI(url).path }.getOrNull()?.trimEnd('/')?.substringAfterLast('/')
        ?.removeSuffix(".html")?.takeIf { Regex("""[a-z0-9-]+""").matches(it) }

    fun matches(m: JSONObject, mine: Set<String>) = names(m).any { it in mine }

    /**
     * Các tên của một phim KKPhim (đã chuẩn hoá): tên, tên gốc; tên có phần trong ngoặc thì thêm phần ngoài / trong ngoặc
     * ("Thần Y Cổ Đại Ở Đô Thị (Đô Thị Cổ Tiên Y)") — trừ khi trong ngoặc chỉ là số phần ("Đấu Phá Thương Khung (Phần 5)").
     */
    fun names(m: JSONObject): Set<String> {
        val out = HashSet<String>()
        for (raw in listOf(str(m, "name"), str(m, "origin_name"))) {
            out += norm(raw)
            Regex("""^(.+?)\s*\((.+)\)\s*$""").find(raw)?.let { p ->
                val inner = norm(p.groupValues[2])
                if (!inner.matches(Regex("""\d+"""))) {
                    out += norm(p.groupValues[1])
                    out += inner
                }
            }
        }
        return out - ""
    }

    /**
     * Tên để so: bỏ dấu, chữ thường, chỉ chữ + số; "Phần 5" / "Season 5" / "SS5" → "5", nên "Đấu Phá Thương Khung Phần 5"
     * = "Đấu Phá Thương Khung (Phần 5)", "Sword of Coming SS2" = "Sword of Coming (Season 2)".
     */
    fun norm(s: String): String {
        val t = Normalizer.normalize(s.replace('đ', 'd').replace('Đ', 'D'), Normalizer.Form.NFD)
            .replace(Regex("\\p{M}+"), "").lowercase()
            .replace(Regex("[^a-z0-9]+"), " ")
            .replace(Regex("""\b(phan|season|mua|part|ss)\s*(\d+)"""), "$2")
        return t.trim().replace(Regex("\\s+"), " ")
    }

    private fun objects(a: JSONArray?): List<JSONObject> =
        if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }

    private fun str(o: JSONObject, key: String): String = (o.opt(key) as? String)?.trim().orEmpty()
}
