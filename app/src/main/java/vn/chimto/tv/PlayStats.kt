package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONObject
import java.text.Normalizer
import kotlin.math.roundToInt

/**
 * Chấm điểm từng server video theo những lần xem thật trên chính TV này (mạng mỗi nhà mỗi khác):
 * mất bao lâu mới phát, giật (trình phát đứng chờ tải) bao nhiêu. Giữ vài lần xem gần nhất cho mỗi trang + server,
 * nên điểm tự đổi theo giờ cao điểm / nhà mạng.
 */
object PlayStats {
    private const val KEEP = 8

    /** level: 0 mượt, 1 hơi giật, 2 hay giật, 3 hay lỗi (không chạy được), -1 chưa đủ dữ liệu. */
    class Score(val level: Int, val startSec: Int?, val stallPct: Double) {
        val label: String?
            get() = when (level) {
                0 -> "mượt"
                1 -> "hơi giật"
                2 -> "hay giật"
                3 -> "hay lỗi"
                else -> null
            }

        /** Mô tả ngắn cho nút server, vd "mượt, mở 4 s". */
        val summary: String? get() = listOfNotNull(label, startSec?.let { "mở $it s" }).joinToString(", ").ifEmpty { null }
    }

    private fun key(host: String?, server: String) = "$host|$server"

    /** startMs < 0: không đo được thời gian mở (vd xem tiếp sau khi rời màn hình). */
    fun record(host: String?, server: String, startMs: Long, play: Int, stall: Int, stalls: Int, failed: Boolean) {
        val all = Prefs.playStats
        val k = key(host, server)
        val list = all.optJSONArray(k) ?: JSONArray()
        list.put(JSONObject().put("s", startMs).put("p", play).put("w", stall).put("n", stalls).put("x", failed))
        while (list.length() > KEEP) list.remove(0)
        all.put(k, list)
        Prefs.playStats = all
    }

    fun score(host: String?, server: String): Score = scoreOf(Prefs.playStats.optJSONArray(key(host, server)))

    private fun scoreOf(list: JSONArray?): Score {
        if (list == null || list.length() == 0) return Score(-1, null, 0.0)
        var play = 0
        var stall = 0
        var stalls = 0
        var fails = 0
        val starts = ArrayList<Long>()
        for (i in 0 until list.length()) {
            val o = list.getJSONObject(i)
            if (o.optBoolean("x")) {
                fails++
                continue
            }
            play += o.optInt("p")
            stall += o.optInt("w")
            stalls += o.optInt("n")
            o.optLong("s", -1).takeIf { it > 0 }?.let { starts += it }
        }
        val watched = play + stall
        val start = if (starts.isEmpty()) null else (starts.average() / 1000).roundToInt().coerceAtLeast(1)
        return Score(level(list.length(), fails, play, stall, stalls), start, if (watched > 0) stall.toDouble() / watched else 0.0)
    }

    /**
     * Mức của một server từ tổng các lần xem gần đây (giây chạy, giây đứng hình, số lần giật).
     * Số lần giật tính trên ít nhất 10 phút xem: một lần khựng trong 1–2 phút đầu không bị quy thành "hay giật".
     */
    fun level(sessions: Int, fails: Int, play: Int, stall: Int, stalls: Int): Int {
        val watched = play + stall
        val pct = if (watched > 0) stall.toDouble() / watched else 0.0
        val per10min = stalls * 600.0 / maxOf(watched, 600)
        return when {
            fails > 0 && fails * 2 >= sessions -> 3
            watched < 60 -> -1
            pct < 0.02 && per10min < 2 -> 0
            pct < 0.06 && per10min < 5 -> 1
            else -> 2
        }
    }

    /**
     * Server nên mở cho một tập (current = server người xem đã chọn, chưa chọn thì server đầu tiên của trang):
     * giữ nguyên, trừ khi nó hay giật / hay lỗi mà có server cùng ngôn ngữ đã chấm tốt.
     * Riêng server hay lỗi (không phát được, vd OPhim của phimmoir.fun khi tên miền video chết): sang server cùng
     * ngôn ngữ còn phát được dù hay giật, rồi tới server chưa thử (không sang server chưa thử mà nét hơn).
     */
    fun choose(host: String?, servers: List<Server>, current: Server?): Server? =
        choose(servers, current) { score(host, it.type) }

