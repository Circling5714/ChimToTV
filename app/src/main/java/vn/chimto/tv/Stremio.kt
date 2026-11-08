package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.util.Locale

/**
 * CLB Phim Xưa qua add-on Stremio «K20 Phim Tổng Hợp» (sc.k-20.xyz, từ 2.7): phim Hồng Kông / TVB / kiếm hiệp cũ. Add-on
 * theo giao thức Stremio: /catalog/movie/clbpx-movie[/genre=…&search=…&skip=N].json (30 phim mỗi lần),
 * /meta/movie/clbpx:<slug>.json (videos = các tập; phim lẻ không có), /stream/movie/<mã tập>.json (link MP4 qua máy chủ
 * trung gian của k-20: mở chậm ~10 s, tua ~10–20 s). Chỉ trả dữ liệu cho User-Agent kiểu trình duyệt.
 *
 * Địa chỉ trong app: danh sách = địa chỉ catalog; phim = https://sc.k-20.xyz/clbpx/<slug>; tập = …/clbpx/<slug>/<mã tập>
 * (phim lẻ: …/clbpx/<slug>/full).
 */
object Stremio {
    const val HOST = "sc.k-20.xyz"

    /** Tên hiện trong app (tên miền của add-on không nói lên gì). */
    const val NAME = "CLB Phim Xưa"

    private const val TYPE = "movie"
    private const val CATALOG = "clbpx-movie"
    private const val ID = "clbpx"
    private const val FULL = "full"

    /** Thể loại của catalog (manifest: extra genre). Mục đầu = mới cập nhật. */
    private val GENRES = listOf(
        "Mới Cập Nhật", "Kiếm Hiệp", "Tiên Hiệp", "Tâm Lý", "Ma Kinh Dị", "Điện Ảnh Châu Á", "Điện Ảnh Âu Mỹ", "Hàn Quốc", "Anime",
        "TV Series", "Thập Niên 60", "Thập Niên 70", "Thập Niên 80", "Thập Niên 90", "Thập Niên 2000",
    )

    fun isSite(url: String) = Site.hostOf(url) == HOST

    fun home(base: String): Home {
        val o = ApiSite.origin(base)
        val blocks = listOf("Mới cập nhật", "Kiếm Hiệp", "TV Series", "Thập Niên 90", "Thập Niên 80")
            .map { it to catalog(o, genre = if (it == "Mới cập nhật") null else it) }
        val sections = ApiSite.sections(blocks) { parseList(get(it), it).films }
        return Home(sections, GENRES.drop(1).map { Category(it, catalog(o, genre = it)) }, null)
    }

    fun newestUrl(base: String) = catalog(ApiSite.origin(base))

    fun searchUrl(base: String, query: String) = catalog(ApiSite.origin(base), search = query.trim())

    fun filters(base: String): Pair<List<Category>, List<Category>> {
        val o = ApiSite.origin(base)
        return GENRES.map { Category(it, catalog(o, genre = it)) } to emptyList()
    }

    /** Địa chỉ catalog; tham số theo kiểu Stremio: /catalog/movie/clbpx-movie/genre=…&skip=30.json. */
    fun catalog(origin: String, genre: String? = null, search: String? = null, skip: Int = 0): String {
        val extra = listOfNotNull(
            genre?.let { "genre=" + enc("Thể loại: $it") },
            search?.let { "search=" + enc(it) },
            skip.takeIf { it > 0 }?.let { "skip=$it" },
        )
        return "$origin/catalog/$TYPE/$CATALOG" + (if (extra.isEmpty()) "" else "/" + extra.joinToString("&")) + ".json"
    }

    fun list(url: String): FilmPage = parseList(get(url), url)

    /** {metas:[{id:"clbpx:<slug>", name, poster}]}; trang sau = skip + số phim (hết khi trả rỗng; tìm kiếm chỉ một trang). */
    fun parseList(body: String, url: String): FilmPage {
        val o = ApiSite.origin(url)
        val films = objects(JSONObject(body).optJSONArray("metas")).mapNotNull { m ->
            val slug = str(m, "id").removePrefix("$ID:").takeIf { it != str(m, "id") && it.isNotEmpty() } ?: return@mapNotNull null
            val name = str(m, "name").ifEmpty { return@mapNotNull null }
            Film("$o/$ID/$slug", name, str(m, "poster").takeIf { it.startsWith("http") }, badge(name))
        }.distinctBy { it.url }
        val extra = extraOf(url)
        val next = if (films.isEmpty() || "search" in extra) null else {
            val skip = (extra["skip"]?.toIntOrNull() ?: 0) + films.size
            catalog(o, extra["genre"]?.removePrefix("Thể loại: "), null, skip)
        }
        return FilmPage(films, next)
    }

    /** "Nguyên Chấn Hiệp 1993 – 20 Tập" → "20 tập" (tên phim của CLB Phim Xưa ghi số tập ở cuối). */
    private fun badge(name: String): String? =
        Regex("""(\d+)\s*Tập\s*$""", RegexOption.IGNORE_CASE).find(name)?.groupValues?.get(1)?.let { "$it tập" }

