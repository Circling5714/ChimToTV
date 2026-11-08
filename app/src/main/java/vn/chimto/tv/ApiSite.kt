package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONObject
import org.jsoup.Jsoup
import java.io.IOException
import java.net.URI
import java.net.URL
import java.net.URLEncoder
import java.util.concurrent.ConcurrentHashMap
import kotlin.concurrent.thread

/**
 * Trang phim dựng bằng React (trang trắng + JS, vd mephim.ddns.net) có API JSON riêng ở /api/movies/… (nguồn KKPhim,
 * định dạng của phimapi.com). Không đọc HTML: danh sách, chi tiết, tập và link video (.m3u8 — app tự phát, bỏ quảng cáo;
 * hoặc trình phát nhúng của KKPhim) đều lấy từ API.
 *
 * Địa chỉ trong app theo đường dẫn của chính trang (mở web vẫn đúng chỗ): phim = /phim/<slug>, tập =
 * /phim/<slug>/<tập>?sv=<số thứ tự bản> (Vietsub / Thuyết Minh cùng tên tập nên phải ghi bản). Danh sách = địa chỉ API.
 */
object ApiSite {
    /**
     * Trang KKPhim (www.kkphim.com, từ 2.6): không có API /api/movies riêng — mọi dữ liệu gọi thẳng API công khai
     * phimapi.com (cùng định dạng, qua fallback). Địa chỉ phim / tập trong app vẫn theo tên miền kkphim.com.
     */
    const val KK_HOST = "kkphim.com"

    /** Trang đã biết là kiểu này (có sẵn + trang dò thấy lúc chạy, xem probe). */
    private val known = ConcurrentHashMap.newKeySet<String>().apply { add("mephim.ddns.net"); add(KK_HOST) }
    private val notApi = ConcurrentHashMap.newKeySet<String>()
    private const val CDN = "https://phimimg.com"

    /** Menu cố định (API không có danh sách mục): hoạt hình Trung Quốc lên đầu như các trang khác của app. */
    private val MENU = listOf(
        "Hoạt hình Trung Quốc" to "category/hoat-hinh?country=trung-quoc",
        "Hoạt hình" to "category/hoat-hinh",
        "Phim bộ" to "category/phim-bo",
        "Phim lẻ" to "category/phim-le",
        "TV Shows" to "category/tv-shows",
        "Phim thuyết minh" to "category/phim-thuyet-minh",
        "Phim lồng tiếng" to "category/phim-long-tieng",
        "Phim Trung Quốc" to "quoc-gia/trung-quoc",
        "Phim Hàn Quốc" to "quoc-gia/han-quoc",
        "Phim Âu Mỹ" to "quoc-gia/au-my",
        "Phim Nhật Bản" to "quoc-gia/nhat-ban",
        "Phim Thái Lan" to "quoc-gia/thai-lan",
    )

    /** Thể loại / quốc gia cho mục "Thể loại…" / "Quốc gia…" (slug theo KKPhim). */
    private val GENRES = listOf(
        "Hành Động" to "hanh-dong", "Cổ Trang" to "co-trang", "Viễn Tưởng" to "vien-tuong", "Kinh Dị" to "kinh-di",
        "Tình Cảm" to "tinh-cam", "Hài Hước" to "hai-huoc", "Võ Thuật" to "vo-thuat", "Tâm Lý" to "tam-ly",
        "Phiêu Lưu" to "phieu-luu", "Hình Sự" to "hinh-su", "Chiến Tranh" to "chien-tranh", "Thần Thoại" to "than-thoai",
        "Bí Ẩn" to "bi-an", "Học Đường" to "hoc-duong", "Gia Đình" to "gia-dinh", "Thể Thao" to "the-thao",
        "Âm Nhạc" to "am-nhac", "Chính Kịch" to "chinh-kich", "Khoa Học" to "khoa-hoc", "Tài Liệu" to "tai-lieu",
        "Lịch Sử" to "lich-su", "Kinh Điển" to "kinh-dien", "Phim 18+" to "phim-18",
    )
    private val COUNTRIES = listOf(
        "Trung Quốc" to "trung-quoc", "Hàn Quốc" to "han-quoc", "Nhật Bản" to "nhat-ban", "Âu Mỹ" to "au-my",
        "Thái Lan" to "thai-lan", "Đài Loan" to "dai-loan", "Hồng Kông" to "hong-kong", "Ấn Độ" to "an-do",
        "Việt Nam" to "viet-nam", "Anh" to "anh", "Pháp" to "phap", "Canada" to "canada", "Philippines" to "philippines",
        "Indonesia" to "indonesia", "Malaysia" to "malaysia", "Nga" to "nga", "Đức" to "duc", "Tây Ban Nha" to "tay-ban-nha",
        "Thổ Nhĩ Kỳ" to "tho-nhi-ky", "Úc" to "uc", "Quốc Gia Khác" to "quoc-gia-khac",
    )
    private const val LIMIT = 24

