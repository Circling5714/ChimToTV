package vn.chimto.tv

import android.util.LruCache
import org.jsoup.Jsoup
import org.jsoup.nodes.Document
import org.jsoup.nodes.Element
import org.json.JSONObject
import java.io.IOException
import java.net.URL
import java.net.URLEncoder
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

data class Film(val url: String, val title: String, val poster: String?, val badge: String?)
data class Section(val title: String, val films: List<Film>)
data class Category(val title: String, val url: String)

/** Đo nhanh một trang: thời gian tải trang chủ (ms) và số phim đọc được (0 = có thể là trang chặn của nhà mạng). */
data class SiteCheck(val ms: Long, val films: Int)

/** movedTo: trang đã chuyển sang tên miền khác (theo chuyển hướng) → gốc mới. */
data class Home(val sections: List<Section>, val categories: List<Category>, val movedTo: String?)
/** sections: trang chia nhóm (vd lịch chiếu theo ngày) — khi có thì hiển thị thay cho films. */
data class FilmPage(val films: List<Film>, val nextUrl: String?, val sections: List<Section> = emptyList())
data class Episode(val title: String, val url: String)

/** Một danh sách tập, vd "Việt Sub" / "Thuyết Minh" (trang HaLim chia tập theo server). */
data class EpisodeGroup(val name: String, val episodes: List<Episode>)

data class FilmDetail(
    val url: String, val title: String, val otherTitle: String?, val poster: String?,
    val description: String?, val info: String?, val groups: List<EpisodeGroup>,
) {
    val episodeCount get() = groups.maxOfOrNull { it.episodes.size } ?: 0

    /** Tập mới nhất của bản nhiều tập nhất (bản Vietsub thường ra trước Thuyết Minh). */
    val latestEpisode get() = groups.maxByOrNull { it.episodes.size }?.episodes?.lastOrNull()
    fun groupOf(epUrl: String): EpisodeGroup? = groups.firstOrNull { g -> g.episodes.any { it.url == epUrl } }
}

data class Server(val label: String, val type: String)

/** Link phát của một server: iframe trình phát / trang tập, hoặc luồng .m3u8 (kèm phụ đề rời nếu có). */
data class Video(val url: String, val subtitle: String? = null)

/**
 * Cách phát một tập.
 * - Có player.php (yanhh3d, hoathinh3d.ee, hhkungfu): xin địa chỉ iframe rồi phát trong trang bọc của app.
 * - pageMode: trang tự nạp trình phát bằng cơ chế riêng (vd hoathinh3d.pw có khoá xác thực) → mở thẳng
 *   trang tập trong WebView, app chỉ phóng trình phát ra toàn màn hình.
 * - next: trang Next.js (phimmoir.fun) — link video (.m3u8 hoặc trình phát nhúng) đọc từ dữ liệu trang tập;
 *   links = link đã có sẵn theo server.
 */
data class EpisodeSource(
    val epUrl: String, val servers: List<Server>, val postId: String, val ep: String, val sv: String,
    val playerUrl: String?, val directEmbed: String?, val pageMode: Boolean,
    val links: Map<String, Video> = emptyMap(), val next: Boolean = false,
)

/**
 * Đọc dữ liệu từ trang phim. Không viết riêng cho từng trang: nhận dạng theo các mẫu giao diện phổ biến của
 * trang hoạt hình 3D (thẻ "flw-item" kiểu yanhh3d, thẻ "halim-item" của theme HaLim) và có phương án dự phòng.
 * Trang dựng bằng Next.js (kiểu PhimMoi) thì chuyển cho NextSite.
 * Mọi đường dẫn lấy từ chính trang → đổi tên miền / thêm trang tương tự trong Cài đặt là chạy.
 */
object Site {
    /** Thẻ phim ở lưới / trượt / bảng xếp hạng. */
    private const val CARD = ".flw-item, li.item-top, .halim-item, a.halim-trending-link"

    /** Tiêu đề khối trên trang chủ. */
    private const val HEADING = ".cat-heading, .section-title"