    /** Tham số trong đoạn cuối địa chỉ catalog: "genre=Th%E1…&skip=30.json" → {genre: "Thể loại: …", skip: "30"}. */
    private fun extraOf(url: String): Map<String, String> {
        val last = URI(url).rawPath.substringAfterLast('/').removeSuffix(".json")
        if ('=' !in last) return emptyMap()
        return last.split('&').mapNotNull { p ->
            val (k, v) = p.split('=', limit = 2).takeIf { it.size == 2 } ?: return@mapNotNull null
            k to URLDecoder.decode(v, "UTF-8")
        }.toMap()
    }

    fun detail(url: String): FilmDetail {
        val film = filmUrl(url)
        return parseDetail(get(metaUrl(film)), film)
    }

    /** {meta:{name, poster, background, description, releaseInfo, genres, videos:[{id, title, season, episode}]}}. */
    fun parseDetail(body: String, url: String): FilmDetail {
        val m = JSONObject(body).optJSONObject("meta") ?: throw IOException("Không đọc được thông tin phim")
        val name = str(m, "name").ifEmpty { url }
        val prefix = str(m, "id") + ":"
        val videos = objects(m.optJSONArray("videos"))
            .sortedWith(compareBy({ it.optInt("season", 1) }, { it.optInt("episode", 0) }))
        val eps = if (videos.isEmpty()) {
            listOf(Episode("Xem phim", "$url/$FULL"))
        } else {
            videos.mapNotNull { v ->
                val code = str(v, "id").removePrefix(prefix).takeIf { it != str(v, "id") && it.isNotEmpty() } ?: return@mapNotNull null
                val t = str(v, "title").ifEmpty { "Tập ${v.optInt("episode")}" }
                Episode(Regex("""^(Tập\s*)0+(\d)""", RegexOption.IGNORE_CASE).replace(t) { it.groupValues[1] + it.groupValues[2] }, "$url/${enc(code)}")
            }.distinctBy { it.url }
        }
        val genres = (0 until (m.optJSONArray("genres")?.length() ?: 0)).mapNotNull { m.optJSONArray("genres")?.optString(it) }
        val info = listOf(str(m, "releaseInfo"), genres.joinToString(", ")).filter { it.isNotEmpty() }.joinToString("  •  ").ifEmpty { null }
        val desc = str(m, "description").ifEmpty { null }
        return FilmDetail(url, name, null, str(m, "poster").takeIf { it.startsWith("http") }, desc, info,
            listOf(EpisodeGroup("Tập phim", eps)))
    }

    /** Mỗi link MP4 là một server: "720p", "1080p · 3,1 GB"… (cùng chất lượng có nhiều bản thì ghi dung lượng). */
    fun source(epUrl: String): EpisodeSource {
        val film = filmUrl(epUrl)
        val code = URLDecoder.decode(URI(epUrl).rawPath.trimEnd('/').substringAfterLast('/'), "UTF-8")
        val slug = film.substringAfterLast('/')
        val id = if (code == FULL) "$ID:$slug" else "$ID:$slug:$code"
        return parseSource(get("${ApiSite.origin(film)}/stream/$TYPE/${enc(id)}.json"), epUrl)
    }

    fun parseSource(body: String, epUrl: String): EpisodeSource {
        val streams = objects(JSONObject(body).optJSONArray("streams")).filter { str(it, "url").startsWith("http") }
        if (streams.isEmpty()) throw IOException("Tập này chưa có video")
        val quality = streams.map { str(it, "title").lineSequence().first().trim().ifEmpty { "MP4" } }
        val servers = ArrayList<Server>()
        val links = LinkedHashMap<String, Video>()
        streams.forEachIndexed { i, s ->
            val size = Regex("""[?&]size=(\d+)""").find(str(s, "url"))?.groupValues?.get(1)?.toLongOrNull()
            val label = quality[i] + if (quality.count { it == quality[i] } > 1 && size != null) " · " + gb(size) else ""
            val type = "mp4-$i"
            servers += Server(label, type)
            links[type] = Video(str(s, "url"), file = true)
        }
        return EpisodeSource(epUrl, servers, "", "", "", null, null, false, links, next = true)
    }

    private fun gb(bytes: Long) = String.format(Locale.US, "%.1f GB", bytes / 1e9).replace('.', ',')

    /** ".../clbpx/nch1993/LM4bPc-3a" → ".../clbpx/nch1993". */
    fun filmUrl(url: String): String {
        val parts = URI(url).path.trim('/').split('/')
        val i = parts.indexOf(ID)
        val slug = parts.getOrNull(i + 1)?.takeIf { i >= 0 && it.isNotEmpty() } ?: throw IOException("Địa chỉ phim lạ: $url")
        return "${ApiSite.origin(url)}/$ID/$slug"
    }

    private fun metaUrl(film: String) = "${ApiSite.origin(film)}/meta/$TYPE/" + enc("$ID:" + film.substringAfterLast('/')) + ".json"

    private fun get(url: String) = try {
        Http.get(url, mapOf("Accept" to "application/json")).body
    } catch (e: Http.HttpError) {
        if (e.code == 404) throw IOException("CLB Phim Xưa không có phim / tập này") else throw e
    }

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8").replace("+", "%20")

    private fun objects(a: JSONArray?): List<JSONObject> =
        if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }

    private fun str(o: JSONObject, key: String): String = (o.opt(key) as? String)?.trim().orEmpty()
}