    /** Trang KKPhim (gọi thẳng phimapi.com) thêm vài mục theo các danh sách riêng của KKPhim. */
    private val KK_MENU = MENU.take(1) + listOf(
        "Hoạt hình Trung Quốc thuyết minh" to "category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh",
    ) + MENU.drop(1) + listOf("Phim chiếu rạp" to "category/phim-chieu-rap")

    fun isKnown(url: String) = Site.hostOf(url) in known

    fun isKk(url: String) = Site.hostOf(url) == KK_HOST

    /**
     * Trang chưa biết (người dùng vừa thêm) mà HTML là trang trắng của ứng dụng JS: thử API (một lần mỗi trang) —
     * trả về true nếu là kiểu trang này.
     */
    fun probe(base: String): Boolean {
        val host = Site.hostOf(base) ?: return false
        if (host in known) return true
        if (host in notApi) return false
        val ok = runCatching { JSONObject(get("${origin(base)}/api/movies/newest?page=1&limit=1")).optJSONArray("items") != null }
            .getOrDefault(false)
        (if (ok) known else notApi) += host
        return ok
    }

    // ---- Trang chủ / danh sách ----

    /** Trang chủ: các khối tải song song (TV mạng chậm đỡ chờ); khối nào lỗi thì bỏ. */
    fun home(base: String): Home {
        val o = origin(base)
        val kk = isKk(base)
        val blocks = if (kk) {
            // KKPhim không có khối thịnh hành / song ngữ: thêm hoạt hình thuyết minh, phim bộ, phim lẻ mới
            listOf(
                "Hoạt hình Trung Quốc mới cập nhật" to api(o, "category/hoat-hinh?country=trung-quoc", 1),
                "Hoạt hình Trung Quốc thuyết minh" to api(o, "category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh", 1),
                "Phim mới cập nhật" to newestUrl(o),
                "Phim bộ mới" to api(o, "category/phim-bo", 1),
                "Phim lẻ mới" to api(o, "category/phim-le", 1),
            )
        } else {
            listOf(
                "Hoạt hình Trung Quốc mới cập nhật" to api(o, "category/hoat-hinh?country=trung-quoc", 1),
                "Phim mới cập nhật" to newestUrl(o),
                "Thịnh hành" to "$o/api/movies/trending",
                "Song ngữ / Thuyết minh" to "$o/api/movies/song-ngu",
            )
        }
        val found = arrayOfNulls<Section>(blocks.size)
        var error: Throwable? = null
        blocks.mapIndexed { i, (title, url) ->
            thread {
                runCatching { parseItems(get(url), o) }
                    .onSuccess { if (it.isNotEmpty()) found[i] = Section(title, it) }
                    .onFailure { error = it }
            }
        }.forEach { it.join() }
        val sections = found.filterNotNull()
        if (sections.isEmpty()) throw (error as? IOException ?: IOException("Không tải được danh sách phim", error))
        return Home(sections, (if (kk) KK_MENU else MENU).map { (t, p) -> Category(t, api(o, p, 1)) }, null)
    }

    fun list(url: String): FilmPage = parseList(get(url), url)

    /** Danh sách có phân trang (data.params.pagination) → trang sau = page + 1. */
    fun parseList(body: String, url: String): FilmPage {
        val o = origin(url)
        val films = parseItems(body, o)
        val root = JSONObject(body)
        val p = (root.optJSONObject("data") ?: root).optJSONObject("params")?.optJSONObject("pagination")
        val page = p?.optInt("currentPage") ?: 0
        val pages = p?.optInt("totalPages") ?: 0
        val next = if (films.isNotEmpty() && page in 1 until pages) withPage(url, page + 1) else null
        return FilmPage(films, next)
    }

    /** Phim mới cập nhật (đo nhanh trang trong Cài đặt). */
    fun newestUrl(base: String) = "${origin(base)}/api/movies/newest?page=1&limit=$LIMIT"

    fun searchUrl(base: String, query: String) =
        "${origin(base)}/api/movies/search?keyword=" + URLEncoder.encode(query.trim(), "UTF-8") + "&page=1&limit=$LIMIT"

    fun filters(base: String): Pair<List<Category>, List<Category>> {
        val o = origin(base)
        return GENRES.map { (n, s) -> Category(n, api(o, "the-loai/$s", 1)) } to
            COUNTRIES.map { (n, s) -> Category(n, api(o, "quoc-gia/$s", 1)) }
    }