    private val detailCache = LruCache<String, FilmDetail>(30)

    fun cachedDetail(url: String): FilmDetail? = detailCache.get(url)

    private fun fetch(url: String, headers: Map<String, String> = emptyMap()): Document {
        val r = Http.get(url, headers)
        return Jsoup.parse(r.body, r.url)
    }

    // ---- Trang chủ ----

    fun home(base: String = Prefs.baseUrl): Home {
        val doc = fetch("$base/")
        val movedTo = originOf(doc.location())?.takeIf { hostOf(it) != hostOf(base) }
        val home = NextSite.data(doc)?.let { NextSite.home(doc, it) } ?: parseHome(doc)
        return home.copy(movedTo = movedTo)
    }

    /** Thời gian tính phần tải qua mạng, không tính lúc đọc HTML (TV yếu đọc chậm, không phải lỗi của trang). */
    fun check(base: String): SiteCheck {
        val t0 = System.nanoTime()
        val r = Http.get("$base/")
        val ms = (System.nanoTime() - t0) / 1_000_000
        return SiteCheck(ms, parseHome(Jsoup.parse(r.body, r.url)).sections.sumOf { it.films.size })
    }

    /** Gom thẻ phim theo tiêu đề khối đứng trước nó (theo thứ tự trong trang). */
    fun parseHome(doc: Document): Home {
        NextSite.data(doc)?.let { return NextSite.parseHome(doc, it) }
        val groups = LinkedHashMap<Element?, MutableList<Element>>()
        var heading: Element? = null
        for (el in doc.select("$HEADING, $CARD")) {
            if (el.parents().any { it.`is`(CARD) }) continue
            if (el.`is`(CARD)) groups.getOrPut(heading) { ArrayList() }.add(el) else heading = el
        }
        val sections = ArrayList<Section>()
        for ((h, cards) in groups) {
            val films = cards.mapNotNull { parseCard(it) }.distinctBy { it.url }
            if (films.isEmpty()) continue
            val text = h?.text()?.trim().orEmpty()
            val title = when {
                h == null -> "Nổi bật"
                text.isEmpty() -> if (h.closest(".block_area-realtime") != null) "Xem nhiều" else "Phim"
                h.closest(".block_area-schedule") != null -> "$text hôm nay"
                else -> text
            }
            sections += Section(title, films)
        }
        return Home(sections, parseCategories(doc), null)
    }

    private fun parseCategories(doc: Document): List<Category> {
        val home = originOf(doc.location())?.let { hostOf(it) }
        val out = LinkedHashMap<String, Category>()
        var links = doc.select("a.nav-link[href], li.menu-item > a[href], #header nav a[href], .navbar a[href], #menu a[href]")
        if (links.isEmpty()) links = doc.select(".block_area-genres a[href]")
        for (a in links) {
            // Bỏ mục cha của menu thả xuống (href="#")
            if (a.attr("href").trim().let { it.startsWith("#") || it.startsWith("javascript", true) }) continue
            val url = a.absUrl("href").trimEnd('/')
            val title = a.text().trim().ifEmpty { a.attr("title").trim() }
            if (url.isEmpty() || title.isEmpty() || url in out) continue
            if (hostOf(url) != home || URL(url).path.trim('/').isEmpty()) continue
            if (Regex("account|login|dang-nhap|lich-su|history|bookmark|theo-doi|sitemap|feed").containsMatchIn(url)) continue
            out[url] = Category(title, url)
        }
        return out.values.toList()
    }

    /** Thẻ phim → Film. Hỗ trợ flw-item (yanhh3d), halim-item (HaLim), item-top (bảng xếp hạng). */
    private fun parseCard(el: Element): Film? {
        val a = el.selectFirst("a.film-poster-ahref[href], a.halim-thumb[href], .film-name a[href]")
            ?: el.selectFirst("a[href]") ?: el.takeIf { it.tagName() == "a" && it.hasAttr("href") } ?: return null
        val url = a.absUrl("href")
        if (url.isEmpty()) return null
        val img = el.selectFirst("img")
        val title = listOf(
            el.selectFirst(".film-name a, .entry-title, .halim-trending-title-text, h4")?.text(),
            a.attr("title"), img?.attr("alt"),
        ).firstOrNull { !it.isNullOrBlank() }?.trim() ?: return null
        val badge = el.selectFirst(".tick-rate, .fdi-item, .episode")?.text()?.trim()?.takeIf { it.isNotEmpty() }
        return Film(url, title, img?.let { imageUrl(it) }, badge)
    }

