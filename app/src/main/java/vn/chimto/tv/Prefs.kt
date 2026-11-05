package vn.chimto.tv

import android.content.Context
import android.content.SharedPreferences
import org.json.JSONArray
import org.json.JSONObject
import java.net.URL
import java.security.MessageDigest

data class HistoryItem(
    val filmUrl: String, val title: String, val poster: String?,
    val epTitle: String, val epUrl: String, val time: Long,
)

/** Phim theo dõi: count = số tập lần kiểm gần nhất, seen = số tập lúc người xem mở phim lần cuối. */
data class Follow(
    val filmUrl: String, val title: String, val poster: String?,
    val count: Int, val latest: String, val seen: Int,
    /** Lúc bấm theo dõi (ms; 0 = bản cũ) — đồng bộ trong nhà. */
    val t: Long = 0,
) {
    val newCount get() = (count - seen).coerceAtLeast(0)
}

/**
 * Người xem (hồ sơ). id "" = người xem đầu tiên: dữ liệu nằm ở các khoá cũ (như trước khi có hồ sơ — cài đè / hạ bản
 * không mất gì); người thêm sau dùng khoá có hậu tố "#<id>". color = chỉ số trong Prefs.PROFILE_COLORS; pin = mã băm của
 * mã PIN (null = không khoá).
 */
data class Profile(val id: String, val name: String, val color: Int, val pin: String? = null) {
    val locked get() = pin != null

    /** Chữ cái đầu tên (trên vòng tròn màu). */
    val initial get() = name.trim().firstOrNull()?.uppercaseChar()?.toString() ?: "?"
}

/** Cài đặt + lịch sử xem, lưu trong SharedPreferences. */
object Prefs {
    const val DEFAULT_BASE = "https://yanhh3d.ee"

    /** Các trang mẫu có sẵn trong danh sách lần đầu cài. */
    val PRESET_SITES = listOf(DEFAULT_BASE, "https://hoathinh3d.pw", "https://hoathinh3d.ee", "https://hhkungfu.ee", "https://phimmois.fun")

    /** Trang mẫu thêm ở bản sau: thêm một lần vào danh sách người xem đã lưu (xoá đi thì thôi, không thêm lại). */
    private val ADDED_SITES = listOf("https://phimmois.fun")

    /**
     * Trang đổi hẳn tên miền, đường dẫn giữ nguyên (tên cũ chuyển hướng 301 sang tên mới). Tên cũ hay bị nhà mạng chặn nên
     * app không tự thấy trang đã chuyển → đổi tên cũ → mới trong mọi dữ liệu đã lưu (xem migrateMovedHosts).
     * phimmoir.fun → phimmois.fun: 11/2026.
     */
    internal val MOVED_HOSTS = mapOf("phimmoir.fun" to "phimmois.fun")
    private const val MAX_HISTORY = 60
    private const val MAX_POSITIONS = 400

    private lateinit var sp: SharedPreferences

    fun init(context: Context) = init(context.getSharedPreferences("chimto", Context.MODE_PRIVATE))

    /** Test JVM truyền SharedPreferences giả. */
    internal fun init(prefs: SharedPreferences) {
        sp = prefs
        migrateMovedHosts()
        privatePositions.clear()
        pid = sp.getString("profile", "") ?: ""
        if (profiles.none { it.id == pid }) pid = profiles.first().id
        for (url in ADDED_SITES) {
            val key = "preset_added@$url"
            if (sp.getBoolean(key, false)) continue
            if (sp.contains("sites") && url !in sites) sites = sites + url
            sp.edit().putBoolean(key, true).apply()
        }
    }

