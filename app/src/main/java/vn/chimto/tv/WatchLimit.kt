package vn.chimto.tv

import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.Locale

/**
 * Giới hạn giờ xem theo người xem (Cài đặt → Người xem → Giới hạn giờ xem…): số phút xem mỗi ngày và giờ đi ngủ (từ
 * giờ đó tới 6 giờ sáng không xem được). Đếm thời gian phim thật sự chạy của người đang xem; hết giờ thì trình phát
 * dừng phim, xem thêm phải có mã PIN phụ huynh (ProfileUi.grant). Bản iPad: WatchLimit.swift.
 */
object WatchLimit {
    /** Hết giờ đi ngủ lúc 6:00 sáng. */
    const val MORNING = 6 * 60

    /** Lựa chọn số phút mỗi ngày (0 = không giới hạn) và giờ đi ngủ (phút trong ngày, -1 = không). */
    val DAILY = listOf(0, 30, 45, 60, 90, 120, 180)
    val BEDTIMES = listOf(-1, 19 * 60 + 30, 20 * 60, 20 * 60 + 30, 21 * 60, 21 * 60 + 30, 22 * 60, 23 * 60)

    /** Cho xem thêm (phút) khi hết giờ. */
    val GRANTS = listOf(15, 30, 60)

    enum class Block { DAILY, BEDTIME }

    /** Đồng hồ (test thay được). */
    internal var clock: () -> Long = System::currentTimeMillis

    /** Giây xem chưa ghi xuống (ghi mỗi 30 giây và khi rời trình phát — đỡ ghi bộ nhớ TV mỗi giây). */
    private var pending = 0.0
    private var lastTick = 0L

    fun day(now: Long = clock()): String = SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date(now))

    private fun minuteOfDay(now: Long): Int =
        Calendar.getInstance().apply { timeInMillis = now }.let { it.get(Calendar.HOUR_OF_DAY) * 60 + it.get(Calendar.MINUTE) }

    /** Giây người đang xem đã xem hôm nay (kể cả phần chưa ghi). */
    fun usedToday(now: Long = clock()): Int = (if (Prefs.watchDay == day(now)) Prefs.watchSeconds else 0) + pending.toInt()

    /** Giây còn được xem hôm nay; null = không giới hạn theo ngày. */
    fun remaining(now: Long = clock()): Int? {
        val limit = Prefs.dailyLimit().takeIf { it > 0 } ?: return null
        val extra = if (Prefs.watchDay == day(now)) Prefs.watchExtra else 0
        return (limit + extra) * 60 - usedToday(now)
    }

    /** Người đang xem có bị chặn không (giờ đi ngủ trước, rồi hết phút trong ngày); null = xem được. */
    fun block(now: Long = clock()): Block? {
        if (now < Prefs.unblockUntil) return null
        val bed = Prefs.bedtime()
        if (bed >= 0) {
            val m = minuteOfDay(now)
            if (m >= bed || m < MORNING) return Block.BEDTIME
        }
        val r = remaining(now) ?: return null
        return if (r <= 0) Block.DAILY else null
    }

    /** Gọi mỗi lần đọc trạng thái trình phát: phim đang chạy thì cộng thời gian từ lần trước (bỏ khoảng hở > 3 giây). */
    fun tick(playing: Boolean, now: Long = clock()) {
        val dt = (now - lastTick) / 1000.0
        lastTick = now
        if (!playing || dt <= 0 || dt > 3) return
        pending += dt
        if (pending >= 30) flush(now)
    }

    /** Ghi phần đã đếm (rời trình phát, đổi người xem, sang ngày mới). */
    fun flush(now: Long = clock()) {
        val d = day(now)
        if (Prefs.watchDay != d) {
            Prefs.watchDay = d
            Prefs.watchSeconds = 0
            Prefs.watchExtra = 0
        }
        val whole = pending.toInt()
        if (whole > 0) Prefs.watchSeconds = Prefs.watchSeconds + whole
        pending -= whole
    }

    /** Bỏ phần đếm chưa ghi (đổi người xem giữa chừng đã flush trước). */
    internal fun reset() {
        pending = 0.0
        lastTick = 0L
    }

    /** Cho xem thêm (đã nhập mã phụ huynh): đang giờ đi ngủ → mở khoá thêm từng ấy phút; hết phút trong ngày → cộng thêm. */
    fun grant(minutes: Int, now: Long = clock()) {
        flush(now)
        if (block(now) == Block.BEDTIME) {
            Prefs.unblockUntil = now + minutes * 60_000L
        } else {
            Prefs.watchExtra = Prefs.watchExtra + minutes
        }
    }

    /** Không giới hạn tới 6 giờ sáng (hôm nay nếu đang trước 6 giờ, không thì sáng mai). */
    fun grantUntilMorning(now: Long = clock()) {
        val c = Calendar.getInstance().apply {
            timeInMillis = now
            set(Calendar.HOUR_OF_DAY, MORNING / 60)
            set(Calendar.MINUTE, 0)
            set(Calendar.SECOND, 0)
            set(Calendar.MILLISECOND, 0)
            if (timeInMillis <= now) add(Calendar.DAY_OF_MONTH, 1)
        }
        Prefs.unblockUntil = c.timeInMillis
    }

    /** "30 phút", "1 giờ", "1 giờ 30 phút", "Không giới hạn". */
    fun dailyLabel(minutes: Int): String = when {
        minutes <= 0 -> "Không giới hạn"
        minutes < 60 -> "$minutes phút"
        minutes % 60 == 0 -> "${minutes / 60} giờ"
        else -> "${minutes / 60} giờ ${minutes % 60} phút"
    }

    /** "21:00" hoặc "Không". */
    fun bedtimeLabel(minute: Int): String = if (minute < 0) "Không" else "%d:%02d".format(minute / 60, minute % 60)

    /** Giây còn lại → "25 phút" (làm tròn lên, tối thiểu 1 phút). */
    fun minutesText(seconds: Int): String = dailyLabel(maxOf(1, (seconds + 59) / 60))

    /** Tóm tắt giới hạn của một người xem, vd "1 giờ/ngày, ngủ lúc 21:00". */
    fun summary(id: String): String {
        val parts = ArrayList<String>()
        Prefs.dailyLimit(id).takeIf { it > 0 }?.let { parts += "${dailyLabel(it)}/ngày" }
        Prefs.bedtime(id).takeIf { it >= 0 }?.let { parts += "ngủ lúc ${bedtimeLabel(it)}" }
        return parts.joinToString(", ").ifEmpty { "không giới hạn" }
    }
}
