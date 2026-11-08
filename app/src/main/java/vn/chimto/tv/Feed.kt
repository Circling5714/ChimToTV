package vn.chimto.tv

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest
import java.util.Calendar
import java.util.TimeZone
import java.util.concurrent.Executors

/** Thể loại / quốc gia / loại phim theo slug của KKPhim (hanh-dong, trung-quoc; loại: hoathinh, series, single, tvshows). */
data class Tags(val genres: List<String>, val countries: List<String>, val type: String?) {
    fun json(): JSONObject = JSONObject().put("g", JSONArray(genres)).put("c", JSONArray(countries)).put("t", type ?: "")

    companion object {
        fun of(o: JSONObject?): Tags? = o?.let {
            Tags(strings(it.optJSONArray("g")), strings(it.optJSONArray("c")), it.optString("t").ifEmpty { null })
        }

        /** category / country [{name, slug}] + type của KKPhim / VSMOV. */
        fun fromKk(m: JSONObject): Tags? {
            val g = slugs(m.optJSONArray("category"))
            val c = slugs(m.optJSONArray("country"))
            val t = (m.opt("type") as? String)?.trim()?.ifEmpty { null }
            return if (g.isEmpty() && c.isEmpty() && t == null) null else Tags(g, c, t)
        }

        private fun slugs(a: JSONArray?): List<String> = if (a == null) emptyList() else (0 until a.length()).mapNotNull { i ->
            a.optJSONObject(i)?.let { o -> o.optString("slug").ifEmpty { Feed.slug(o.optString("name")) }.ifEmpty { null } }
        }

        private fun strings(a: JSONArray?): List<String> =
            if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optString(it).ifEmpty { null } }
    }
}

/**
 * Một phim trong danh sách của một nguồn. names: các tên đã chuẩn hoá (KkPhim.norm — tên, tên gốc…) để gộp phim trùng giữa
 * các nguồn; updated: lúc nguồn cập nhật phim (ms; 0 = nguồn không ghi); ep: số tập mới nhất (null = không rõ / phim lẻ).
 */
data class FeedItem(
    val url: String, val title: String, val poster: String?, val badge: String?,
    val names: Set<String>, val year: Int?, val updated: Long, val ep: Double?, val tags: Tags?,
) {
    /** Tên miền nguồn (tính một lần — dùng rất nhiều lần khi gộp / xếp, đọc địa chỉ mỗi lần thì TV yếu chậm). */
    val host: String = Site.hostOf(url).orEmpty()

    fun json(): JSONObject = JSONObject().put("u", url).put("n", title).put("p", poster ?: "").put("b", badge ?: "")
        .put("k", JSONArray(names.toList())).put("y", year ?: 0).put("t", updated).put("e", ep ?: -1.0)
        .apply { tags?.let { put("g", it.json()) } }

    companion object {
        fun of(o: JSONObject): FeedItem? {
            val url = o.optString("u").ifEmpty { return null }
            val k = o.optJSONArray("k") ?: JSONArray()
            return FeedItem(
                url, o.optString("n").ifEmpty { return null }, o.optString("p").ifEmpty { null }, o.optString("b").ifEmpty { null },
                (0 until k.length()).map { k.optString(it) }.filter { it.isNotEmpty() }.toSet(),
                o.optInt("y").takeIf { it > 0 }, o.optLong("t"), o.optDouble("e", -1.0).takeIf { it >= 0 }, Tags.of(o.optJSONObject("g")),
            )
        }
    }
}

/** Phim đã gộp từ các nguồn (items: mỗi nguồn một mục, cùng một phim). */
class FeedFilm(val items: List<FeedItem>) {
    val names: Set<String> = items.flatMapTo(HashSet()) { it.names }
    val tags: Tags? get() = items.firstNotNullOfOrNull { it.tags }
    val year: Int? get() = items.firstNotNullOfOrNull { it.year }
    val updated: Long = items.maxOf { it.updated }
    val hosts: List<String> get() = items.map { it.host }.distinct()