    /**
     * Đổi tên miền cũ → mới (MOVED_HOSTS) trong mọi khoá / giá trị đã lưu: danh sách trang, trang đang dùng của từng người
     * xem, lịch sử, Xem tiếp, phim theo dõi, chỗ đang xem, server ưu tiên, điểm server… Khoá mới đã có thì giữ khoá mới.
     */
    private fun migrateMovedHosts() {
        val all = sp.all
        fun moved(s: String) = MOVED_HOSTS.keys.any { it in s }
        if (all.none { (k, v) -> moved(k) || (v is String && moved(v)) }) return
        fun rename(s: String) = MOVED_HOSTS.entries.fold(s) { acc, (old, new) -> acc.replace(old, new) }
        val e = sp.edit()
        for ((k, v) in all) {
            if (!moved(k) && !(v is String && moved(v))) continue
            val nk = rename(k)
            if (nk != k) {
                e.remove(k)
                if (all.containsKey(nk)) continue
            }
            when (v) {
                is String -> e.putString(nk, rename(v))
                is Boolean -> e.putBoolean(nk, v)
                is Int -> e.putInt(nk, v)
                is Long -> e.putLong(nk, v)
                is Float -> e.putFloat(nk, v)
            }
        }
        e.apply()
    }

    /** Trang tự chuyển sang tên miền mới: thay tên cũ trong danh sách (giữ chỗ), rồi dùng tên mới. */
    fun replaceSite(old: String, new: String) {
        sites = sites.map { if (it == old) new else it }
        baseUrl = new
    }

    /** Vượt chặn nhà mạng: chia nhỏ gói bắt tay HTTPS để nhà mạng không đọc được tên trang (Socks5Bridge.writeHello). */
    var unblock: Boolean
        get() = sp.getBoolean("unblock", false)
        set(value) = sp.edit().putBoolean("unblock", value).apply()

    /** Mức vượt chặn khi bật: 1 = chia nhỏ gói bắt tay, 2 = chia nhỏ + đảo thứ tự (byte đầu gửi với TTL 1). */
    var unblockLevel: Int
        get() = sp.getInt("unblock_level", 1).coerceIn(1, 2)
        set(value) = sp.edit().putInt("unblock_level", value).apply()

    // ---- Người xem (hồ sơ) ----

    const val MAX_PROFILES = 8
    const val PIN_LENGTH = 4
    private const val FIRST_PROFILE_NAME = "Cả nhà"

    /** Màu vòng tròn của người xem (người thêm sau lấy màu chưa ai dùng). */
    val PROFILE_COLORS = intArrayOf(
        0xFF2BAF85.toInt(), 0xFFE5533D.toInt(), 0xFF3B82F6.toInt(), 0xFFF59E0B.toInt(),
        0xFF8B5CF6.toInt(), 0xFF06B6D4.toInt(), 0xFFEC4899.toInt(), 0xFF84CC16.toInt(),
    )
    val PROFILE_COLOR_NAMES = listOf("Xanh lá", "Đỏ cam", "Xanh dương", "Vàng", "Tím", "Xanh ngọc", "Hồng", "Xanh nõn chuối")

    /**
     * Khoá riêng từng người xem: lịch sử, chỗ đang xem, phim theo dõi, Xem riêng, trang phim đang dùng, cách xem (bước
     * tua, tự chuyển tập, khung hình) + server ưu tiên theo trang ("server_type@<trang>"). Còn lại (danh sách trang,
     * proxy, DNS, điểm server, mốc nhạc mở đầu, cập nhật…) dùng chung cả nhà.
     */
    private val PROFILE_KEYS = setOf(
        "history", "positions", "pos_film", "follows", "last_follow_check", "private_mode",
        // Đồng bộ trong nhà: lúc lưu chỗ xem, lúc xoá phim khỏi lịch sử / bỏ theo dõi
        "pos_time", "hist_del", "follow_del",
        "base_url", "seek_step", "auto_next", "video_fill",
        // Giới hạn giờ xem (WatchLimit)
        "limit_min", "bedtime", "watch_day", "watch_sec", "watch_extra", "unblock_until",
    )
    private val PROFILE_PREFIXES = listOf("server_type@", "server_label@")

    /** Người thêm sau bắt đầu với cài đặt của người đang dùng (lịch sử, theo dõi, Xem riêng thì trống). */
    private val COPIED_KEYS = setOf("base_url", "seek_step", "auto_next", "video_fill")

    @Volatile private var pid = ""

    /** Khoá của người xem đang dùng. */
    private fun k(key: String) = if (pid.isEmpty()) key else "$key#$pid"

    private fun isProfileKey(base: String) = base in PROFILE_KEYS || PROFILE_PREFIXES.any { base.startsWith(it) }

