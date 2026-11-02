package vn.chimto.tv

import android.util.LruCache
import org.jsoup.Jsoup
import org.jsoup.nodes.Document
import org.jsoup.nodes.Element
import org.json.JSONObject
import java.io.IOException
import java.net.URL
import java.net.URLEncoder

data class Film(val url: String, val title: String, val poster: String?, val badge: String?)
data class Section(val title: String, val films: List<Film>)
data class Category(val title: String, val url: String)

/** movedTo: trang đã chuyển sang tên miền khác (theo chuyển hướng) → gốc mới. */
data class Home(val sections: List<Section>, val categories: List<Category>, val movedTo: String?)
/** sections: trang chia nhóm (vd lịch chiếu theo ngày) — khi có thì hiển thị thay cho films. */
data class FilmPage(val films: List<Film>, val nextUrl: String?, val sections: List<Section> = emptyList())
data class Episode(val title: String, val url: String)
data class FilmDetail(
    val url: String, val title: String, val otherTitle: String?, val poster: String?,
    val description: String?, val info: String?, val episodes: List<Episode>,
)

data class Server(val label: String, val type: String)

/** Thông tin để xin trình phát của một tập (gọi player.php như trang web làm). */
data class EpisodeSource(
    val epUrl: String, val servers: List<Server>, val postId: String, val ep: String,
    val playerUrl: String, val directEmbed: String?,
)

/**
 * Đọc dữ liệu từ trang phim (giao diện kiểu "flw-item" như yanhh3d).
 * Mọi đường dẫn lấy từ chính trang → đổi tên miền trong Cài đặt là chạy tiếp.
 */
object Site {
    private val detailCache = LruCache<String, FilmDetail>(30)

    fun cachedDetail(url: String): FilmDetail? = detailCache.get(url)

    private fun fetch(url: String, headers: Map<String, String> = emptyMap()): Document {
        val r = Http.get(url, headers)
        return Jsoup.parse(r.body, r.url)
    }

    fun home(base: String = Prefs.baseUrl): Home {
        val doc = fetch("$base/")
        val finalOrigin = originOf(doc.location())
        val movedTo = finalOrigin?.takeIf { hostOf(it) != hostOf(base) }

        val sections = ArrayList<Section>()
        for (sec in doc.select("section.block_area")) {
            val films = parseCards(sec)
            if (films.isEmpty()) continue
            val heading = sec.selectFirst(".cat-heading")?.text()?.trim().orEmpty()
            val title = when {
                heading.isNotEmpty() && sec.hasClass("block_area-schedule") -> "$heading hôm nay"
                heading.isNotEmpty() -> heading
                sec.hasClass("block_area_trending") -> "Nổi bật"
                sec.hasClass("block_area-realtime") -> "Xem nhiều"
                else -> "Phim"
            }
            sections += Section(title, films)
        }
        if (sections.isEmpty()) {
            val all = parseCards(doc)
            if (all.isNotEmpty()) sections += Section("Phim mới", all)
        }
        return Home(sections, parseCategories(doc), movedTo)
    }

    private fun parseCategories(doc: Document): List<Category> {
        val home = originOf(doc.location())
        val out = LinkedHashMap<String, Category>()
        var links = doc.select("a.nav-link[href]")
        if (links.isEmpty()) links = doc.select(".block_area-genres a[href]")
        for (a in links) {
            // Bỏ mục cha của menu thả xuống (href="#")
            if (a.attr("href").trim().let { it.startsWith("#") || it.startsWith("javascript", true) }) continue
            val url = a.absUrl("href").trimEnd('/')
            val title = a.text().trim()
            if (url.isEmpty() || title.isEmpty() || url == home || url in out) continue
            if (url.contains("/account") || url.contains("login")) continue
            out[url] = Category(title, url)
        }
        return out.values.toList()
    }

    /** Thẻ phim: .flw-item (lưới / trượt) và li.item-top (bảng xếp hạng). */
    fun parseCards(root: Element): List<Film> {
        val out = LinkedHashMap<String, Film>()
        for (el in root.select(".flw-item, li.item-top")) {
            val a = el.selectFirst("a.film-poster-ahref[href]")
                ?: el.selectFirst(".film-name a[href]")
                ?: el.selectFirst("a[href]")
                ?: continue
            val url = a.absUrl("href")
            if (url.isEmpty() || url in out) continue
            val img = el.selectFirst("img")
            val title = listOf(
                el.selectFirst(".film-name a")?.text(), a.attr("title"), el.selectFirst("h4")?.text(), img?.attr("alt"),
            ).firstOrNull { !it.isNullOrBlank() }?.trim() ?: continue
            val badge = el.selectFirst(".tick-rate, .fdi-item")?.text()?.trim()?.takeIf { it.isNotEmpty() }
            out[url] = Film(url, title, img?.let { imageUrl(it) }, badge)
        }
        return out.values.toList()
    }