    fun parseCards(root: Element): List<Film> =
        root.select(CARD).filter { c -> c.parents().none { it.`is`(CARD) } }.mapNotNull { parseCard(it) }.distinctBy { it.url }

    private fun imageUrl(img: Element): String? =
        listOf("data-src", "data-lazy-src", "data-original", "src")
            .map { img.absUrl(it) }
            .firstOrNull { it.startsWith("http") }

    // ---- Danh sách / tìm kiếm ----

    fun list(url: String): FilmPage {
        val doc = fetch(url)
        val page = parseList(doc, url)
        if (page.films.isEmpty()) {
            val days = doc.select(".day-tabs .day-tab[data-id]")
            if (days.isNotEmpty()) return FilmPage(emptyList(), null, schedule(doc, days))
        }
        return page
    }

    fun parseList(doc: Document, url: String): FilmPage {
        NextSite.data(doc)?.let { return NextSite.parseList(doc, it, url) }
        val main = doc.select("#main-content, #main-contents, main").firstOrNull { parseCards(it).isNotEmpty() }
        // Trang lịch chiếu yanhh3d: khung chính trống, chỉ có bảng xếp hạng ở cột phụ → để list() nạp theo ngày
        val films = when {
            main != null -> parseCards(main)
            doc.select(".day-tabs .day-tab[data-id]").isNotEmpty() -> emptyList()
            else -> parseCards(doc)
        }
        val next = doc.selectFirst(".pagination a.next[href], a.next.page-numbers[href], a[rel=next][href]")
            ?.absUrl("href")?.takeIf { it.isNotEmpty() && it != url }
        return FilmPage(films, next)
    }

    /** Trang lịch chiếu yanhh3d: mỗi ngày nạp riêng qua admin-ajax (action yan_ajax_schedule), bắt đầu từ hôm nay. */
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

    /**
     * Trang WordPress nhận ?s= (trang HaLim tự chuyển sang /search/…); trang Next.js tìm ở /tim-kiem
     * (chưa biết loại trang thì thử ?s= trước, ra trang Next.js thì tìm lại đúng chỗ).
     */
    fun search(query: String, base: String = Prefs.baseUrl): FilmPage {
        if (!NextSite.isKnown(base)) {
            val page = list("$base/?s=" + URLEncoder.encode(query.trim(), "UTF-8"))
            if (!NextSite.isKnown(base)) return page
        }
        return list(NextSite.searchUrl(base, query))
    }

    /**
     * Tìm cùng lúc trên mọi trang trong danh sách: mỗi trang một nhóm ("yanhh3d.ee — 12 phim"), trang đang dùng đứng
     * đầu. Trang lỗi / chậm quá 20 giây thì bỏ qua; mọi trang đều lỗi thì báo lỗi của trang đang dùng.
     */
    fun searchAll(query: String): FilmPage {
        val sites = (listOf(Prefs.baseUrl) + Prefs.sites).distinct()
        val pool = Executors.newFixedThreadPool(minOf(sites.size, 4))
        try {
            val jobs = sites.map { base -> pool.submit(Callable { search(query, base) }) }
            val results = jobs.map { runCatching { it.get(20, TimeUnit.SECONDS) } }
            if (results.all { it.isFailure }) throw results.first().exceptionOrNull()!!
            val sections = sites.indices.mapNotNull { i ->
                val page = results[i].getOrNull() ?: return@mapNotNull null
                val films = (page.films + page.sections.flatMap { it.films }).distinctBy { it.url }
                if (films.isEmpty()) null else Section("${hostOf(sites[i]) ?: sites[i]} — ${films.size} phim", films)
            }
            return FilmPage(emptyList(), null, sections)
        } finally {
            pool.shutdownNow()
        }
    }

