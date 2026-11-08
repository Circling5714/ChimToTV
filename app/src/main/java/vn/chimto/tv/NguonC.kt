package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONObject
import org.jsoup.Jsoup
import java.io.IOException
import java.net.URI
import java.net.URLEncoder

/**
 * NguonC (phim.nguonc.com, từ 2.7): API công khai /api/films/… (định dạng riêng, không phải KKPhim). Dùng hai cách:
 * - Trang riêng: trang chủ / danh sách / tìm kiếm / chi tiết / tập đọc từ API. Địa chỉ phim = /phim/<slug> (như trang web),
 *   tập = /phim/<slug>/<tập>?sv=<thứ tự server>; danh sách = địa chỉ API.
 * - Server dự phòng thứ hai (cạnh KKPhim) cho yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw: NguonC hay có tập mới sớm.
 * Video chỉ có trình phát nhúng (embedN.streamc.xyz) → phát qua iframe, player-agent.js tự bỏ quảng cáo cá cược của JW.
 */
object NguonC {
    const val HOST = "phim.nguonc.com"
    const val API = "https://phim.nguonc.com/api"

    /** Đầu tên loại server dự phòng NguonC, vd "nc-vs", "nc-tm". */
    const val PREFIX = "nc-"

    private const val TTL = 10 * 60_000L
    private const val MISS_TTL = 30 * 60_000L

    private val MENU = listOf(
        "Hoạt hình" to "films/danh-sach/hoat-hinh",
        "Phim bộ" to "films/danh-sach/phim-bo",
        "Phim lẻ" to "films/danh-sach/phim-le",
        "TV Shows" to "films/danh-sach/tv-shows",
        "Phim thuyết minh" to "films/ngon-ngu/thuyet-minh",
        "Phim lồng tiếng" to "films/ngon-ngu/long-tieng",
        "Phim Trung Quốc" to "films/quoc-gia/trung-quoc",
        "Phim Hàn Quốc" to "films/quoc-gia/han-quoc",
        "Phim Mỹ" to "films/quoc-gia/my",
        "Phim Nhật Bản" to "films/quoc-gia/nhat-ban",
        "Phim Thái Lan" to "films/quoc-gia/thai-lan",
    )
    private val GENRES = listOf(
        "Hành Động" to "hanh-dong", "Cổ Trang" to "co-trang", "Hoạt Hình" to "hoat-hinh", "Giả Tưởng" to "gia-tuong",
        "Khoa Học Viễn Tưởng" to "khoa-hoc-vien-tuong", "Kinh Dị" to "kinh-di", "Tình Cảm" to "tinh-cam", "Lãng Mạn" to "lang-man",
        "Hài" to "phim-hai", "Tâm Lý" to "tam-ly", "Phiêu Lưu" to "phieu-luu", "Hình Sự" to "hinh-su", "Gây Cấn" to "gay-can",
        "Bí Ẩn" to "bi-an", "Chiến Tranh" to "chien-tranh", "Chính Kịch" to "chinh-kich", "Gia Đình" to "gia-dinh",
        "Lịch Sử" to "lich-su", "Tài Liệu" to "tai-lieu", "Nhạc" to "phim-nhac", "Miền Tây" to "mien-tay", "Phim 18+" to "phim-18",
    )
    private val COUNTRIES = listOf(
        "Trung Quốc" to "trung-quoc", "Hàn Quốc" to "han-quoc", "Nhật Bản" to "nhat-ban", "Mỹ" to "my", "Anh" to "anh",
        "Thái Lan" to "thai-lan", "Đài Loan" to "dai-loan", "Hồng Kông" to "hong-kong", "Ấn Độ" to "an-do",
        "Việt Nam" to "viet-nam", "Pháp" to "phap", "Đức" to "duc", "Tây Ban Nha" to "tay-ban-nha", "Nga" to "nga",
        "Canada" to "canada", "Úc" to "uc", "Italy" to "italy", "Philippines" to "philippines", "Brazil" to "brazil", "Bỉ" to "bi",
    )