    private fun imageUrl(img: Element): String? =
        listOf("data-src", "data-lazy-src", "data-original", "src")
            .map { img.absUrl(it) }
            .firstOrNull { it.startsWith("http") }

    fun list(url: String): FilmPage {
        val doc = fetch(url)
        val mainFilms = doc.selectFirst("#main-content")?.let { parseCards(it) }.orEmpty()
        if (mainFilms.isEmpty()) {
            val days = doc.select(".day-tabs .day-tab[data-id]")
            if (days.isNotEmpty()) return FilmPage(emptyList(), null, schedule(doc, days))
        }
        val films = mainFilms.ifEmpty { parseCards(doc) }
        val next = doc.selectFirst(".pagination a.next[href], a.next.page-numbers[href], a[rel=next][href]")
            ?.absUrl("href")?.takeIf { it.isNotEmpty() && it != url }
        return FilmPage(films, next)
    }

    /** Trang lịch chiếu: mỗi ngày nạp riêng qua admin-ajax (action yan_ajax_schedule), bắt đầu từ hôm nay. */
    private fun schedule(doc: Document, days: List<Element>): List<Section> {
        val scripts = doc.select("script").joinToString("\n") { it.data() }
        val ajax = Regex("""ajax_url\s*=\s*['"]([^'"]+)['"]""").find(scripts)?.groupValues?.get(1)
            ?: resolve(doc.location(), "/wp-admin/admin-ajax.php")
        val today = days.indexOfFirst { it.hasClass("active") }.coerceAtLeast(0)
        val ordered = days.drop(today) + days.take(today)
        return ordered.mapIndexedNotNull { i, day ->
            runCatching {
                val r = Http.post(
                    ajax, mapOf("action" to "yan_ajax_schedule", "day" to day.attr("data-id")),
                    mapOf("Referer" to doc.location(), "X-Requested-With" to "XMLHttpRequest"),
                )
                val films = parseCards(Jsoup.parse(JSONObject(r.body).optString("html"), doc.location()))
                Section(day.text().trim() + if (i == 0) " (hôm nay)" else "", films)
            }.getOrNull()?.takeIf { it.films.isNotEmpty() }
        }
    }

    fun search(query: String): FilmPage =
        list("${Prefs.baseUrl}/?s=" + URLEncoder.encode(query.trim(), "UTF-8"))

    fun detail(url: String): FilmDetail {
        val doc = fetch(url)
        val title = doc.selectFirst(".anisc-detail .film-name, h1.film-name, h1")?.text()?.trim()
            ?.takeIf { it.isNotEmpty() } ?: meta(doc, "og:title") ?: url
        val other = doc.selectFirst(".film-name-real")?.text()?.trim()?.takeIf { it.isNotEmpty() && it != title }
        val poster = doc.selectFirst(".anisc-poster img, .film-poster img")?.let { imageUrl(it) } ?: meta(doc, "og:image")
        val desc = doc.selectFirst(".film-description .text, .film-description, .short-content")?.text()?.trim()
            ?.takeIf { it.isNotEmpty() } ?: meta(doc, "og:description")
        val info = listOfNotNull(
            doc.selectFirst(".movie-status")?.text()?.trim(),
            doc.select(".anisc-info .item").firstOrNull { it.text().startsWith("Thời lượng") }
                ?.selectFirst(".name")?.text()?.trim(),
            doc.select(".item-cate a").joinToString(", ") { it.text().trim() },
        ).filter { it.isNotEmpty() }.joinToString("  •  ").ifEmpty { null }

        var episodes = parseEpisodes(doc)
        if (episodes.isEmpty()) {
            doc.selectFirst("a.btn-play[href]")?.absUrl("href")?.takeIf { it.isNotEmpty() }
                ?.let { episodes = listOf(Episode("Xem phim", it)) }
        }
        return FilmDetail(url, title, other, poster, desc, info, episodes).also { detailCache.put(url, it) }
    }