    // ---- Chi tiết phim ----

    fun detail(url: String): FilmDetail {
        val doc = fetch(url)
        val d = NextSite.data(doc)?.let { NextSite.detail(doc, it, url) } ?: parseDetail(doc, url)
        return d.also { detailCache.put(url, it) }
    }

    fun parseDetail(doc: Document, url: String): FilmDetail {
        NextSite.data(doc)?.let { return NextSite.parseDetail(doc, it, url) }
        val title = doc.selectFirst(".anisc-detail .film-name, h1.film-name, h1")?.text()?.trim()
            ?.takeIf { it.isNotEmpty() } ?: meta(doc, "og:title") ?: url
        val other = doc.selectFirst(".film-name-real, .info-hero__alt, .movie-detail .original_title, h2.org_title")
            ?.text()?.trim()?.takeIf { it.isNotEmpty() && it != title }
        val poster = doc.selectFirst(".anisc-poster img, .info-hero__poster img, .movie-thumb, .halim-movie-wrapper .movie-poster img")
            ?.let { imageUrl(it) } ?: meta(doc, "og:image")
        val desc = doc.selectFirst(
            ".film-description .text, .film-description, .short-content, .info-hero__desc, #film-content, .film-content, .entry-content .video-item",
        )?.text()?.trim()?.takeIf { it.length > 20 } ?: meta(doc, "og:description")
        val info = listOfNotNull(
            doc.selectFirst(".movie-status, .info-hero__status, .status-badge")?.text()?.trim(),
            doc.select(".anisc-info .item").firstOrNull { it.text().startsWith("Thời lượng") }
                ?.selectFirst(".name")?.text()?.trim(),
            doc.select(".item-cate a, .info-hero__tags a, .category a[rel~=tag]").joinToString(", ") { it.text().trim() },
        ).filter { it.isNotEmpty() }.distinct().joinToString("  •  ").ifEmpty { null }

        var groups = parseEpisodeGroups(doc)
        if (groups.isEmpty()) {
            doc.selectFirst("a.btn-play[href], a.watch-btn[href], a.watch-movie[href]")?.absUrl("href")?.takeIf { it.isNotEmpty() }
                ?.let { groups = listOf(EpisodeGroup("Tập phim", listOf(Episode("Xem phim", it)))) }
        }
        return FilmDetail(url, title, other, poster, desc, info, groups)
    }

    /** Danh sách tập theo nhóm, mỗi nhóm sắp tăng dần (trang web thường xếp tập mới nhất lên đầu). */
    private fun parseEpisodeGroups(doc: Document): List<EpisodeGroup> {
        // HaLim: mỗi <ul class="halim-list-eps"> là danh sách tập của một server (Việt Sub, Thuyết Minh…)
        val lists = doc.select("ul.halim-list-eps")
        if (lists.isNotEmpty()) {
            val tabLabels = doc.select(".hh3d-server-tab__label, .halim-server-tab .label")
            val groups = lists.mapIndexedNotNull { i, ul ->
                val eps = episodesFrom(ul.select("a[href]"))
                if (eps.isEmpty()) return@mapIndexedNotNull null
                val clean = { e: Element? -> e?.text()?.trim()?.trim('#', ':', ' ')?.takeIf { it.isNotEmpty() } }
                val name = clean(ul.closest(".halim-server")?.selectFirst(".halim-server-name"))
                    ?: clean(tabLabels.getOrNull(i))
                    ?: "Server ${i + 1}"
                EpisodeGroup(name, eps)
            }
            return groups.distinctBy { g -> g.episodes.map { it.url } }
        }
        var links = doc.select("a.ep-item[href]")
        if (links.isEmpty()) links = doc.select(".ss-list a[href], .episodes a[href]")
        if (links.isEmpty()) links = doc.select("a[href*=/tap-]")
        val eps = episodesFrom(links)
        return if (eps.isEmpty()) emptyList() else listOf(EpisodeGroup("Tập phim", eps))
    }