    fun isSite(url: String) = Site.hostOf(url) == HOST

    // ---- Trang riêng ----

    fun home(base: String): Home {
        val o = ApiSite.origin(base)
        val blocks = listOf(
            "Hoạt hình mới cập nhật" to api(o, "films/danh-sach/hoat-hinh"),
            "Phim mới cập nhật" to newestUrl(o),
            "Phim bộ mới" to api(o, "films/danh-sach/phim-bo"),
            "Phim lẻ mới" to api(o, "films/danh-sach/phim-le"),
            "Phim thuyết minh" to api(o, "films/ngon-ngu/thuyet-minh"),
        )
        val sections = ApiSite.sections(blocks) { parseList(get(it), it).films }
        return Home(sections, MENU.map { (t, p) -> Category(t, api(o, p)) }, null)
    }

    fun newestUrl(base: String) = api(ApiSite.origin(base), "films/phim-moi-cap-nhat")

    fun searchUrl(base: String, query: String) =
        "${ApiSite.origin(base)}/api/films/search?keyword=" + URLEncoder.encode(query.trim(), "UTF-8") + "&page=1"

    fun filters(base: String): Pair<List<Category>, List<Category>> {
        val o = ApiSite.origin(base)
        return GENRES.map { (n, s) -> Category(n, api(o, "films/the-loai/$s")) } to
            COUNTRIES.map { (n, s) -> Category(n, api(o, "films/quoc-gia/$s")) }
    }

    private fun api(origin: String, path: String) = "$origin/api/$path?page=1"

    fun list(url: String): FilmPage = parseList(get(url), url)

    /** {status, paginate:{current_page, total_page}, items:[{name, slug, thumb_url (dọc), poster_url (ngang), current_episode}]}. */
    fun parseList(body: String, url: String): FilmPage {
        val o = ApiSite.origin(url)
        val root = JSONObject(body)
        val films = objects(root.optJSONArray("items")).mapNotNull { film(it, o) }.distinctBy { it.url }
        val p = root.optJSONObject("paginate")
        val page = p?.optInt("current_page") ?: 0
        val pages = p?.optInt("total_page") ?: 0
        val next = if (films.isNotEmpty() && page in 1 until pages) ApiSite.withPage(url, page + 1) else null
        return FilmPage(films, next)
    }

    private fun film(m: JSONObject, origin: String): Film? {
        val slug = str(m, "slug").ifEmpty { return null }
        val name = str(m, "name").ifEmpty { return null }
        val poster = str(m, "thumb_url").ifEmpty { str(m, "poster_url") }.takeIf { it.startsWith("http") }
        return Film("$origin/phim/$slug", name, poster, NextSite.badge(str(m, "current_episode")))
    }

    fun detail(url: String): FilmDetail {
        val film = filmUrl(url)
        return parseDetail(get(detailApi(film)), film)
    }

    fun parseDetail(body: String, url: String): FilmDetail {
        val m = JSONObject(body).optJSONObject("movie") ?: throw IOException("Không đọc được thông tin phim")
        val desc = str(m, "description").takeIf { it.isNotEmpty() }?.let { Jsoup.parse(it).text().trim() }?.ifEmpty { null }
        val cats = categories(m)
        val info = listOf(
            str(m, "current_episode"),
            str(m, "time"),
            str(m, "language"),
            cats["Quốc gia"].orEmpty(),
            m.opt("year")?.toString()?.takeIf { it.isNotEmpty() && it != "0" && it != "null" }.orEmpty(),
            cats["Thể loại"].orEmpty(),
        ).filter { it.isNotEmpty() }.joinToString("  •  ").ifEmpty { null }
        val groups = objects(m.optJSONArray("episodes")).mapIndexedNotNull { i, s ->
            val eps = objects(s.optJSONArray("items")).mapNotNull { e ->
                val slug = str(e, "slug").ifEmpty { return@mapNotNull null }
                if (!str(e, "embed").startsWith("http") && !str(e, "m3u8").startsWith("http")) return@mapNotNull null
                Episode(title(str(e, "name").ifEmpty { slug }), "$url/$slug?sv=$i")
            }.distinctBy { it.url }
            if (eps.isEmpty()) null else EpisodeGroup(str(s, "server_name").ifEmpty { "Server ${i + 1}" }, eps)
        }
        val poster = str(m, "thumb_url").ifEmpty { str(m, "poster_url") }.takeIf { it.startsWith("http") }
        return FilmDetail(url, str(m, "name").ifEmpty { url }, str(m, "original_name").ifEmpty { null }, poster, desc, info, groups)
    }