    /** Danh sách tập, sắp tăng dần (trang web xếp tập mới nhất lên đầu). */
    private fun parseEpisodes(doc: Document): List<Episode> {
        var links = doc.select("a.ep-item[href]")
        if (links.isEmpty()) links = doc.select(".ss-list a[href], .episodes a[href]")
        if (links.isEmpty()) links = doc.select("a[href*=/tap-]")
        val map = LinkedHashMap<String, Episode>()
        for (a in links) {
            val u = a.absUrl("href")
            if (u.isEmpty() || u in map) continue
            map[u] = Episode(a.text().trim().ifEmpty { a.attr("title").trim() }, u)
        }
        val eps = map.values.toList()
        val nums = eps.map { epNumber(it.title) ?: epNumber(it.url.substringAfterLast('/')) }
        return if (nums.count { it != null } >= eps.size * 0.8) {
            eps.indices.sortedWith(compareBy(nullsLast()) { nums[it] }).map { eps[it] }
        } else {
            eps.reversed()
        }
    }

    private fun epNumber(s: String): Double? =
        (Regex("""(?i)t[aậ]p[\s\-_]*(\d+(?:\.\d+)?)""").find(s) ?: Regex("""(\d+(?:\.\d+)?)""").find(s))
            ?.groupValues?.get(1)?.toDoubleOrNull()

    fun source(epUrl: String): EpisodeSource {
        val doc = fetch(epUrl)
        val scripts = doc.select("script").joinToString("\n") { it.data() }
        var servers = doc.select("#list_sv [data-type]").map { Server(it.text().trim(), it.attr("data-type")) }
        if (servers.isEmpty()) {
            servers = doc.select(".player-servers [data-type], .btn3dsv[data-type]")
                .map { Server(it.text().trim(), it.attr("data-type")) }
        }
        servers = servers.filter { it.type.isNotEmpty() }.distinctBy { it.type }
            .map { if (it.label.isEmpty()) it.copy(label = it.type) else it }

        val active = doc.selectFirst("a.ep-item.active[data-post-id]") ?: doc.selectFirst("[data-post-id][data-ep]")
        val postId = active?.attr("data-post-id")?.takeIf { it.isNotEmpty() }
            ?: Regex("""MovieInfo\s*=\s*\{\s*id:\s*(\d+)""").find(scripts)?.groupValues?.get(1)
            ?: ""
        val ep = active?.attr("data-ep")?.takeIf { it.isNotEmpty() }
            ?: epUrl.trimEnd('/').substringAfterLast('/').removeSuffix(".html")
        val playerUrl = Regex("""url:\s*['"]([^'"]*player\.php)['"]""").find(scripts)?.groupValues?.get(1)
            ?.let { resolve(epUrl, it) } ?: resolve(epUrl, "/player/player.php")
        val direct = doc.selectFirst("#video-player iframe[src], .player-frame iframe[src]")?.absUrl("src")
            ?.takeIf { it.startsWith("http") }
        if ((servers.isEmpty() || postId.isEmpty()) && direct == null) {
            throw IOException("Không tìm thấy trình phát trên trang tập này")
        }
        return EpisodeSource(epUrl, servers, postId, ep, playerUrl, direct)
    }

    /** Hỏi player.php địa chỉ trình phát nhúng (iframe) của một server. */
    fun embedUrl(src: EpisodeSource, server: Server?): String {
        if (server == null) return src.directEmbed ?: throw IOException("Không có server")
        val url = src.playerUrl + "?action=dox_ajax_player&post_id=" + enc(src.postId) +
            "&chapter_st=" + enc(src.ep) + "&type=" + enc(server.type) + "&sv=1"
        val r = Http.get(url, mapOf("Referer" to src.epUrl, "X-Requested-With" to "XMLHttpRequest"))
        val doc = Jsoup.parse(r.body, src.epUrl)
        doc.selectFirst("iframe[src]")?.absUrl("src")?.takeIf { it.startsWith("http") }?.let { return it }
        throw IOException("Server \"${server.label}\" chưa có video cho tập này")
    }

    private fun meta(doc: Document, property: String): String? =
        doc.selectFirst("meta[property=$property], meta[name=$property]")?.attr("content")?.trim()?.takeIf { it.isNotEmpty() }

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")

    private fun resolve(base: String, ref: String): String = URL(URL(base), ref).toString()

    fun hostOf(url: String): String? = try {
        URL(url).host.lowercase().removePrefix("www.")
    } catch (e: Exception) {
        null
    }

    fun originOf(url: String): String? = try {
        val u = URL(url)
        "${u.protocol}://${u.host}" + if (u.port != -1) ":${u.port}" else ""
    } catch (e: Exception) {
        null
    }
}