    /** Phim trong một phản hồi danh sách (items ở gốc hoặc trong data). */
    fun parseItems(body: String, origin: String): List<Film> {
        val root = JSONObject(body)
        val data = root.optJSONObject("data") ?: root
        val cdn = str(data, "APP_DOMAIN_CDN_IMAGE").ifEmpty { str(root, "cdnDomain") }.ifEmpty { CDN }
        val items = data.optJSONArray("items") ?: return emptyList()
        return objects(items).mapNotNull { film(it, origin, cdn) }.distinctBy { it.url }
    }

    private fun film(o: JSONObject, origin: String, cdn: String): Film? {
        val slug = str(o, "slug").ifEmpty { return null }
        val name = str(o, "name").ifEmpty { return null }
        val poster = image(str(o, "poster_url").ifEmpty { str(o, "thumb_url") }, str(o, "cdnDomain").ifEmpty { cdn })
        return Film("$origin/phim/$slug", name, poster, NextSite.badge(str(o, "episode_current")))
    }

    // ---- Chi tiết phim ----

    fun detail(url: String): FilmDetail = parseDetail(get(detailApi(url)), filmUrl(url))

    fun parseDetail(body: String, url: String): FilmDetail {
        val root = JSONObject(body)
        val m = root.optJSONObject("movie") ?: throw IOException("Không đọc được thông tin phim")
        val desc = str(m, "content").takeIf { it.isNotEmpty() }?.let { Jsoup.parse(it).text().trim() }?.ifEmpty { null }
        val info = listOf(
            str(m, "episode_current"),
            str(m, "time"),
            str(m, "lang"),
            names(m.optJSONArray("country")),
            m.opt("year")?.toString()?.takeIf { it.isNotEmpty() && it != "0" && it != "null" }.orEmpty(),
            names(m.optJSONArray("category")),
        ).filter { it.isNotEmpty() }.joinToString("  •  ").ifEmpty { null }
        val groups = objects(root.optJSONArray("episodes")).mapIndexedNotNull { i, s ->
            val eps = objects(s.optJSONArray("server_data")).mapNotNull { e ->
                val slug = str(e, "slug").ifEmpty { return@mapNotNull null }
                if (str(e, "link_m3u8").isEmpty() && str(e, "link_embed").isEmpty()) return@mapNotNull null
                Episode(title(str(e, "name").ifEmpty { slug }), "$url/$slug?sv=$i")
            }.distinctBy { it.url }
            if (eps.isEmpty()) null else EpisodeGroup(str(s, "server_name").ifEmpty { "Bản ${i + 1}" }, eps)
        }
        return FilmDetail(
            url, str(m, "name").ifEmpty { url }, str(m, "origin_name").ifEmpty { null },
            image(str(m, "poster_url").ifEmpty { str(m, "thumb_url") }, CDN), desc, info, groups,
        )
    }

    /** "Tập 01" → "Tập 1"; "Full" giữ nguyên. */
    private fun title(name: String): String =
        Regex("""^(Tập\s*)0+(\d)""", RegexOption.IGNORE_CASE).replace(name.trim()) { it.groupValues[1] + it.groupValues[2] }

    // ---- Tập phim ----

    /**
     * Hai server cho mỗi tập: luồng HLS (app tự phát, bỏ quảng cáo chèn giữa phim) và trình phát nhúng của KKPhim (dự
     * phòng). Link đọc sẵn từ chi tiết phim (đã có trong bộ nhớ đệm của Site khi mở từ trang phim).
     */
    fun source(epUrl: String): EpisodeSource {
        val film = filmUrl(epUrl)
        val body = get(detailApi(film))
        val root = JSONObject(body)
        val sv = Regex("""[?&]sv=(\d+)""").find(epUrl)?.groupValues?.get(1)?.toIntOrNull() ?: 0
        val tap = URI(epUrl).path.trimEnd('/').substringAfterLast('/')
        val servers = objects(root.optJSONArray("episodes"))
        val e = (listOfNotNull(servers.getOrNull(sv)) + servers).firstNotNullOfOrNull { s ->
            objects(s.optJSONArray("server_data")).firstOrNull { str(it, "slug") == tap }
        } ?: throw IOException("Không tìm thấy tập này trên trang")
        val links = LinkedHashMap<String, Video>()
        str(e, "link_m3u8").takeIf { it.startsWith("http") }?.let { links["m3u8"] = Video(it) }
        str(e, "link_embed").takeIf { it.startsWith("http") }?.let { links["embed"] = Video(it) }
        if (links.isEmpty()) throw IOException("Tập này chưa có video")
        val list = links.keys.map { if (it == "m3u8") Server("HLS (bỏ quảng cáo)", it) else Server("Trình phát KKPhim", it) }
        return EpisodeSource(epUrl, list, "", tap, sv.toString(), null, null, false, links, next = true)
    }