    /** Nguồn có tập mới nhất (null = không nguồn nào ghi số tập). */
    val latest: FeedItem? get() = items.filter { it.ep != null }.maxByOrNull { it.ep!! }

    /** Xếp lại các nguồn theo thứ tự ưu tiên mở (host đứng trước trong prefer thì trước; không có trong prefer thì sau). */
    fun ordered(prefer: List<String>): FeedFilm {
        fun rank(i: FeedItem) = prefer.indexOf(i.host).let { if (it < 0) prefer.size else it }
        return FeedFilm(items.sortedBy(::rank))
    }

    fun matches(otherNames: Set<String>) = otherNames.any { it in names }
}

/**
 * Trang chủ tổng hợp (từ 2.8): đọc danh sách phim mới của các nguồn trong danh sách trang (API KKPhim / NguonC / VSMOV /
 * CLB Phim Xưa + trang web người xem hay dùng), gộp phim trùng giữa các nguồn, giữ bản lưu từng nguồn. Nguồn lỗi / quá hạn
 * không làm hỏng trang chủ: dùng bản lưu (dưới 24 giờ), lỗi 3 lần liên tiếp thì tạm nghỉ 30 phút (rồi 2 giờ, 6 giờ).
 */
object Feed {
    /** Mỗi nguồn chờ tối đa 6 giây; quá hạn thì dùng bản lưu, kết quả về muộn vẫn lưu cho lần sau. */
    const val TIMEOUT = 6_000L

    /** Cả lượt tải (kể cả nguồn phải chờ chỗ chạy) không quá 15 giây. */
    private const val TOTAL = 15_000L

    /** Bản lưu cũ hơn 24 giờ thì không dùng. */
    const val CACHE_MAX = 24 * 3_600_000L

    /** Nguồn không ghi giờ cập nhật (trang web, CLB Phim Xưa): coi như cứ 15 phút một phim, theo thứ tự trên trang. */
    private const val PSEUDO_GAP = 15 * 60_000L

    /** Lỗi liên tiếp → tạm nghỉ: 30 phút, 2 giờ, 6 giờ. */
    private const val FAILS_TO_REST = 3
    private val REST = longArrayOf(30 * 60_000L, 2 * 3_600_000L, 6 * 3_600_000L)

    private var dir: File? = null
    /**
     * Luồng tải ưu tiên thấp hơn màn hình một chút (4): TV yếu vẫn vẽ mượt trong lúc tải. Không dùng mức «nền» (10) — Android
     * dồn luồng nền vào nhóm bị giới hạn CPU, tải chậm hẳn (đo trên TV box: nguồn API quá 6 giây).
     */
    private val pool = Executors.newFixedThreadPool(6) { r ->
        Thread({
            runCatching { android.os.Process.setThreadPriority(4) }
            r.run()
        }, "feed").apply { isDaemon = true }
    }

    fun init(context: Context) {
        dir = File(context.filesDir, "feed").apply { mkdirs() }
    }

    // ---- Đọc danh sách của từng nguồn ----

    /** KKPhim (phimapi.com v1, cả bản qua www.kkphim.com/api/movies/…): có thể loại, quốc gia, loại phim, giờ cập nhật. */
    fun parseKk(body: String, origin: String): List<FeedItem> {
        val root = JSONObject(body)
        val data = root.optJSONObject("data") ?: root
        val cdn = str(data, "APP_DOMAIN_CDN_IMAGE").ifEmpty { str(root, "pathImage") }.ifEmpty { "https://phimimg.com" }
        return objects(data.optJSONArray("items")).mapNotNull { o ->
            val slug = str(o, "slug").ifEmpty { return@mapNotNull null }
            val name = str(o, "name").ifEmpty { return@mapNotNull null }
            val ep = str(o, "episode_current")
            FeedItem(
                "$origin/phim/$slug", name, ApiSite.image(str(o, "poster_url").ifEmpty { str(o, "thumb_url") }, cdn),
                NextSite.badge(ep), KkPhim.names(o), year(o), time(o.optJSONObject("modified")?.let { str(it, "time") }),
                epOf(ep), Tags.fromKk(o),
            )
        }.distinctBy { it.url }
    }