    /** category: {"1": {group:{name:"Định dạng"}, list:[{name}]}, "2": {group:{name:"Thể loại"}…}} → tên nhóm → "a, b". */
    private fun categories(m: JSONObject): Map<String, String> {
        val c = m.optJSONObject("category") ?: return emptyMap()
        val out = HashMap<String, String>()
        for (k in c.keys()) {
            val g = c.optJSONObject(k) ?: continue
            val name = g.optJSONObject("group")?.let { str(it, "name") } ?: continue
            out[name] = objects(g.optJSONArray("list")).map { str(it, "name") }.filter { it.isNotEmpty() }.joinToString(", ")
        }
        return out
    }

    private fun title(name: String) = ApiSite.title(name)

    fun source(epUrl: String): EpisodeSource {
        val film = filmUrl(epUrl)
        val root = JSONObject(get(detailApi(film))).optJSONObject("movie") ?: throw IOException("Không đọc được thông tin phim")
        val sv = Regex("""[?&]sv=(\d+)""").find(epUrl)?.groupValues?.get(1)?.toIntOrNull() ?: 0
        val tap = URI(epUrl).path.trimEnd('/').substringAfterLast('/')
        val servers = objects(root.optJSONArray("episodes"))
        val e = (listOfNotNull(servers.getOrNull(sv)) + servers).firstNotNullOfOrNull { s ->
            objects(s.optJSONArray("items")).firstOrNull { str(it, "slug") == tap }
        } ?: throw IOException("Không tìm thấy tập này trên trang")
        val links = LinkedHashMap<String, Video>()
        str(e, "m3u8").takeIf { it.startsWith("http") }?.let { links["m3u8"] = Video(it) }
        str(e, "embed").takeIf { it.startsWith("http") }?.let { links["embed"] = Video(it) }
        if (links.isEmpty()) throw IOException("Tập này chưa có video")
        val list = links.keys.map { if (it == "m3u8") Server("HLS", it) else Server("Trình phát NguonC", it) }
        return EpisodeSource(epUrl, list, "", tap, sv.toString(), null, null, false, links, next = true)
    }

    /** ".../phim/muc-than-ky/tap-1?sv=0" → ".../phim/muc-than-ky". */
    fun filmUrl(url: String): String {
        val parts = URI(url).path.trim('/').split('/')
        val i = parts.indexOf("phim")
        val slug = parts.getOrNull(i + 1)?.takeIf { i >= 0 } ?: throw IOException("Địa chỉ phim lạ: $url")
        return "${ApiSite.origin(url)}/phim/$slug"
    }

    private fun detailApi(film: String) = "${ApiSite.origin(film)}/api/film/" + film.substringAfterLast('/')

    // ---- Server dự phòng ----

    private class Entry(val at: Long, val movie: JSONObject?)