    // ---- Tiện ích ----

    /** Địa chỉ phim từ địa chỉ phim / tập: ".../phim/tien-nghich/tap-01?sv=1" → ".../phim/tien-nghich". */
    fun filmUrl(url: String): String {
        val u = URI(url)
        val parts = u.path.trim('/').split('/')
        val i = parts.indexOf("phim")
        val slug = parts.getOrNull(i + 1)?.takeIf { i >= 0 } ?: throw IOException("Địa chỉ phim lạ: $url")
        return "${origin(url)}/phim/$slug"
    }

    private fun detailApi(url: String) = "${origin(url)}/api/movies/detail/" + filmUrl(url).substringAfterLast('/')

    private fun api(origin: String, path: String, page: Int): String {
        val (p, q) = path.split('?', limit = 2).let { it[0] to it.getOrNull(1) }
        return "$origin/api/movies/$p?" + (q?.let { "$it&" } ?: "") + "page=$page&limit=$LIMIT"
    }

    private fun withPage(url: String, page: Int): String {
        val p = Regex("""([?&]page=)\d+""")
        return if (p.containsMatchIn(url)) url.replace(p, "\$1$page") else url + (if ('?' in url) "&" else "?") + "page=$page"
    }

    /**
     * Gọi API của trang; trang lỗi (máy chủ của trang hay báo 500 «Failed to fetch … from providers» khi nguồn chậm) thì
     * gọi thẳng nguồn KKPhim (phimapi.com — cùng định dạng), như chính trang web làm.
     */
    private fun get(url: String): String {
        // Trang KKPhim: gọi thẳng phimapi.com (kkphim.com không có /api/movies)
        if (isKk(url)) return Http.get(fallback(url) ?: url, mapOf("Accept" to "application/json")).body.also(::checkJson)
        val first = runCatching { Http.get(url, mapOf("Accept" to "application/json")).body.also(::checkJson) }
        val alt = first.exceptionOrNull()?.let { fallback(url) } ?: return first.getOrThrow()
        return runCatching { Http.get(alt, mapOf("Accept" to "application/json")).body.also(::checkJson) }
            .getOrElse { throw first.exceptionOrNull()!! }
    }

    /** Phản hồi lỗi dạng {"error": "…"} (HTTP 200 vẫn có thể là lỗi) → ngoại lệ. */
    private fun checkJson(body: String) {
        val o = JSONObject(body)
        if (o.has("error") && !o.has("items") && !o.has("data") && !o.has("movie")) throw IOException(o.optString("error"))
    }

    /** Địa chỉ tương ứng trên phimapi.com của một API của trang (không có thì null, vd thịnh hành / song ngữ). */
    fun fallback(url: String): String? {
        val u = URI(url)
        val path = u.path.substringAfter("/api/movies/", "").ifEmpty { return null }
        val q = u.rawQuery?.let { "?$it" }.orEmpty()
        val kk = "https://phimapi.com"
        return when {
            path == "newest" -> "$kk/v1/api/danh-sach/phim-moi-cap-nhat$q"
            path.startsWith("category/") -> "$kk/v1/api/danh-sach/${path.removePrefix("category/")}$q"
            path.startsWith("the-loai/") || path.startsWith("quoc-gia/") -> "$kk/v1/api/$path$q"
            path == "search" -> "$kk/v1/api/tim-kiem$q"
            path.startsWith("detail/") -> "$kk/phim/${path.removePrefix("detail/")}"
            else -> null
        }
    }

    private fun origin(url: String) = Site.originOf(url) ?: url.trimEnd('/')

    /** Ảnh: đường dẫn đầy đủ giữ nguyên; "uploads/movies/…" → nối vào máy chủ ảnh (cdn có thể đã có /uploads/movies). */
    private fun image(ref: String, cdn: String): String? {
        if (ref.isEmpty()) return null
        if (ref.startsWith("http")) return ref
        val host = Site.originOf(cdn) ?: CDN
        return runCatching { URL(URL("$host/"), ref.trimStart('/')).toString() }.getOrNull()
    }

    private fun names(a: JSONArray?) = objects(a).map { str(it, "name") }.filter { it.isNotEmpty() }.joinToString(", ")

    private fun objects(a: JSONArray?): List<JSONObject> =
        if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }

    private fun str(o: JSONObject, key: String): String = (o.opt(key) as? String)?.trim().orEmpty()
}