    private fun episodesFrom(links: List<Element>): List<Episode> {
        val map = LinkedHashMap<String, Episode>()
        for (a in links) {
            val u = a.absUrl("href")
            if (u.isEmpty() || u in map) continue
            var title = a.text().trim().ifEmpty { a.attr("title").trim() }
            if (title.matches(Regex("""\d+(\.\d+)?"""))) title = "Tập $title"
            map[u] = Episode(title, u)
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

    // ---- Trang tập phim ----

    fun source(epUrl: String): EpisodeSource = parseSource(fetch(epUrl), epUrl)

    fun parseSource(doc: Document, epUrl: String): EpisodeSource {
        NextSite.data(doc)?.let { return NextSite.parseSource(doc, it, epUrl) }
        val scripts = doc.select("script").joinToString("\n") { it.data() }
        val servers = doc.select("#list_sv [data-type], .get-eps[data-type], .btn3dsv[data-type]")
            .map { Server(it.text().trim(), it.attr("data-type")) }
            .filter { it.type.isNotEmpty() }.distinctBy { it.type }
            .map { if (it.label.isEmpty()) it.copy(label = it.type) else it }

        val active = doc.selectFirst("a.ep-item.active[data-post-id], .halim-episode.active a[data-post-id], .active > a[data-post-id]")
            ?: doc.selectFirst("[data-post-id][data-ep]")
        val postId = active?.attr("data-post-id")?.takeIf { it.isNotEmpty() }
            ?: Regex("""MovieInfo\s*=\s*\{\s*id:\s*(\d+)""").find(scripts)?.groupValues?.get(1)
            ?: ""
        val ep = active?.attr("data-ep")?.takeIf { it.isNotEmpty() }
            ?: epUrl.trimEnd('/').substringAfterLast('/').removeSuffix(".html")
        val sv = active?.attr("data-sv")?.takeIf { it.isNotEmpty() } ?: "1"

        // player.php kiểu "dox_ajax_player" (yanhh3d và các trang cùng nhóm)
        val playerUrl = if (scripts.contains("dox_ajax_player")) {
            Regex("""url:\s*['"]([^'"]*player\.php)['"]""").find(scripts)?.groupValues?.get(1)?.let { resolve(epUrl, it) }
                ?: resolve(epUrl, "/player/player.php")
        } else {
            null
        }
        // Bỏ qua iframe giữ chỗ src="" (absUrl của chuỗi rỗng ra chính địa chỉ trang)
        val direct = doc.select("#video-player iframe, .player-frame iframe, #halim-player-wrapper iframe")
            .firstOrNull { it.attr("src").let { s -> s.startsWith("http") || s.startsWith("//") } }
            ?.absUrl("src")
        val pageMode = (playerUrl == null || servers.isEmpty() || postId.isEmpty()) && direct == null
        return EpisodeSource(epUrl, servers, postId, ep, sv, playerUrl, direct, pageMode)
    }

    /** Hỏi player.php địa chỉ trình phát nhúng (iframe) của một server (trang Next.js: link .m3u8 / iframe). */
    fun video(src: EpisodeSource, server: Server?): Video {
        if (src.next) return NextSite.video(src, server ?: throw IOException("Không có server"))
        val playerUrl = src.playerUrl
        if (server == null || playerUrl == null) return Video(src.directEmbed ?: throw IOException("Không có server"))
        val url = playerUrl + "?action=dox_ajax_player&post_id=" + enc(src.postId) +
            "&chapter_st=" + enc(src.ep) + "&type=" + enc(server.type) + "&sv=" + enc(src.sv)
        val r = Http.get(url, mapOf("Referer" to src.epUrl, "X-Requested-With" to "XMLHttpRequest"))
        val doc = Jsoup.parse(r.body, src.epUrl)
        doc.selectFirst("iframe[src]")?.absUrl("src")?.takeIf { it.startsWith("http") }?.let { return Video(it) }
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
