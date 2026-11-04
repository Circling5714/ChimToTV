package vn.chimto.tv

import android.content.Context
import android.content.SharedPreferences
import org.json.JSONArray
import org.json.JSONObject
import java.net.URL

data class HistoryItem(
    val filmUrl: String, val title: String, val poster: String?,
    val epTitle: String, val epUrl: String, val time: Long,
)

/** Phim theo dõi: count = số tập lần kiểm gần nhất, seen = số tập lúc người xem mở phim lần cuối. */
data class Follow(
    val filmUrl: String, val title: String, val poster: String?,
    val count: Int, val latest: String, val seen: Int,
) {
    val newCount get() = (count - seen).coerceAtLeast(0)
}

/** Cài đặt + lịch sử xem, lưu trong SharedPreferences. */
object Prefs {
    const val DEFAULT_BASE = "https://yanhh3d.ee"

    /** Các trang mẫu có sẵn trong danh sách lần đầu cài. */
    val PRESET_SITES = listOf(DEFAULT_BASE, "https://hoathinh3d.pw", "https://hoathinh3d.ee", "https://hhkungfu.ee", "https://phimmoir.fun")

    /** Trang mẫu thêm ở bản sau: thêm một lần vào danh sách người xem đã lưu (xoá đi thì thôi, không thêm lại). */
    private val ADDED_SITES = listOf("https://phimmoir.fun")
    private const val MAX_HISTORY = 60
    private const val MAX_POSITIONS = 400

    private lateinit var sp: SharedPreferences

    fun init(context: Context) {
        sp = context.getSharedPreferences("chimto", Context.MODE_PRIVATE)
        for (url in ADDED_SITES) {
            val key = "preset_added@$url"
            if (sp.getBoolean(key, false)) continue
            if (sp.contains("sites") && url !in sites) sites = sites + url
            sp.edit().putBoolean(key, true).apply()
        }
    }

    /** Trang phim đang dùng (địa chỉ gốc, không có dấu / ở cuối). Đặt trang mới thì tự thêm vào danh sách. */
    var baseUrl: String
        get() = sp.getString("base_url", null) ?: DEFAULT_BASE
        set(value) {
            sp.edit().putString("base_url", value).apply()
            if (value !in sites) sites = sites + value
        }

    /** Danh sách trang phim để chọn nhanh (mặc định: các trang mẫu; bản 1.x cũ lưu ở "recent_urls"). */
    var sites: List<String>
        get() {
            val s = sp.getString("sites", null) ?: sp.getString("recent_urls", null)
            val saved = s?.let { j -> JSONArray(j).let { a -> List(a.length()) { a.getString(it) } } }.orEmpty()
            return if (sp.contains("sites")) saved else (PRESET_SITES + saved).distinct()
        }
        set(value) = sp.edit().putString("sites", JSONArray(value.distinct()).toString()).apply()

    /** data-type của server người xem chọn lần gần nhất trên từng trang (vd "tiktik"). */
    fun serverType(host: String?): String? =
        sp.getString("server_type@$host", null) ?: sp.getString("server_type", null)

    fun serverLabel(host: String?): String? =
        sp.getString("server_label@$host", null) ?: sp.getString("server_label", null)

    fun setServer(host: String?, type: String?, label: String?) {
        sp.edit().putString("server_type@$host", type).putString("server_label@$host", label)
            .remove("server_type").remove("server_label").apply()
    }

    var seekStep: Int
        get() = sp.getInt("seek_step", 10)
        set(value) = sp.edit().putInt("seek_step", value).apply()

    var autoNext: Boolean
        get() = sp.getBoolean("auto_next", true)
        set(value) = sp.edit().putBoolean("auto_next", value).apply()

    /** Điểm các lần xem theo trang + server (xem PlayStats). */
    var playStats: JSONObject
        get() = try {
            JSONObject(sp.getString("play_stats", "{}") ?: "{}")
        } catch (e: Exception) {
            JSONObject()
        }
        set(value) = sp.edit().putString("play_stats", value.toString()).apply()

    /** User-Agent rút gọn dựng từ WebView của máy (xem Privacy), để Http.kt có ngay lúc mở app. */
    var userAgent: String?
        get() = sp.getString("ua_reduced", null)
        set(value) = sp.edit().putString("ua_reduced", value).apply()

    // ---- Proxy SOCKS5 (xem AppProxy) ----

    var proxyOn: Boolean
        get() = sp.getBoolean("proxy_on", false)
        set(value) = sp.edit().putBoolean("proxy_on", value).apply()

    var proxyHost: String
        get() = sp.getString("proxy_host", "") ?: ""
        set(value) = sp.edit().putString("proxy_host", value).apply()

    var proxyPort: Int
        get() = sp.getInt("proxy_port", 1080)
        set(value) = sp.edit().putInt("proxy_port", value).apply()

    var proxyUser: String
        get() = sp.getString("proxy_user", "") ?: ""
        set(value) = sp.edit().putString("proxy_user", value).apply()

