package vn.chimto.tv

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.util.Calendar

/** Giới hạn giờ xem theo người xem: đếm giờ, hết giờ trong ngày, giờ đi ngủ, cho xem thêm, mã PIN phụ huynh. */
class WatchLimitTest {
    private var now = 0L

    private fun at(h: Int, m: Int, day: Int = 5): Long =
        Calendar.getInstance().apply {
            clear()
            set(2026, Calendar.NOVEMBER, day, h, m, 0)
        }.timeInMillis

    /** Phim chạy `seconds` giây (trình phát báo trạng thái mỗi giây). */
    private fun watch(seconds: Int) = repeat(seconds) {
        now += 1000
        WatchLimit.tick(true)
    }

    @Before
    fun setUp() {
        Prefs.init(MemoryPrefs())
        WatchLimit.reset()
        WatchLimit.clock = { now }
    }

    @After
    fun tearDown() {
        WatchLimit.clock = System::currentTimeMillis
        WatchLimit.reset()
    }

    @Test
    fun noLimitNeverBlocks() {
        now = at(23, 30)
        WatchLimit.tick(false)
        watch(4000)
        assertNull(WatchLimit.block())
        assertNull(WatchLimit.remaining())
    }

    @Test
    fun dailyLimitBlocksThenGrantAddsMinutesAndNextDayStartsOver() {
        Prefs.setDailyLimit(Prefs.profileId, 30)
        now = at(15, 0)
        WatchLimit.tick(false)
        watch(29 * 60)
        assertNull(WatchLimit.block())
        assertEquals(60, WatchLimit.remaining())
        watch(61)
        assertEquals(WatchLimit.Block.DAILY, WatchLimit.block())
        WatchLimit.grant(15)
        assertNull(WatchLimit.block())
        assertTrue(WatchLimit.remaining()!! in 14 * 60..15 * 60)
        now = at(15, 0, day = 6)
        WatchLimit.tick(false)
        assertEquals(30 * 60, WatchLimit.remaining())
    }

    @Test
    fun pausedTimeAndGapsDoNotCount() {
        Prefs.setDailyLimit(Prefs.profileId, 30)
        now = at(10, 0)
        repeat(100) {
            now += 1000
            WatchLimit.tick(false)
        }
        now += 60_000 // app ngủ / trình phát đóng một lúc
        WatchLimit.tick(true)
        assertEquals(30 * 60, WatchLimit.remaining())
    }

    @Test
    fun bedtimeBlocksUntilSixInTheMorning() {
        Prefs.setBedtime(Prefs.profileId, 21 * 60)
        now = at(20, 59)
        assertNull(WatchLimit.block())
        now = at(21, 0)
        assertEquals(WatchLimit.Block.BEDTIME, WatchLimit.block())
        now = at(5, 59, day = 6)
        assertEquals(WatchLimit.Block.BEDTIME, WatchLimit.block())
        now = at(6, 0, day = 6)
        assertNull(WatchLimit.block())
    }

    @Test
    fun grantAtBedtimeUnblocksForAWhileOrUntilMorning() {
        Prefs.setBedtime(Prefs.profileId, 21 * 60)
        now = at(21, 30)
        WatchLimit.grant(30)
        assertNull(WatchLimit.block())
        now = at(22, 1)
        assertEquals(WatchLimit.Block.BEDTIME, WatchLimit.block())
        WatchLimit.grantUntilMorning()
        now = at(5, 59, day = 6)
        assertNull(WatchLimit.block())
        now = at(21, 0, day = 6)
        assertEquals(WatchLimit.Block.BEDTIME, WatchLimit.block())
    }

    @Test
    fun limitsBelongToEachProfile() {
        val kid = Prefs.addProfile("Bé Na")
        Prefs.setDailyLimit(kid.id, 45)
        assertNull(WatchLimit.remaining()) // "Cả nhà" không bị giới hạn
        assertTrue(Prefs.anyLimits)
        Prefs.switchProfile(kid.id)
        now = at(9, 0)
        assertEquals(45 * 60, WatchLimit.remaining())
        assertEquals("45 phút/ngày", WatchLimit.summary(kid.id))
        Prefs.switchProfile("")
        Prefs.deleteProfile(kid.id)
        assertFalse(Prefs.anyLimits)
    }

    @Test
    fun parentPin() {
        assertFalse(Prefs.hasParentPin)
        Prefs.setParentPin("1234")
        assertTrue(Prefs.hasParentPin)
        assertTrue(Prefs.checkParentPin("1234"))
        assertFalse(Prefs.checkParentPin("4321"))
        Prefs.setParentPin(null)
        assertFalse(Prefs.hasParentPin)
    }

    @Test
    fun labels() {
        assertEquals("Không giới hạn", WatchLimit.dailyLabel(0))
        assertEquals("45 phút", WatchLimit.dailyLabel(45))
        assertEquals("2 giờ", WatchLimit.dailyLabel(120))
        assertEquals("1 giờ 30 phút", WatchLimit.dailyLabel(90))
        assertEquals("21:30", WatchLimit.bedtimeLabel(21 * 60 + 30))
        assertEquals("1 phút", WatchLimit.minutesText(5))
        assertEquals("5 phút", WatchLimit.minutesText(300))
    }
}
