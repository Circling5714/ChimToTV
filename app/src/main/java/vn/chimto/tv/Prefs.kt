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

/** Cài đặt + lịch sử xem, lưu trong SharedPreferences. */
object Prefs {
    const val DEFAULT_BASE = "https://yanhh3d.ee"

    /** Các trang mẫu có sẵn trong danh sách lần đầu cài. */
    val PRESET_SITES = listOf(DEFAULT_BASE, "https://hoathinh3d.pw", "https://hoathinh3d.ee", "https://hhkungfu.ee")
    private const val MAX_HISTORY = 60
    private const val MAX_POSITIONS = 400

    private lateinit var sp: SharedPreferences

    fun init(context: Context) {
        sp = context.getSharedPreferences("chimto", Context.MODE_PRIVATE)
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
        val a = JSONArray()
        list.take(MAX_HISTORY).forEach {
            a.put(
                JSONObject().put("film", it.filmUrl).put("title", it.title).put("poster", it.poster ?: "")
                    .put("ep", it.epTitle).put("epUrl", it.epUrl).put("time", it.time)
            )
        }
        sp.edit().putString("history", a.toString()).apply()
    }

    fun clearHistory() {
        sp.edit().remove("history").remove("positions").apply()
    }

    // ---- Vị trí đang xem từng tập (giây) ----

    private fun positions(): JSONObject = JSONObject(sp.getString("positions", "{}") ?: "{}")

    fun position(epUrl: String): Int = positions().optInt(epUrl, 0)

    fun watchedEpisodes(): Set<String> = positions().keys().asSequence().toHashSet()

    fun savePosition(epUrl: String, seconds: Int) {
        val o = positions()
        o.remove(epUrl)
        o.put(epUrl, seconds) // JSONObject giữ thứ tự chèn → khoá đầu là cũ nhất
        val keys = o.keys().asSequence().toList()
        if (keys.size > MAX_POSITIONS) keys.take(keys.size - MAX_POSITIONS).forEach { o.remove(it) }
        sp.edit().putString("positions", o.toString()).apply()
    }
}