    /** NguonC /api/films/…: tên gốc (có thể nhiều tên), năm, tập hiện tại, giờ cập nhật; không có thể loại. */
    fun parseNc(body: String, origin: String): List<FeedItem> = objects(JSONObject(body).optJSONArray("items")).mapNotNull { o ->
        val slug = str(o, "slug").ifEmpty { return@mapNotNull null }
        val name = str(o, "name").ifEmpty { return@mapNotNull null }
        val ep = str(o, "current_episode")
        FeedItem(
            "$origin/phim/$slug", name, str(o, "thumb_url").ifEmpty { str(o, "poster_url") }.takeIf { it.startsWith("http") },
            NextSite.badge(ep), KkPhim.names(o), year(o), time(str(o, "modified")), epOf(ep), null,
        )
    }.distinctBy { it.url }

    /** Phim từ trang web / CLB Phim Xưa (chỉ có tên, ảnh, nhãn tập). */
    fun fromFilms(films: List<Film>): List<FeedItem> = films.map { f ->
        FeedItem(f.url, f.title, f.poster, f.badge, KkPhim.names(JSONObject().put("name", f.title)), null, 0, epOf(f.badge), null)
    }.distinctBy { it.url }

    /** Số tập trong nhãn: "Tập 108", "Hoàn Tất (16/16)", "108/120", "108"; "Full", "Top 1", "HD" → null. */
    fun epOf(label: String?): Double? {
        val s = label?.trim().orEmpty()
        if (s.isEmpty()) return null
        Regex("""(?i)t[aậ]p\s*(\d+(?:\.\d+)?)""").find(s)?.let { return it.groupValues[1].toDouble() }
        Regex("""(\d+)\s*/\s*(\d+|\?+)""").find(s)?.let { return it.groupValues[1].toDouble() }
        Regex("""^(\d+(?:\.\d+)?)$""").find(s)?.let { return it.groupValues[1].toDouble() }
        return null
    }

    private val ISO = Regex("""(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2}))?(?:\.\d+)?\s*(Z|[+-]\d{2}:?\d{2})?""")

    /** "2026-11-08T18:55:29.000Z", "2026-11-08T21:12:36+07:00" → ms; không ghi múi giờ thì coi là giờ Việt Nam. */
    fun time(s: String?): Long {
        val m = ISO.find(s ?: return 0) ?: return 0
        val g = m.groupValues
        val c = Calendar.getInstance(TimeZone.getTimeZone("UTC"))
        c.clear()
        c.set(g[1].toInt(), g[2].toInt() - 1, g[3].toInt(), g[4].toInt(), g[5].toInt(), g[6].ifEmpty { "0" }.toInt())
        val zone = g[7]
        val offset = when {
            zone == "Z" -> 0
            zone.isEmpty() -> 7 * 60
            else -> {
                val d = zone.substring(1).replace(":", "")
                val min = d.take(2).toInt() * 60 + d.drop(2).toInt()
                if (zone[0] == '-') -min else min
            }
        }
        return c.timeInMillis - offset * 60_000L
    }

    private fun year(o: JSONObject): Int? = o.opt("year")?.toString()?.trim()?.toIntOrNull()?.takeIf { it in 1900..2100 }

    /** "Hành Động" → "hanh-dong" (NguonC ghi tên thể loại, không có slug). */
    fun slug(name: String) = KkPhim.norm(name).replace(' ', '-')

    // ---- Gộp phim trùng ----