    /** Các khoá riêng đang có của một người xem: (khoá thật, khoá gốc không hậu tố). */
    private fun ownKeys(id: String): List<Pair<String, String>> = sp.all.keys.mapNotNull { key ->
        val base = if (id.isEmpty()) key.takeIf { '#' !in it } else key.takeIf { it.endsWith("#$id") }?.removeSuffix("#$id")
        base?.takeIf { isProfileKey(it) }?.let { key to it }
    }

    val profiles: List<Profile>
        get() {
            val list = try {
                val a = JSONArray(sp.getString("profiles", null) ?: "[]")
                List(a.length()) { i ->
                    val o = a.getJSONObject(i)
                    Profile(o.getString("id"), o.getString("name"), o.optInt("color"), o.optString("pin").ifEmpty { null })
                }
            } catch (e: Exception) {
                emptyList()
            }
            return list.ifEmpty { listOf(Profile("", FIRST_PROFILE_NAME, 0)) }
        }

    private fun saveProfiles(list: List<Profile>) {
        val a = JSONArray()
        list.forEach {
            a.put(JSONObject().put("id", it.id).put("name", it.name).put("color", it.color).put("pin", it.pin ?: ""))
        }
        sp.edit().putString("profiles", a.toString()).apply()
    }

    /** Người xem đang dùng. */
    val profile: Profile get() = profiles.let { list -> list.firstOrNull { it.id == pid } ?: list.first() }

    val profileId: String get() = pid

    fun switchProfile(id: String) {
        if (profiles.none { it.id == id }) return
        pid = id
        privatePositions.clear()
        sp.edit().putString("profile", id).apply()
    }

    /** Thêm người xem (chưa chuyển sang): lịch sử trống, trang phim / server ưu tiên / cách xem chép từ người đang dùng. */
    fun addProfile(name: String): Profile {
        val list = profiles
        var n = 1
        while (list.any { it.id == "p$n" }) n++
        val id = "p$n"
        val used = list.map { it.color }.toSet()
        val color = PROFILE_COLORS.indices.firstOrNull { it !in used } ?: (list.size % PROFILE_COLORS.size)
        val all = sp.all
        val e = sp.edit()
        for ((key, base) in ownKeys(pid)) {
            if (base !in COPIED_KEYS && PROFILE_PREFIXES.none { base.startsWith(it) }) continue
            when (val v = all[key]) {
                is String -> e.putString("$base#$id", v)
                is Int -> e.putInt("$base#$id", v)
                is Boolean -> e.putBoolean("$base#$id", v)
                is Long -> e.putLong("$base#$id", v)
            }
        }
        e.putString("base_url#$id", baseUrl).apply()
        val p = Profile(id, name.trim(), color)
        saveProfiles(list + p)
        return p
    }

    fun renameProfile(id: String, name: String) = saveProfiles(profiles.map { if (it.id == id) it.copy(name = name.trim()) else it })

    fun setProfileColor(id: String, color: Int) = saveProfiles(profiles.map { if (it.id == id) it.copy(color = color) else it })

    /** Đặt mã PIN (4 số); null = tắt khoá. */
    fun setProfilePin(id: String, pin: String?) =
        saveProfiles(profiles.map { if (it.id == id) it.copy(pin = pin?.let { p -> pinHash(id, p) }) else it })

    fun checkPin(p: Profile, pin: String): Boolean = p.pin == null || p.pin == pinHash(p.id, pin)

    /** Không lưu mã PIN thật, chỉ lưu mã băm (khoá cho trẻ em khỏi vào nhầm, không phải mật khẩu bảo mật). */
    private fun pinHash(id: String, pin: String): String =
        MessageDigest.getInstance("SHA-256").digest("chimto-pin:$id:$pin".toByteArray()).joinToString("") { "%02x".format(it) }

    /** Xoá người xem cùng toàn bộ dữ liệu riêng (lịch sử, theo dõi…); còn một người thì không xoá được. */
    fun deleteProfile(id: String) {
        val list = profiles
        if (list.size < 2 || list.none { it.id == id }) return
        val e = sp.edit()
        ownKeys(id).forEach { e.remove(it.first) }
        e.apply()
        val rest = list.filter { it.id != id }
        saveProfiles(rest)
        if (pid == id) switchProfile(rest.first().id)
    }

    // ---- Giới hạn giờ xem (xem WatchLimit) ----