    /** Lưu trong bộ nhớ riêng của app (app khác không đọc được), không mã hoá. */
    var proxyPass: String
        get() = sp.getString("proxy_pass", "") ?: ""
        set(value) = sp.edit().putString("proxy_pass", value).apply()

    /** true: DNS của proxy; false: DNS của thiết bị. */
    var proxyRemoteDns: Boolean
        get() = sp.getBoolean("proxy_remote_dns", true)
        set(value) = sp.edit().putBoolean("proxy_remote_dns", value).apply()

    /** true: chỉ trang phim đi qua proxy; false: toàn bộ app. */
    var proxySitesOnly: Boolean
        get() = sp.getBoolean("proxy_sites_only", false)
        set(value) = sp.edit().putBoolean("proxy_sites_only", value).apply()

    /** Proxy đang dùng; null = tắt (hoặc chưa nhập máy chủ / cổng hợp lệ). */
    fun proxyConfig(): SocksConfig? {
        if (!proxyOn) return null
        val host = proxyHost.trim()
        if (host.isEmpty() || proxyPort !in 1..65535) return null
        return SocksConfig(host, proxyPort, proxyUser, proxyPass, proxyRemoteDns, proxySitesOnly)
    }

    // ---- Cập nhật OTA ----

    var autoUpdate: Boolean
        get() = sp.getBoolean("auto_update", true)
        set(value) = sp.edit().putBoolean("auto_update", value).apply()

    var lastUpdateCheck: Long
        get() = sp.getLong("last_update_check", 0)
        set(value) = sp.edit().putLong("last_update_check", value).apply()

    /** Người xem chọn "Để sau" cho bản này → không hỏi lại tới thời điểm này. */
    fun snoozeUpdate(version: String, until: Long) {
        sp.edit().putString("snooze_version", version).putLong("snooze_until", until).apply()
    }

    fun isUpdateSnoozed(version: String): Boolean =
        sp.getString("snooze_version", null) == version && System.currentTimeMillis() < sp.getLong("snooze_until", 0)

    /** Chuẩn hoá chuỗi người dùng gõ ("yanhh3d.ee", "https://yanhh3d.ee/") → "https://yanhh3d.ee"; sai thì null. */
    fun normalizeUrl(input: String): String? {
        var s = input.trim().trimEnd('/')
        if (s.isEmpty() || s.contains(' ')) return null
        if (!s.startsWith("http://", true) && !s.startsWith("https://", true)) s = "https://$s"
        return try {
            val u = URL(s)
            if (u.host.isNullOrBlank() || !u.host.contains('.')) null else s
        } catch (e: Exception) {
            null
        }
    }

    // ---- Lịch sử xem ----

    val history: List<HistoryItem>
        get() {
            val a = JSONArray(sp.getString("history", "[]"))
            return List(a.length()) { i ->
                val o = a.getJSONObject(i)
                HistoryItem(
                    o.getString("film"), o.getString("title"), o.optString("poster").ifEmpty { null },
                    o.getString("ep"), o.getString("epUrl"), o.getLong("time"),
                )
            }
        }

    fun historyFor(filmUrl: String): HistoryItem? = history.firstOrNull { it.filmUrl == filmUrl }

    fun addHistory(item: HistoryItem) {
        val list = listOf(item) + history.filter { it.filmUrl != item.filmUrl }
        sp.edit().putString("history", historyJson(list.take(MAX_HISTORY))).apply()
    }

    private fun historyJson(list: List<HistoryItem>): String {
        val a = JSONArray()
        list.forEach {
            a.put(
                JSONObject().put("film", it.filmUrl).put("title", it.title).put("poster", it.poster ?: "")
                    .put("ep", it.epTitle).put("epUrl", it.epUrl).put("time", it.time)
            )
        }
        return a.toString()
    }

    fun clearHistory() {
        privatePositions.clear()
        sp.edit().remove("history").remove("positions").remove("pos_film").apply()
    }

    /** Xoá một phim khỏi lịch sử, kèm chỗ đang xem / đánh dấu đã xem của các tập phim đó. */
    fun removeHistory(filmUrl: String) {
        val item = historyFor(filmUrl)
        val o = positions()
        val films = posFilms()
        // Vị trí lưu trước 1.8.2 không ghi phim → chỉ biết tập cuối trong lịch sử
        val eps = films.keys().asSequence().filter { films.optString(it) == filmUrl }.toList() + listOfNotNull(item?.epUrl)
        eps.forEach {
            o.remove(it)
            films.remove(it)
            privatePositions.remove(it)
        }
        sp.edit().putString("history", historyJson(history.filter { it.filmUrl != filmUrl }))
            .putString("positions", o.toString())
            .putString("pos_film", films.toString()).apply()
    }

    // ---- Phim theo dõi ----

    val follows: List<Follow>
        get() {
            val a = JSONArray(sp.getString("follows", "[]"))
            return List(a.length()) { i ->
                val o = a.getJSONObject(i)
                Follow(
                    o.getString("film"), o.getString("title"), o.optString("poster").ifEmpty { null },
                    o.optInt("count"), o.optString("latest"), o.optInt("seen"),
                )
            }
        }