    /**
     * Gộp các danh sách (thứ tự = ưu tiên). Hai mục là một phim khi có tên chung (đã chuẩn hoá), năm không lệch quá 1 và
     * không cùng một nguồn (một nguồn không có hai bản cùng phim — hai mục cùng tên của một trang là hai phần khác nhau).
     * Mục không ghi giờ cập nhật được tính giờ giả theo thứ tự trên trang. Kết quả xếp mới cập nhật trước.
     */
    fun merge(lists: List<List<FeedItem>>, now: Long = System.currentTimeMillis()): List<FeedFilm> {
        val groups = ArrayList<MutableList<FeedItem>>()
        val byName = HashMap<String, MutableList<Int>>()
        val byUrl = HashSet<String>()
        for (list in lists) {
            for ((rank, raw) in list.withIndex()) {
                // Cùng một phim có trong nhiều danh sách của một nguồn (mới cập nhật + theo thể loại…): đã có nhóm rồi
                if (!byUrl.add(raw.url)) continue
                val item = if (raw.updated > 0) raw else raw.copy(updated = now - (rank + 1) * PSEUDO_GAP)
                val hit = item.names.asSequence().flatMap { byName[it].orEmpty().asSequence() }.distinct().firstOrNull { gi ->
                    val g = groups[gi]
                    val y = g.firstNotNullOfOrNull { it.year }
                    g.none { it.host == item.host || it.url == item.url } &&
                        (y == null || item.year == null || kotlin.math.abs(y - item.year) <= 1)
                }
                val gi = hit ?: groups.size.also { groups += ArrayList<FeedItem>() }
                if (groups[gi].any { it.url == item.url }) continue
                groups[gi] += item
                item.names.forEach { n -> byName.getOrPut(n) { ArrayList() }.let { if (gi !in it) it += gi } }
            }
        }
        return groups.map { FeedFilm(it) }.sortedByDescending { it.updated }
    }

    // ---- Tải các nguồn: song song, hạn 6 giây, bản lưu, tạm nghỉ ----

    /**
     * Một lần đọc: key = tên bản lưu (thường là địa chỉ), host = nguồn (để tính lỗi / tạm nghỉ); keep = false: không lưu
     * bản (vd tra thể loại phim).
     */
    class Req(val key: String, val host: String, val keep: Boolean = true, val load: () -> Loaded)

    /** movedTo: trang web đã chuyển sang tên miền khác (gốc mới) — không lưu vào bản lưu. */
    class Loaded(val items: List<FeedItem>, val categories: List<Category>? = null, val movedTo: String? = null)

    enum class State { OK, CACHED, NO_DATA }

    /**
     * Kết quả của một nguồn. state: OK = vừa tải; CACHED = dùng bản lưu (nguồn lỗi / quá hạn / tạm nghỉ); NO_DATA = không
     * có gì. error: lý do lỗi (null nếu tải được); resting: nguồn đang tạm nghỉ (không hỏi lần này); ms: thời gian tải.
     */
    class Result(
        val req: Req, val state: State, val items: List<FeedItem>, val categories: List<Category>?,
        val error: String?, val resting: Boolean, val ms: Long, val savedAt: Long, val movedTo: String? = null,
    )

    private class Job(val req: Req) {
        @Volatile var startedAt = 0L
        @Volatile var done: Loaded? = null
        @Volatile var failed: Throwable? = null
        @Volatile var finishedAt = 0L

        /** Đã tính vào số lỗi liên tiếp (quá hạn) — về muộn mà lỗi thì không tính thêm lần nữa. */
        @Volatile var counted = false
    }