    /** Khoá của người xem id (người đầu tiên — id "" — giữ khoá cũ không hậu tố). */
    private fun pk(id: String, key: String) = if (id.isEmpty()) key else "$key#$id"

    /** Số phút xem mỗi ngày của người xem id (0 = không giới hạn). */
    fun dailyLimit(id: String = pid): Int = sp.getInt(pk(id, "limit_min"), 0)

    fun setDailyLimit(id: String, minutes: Int) = sp.edit().putInt(pk(id, "limit_min"), minutes).apply()

    /** Giờ đi ngủ của người xem id (phút trong ngày, vd 21:00 = 1260; -1 = không): từ giờ đó tới 6 giờ sáng không xem. */
    fun bedtime(id: String = pid): Int = sp.getInt(pk(id, "bedtime"), -1)

    fun setBedtime(id: String, minute: Int) = sp.edit().putInt(pk(id, "bedtime"), minute).apply()

    /** Người đang xem: ngày đang đếm ("yyyy-MM-dd"), số giây đã xem, số phút được cho thêm trong ngày đó. */
    var watchDay: String
        get() = sp.getString(k("watch_day"), "") ?: ""
        set(value) = sp.edit().putString(k("watch_day"), value).apply()

    var watchSeconds: Int
        get() = sp.getInt(k("watch_sec"), 0)
        set(value) = sp.edit().putInt(k("watch_sec"), value).apply()

    var watchExtra: Int
        get() = sp.getInt(k("watch_extra"), 0)
        set(value) = sp.edit().putInt(k("watch_extra"), value).apply()

    /** Người đang xem được xem không giới hạn tới thời điểm này (ms) — phụ huynh cho thêm lúc giờ đi ngủ. */
    var unblockUntil: Long
        get() = sp.getLong(k("unblock_until"), 0L)
        set(value) = sp.edit().putLong(k("unblock_until"), value).apply()

    /** Có ai đang bị giới hạn giờ xem (khi đó thêm / xoá người xem cũng cần mã PIN phụ huynh). */
    val anyLimits: Boolean get() = profiles.any { dailyLimit(it.id) > 0 || bedtime(it.id) >= 0 }

    /** Mã PIN phụ huynh (cả nhà): đặt / đổi giới hạn giờ xem, cho xem thêm khi hết giờ. */
    val hasParentPin: Boolean get() = sp.contains("parent_pin")

    /** Đặt / đổi mã PIN phụ huynh; null = xoá. */
    fun setParentPin(pin: String?) {
        if (pin == null) sp.edit().remove("parent_pin").apply() else sp.edit().putString("parent_pin", pinHash("parent", pin)).apply()
    }

    fun checkParentPin(pin: String): Boolean = sp.getString("parent_pin", null) == pinHash("parent", pin)

    /** Hỏi "Ai đang xem?" khi mở app (nhà có từ 2 người xem). */
    var askProfile: Boolean
        get() = sp.getBoolean("ask_profile", true)
        set(value) = sp.edit().putBoolean("ask_profile", value).apply()

    /** Bấm Home khi đang xem: thu nhỏ phim vào góc màn hình (hình trong hình) thay vì dừng. */
    var pipOnHome: Boolean
        get() = sp.getBoolean("pip_on_home", true)
        set(value) = sp.edit().putBoolean("pip_on_home", value).apply()

    /** Trang phim đang dùng (địa chỉ gốc, không có dấu / ở cuối). Đặt trang mới thì tự thêm vào danh sách. */
    var baseUrl: String
        get() = sp.getString(k("base_url"), null) ?: sp.getString("base_url", null) ?: DEFAULT_BASE
        set(value) {
            sp.edit().putString(k("base_url"), value).apply()
            if (value !in sites) sites = sites + value
        }