    fun choose(servers: List<Server>, current: Server?, score: (Server) -> Score): Server? {
        if (current == null || score(current).level < 2) return current
        val same = servers.filter { sameLang(it, current) }.map { it to score(it) }
        same.filter { it.second.level in 0..1 }
            .minWithOrNull(compareBy({ it.second.level }, { it.second.stallPct }))?.let { return it.first }
        if (score(current).level < 3) return current
        val r = resolution(current.label)
        return same.filter { (s, sc) -> s != current && sc.level != 3 && (sc.level >= 0 || r == null || (resolution(s.label) ?: r) <= r) }
            .minWithOrNull(compareBy({ if (it.second.level < 0) 3 else it.second.level }, { it.second.stallPct }))?.first ?: current
    }

    /**
     * Server nên thử thay cho server đang giật, cùng ngôn ngữ (không tự đổi Vietsub ↔ Thuyết minh):
     * server đã chấm tốt trên TV này trước, rồi tới server chưa thử cùng độ phân giải, rồi thấp hơn.
     * Không gợi ý server chưa thử mà nét hơn (đang giật mà sang 4K thì càng giật), bỏ qua server hay giật.
     */
    fun alternative(host: String?, servers: List<Server>, current: Server): Pair<Server, Score>? =
        alternative(servers, current) { score(host, it.type) }

    fun alternative(servers: List<Server>, current: Server, score: (Server) -> Score): Pair<Server, Score>? {
        val r = resolution(current.label)
        val lower = { s: Server -> r != null && (resolution(s.label) ?: r) < r }
        return servers.filter { it != current && sameLang(it, current) }.map { it to score(it) }
            .filter { (s, sc) -> sc.level in 0..1 || (sc.level < 0 && (r == null || (resolution(s.label) ?: r) <= r)) }
            .minWithOrNull(compareBy({ if (it.second.level < 0) 2 else it.second.level }, { lower(it.first) }, { it.second.stallPct }))
    }

    /** Tên server bỏ dấu, chữ thường: "Thuyết minh 1080 V1" → "thuyet minh 1080 v1". */
    private fun plain(label: String) =
        Normalizer.normalize(label, Normalizer.Form.NFD).replace(Regex("\\p{M}+"), "").lowercase()

    /** Ngôn ngữ ghi trong tên server; null = không rõ (coi như cùng ngôn ngữ). */
    fun lang(label: String): String? {
        val s = plain(label)
        return when {
            "thuyet minh" in s || Regex("""\btm\b""").containsMatchIn(s) -> "tm"
            "long tieng" in s -> "lt"
            "sub" in s -> "vs"
            else -> null
        }
    }

    private fun sameLang(a: Server, b: Server): Boolean {
        val x = lang(a.label)
        val y = lang(b.label)
        return x == null || y == null || x == y
    }

    /** Độ phân giải ghi trong tên server (4K → 2160); null = không rõ. */
    fun resolution(label: String): Int? {
        val s = plain(label)
        return when {
            "4k" in s || "2160" in s -> 2160
            "2k" in s || "1440" in s -> 1440
            else -> Regex("""\b(1080|720|480|360)p?\b""").find(s)?.groupValues?.get(1)?.toInt()
        }
    }

    /** Điểm server tốt nhất của một trang, cho danh sách "Đổi trang phim", vd "video mượt". */
    fun siteSummary(host: String?): String? {
        val all = Prefs.playStats
        val best = all.keys().asSequence().filter { it.startsWith("$host|") }
            .map { scoreOf(all.optJSONArray(it)) }.filter { it.level >= 0 }.minByOrNull { it.level }
        return best?.label?.let { "video $it" }
    }
}

/**
 * Theo dõi một lần phát (một server của một tập) từ trạng thái trình phát báo lên mỗi giây.
 * "Giật" = đang phát mà trình phát đứng chờ tải; chờ tải ngay sau lệnh tua là bình thường nên không tính.
 */