    /** Tải các nguồn (gọi từ luồng nền). Trả về khi mọi nguồn xong / quá hạn — nguồn quá hạn vẫn chạy tiếp để lưu bản mới. */
    fun load(reqs: List<Req>): List<Result> {
        val t0 = System.currentTimeMillis()
        val jobs = ArrayList<Job?>()
        // Một lượt tải có thể hỏi một nguồn nhiều lần (KKPhim: mới cập nhật + các hàng gợi ý) — chỉ tính một lần lỗi mỗi lượt
        val failedHosts = java.util.Collections.newSetFromMap(java.util.concurrent.ConcurrentHashMap<String, Boolean>())
        fun failedOnce(host: String) {
            if (failedHosts.add(host)) failed(host)
        }
        val results = arrayOfNulls<Result>(reqs.size)
        for ((i, r) in reqs.withIndex()) {
            if (resting(r.host)) {
                results[i] = cached(r, null, resting = true, ms = 0)
                jobs += null
                continue
            }
            val job = Job(r)
            pool.execute {
                job.startedAt = System.currentTimeMillis()
                try {
                    // Bản debug: tệp files/feed_fail_<tên miền> (hoặc feed_fail_all) → nguồn đó giả lỗi, để thử trên TV thật
                    if (BuildConfig.DEBUG) dir?.parentFile?.let { d ->
                        if (File(d, "feed_fail_${r.host}").exists() || File(d, "feed_fail_all").exists()) {
                            Thread.sleep(400)
                            throw java.io.IOException("máy chủ trả lỗi 502 (giả lỗi — bản debug)")
                        }
                    }
                    val got = r.load()
                    job.done = got
                    if (r.keep) save(r.key, got)
                    healthy(r.host)
                    if (BuildConfig.DEBUG && job.counted) {
                        android.util.Log.d("ChimToFeed", "${r.key} về muộn sau ${System.currentTimeMillis() - job.startedAt} ms (đã lưu)")
                    }
                } catch (e: Throwable) {
                    job.failed = e
                    if (BuildConfig.DEBUG) android.util.Log.d("ChimToFeed", "${r.key} lỗi sau ${System.currentTimeMillis() - job.startedAt} ms: $e")
                    if (!job.counted) failedOnce(r.host)
                } finally {
                    job.finishedAt = System.currentTimeMillis()
                }
            }
            jobs += job
        }
        while (true) {
            val now = System.currentTimeMillis()
            var waiting = false
            for ((i, j) in jobs.withIndex()) {
                if (j == null || results[i] != null) continue
                val ms = (if (j.finishedAt > 0) j.finishedAt else now) - (j.startedAt.takeIf { it > 0 } ?: now)
                when {
                    j.done != null -> j.done!!.let { d ->
                        results[i] = Result(j.req, State.OK, d.items, d.categories, null, false, ms, now, d.movedTo)
                    }
                    j.failed != null -> results[i] = cached(j.req, friendly(j.failed!!), resting = false, ms = ms)
                    j.startedAt > 0 && now - j.startedAt > TIMEOUT -> {
                        // Nguồn chạy quá 6 giây: tính một lần lỗi (về muộn vẫn lưu bản mới cho lần sau)
                        j.counted = true
                        failedOnce(j.req.host)
                        results[i] = cached(j.req, "quá ${TIMEOUT / 1000} giây", resting = false, ms = ms)
                    }
                    // Hết thời gian cả lượt mà phần này mới chạy / chưa tới lượt (TV yếu, nhiều phần): dùng bản lưu, không
                    // tính là lỗi của nguồn
                    now - t0 > TOTAL -> results[i] = cached(j.req, null, resting = false, ms = ms)
                    else -> waiting = true
                }
            }
            if (!waiting) break
            Thread.sleep(80)
        }
        if (BuildConfig.DEBUG) results.forEach { r ->
            android.util.Log.d("ChimToFeed", "${r!!.req.key} ${r.state} ${r.ms} ms ${r.items.size} phim ${r.error ?: ""}${if (r.resting) " (tạm nghỉ)" else ""}")
        }
        return results.map { it!! }
    }

    private fun friendly(e: Throwable): String = when (e) {
        is Http.HttpError -> "lỗi ${e.code}"
        is java.net.UnknownHostException -> "không tìm thấy máy chủ"
        is java.net.SocketTimeoutException -> "không trả lời"
        is java.io.IOException -> e.message?.take(60) ?: "lỗi mạng"
        else -> "đọc dữ liệu lỗi"
    }

    private fun cached(r: Req, error: String?, resting: Boolean, ms: Long): Result {
        val c = if (r.keep) readCache(r.key) else null
        return if (c != null) Result(r, State.CACHED, c.first.items, c.first.categories, error, resting, ms, c.second)
        else Result(r, State.NO_DATA, emptyList(), null, error, resting, ms, 0)
    }