    /** Danh sách trang phim để chọn nhanh (mặc định: các trang mẫu; bản 1.x cũ lưu ở "recent_urls"). */
    var sites: List<String>
        get() {
            val s = sp.getString("sites", null) ?: sp.getString("recent_urls", null)
            val saved = s?.let { j -> JSONArray(j).let { a -> List(a.length()) { a.getString(it) } } }.orEmpty()
            return if (sp.contains("sites")) saved.distinct() else (PRESET_SITES + saved).distinct()
        }
        set(value) {
            // Đồng bộ trong nhà: ghi lúc thêm / bỏ từng trang
            val old = sites.toSet()
            val list = value.distinct()
            val now = System.currentTimeMillis()
            val add = stamps("sites_add")
            val del = stamps("sites_del")
            list.filter { it !in old }.forEach { add[it] = now }
            old.filter { it !in list }.forEach { del[it] = now }
            sp.edit().putString("sites", JSONArray(list).toString())
                .putString("sites_add", stampsJson(add)).putString("sites_del", stampsJson(del)).apply()
        }

    /** Thêm trang vào danh sách (không đổi trang đang dùng); đã có thì thôi. */
    fun addSite(url: String) {
        if (url !in sites) sites = sites + url
    }

    /** Trang mẫu không còn trong danh sách (người xem đã xoá) — để thêm lại nhanh. */
    val missingPresets: List<String> get() = sites.let { list -> PRESET_SITES.filter { it !in list } }

    /** data-type của server người xem chọn lần gần nhất trên từng trang (vd "tiktik"). Bản rất cũ lưu chung một khoá. */
    fun serverType(host: String?): String? =
        sp.getString(k("server_type@$host"), null) ?: sp.getString("server_type", null)?.takeIf { pid.isEmpty() }

    fun serverLabel(host: String?): String? =
        sp.getString(k("server_label@$host"), null) ?: sp.getString("server_label", null)?.takeIf { pid.isEmpty() }

    fun setServer(host: String?, type: String?, label: String?) {
        val e = sp.edit().putString(k("server_type@$host"), type).putString(k("server_label@$host"), label)
        if (pid.isEmpty()) e.remove("server_type").remove("server_label")
        e.apply()
    }

    var seekStep: Int
        get() = sp.getInt(k("seek_step"), 10)
        set(value) = sp.edit().putInt(k("seek_step"), value).apply()

    var autoNext: Boolean
        get() = sp.getBoolean(k("auto_next"), true)
        set(value) = sp.edit().putBoolean(k("auto_next"), value).apply()

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

    /** Mật khẩu proxy đã giải mã (đọc Keystore chậm, proxyConfig() được gọi nhiều lần). */
    @Volatile private var proxyPassCache: String? = null

    /**
     * Lưu mã hoá bằng khoá trong Android Keystore (Secret) từ 1.9.1; bản cũ lưu chữ thường ở "proxy_pass" → đọc lần
     * đầu thì mã hoá lại và xoá bản chữ thường. Android 5 (không có Keystore AES) vẫn lưu như cũ.
     */
    var proxyPass: String
        get() {
            proxyPassCache?.let { return it }
            val enc = sp.getString("proxy_pass_enc", null)
            val plain = sp.getString("proxy_pass", null)
            val v = when {
                enc != null -> Secret.decrypt(enc).orEmpty()
                plain != null -> plain.also { if (it.isNotEmpty() && Secret.supported) proxyPass = it }
                else -> ""
            }
            proxyPassCache = v
            return v
        }
        set(value) {
            proxyPassCache = value
            val enc = if (Secret.supported && value.isNotEmpty()) Secret.encrypt(value) else null
            val e = sp.edit()
            if (enc != null) e.putString("proxy_pass_enc", enc).remove("proxy_pass") else e.putString("proxy_pass", value).remove("proxy_pass_enc")
            e.apply()
        }

    /** DNS mã hoá (DoH) cho tên miền thiết bị tự phân giải; null = tắt. */
    var dohServer: Doh.Server?
        get() = sp.getString("doh", null)?.let { n -> Doh.Server.values().firstOrNull { it.name == n } }
        set(value) = sp.edit().putString("doh", value?.name).apply()

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
            val a = JSONArray(sp.getString(k("history"), "[]"))
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
        sp.edit().putString(k("history"), historyJson(list.take(MAX_HISTORY))).apply()
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

    /** Xoá lịch sử xem + chỗ đang xem của người xem đang dùng. */
    fun clearHistory() {
        val del = stamps(k("hist_del"))
        val now = System.currentTimeMillis()
        val films = posFilms()
        (history.map { it.filmUrl } + films.keys().asSequence().map { films.optString(it) }).filter { it.isNotEmpty() }
            .forEach { del[it] = now }
        privatePositions.clear()
        sp.edit().remove(k("history")).remove(k("positions")).remove(k("pos_film")).remove(k("pos_time"))
            .putString(k("hist_del"), stampsJson(del)).apply()
    }