class PlaySession(private val host: String?, private val server: String, private val openedAt: Long) {
    private var firstPlay = -1L // ms từ lúc mở tới khi video thật sự chạy
    private var play = 0 // giây đang chạy
    private var stall = 0 // giây đứng chờ tải
    private var stalls = 0 // số lần giật
    private var savedPlay = 0
    private var savedStall = 0
    private var savedStalls = 0
    private var startSaved = false
    private var failSaved = false
    private var stalling = false
    private var stallRun = 0
    private var afterSeek = false
    private var lastPaused = false
    private var inAd = false
    private val recent = ArrayDeque<Long>()
    private var frames0 = -1
    private var dropped0 = 0
    private var frames = 0
    private var dropped = 0

    /** Vừa ra lệnh tua: trình phát chờ tải ngay sau đó là bình thường, không tính là giật. */
    fun seeked() {
        afterSeek = true
    }

    /** Trình phát đang chạy quảng cáo: chưa phải phim, rời đi lúc này không tính là server lỗi. */
    fun ad() {
        inAd = true
    }

    fun update(now: Long, paused: Boolean, waiting: Boolean, ready: Boolean, droppedFrames: Int, totalFrames: Int) {
        inAd = false
        lastPaused = paused
        if (totalFrames > 0) {
            if (frames0 < 0 || totalFrames < frames0) {
                frames0 = totalFrames
                dropped0 = droppedFrames
            }
            frames = totalFrames - frames0
            dropped = droppedFrames - dropped0
        }
        if (firstPlay < 0) {
            if (ready && !paused && !waiting) firstPlay = now - openedAt
            return
        }
        when {
            paused -> {
                stalling = false
                stallRun = 0
            }
            waiting -> if (!afterSeek) {
                if (!stalling) {
                    stalling = true
                    stalls++
                    recent.addLast(now)
                }
                stall++
                stallRun++
            }
            else -> {
                play++
                stalling = false
                stallRun = 0
                afterSeek = false
            }
        }
    }

    /** Giật nhiều: ≥ 3 lần trong 3 phút, hoặc đứng hình liền ≥ 10 giây. */
    fun struggling(now: Long): Boolean {
        while (recent.isNotEmpty() && now - recent.first() > 180_000) recent.removeFirst()
        return recent.size >= 3 || stallRun >= 10
    }

    /** Mô tả lần xem này cho menu ▼, vd "mở 4 s · giật 2 lần (9 s)". */
    fun describe(): String? {
        if (firstPlay < 0) return null
        val parts = arrayListOf("mở ${(firstPlay / 1000).coerceAtLeast(1)} s")
        parts += if (stalls == 0) "chưa giật lần nào" else "giật $stalls lần ($stall s)"
        // Rớt khung hình là do TV giải mã / vẽ không kịp, không phải do mạng
        if (frames >= 300 && dropped * 10 >= frames) {
            parts += "TV rớt ${dropped * 100 / frames}% khung hình (TV xử lý không kịp, đổi server không đỡ)"
        }
        return parts.joinToString(" · ")
    }

    /** Trình phát báo lỗi hẳn (vd không tải được video) → tính là lỗi ngay, không chờ 20 giây. */
    fun fail() {
        if (firstPlay >= 0 || failSaved) return
        failSaved = true
        PlayStats.record(host, server, -1, 0, 0, 0, failed = true)
    }

    /** Ghi phần chưa ghi vào thống kê (khi đổi server / tập, hoặc rời màn hình). */
    fun save(now: Long) {
        if (firstPlay < 0) {
            // Chờ ≥ 20 giây mà video vẫn chưa chạy (không phải do người xem dừng, không phải đang quảng cáo) → tính là lỗi
            if (!failSaved && !lastPaused && !inAd && now - openedAt >= 20_000) {
                failSaved = true
                PlayStats.record(host, server, -1, 0, 0, 0, failed = true)
            }
            return
        }
        val p = play - savedPlay
        val w = stall - savedStall
        if (p + w < 20) return
        val start = if (startSaved || firstPlay > 60_000) -1 else firstPlay
        PlayStats.record(host, server, start, p, w, stalls - savedStalls, failed = false)
        startSaved = true
        savedPlay = play
        savedStall = stall
        savedStalls = stalls
    }
}