    /** Bản lưu của một nguồn (dưới 24 giờ) — trang chủ hiện ngay lúc mở app trong khi tải bản mới. */
    fun readCache(key: String): Pair<Loaded, Long>? {
        val f = fileOf(key) ?: return null
        return runCatching {
            if (!f.exists()) return null
            val o = JSONObject(f.readText())
            val at = o.optLong("at")
            if (System.currentTimeMillis() - at > CACHE_MAX) return null
            val a = o.optJSONArray("items") ?: JSONArray()
            val items = (0 until a.length()).mapNotNull { a.optJSONObject(it)?.let(FeedItem::of) }
            val cats = o.optJSONArray("cats")?.let { c ->
                (0 until c.length()).mapNotNull { i -> c.optJSONObject(i)?.let { Category(it.optString("t"), it.optString("u")) } }
            }
            Loaded(items, cats) to at
        }.getOrNull()
    }

    private fun save(key: String, l: Loaded) {
        val f = fileOf(key) ?: return
        runCatching {
            val o = JSONObject().put("at", System.currentTimeMillis()).put("items", JSONArray(l.items.map { it.json() }))
            l.categories?.let { cats -> o.put("cats", JSONArray(cats.map { JSONObject().put("t", it.title).put("u", it.url) })) }
            val tmp = File(f.path + ".tmp")
            tmp.writeText(o.toString())
            tmp.renameTo(f)
        }
    }

    /** Tệp lưu tự do theo khoá (vd trang chủ lần trước của từng người xem). */
    fun writeText(key: String, text: String) {
        val f = fileOf(key) ?: return
        runCatching {
            val tmp = File(f.path + ".tmp")
            tmp.writeText(text)
            tmp.renameTo(f)
        }
    }

    fun readText(key: String): String? = fileOf(key)?.takeIf { it.exists() }?.let { runCatching { it.readText() }.getOrNull() }

    private fun fileOf(key: String): File? {
        val d = dir ?: return null
        val h = MessageDigest.getInstance("MD5").digest(key.toByteArray()).joinToString("") { "%02x".format(it) }
        return File(d, "$h.json")
    }

    // ---- Tình trạng nguồn ----

    /** host → {f: lỗi liên tiếp, r: tạm nghỉ tới (ms), l: số lần đã tạm nghỉ liên tiếp}. */
    @Synchronized
    private fun health(): JSONObject = runCatching { JSONObject(Prefs.feedHealth) }.getOrDefault(JSONObject())

    @Synchronized
    fun resting(host: String): Boolean = (health().optJSONObject(host)?.optLong("r") ?: 0) > System.currentTimeMillis()

    /** Nguồn đang tạm nghỉ: host → tạm nghỉ tới (ms). */
    @Synchronized
    fun restingHosts(): Map<String, Long> {
        val h = health()
        val now = System.currentTimeMillis()
        return h.keys().asSequence().mapNotNull { k -> h.optJSONObject(k)?.optLong("r")?.takeIf { it > now }?.let { k to it } }.toMap()
    }

    @Synchronized
    private fun healthy(host: String) {
        val h = health()
        if (!h.has(host)) return
        h.remove(host)
        Prefs.feedHealth = h.toString()
    }

    @Synchronized
    private fun failed(host: String) {
        val h = health()
        val o = h.optJSONObject(host) ?: JSONObject()
        val f = o.optInt("f") + 1
        o.put("f", f)
        if (f >= FAILS_TO_REST) {
            val level = o.optInt("l")
            o.put("r", System.currentTimeMillis() + REST[level.coerceAtMost(REST.size - 1)]).put("l", level + 1).put("f", 0)
        }
        h.put(host, o)
        Prefs.feedHealth = h.toString()
    }

    /** «Thử lại ngay» trong Cài đặt: bỏ tạm nghỉ của mọi nguồn. */
    @Synchronized
    fun clearRest() {
        Prefs.feedHealth = "{}"
    }

    private fun objects(a: JSONArray?): List<JSONObject> =
        if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }

    private fun str(o: JSONObject, key: String): String = (o.opt(key) as? String)?.trim().orEmpty()
}
