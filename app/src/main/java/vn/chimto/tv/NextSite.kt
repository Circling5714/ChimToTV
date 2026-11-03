package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import org.json.JSONTokener
import org.jsoup.Jsoup
import org.jsoup.nodes.Document
import java.io.IOException
import java.net.URL
import java.net.URLEncoder
import java.util.concurrent.ConcurrentHashMap
import kotlin.concurrent.thread

/**
 * Trang phim dựng bằng Next.js, giao diện kiểu "PhimMoi" (vd phimmoir.fun; nguồn KKPhim / NguonC / OPhim).
 * Không đọc HTML: mọi dữ liệu (phim, đủ danh sách tập, server, link video) nằm sẵn trong các đoạn
 * self.__next_f.push([1,"…"]) của trang. Tập chia theo ngôn ngữ (Vietsub / Thuyết Minh), mỗi ngôn ngữ có vài
 * server; link video là HLS trực tiếp (.m3u8 — app tự phát, xem assets/hls.html) hoặc trình phát nhúng (iframe).
 */
object NextSite {
    private const val PUSH = "self.__next_f.push([1,"
    private const val EPISODE = "\"episode\":{\"episodes\""

    /** Khối phim trang chủ: ["$","$L26","Trung Quốc Hot Nhất",{"movies":[…]}] */
    private val SECTION = Regex("""\["\${'$'}","\${'$'}L\w+","([^"]+)",\{"movies":\[""")
    private val MOVIE = Regex("""\{"id":\d+,"name":"""")
    private val PAGING = Regex(""""total":(\d+),"page":(\d+),"limit":(\d+)""")

    /**
     * Menu của trang chỉ có 10 thể loại, không có mục hoạt hình / quốc gia → thêm mục cố định (đường dẫn bộ lọc
     * của chính trang, mới cập nhật lên đầu).
     */
    private val MENU = listOf(
        "Hoạt hình Trung Quốc" to "/loc-phim?genre=hoat-hinh&country=trung-quoc&sort=updated_at&order=desc",
        "Hoạt hình" to "/loc-phim?genre=hoat-hinh&sort=updated_at&order=desc",
        "Phim bộ mới" to "/loc-phim?type=phim-bo&sort=updated_at&order=desc",
        "Phim lẻ mới" to "/loc-phim?type=phim-le&sort=updated_at&order=desc",
        "Phim Hàn Quốc" to "/loc-phim?country=han-quoc&sort=updated_at&order=desc",
        "Phim Trung Quốc" to "/loc-phim?country=trung-quoc&sort=updated_at&order=desc",
        "Phim Âu Mỹ" to "/loc-phim?country=au-my&sort=updated_at&order=desc",
        "Phim Nhật Bản" to "/loc-phim?country=nhat-ban&sort=updated_at&order=desc",
        "Phim Thái Lan" to "/loc-phim?country=thai-lan&sort=updated_at&order=desc",
    )

    /** Trang đã gặp là Next.js (để tìm kiếm đi thẳng /tim-kiem thay cho ?s= của WordPress). */
    private val hosts = ConcurrentHashMap.newKeySet<String>()

    fun isKnown(base: String) = Site.hostOf(base) in hosts

    /** Dữ liệu Next.js của trang (các chuỗi push nối lại); null nếu không phải trang Next.js. */
    fun data(doc: Document): String? {
        val sb = StringBuilder()
        for (s in doc.select("script")) {
            val js = s.data()
            val i = js.indexOf(PUSH)
            if (i >= 0) (value(js, i + PUSH.length) as? String)?.let { sb.append(it) }
        }
        if (sb.isEmpty()) return null
        Site.hostOf(doc.location())?.let { hosts += it }
        return sb.toString()
    }

    // ---- Trang chủ / danh sách ----

    /** Trang chủ, thêm khối hoạt hình Trung Quốc mới cập nhật lên đầu (trang chủ của trang toàn phim truyền hình). */
    fun home(doc: Document, data: String): Home {
        val h = parseHome(doc, data)
        val hh3d = runCatching { list(origin(doc) + MENU.first().second).films }.getOrNull().orEmpty()
        return if (hh3d.isEmpty()) h else h.copy(sections = listOf(Section("Hoạt hình Trung Quốc mới cập nhật", hh3d)) + h.sections)
    }

    fun parseHome(doc: Document, data: String): Home {
        val origin = origin(doc)
        val sections = ArrayList<Section>()
        val first = SECTION.find(data)?.range?.first ?: data.length
        movies(data, origin, 0, first).takeIf { it.isNotEmpty() }?.let { sections += Section("Nổi bật", it) }
        for (m in SECTION.findAll(data)) {
            val arr = value(data, m.range.last) as? JSONArray ?: continue
            val films = (0 until arr.length()).mapNotNull { arr.optJSONObject(it)?.let { o -> film(o, origin) } }.distinctBy { it.url }
            if (films.isNotEmpty()) sections += Section(m.groupValues[1], films)
        }
        return Home(sections, categories(doc, origin), null)
    }

    private fun categories(doc: Document, origin: String): List<Category> {
        val out = LinkedHashMap<String, Category>()
        for ((title, path) in MENU) out[origin + path] = Category(title, origin + path)
        for (a in doc.select("a[href^=/the-loai/]")) {
            val url = a.absUrl("href").trimEnd('/')
            val title = a.text().trim()
            if (title.isNotEmpty() && url !in out) out[url] = Category(title, url)
        }
        return out.values.toList()
    }

    private fun list(url: String): FilmPage {
        val r = Http.get(url)
        val doc = Jsoup.parse(r.body, r.url)
        return parseList(doc, data(doc) ?: throw IOException("Trang không còn dạng Next.js"), url)
    }

    /** Danh sách / tìm kiếm: 21 phim một trang, trang sau = tham số page tiếp theo (dữ liệu có total/page/limit). */
    fun parseList(doc: Document, data: String, url: String): FilmPage {
        val films = movies(data, origin(doc))
        val next = PAGING.find(data)?.groupValues?.drop(1)?.map { it.toInt() }?.let { (total, page, limit) ->
            if (films.isNotEmpty() && page * limit < total) withPage(url, page + 1) else null
        }
        return FilmPage(films, next)
    }

    fun searchUrl(base: String, query: String) = "$base/tim-kiem?keyword=" + URLEncoder.encode(query.trim(), "UTF-8")

    private fun withPage(url: String, page: Int): String {
        val p = Regex("""([?&]page=)\d+""")
        return if (p.containsMatchIn(url)) url.replace(p, "\$1$page") else url + (if ('?' in url) "&" else "?") + "page=$page"
    }

    // ---- Chi tiết phim ----

    /**
     * Chi tiết + danh sách tập của các ngôn ngữ khác (trang phim chỉ có sẵn danh sách của một ngôn ngữ; phim có thể
     * có tới 4: Vietsub, Thuyết Minh, Lồng Tiếng, Song Ngữ → tải song song, TV mạng chậm đỡ chờ).
     */
    fun detail(doc: Document, data: String, url: String): FilmDetail {
        val d = parseDetail(doc, data, url)
        val ep = episodeData(data) ?: return d
        val origin = origin(doc)
        val slug = movie(data, url)?.let { str(it, "slug") }.orEmpty().ifEmpty { return d }
        // Trang chỉ trả danh sách tập của một ngôn ngữ khi tập được hỏi có ở ngôn ngữ đó (không có → danh sách rỗng),
        // nên hỏi tập đầu, không được thì tập cuối của danh sách chính (bản Thuyết Minh có thể bắt đầu muộn hơn)
        val mainEps = d.groups.firstOrNull()?.episodes.orEmpty()
        val probes = listOfNotNull(mainEps.firstOrNull(), mainEps.lastOrNull()).map { epSlug(it.url) }
            .filter { it.isNotEmpty() }.distinct().ifEmpty { listOf("tap-1") }
        val extra = languages(ep).entries.filter { (_, name) -> d.groups.none { it.name == name } }
        val found = arrayOfNulls<EpisodeGroup>(extra.size)
        extra.mapIndexed { i, (lang, name) ->
            thread {
                val server = bestServer(ep, lang) ?: return@thread
                found[i] = probes.firstNotNullOfOrNull { probe ->
                    runCatching {
                        val r = Http.get("$origin/xem-phim/$slug/$probe/$lang?server=$server")
                        val e = episodeData(data(Jsoup.parse(r.body, r.url)) ?: return@runCatching null) ?: return@runCatching null
                        if (str(e, "current_language") != lang) null else episodes(e, origin, slug, lang)
                    }.getOrNull()?.takeIf { it.isNotEmpty() }
                }?.let { EpisodeGroup(name, it) }
            }
        }.forEach { it.join() }
        return d.copy(groups = d.groups + found.filterNotNull())
    }

    fun parseDetail(doc: Document, data: String, url: String): FilmDetail {
        val origin = origin(doc)
        val m = movie(data, url) ?: throw IOException("Không đọc được thông tin phim")
        val slug = str(m, "slug")
        val desc = str(m, "description").takeIf { it.length > 20 && !it.startsWith("$") }
            ?: doc.selectFirst("meta[property=og:description], meta[name=description]")?.attr("content")?.trim()
        val info = listOf(
            str(m, "status_text").ifEmpty { str(m, "display_status") },
            m.optJSONObject("country")?.let { str(it, "name") }.orEmpty(),
            m.opt("year")?.takeIf { it is Number }?.toString().orEmpty(),
            names(m.optJSONArray("genres")),
        ).filter { it.isNotEmpty() }.joinToString("  •  ").ifEmpty { null }

        val groups = ArrayList<EpisodeGroup>()
        episodeData(data)?.let { ep ->
            val lang = str(ep, "current_language")
            val eps = episodes(ep, origin, slug, lang)
            if (eps.isNotEmpty()) groups += EpisodeGroup(languages(ep)[lang] ?: "Tập phim", eps)
        }
        return FilmDetail(
            url, str(m, "name").ifEmpty { url }, str(m, "origin_name").ifEmpty { null },
            str(m, "poster_url").ifEmpty { str(m, "thumb_url") }.ifEmpty { null }?.let { abs(origin, it) },
            desc, info, groups,
        )
    }

    /** Đối tượng phim của trang (trang có thể nhắc tới phim khác → ưu tiên phim trùng slug trong địa chỉ). */
    private fun movie(data: String, url: String): JSONObject? {
        val slug = url.trimEnd('/').substringAfterLast('/')
        var first: JSONObject? = null
        var i = data.indexOf("\"movie\":{")
        while (i >= 0) {
            (value(data, i + "\"movie\":".length) as? JSONObject)?.let {
                if (str(it, "slug") == slug) return it
                if (first == null) first = it
            }
            i = data.indexOf("\"movie\":{", i + 1)
        }
        return first
    }

    private fun episodeData(data: String): JSONObject? {
        val i = data.indexOf(EPISODE)
        return if (i < 0) null else value(data, i + "\"episode\":".length) as? JSONObject
    }

    /** Ngôn ngữ có tập (theo thứ tự trang đưa): slug → tên hiển thị, vd "thuyetminh" → "Thuyết Minh". */
    private fun languages(ep: JSONObject): Map<String, String> {
        val out = LinkedHashMap<String, String>()
        objects(ep.optJSONArray("available_sources")).forEach { s ->
            val slug = str(s, "language_slug")
            if (slug.isNotEmpty() && slug !in out) out[slug] = str(s, "language_name").ifEmpty { slug }
        }
        return out
    }

    /** Server có nhiều tập nhất của một ngôn ngữ (lấy danh sách tập đầy đủ nhất). */
    private fun bestServer(ep: JSONObject, lang: String): String? =
        objects(ep.optJSONArray("available_sources")).filter { str(it, "language_slug") == lang }
            .maxByOrNull { it.optInt("total_episodes") }?.let { str(it, "server_slug") }?.ifEmpty { null }

    private fun episodes(ep: JSONObject, origin: String, slug: String, lang: String): List<Episode> =
        objects(ep.optJSONArray("episodes")).mapNotNull { e ->
            val s = str(e, "slug").ifEmpty { return@mapNotNull null }
            val l = str(e, "language").ifEmpty { lang }
            Episode(str(e, "episode_label").ifEmpty { str(e, "name") }.ifEmpty { s }, "$origin/xem-phim/$slug/$s/$l")
        }.distinctBy { it.url }

    // ---- Trang tập phim ----

    /** Server của tập theo ngôn ngữ đang xem; link video của server trang mở sẵn đọc luôn (khỏi tải lại). */
    fun parseSource(doc: Document, data: String, epUrl: String): EpisodeSource {
        val ep = episodeData(data) ?: throw IOException("Không đọc được trang tập phim")
        val lang = str(ep, "current_language")
        var servers = objects(ep.optJSONArray("episode_sources")).mapNotNull { s ->
            val sv = s.optJSONObject("server") ?: return@mapNotNull null
            val l = s.optJSONObject("language")?.let { str(it, "slug") }.orEmpty()
            if (lang.isNotEmpty() && l.isNotEmpty() && l != lang) null else Server(str(sv, "name"), str(sv, "slug"))
        }
        if (servers.isEmpty()) {
            servers = objects(ep.optJSONArray("available_sources")).filter { lang.isEmpty() || str(it, "language_slug") == lang }
                .map { Server(str(it, "server_name"), str(it, "server_slug")) }
        }
        servers = servers.filter { it.type.isNotEmpty() }.distinctBy { it.type }.map { if (it.label.isEmpty()) it.copy(label = it.type) else it }
        val cur = currentServer(ep)
        val links = videoOf(ep, origin(doc))?.takeIf { cur.isNotEmpty() }?.let { mapOf(cur to it) }.orEmpty()
        return EpisodeSource(epUrl, servers, "", epSlug(epUrl), "", null, null, false, links, next = true)
    }

    /** ".../xem-phim/tien-nghich/tap-165/vietsub" → "tap-165". */
    private fun epSlug(epUrl: String) = epUrl.trimEnd('/').split('/').let { it.getOrNull(it.size - 2) }.orEmpty()

    /** Link video của một server: đã có sẵn thì dùng, không thì mở trang tập với ?server=… để lấy. */
    fun video(src: EpisodeSource, server: Server): Video {
        src.links[server.type]?.let { return it }
        val r = Http.get(src.epUrl + "?server=" + URLEncoder.encode(server.type, "UTF-8"))
        val doc = Jsoup.parse(r.body, r.url)
        val ep = data(doc)?.let { episodeData(it) } ?: throw IOException("Không đọc được trang tập phim")
        // Tập chưa có trên server này thì trang tự mở server khác → coi như chưa có
        if (currentServer(ep) != server.type) throw IOException("Server \"${server.label}\" chưa có video cho tập này")
        return videoOf(ep, origin(doc)) ?: throw IOException("Server \"${server.label}\" chưa có video cho tập này")
    }

    private fun currentServer(ep: JSONObject): String = when (val c = ep.opt("current_server")) {
        is JSONObject -> str(c, "slug")
        is String -> c
        else -> ""
    }

    /** Server riêng của trang ("HD") cho link dạng /public/m3u8/… (cùng tên miền) và phụ đề rời. */
    private fun videoOf(ep: JSONObject, origin: String): Video? {
        val v = ep.optJSONObject("video") ?: return null
        val url = str(v, "link_m3u8").ifEmpty { str(v, "link_embed") }.ifEmpty { return null }
        return Video(abs(origin, url) ?: return null, subtitle(v.optJSONArray("subtitles"), origin))
    }

    /** Phụ đề rời (bản Song Ngữ của server HD không in chữ vào hình): bản mặc định, không có thì tiếng Việt. */
    private fun subtitle(a: JSONArray?, origin: String): String? {
        val subs = objects(a).filter { str(it, "url").isNotEmpty() }
        val s = subs.firstOrNull { it.optBoolean("isDefault") } ?: subs.firstOrNull { str(it, "lang") == "vie" } ?: subs.firstOrNull()
        return s?.let { abs(origin, str(it, "url")) }
    }

    // ---- Tiện ích ----

    private fun movies(data: String, origin: String, from: Int = 0, to: Int = data.length): List<Film> {
        val out = LinkedHashMap<String, Film>()
        for (m in MOVIE.findAll(data, from)) {
            if (m.range.first >= to) break
            val f = (value(data, m.range.first, 16_384) as? JSONObject)?.let { film(it, origin) } ?: continue
            out.putIfAbsent(f.url, f)
        }
        return out.values.toList()
    }

    /** Phim trong danh sách (đối tượng thể loại / quốc gia cũng có id + name + slug nhưng không có poster). */
    private fun film(o: JSONObject, origin: String): Film? {
        if (!o.has("poster_url")) return null
        val slug = str(o, "slug").ifEmpty { return null }
        val name = str(o, "name").ifEmpty { return null }
        val poster = str(o, "poster_url").ifEmpty { str(o, "thumb_url") }.ifEmpty { null }?.let { abs(origin, it) }
        return Film("$origin/phim/$slug", name, poster, badge(str(o, "display_status")))
    }

    /** Nhãn trên thẻ phim (một dòng): "Thuyết Minh 107, Vietsub 165" → "Tập 165", "Vietsub Full" → "Full". */
    fun badge(status: String): String? {
        val n = Regex("""\d+""").findAll(status).mapNotNull { it.value.toIntOrNull() }.maxOrNull()
        return when {
            n != null -> "Tập $n"
            status.contains("full", true) -> "Full"
            else -> status.ifEmpty { null }
        }
    }

    private fun names(a: JSONArray?) = objects(a).map { str(it, "name") }.filter { it.isNotEmpty() }.joinToString(", ")

    private fun objects(a: JSONArray?): List<JSONObject> =
        if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }

    /** Chuỗi của một khoá (null / thiếu / không phải chuỗi → ""; optString của Android trả "null"). */
    private fun str(o: JSONObject, key: String): String = (o.opt(key) as? String)?.trim().orEmpty()

    /**
     * Một giá trị JSON bắt đầu tại vị trí at. Đọc thử một khúc trước (đỡ chép cả chuỗi dài trên TV yếu),
     * khúc thiếu thì đọc tới hết.
     */
    private fun value(s: String, at: Int, window: Int = 65_536): Any? {
        for (end in listOf(minOf(s.length, at + window), s.length).distinct()) {
            try {
                return JSONTokener(s.substring(at, end)).nextValue()
            } catch (e: JSONException) {
                // khúc bị cắt giữa chừng → thử cả chuỗi
            }
        }
        return null
    }

    private fun origin(doc: Document) = Site.originOf(doc.location()) ?: Prefs.baseUrl

    /** Đường dẫn tuyệt đối; dạng lạ (vd "data:…") thì null — bỏ một ảnh / link chứ không hỏng cả trang. */
    private fun abs(origin: String, ref: String): String? = runCatching {
        URL(URL("$origin/"), ref).toString().takeIf { it.startsWith("http") }
    }.getOrNull()
}