    private val cache = object : LinkedHashMap<String, Entry>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Entry>) = size > 40
    }
    private val slugs = HashMap<String, String>()

    /** Trang dùng server dự phòng NguonC: bật trong Cài đặt, không phải trang đã có NguonC (NguonC, phimmoi) / trang API. */
    fun appliesTo(url: String) = Prefs.ncBackup && !ApiSite.isKnown(url) && !NextSite.isKnown(url)

    /** Server NguonC cho một tập (rỗng nếu NguonC không có phim / tập này). Lỗi mạng → ngoại lệ. */
    fun servers(d: FilmDetail, group: EpisodeGroup?, ep: Episode): List<Pair<Server, Video>> {
        val num = Site.epNumber(ep.title) ?: Site.epNumber(ep.url.trimEnd('/').substringAfterLast('/')) ?: return emptyList()
        val movie = movieFor(d) ?: return emptyList()
        return pick(movie, num, group?.name?.let { PlayStats.lang(it) })
    }

    /**
     * Server có tập số [num] trong phim NguonC ({name, episodes:[{server_name, items:[{name, slug, embed}]}]}).
     * lang: ngôn ngữ của bản đang xem ("vs" / "tm" / "lt"; null = mọi ngôn ngữ).
     */
    fun pick(movie: JSONObject, num: Double, lang: String?): List<Pair<Server, Video>> {
        val out = ArrayList<Pair<Server, Video>>()
        val used = HashSet<String>()
        for (s in objects(movie.optJSONArray("episodes"))) {
            val l = PlayStats.lang(str(s, "server_name")) ?: "vs"
            if (lang != null && l != lang) continue
            val e = objects(s.optJSONArray("items"))
                .firstOrNull { (Site.epNumber(str(it, "name")) ?: Site.epNumber(str(it, "slug"))) == num } ?: continue
            val embed = str(e, "embed").takeIf { it.startsWith("http") } ?: continue
            var type = PREFIX + l
            var n = 1
            while (!used.add(type)) type = PREFIX + l + "-" + ++n
            val name = when (l) {
                "tm" -> "Thuyết Minh"
                "lt" -> "Lồng Tiếng"
                else -> "Vietsub"
            }
            out += Server("NguonC $name" + if (n > 1) " $n" else "", type) to Video(embed)
        }
        return out
    }

    private fun movieFor(d: FilmDetail): JSONObject? {
        val now = System.currentTimeMillis()
        synchronized(cache) { cache[d.url] }?.let { if (now - it.at < if (it.movie == null) MISS_TTL else TTL) return it.movie }
        val found = find(d)
        synchronized(cache) { cache[d.url] = Entry(now, found) }
        return found
    }

    /** Phim NguonC khớp tên với phim của trang (slug của trang trước, rồi tìm theo tên); null = NguonC không có. */
    private fun find(d: FilmDetail): JSONObject? {
        val mine = listOfNotNull(d.title, d.otherTitle).map(KkPhim::norm).filter { it.isNotEmpty() }.toSet()
        if (mine.isEmpty()) return null
        synchronized(slugs) { slugs[d.url] }?.let { s -> movie(s)?.let { return it } }
        KkPhim.slugOf(d.url)?.let { own ->
            movie(own)?.takeIf { KkPhim.matches(it, mine) }?.let { remember(d.url, own); return it }
        }
        val q = "$API/films/search?keyword=" + URLEncoder.encode(d.title.take(80), "UTF-8")
        val hit = objects(JSONObject(get(q)).optJSONArray("items")).firstOrNull { KkPhim.matches(it, mine) } ?: return null
        val slug = str(hit, "slug")
        return movie(slug)?.also { remember(d.url, slug) }
    }

    private fun remember(film: String, slug: String) = synchronized(slugs) { slugs[film] = slug }

    /** Phim trên NguonC (movie); null nếu không có slug này. */
    private fun movie(slug: String): JSONObject? {
        val o = try {
            JSONObject(get("$API/film/" + URLEncoder.encode(slug, "UTF-8")))
        } catch (e: Http.HttpError) {
            if (e.code == 404) return null else throw e
        }
        return o.optJSONObject("movie")
    }

    private fun get(url: String) = Http.get(url, mapOf("Accept" to "application/json")).body

    private fun objects(a: JSONArray?): List<JSONObject> =
        if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }

    private fun str(o: JSONObject, key: String): String = (o.opt(key) as? String)?.trim().orEmpty()
}