    private fun saveFollows(list: List<Follow>) {
        val a = JSONArray()
        list.forEach {
            a.put(
                JSONObject().put("film", it.filmUrl).put("title", it.title).put("poster", it.poster ?: "")
                    .put("count", it.count).put("latest", it.latest).put("seen", it.seen)
            )
        }
        sp.edit().putString("follows", a.toString()).apply()
    }

    fun followOf(filmUrl: String): Follow? = follows.firstOrNull { it.filmUrl == filmUrl }

    fun follow(d: FilmDetail) {
        val n = d.episodeCount
        saveFollows(listOf(Follow(d.url, d.title, d.poster, n, d.latestEpisode?.title.orEmpty(), n)) +
            follows.filter { it.filmUrl != d.url })
    }

    fun unfollow(filmUrl: String) = saveFollows(follows.filter { it.filmUrl != filmUrl })

    /** Cập nhật số tập mới nhất; seen = true: người xem vừa mở phim (hết "tập mới"). */
    fun updateFollow(filmUrl: String, count: Int, latest: String, seen: Boolean) {
        val list = follows
        if (list.none { it.filmUrl == filmUrl }) return
        saveFollows(
            list.map {
                if (it.filmUrl != filmUrl) it
                // Trang bớt tập (gộp / sửa danh sách) thì không để "tập mới" kẹt mãi
                else it.copy(count = count, latest = latest, seen = if (seen || count < it.seen) count else it.seen)
            }
        )
    }

    var lastFollowCheck: Long
        get() = sp.getLong("last_follow_check", 0)
        set(value) = sp.edit().putLong("last_follow_check", value).apply()

    // ---- Nhạc mở đầu (mốc riêng từng phim, giây) ----

    /** (đầu, hết) nhạc mở đầu; hết = -1 là mới đánh dấu đầu. */
    fun intro(filmUrl: String): Pair<Int, Int>? {
        val p = sp.getString("intro@$filmUrl", null)?.split(',') ?: return null
        return (p.getOrNull(0)?.toIntOrNull() ?: return null) to (p.getOrNull(1)?.toIntOrNull() ?: -1)
    }

    fun setIntro(filmUrl: String, start: Int, end: Int) = sp.edit().putString("intro@$filmUrl", "$start,$end").apply()

    fun clearIntro(filmUrl: String) = sp.edit().remove("intro@$filmUrl").apply()

    // ---- Riêng tư ----

    /** Xem riêng: không ghi lịch sử xem và chỗ đang xem (vẫn đọc được chỗ đã lưu từ trước). */
    var privateMode: Boolean
        get() = sp.getBoolean("private_mode", false)
        set(value) = sp.edit().putBoolean("private_mode", value).apply()

    /** Tự xoá dữ liệu duyệt web khi rời app (xem AutoClear). */
    var autoClearWeb: Boolean
        get() = sp.getBoolean("auto_clear_web", false)
        set(value) = sp.edit().putBoolean("auto_clear_web", value).apply()

    /** WebView đã chạy từ lần xoá dữ liệu duyệt web gần nhất (mặc định true: chưa xoá lần nào). */
    var webDirty: Boolean
        get() = sp.getBoolean("web_dirty", true)
        set(value) = sp.edit().putBoolean("web_dirty", value).apply()

    // ---- Vị trí đang xem từng tập (giây) ----

    private fun positions(): JSONObject = JSONObject(sp.getString("positions", "{}") ?: "{}")

    /** Xem riêng: chỗ đang xem chỉ giữ trong bộ nhớ tới khi tắt app (đổi bản / lỡ thoát vẫn xem tiếp được). */
    private val privatePositions = HashMap<String, Int>()

    fun position(epUrl: String): Int = privatePositions[epUrl] ?: positions().optInt(epUrl, 0)

    /** Tập đã xem hết (lưu vị trí 0 khi xem tới ~90 giây cuối). */
    fun finished(epUrl: String): Boolean =
        privatePositions[epUrl]?.let { it == 0 } ?: positions().let { it.has(epUrl) && it.optInt(epUrl) == 0 }

    fun watchedEpisodes(): Set<String> = positions().keys().asSequence().toHashSet()

    /** Tập → phim (để xoá một phim khỏi lịch sử thì xoá được cả chỗ đang xem các tập của nó). */
    private fun posFilms(): JSONObject = JSONObject(sp.getString("pos_film", "{}") ?: "{}")

    fun savePosition(epUrl: String, seconds: Int, filmUrl: String) {
        if (privateMode) {
            privatePositions[epUrl] = seconds
            return
        }
        privatePositions.remove(epUrl)
        val o = positions()
        val films = posFilms()
        o.remove(epUrl)
        o.put(epUrl, seconds) // JSONObject giữ thứ tự chèn → khoá đầu là cũ nhất
        films.put(epUrl, filmUrl)
        val keys = o.keys().asSequence().toList()
        if (keys.size > MAX_POSITIONS) {
            keys.take(keys.size - MAX_POSITIONS).forEach {
                o.remove(it)
                films.remove(it)
            }
        }
        sp.edit().putString("positions", o.toString()).putString("pos_film", films.toString()).apply()
    }
}