    /** Xoá một phim khỏi lịch sử, kèm chỗ đang xem / đánh dấu đã xem của các tập phim đó. */
    fun removeHistory(filmUrl: String) {
        val item = historyFor(filmUrl)
        val o = positions()
        val films = posFilms()
        // Vị trí lưu trước 1.8.2 không ghi phim → chỉ biết tập cuối trong lịch sử
        val eps = films.keys().asSequence().filter { films.optString(it) == filmUrl }.toList() + listOfNotNull(item?.epUrl)
        val times = posTimes()
        eps.forEach {
            o.remove(it)
            films.remove(it)
            times.remove(it)
            privatePositions.remove(it)
        }
        val del = stamps(k("hist_del"))
        del[filmUrl] = System.currentTimeMillis()
        sp.edit().putString(k("history"), historyJson(history.filter { it.filmUrl != filmUrl }))
            .putString(k("positions"), o.toString())
            .putString(k("pos_film"), films.toString())
            .putString(k("pos_time"), times.toString())
            .putString(k("hist_del"), stampsJson(del)).apply()
    }

    // ---- Phim theo dõi ----

    val follows: List<Follow>
        get() {
            val a = JSONArray(sp.getString(k("follows"), "[]"))
            return List(a.length()) { i ->
                val o = a.getJSONObject(i)
                Follow(
                    o.getString("film"), o.getString("title"), o.optString("poster").ifEmpty { null },
                    o.optInt("count"), o.optString("latest"), o.optInt("seen"), o.optLong("t"),
                )
            }
        }

    private fun saveFollows(list: List<Follow>) {
        val a = JSONArray()
        list.forEach {
            a.put(
                JSONObject().put("film", it.filmUrl).put("title", it.title).put("poster", it.poster ?: "")
                    .put("count", it.count).put("latest", it.latest).put("seen", it.seen).put("t", it.t)
            )
        }
        sp.edit().putString(k("follows"), a.toString()).apply()
    }

    fun followOf(filmUrl: String): Follow? = follows.firstOrNull { it.filmUrl == filmUrl }

    fun follow(d: FilmDetail) {
        val n = d.episodeCount
        saveFollows(listOf(Follow(d.url, d.title, d.poster, n, d.latestEpisode?.title.orEmpty(), n, System.currentTimeMillis())) +
            follows.filter { it.filmUrl != d.url })
    }

    fun unfollow(filmUrl: String) {
        val del = stamps(k("follow_del"))
        del[filmUrl] = System.currentTimeMillis()
        sp.edit().putString(k("follow_del"), stampsJson(del)).apply()
        saveFollows(follows.filter { it.filmUrl != filmUrl })
    }

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
        get() = sp.getLong(k("last_follow_check"), 0)
        set(value) = sp.edit().putLong(k("last_follow_check"), value).apply()

    // ---- Nhạc mở đầu (mốc riêng từng phim, giây) ----

    /** (đầu, hết) nhạc mở đầu; hết = -1 là mới đánh dấu đầu. */
    fun intro(filmUrl: String): Pair<Int, Int>? {
        val p = sp.getString("intro@$filmUrl", null)?.split(',') ?: return null
        return (p.getOrNull(0)?.toIntOrNull() ?: return null) to (p.getOrNull(1)?.toIntOrNull() ?: -1)
    }

    fun setIntro(filmUrl: String, start: Int, end: Int) = sp.edit().putString("intro@$filmUrl", "$start,$end").apply()

    fun clearIntro(filmUrl: String) = sp.edit().remove("intro@$filmUrl").apply()

    /** Lấp đầy màn hình: video rộng (21:9) phóng cho hết viền đen, cắt bớt hai bên. */
    var videoFill: Boolean
        get() = sp.getBoolean(k("video_fill"), false)
        set(value) = sp.edit().putBoolean(k("video_fill"), value).apply()

    // ---- Riêng tư ----

    /** Xem riêng: không ghi lịch sử xem và chỗ đang xem (vẫn đọc được chỗ đã lưu từ trước). Riêng từng người xem. */
    var privateMode: Boolean
        get() = sp.getBoolean(k("private_mode"), false)
        set(value) = sp.edit().putBoolean(k("private_mode"), value).apply()

    /** Tự xoá dữ liệu duyệt web khi rời app (xem AutoClear). */
    var autoClearWeb: Boolean
        get() = sp.getBoolean("auto_clear_web", false)
        set(value) = sp.edit().putBoolean("auto_clear_web", value).apply()

    /** WebView đã chạy từ lần xoá dữ liệu duyệt web gần nhất (mặc định true: chưa xoá lần nào). */
    var webDirty: Boolean
        get() = sp.getBoolean("web_dirty", true)
        set(value) = sp.edit().putBoolean("web_dirty", value).apply()

    // ---- Vị trí đang xem từng tập (giây) ----

    private fun positions(): JSONObject = JSONObject(sp.getString(k("positions"), "{}") ?: "{}")

    /** Xem riêng: chỗ đang xem chỉ giữ trong bộ nhớ tới khi tắt app (đổi bản / lỡ thoát vẫn xem tiếp được). */
    private val privatePositions = HashMap<String, Int>()

    fun position(epUrl: String): Int = privatePositions[epUrl] ?: positions().optInt(epUrl, 0)

    /** Tập đã xem hết (lưu vị trí 0 khi xem tới ~90 giây cuối). */
    fun finished(epUrl: String): Boolean =
        privatePositions[epUrl]?.let { it == 0 } ?: positions().let { it.has(epUrl) && it.optInt(epUrl) == 0 }

    fun watchedEpisodes(): Set<String> = positions().keys().asSequence().toHashSet()

    /** Tập → phim (để xoá một phim khỏi lịch sử thì xoá được cả chỗ đang xem các tập của nó). */
    private fun posFilms(): JSONObject = JSONObject(sp.getString(k("pos_film"), "{}") ?: "{}")

    /** Tập → lúc lưu chỗ xem (ms) — đồng bộ trong nhà lấy lần lưu mới hơn. Chỗ xem lưu từ bản cũ không có. */
    private fun posTimes(): JSONObject = JSONObject(sp.getString(k("pos_time"), "{}") ?: "{}")

    fun savePosition(epUrl: String, seconds: Int, filmUrl: String) {
        if (privateMode) {
            privatePositions[epUrl] = seconds
            return
        }
        privatePositions.remove(epUrl)
        val o = positions()
        val films = posFilms()
        val times = posTimes()
        o.remove(epUrl)
        o.put(epUrl, seconds) // JSONObject giữ thứ tự chèn → khoá đầu là cũ nhất
        films.put(epUrl, filmUrl)
        times.put(epUrl, System.currentTimeMillis())
        val keys = o.keys().asSequence().toList()
        if (keys.size > MAX_POSITIONS) {
            keys.take(keys.size - MAX_POSITIONS).forEach {
                o.remove(it)
                films.remove(it)
                times.remove(it)
            }
        }
        sp.edit().putString(k("positions"), o.toString()).putString(k("pos_film"), films.toString())
            .putString(k("pos_time"), times.toString()).apply()
    }

    // ---- Đồng bộ trong nhà (Sync.kt) ----

    /** Máy đã ghép (JSON, xem SyncNet.peers). */
    internal var syncPeersJson: String
        get() = sp.getString("sync_peers", "[]") ?: "[]"
        set(value) = sp.edit().putString("sync_peers", value).apply()

    /** Tự đồng bộ khi mở app / xem xong (mặc định bật). */
    var syncAuto: Boolean
        get() = sp.getBoolean("sync_auto", true)
        set(value) = sp.edit().putBoolean("sync_auto", value).apply()

    /** Mốc thời gian theo khoá (lúc xoá / lúc thêm), lưu JSON {"khoá": ms}. */
    internal fun stamps(key: String): MutableMap<String, Long> {
        val o = JSONObject(sp.getString(key, "{}") ?: "{}")
        return o.keys().asSequence().associateWithTo(LinkedHashMap()) { o.optLong(it) }
    }

    internal fun stampsJson(m: Map<String, Long>): String = JSONObject(SyncMerge.tombstones(m, emptyMap()) as Map<*, *>).toString()

    /** Mã máy (ngẫu nhiên, tạo lần đầu) — máy kia nhận ra TV này qua mã này. */
    val deviceId: String
        @Synchronized get() = sp.getString("device_id", null) ?: java.util.UUID.randomUUID().toString().lowercase().also {
            sp.edit().putString("device_id", it).apply()
        }

    /** Dữ liệu đồng bộ của người xem p. */
    fun syncProfile(p: Profile): SyncProfile {
        fun key(base: String) = pk(p.id, base)
        val hist = JSONArray(sp.getString(key("history"), "[]"))
        val pos = JSONObject(sp.getString(key("positions"), "{}") ?: "{}")
        val films = JSONObject(sp.getString(key("pos_film"), "{}") ?: "{}")
        val times = JSONObject(sp.getString(key("pos_time"), "{}") ?: "{}")
        val fol = JSONArray(sp.getString(key("follows"), "[]"))
        return SyncProfile(
            id = p.id, name = p.name,
            history = List(hist.length()) { i ->
                val o = hist.getJSONObject(i)
                SyncHistory(o.getString("film"), o.optString("title"), o.optString("poster"), o.optString("ep"),
                    o.optString("epUrl"), o.optLong("time"))
            },
            histDel = stamps(key("hist_del")),
            pos = pos.keys().asSequence().map { SyncPos(it, pos.optInt(it), films.optString(it), times.optLong(it)) }.toList(),
            follows = List(fol.length()) { i ->
                val o = fol.getJSONObject(i)
                SyncFollow(o.getString("film"), o.optString("title"), o.optString("poster"), o.optInt("count"),
                    o.optString("latest"), o.optInt("seen"), o.optLong("t"))
            },
            followDel = stamps(key("follow_del")),
        )
    }

    /** Ghi dữ liệu đã gộp cho người xem s.id. true = có thay đổi. */
    @Synchronized
    fun applySync(s: SyncProfile): Boolean {
        val p = profiles.firstOrNull { it.id == s.id } ?: return false
        // Thứ tự chỗ xem không tính (JSONObject của một số bản org.json không giữ thứ tự khoá)
        fun same(a: SyncProfile, b: SyncProfile) = a.copy(pos = a.pos.sortedBy { it.ep }) == b.copy(pos = b.pos.sortedBy { it.ep })
        if (same(syncProfile(p), s)) return false
        fun key(base: String) = pk(s.id, base)
        val hist = JSONArray()
        s.history.forEach {
            hist.put(JSONObject().put("film", it.film).put("title", it.title).put("poster", it.poster)
                .put("ep", it.ep).put("epUrl", it.epUrl).put("time", it.time))
        }
        val pos = JSONObject()
        val films = JSONObject()
        val times = JSONObject()
        s.pos.forEach {
            pos.put(it.ep, it.sec)
            films.put(it.ep, it.film)
            if (it.t > 0) times.put(it.ep, it.t)
        }
        val fol = JSONArray()
        s.follows.forEach {
            fol.put(JSONObject().put("film", it.film).put("title", it.title).put("poster", it.poster)
                .put("count", it.count).put("latest", it.latest).put("seen", it.seen).put("t", it.t))
        }
        sp.edit().putString(key("history"), hist.toString())
            .putString(key("positions"), pos.toString()).putString(key("pos_film"), films.toString())
            .putString(key("pos_time"), times.toString())
            .putString(key("follows"), fol.toString())
            .putString(key("hist_del"), stampsJson(s.histDel))
            .putString(key("follow_del"), stampsJson(s.followDel))
            .commit()
        return true
    }

    val syncSites: SyncSites get() = SyncSites(sites, stamps("sites_add"), stamps("sites_del"))

    /** Ghi danh sách trang đã gộp (không ghi lại mốc thêm / bỏ như khi người xem tự sửa). true = có thay đổi. */
    @Synchronized
    fun applySites(s: SyncSites): Boolean {
        if (s == syncSites || s.list.isEmpty()) return false
        sp.edit().putString("sites", JSONArray(s.list).toString())
            .putString("sites_add", stampsJson(s.add)).putString("sites_del", stampsJson(s.del)).commit()
        return true
    }
}
