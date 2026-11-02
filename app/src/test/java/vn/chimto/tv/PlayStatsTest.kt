package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.text.Normalizer

/** Chọn / gợi ý server theo tên server thật của yanhh3d (gặp khi thử trên TX3 ngày 2/11/2026). */
class PlayStatsTest {
    private val v1 = Server("VIETSUB 1080 V1", "a")
    private val v2 = Server("VIETSUB 1080 V2", "b")
    private val k1 = Server("VIETSUB 4K V1", "c")
    private val k2 = Server("VIETSUB 4K V2", "d")
    private val tm = Server("Thuyết minh 1080 V1", "e")
    private val all = listOf(v1, v2, k1, k2, tm)

    private val unknown = PlayStats.Score(-1, null, 0.0)
    private val smooth = PlayStats.Score(0, 4, 0.0)
    private val bad = PlayStats.Score(2, 20, 0.12)

    private fun scores(vararg s: Pair<Server, PlayStats.Score>): (Server) -> PlayStats.Score {
        val m = s.toMap()
        return { m[it] ?: unknown }
    }

    @Test
    fun readsLanguageAndResolution() {
        assertEquals("vs", PlayStats.lang("VIETSUB 1080 V1"))
        assertEquals("tm", PlayStats.lang("Thuyết minh 1080 V1"))
        // Trang có thể trả chữ tiếng Việt dạng tổ hợp (NFD)
        assertEquals("tm", PlayStats.lang(Normalizer.normalize("Thuyết Minh", Normalizer.Form.NFD)))
        assertEquals("lt", PlayStats.lang("Lồng Tiếng"))
        assertNull(PlayStats.lang("VIP"))
        assertEquals(2160, PlayStats.resolution("VIETSUB 4K V1"))
        assertEquals(1080, PlayStats.resolution("VIETSUB 1080 V2"))
        assertEquals(720, PlayStats.resolution("HD 720p"))
        assertNull(PlayStats.resolution("VIP"))
    }

    @Test
    fun suggestsSameResolutionMirrorFirst() {
        assertEquals(v2, PlayStats.alternative(all, v1, scores())?.first)
        assertEquals(k2, PlayStats.alternative(all, k1, scores())?.first)
    }

    @Test
    fun neverSuggestsSharperUntestedServerOrOtherLanguage() {
        // Tình huống trên TX3: V2 giật, V1 đã chấm hay giật → chỉ còn 4K (chưa thử) và Thuyết minh → không gợi ý gì
        assertNull(PlayStats.alternative(all, v2, scores(v1 to bad)))
    }

    @Test
    fun stepsDownWhenSameResolutionIsBad() {
        assertEquals(v1, PlayStats.alternative(all, k1, scores(k2 to bad))?.first)
    }

    @Test
    fun ratedSmoothServerBeatsUntested() {
        assertEquals(k1, PlayStats.alternative(all, v1, scores(k1 to smooth))?.first)
    }

    @Test
    fun ratesRealMeasurements() {
        // Skyworth: 95 s chạy, khựng 1 lần (1 s) → mượt, không phải "hay giật"
        assertEquals(0, PlayStats.level(1, 0, 95, 1, 1))
        // TX3: ba lần xem, đứng hình 24 s / 175 s, giật 13 lần → hay giật
        assertEquals(2, PlayStats.level(3, 0, 151, 24, 13))
        // 30 phút, 6 lần khựng 2 s (5 phút một lần) → hơi giật
        assertEquals(1, PlayStats.level(4, 0, 1788, 12, 6))
        // Chưa đủ 1 phút → chưa chấm
        assertEquals(-1, PlayStats.level(1, 0, 40, 5, 2))
        // Một nửa số lần mở không chạy được → hay lỗi
        assertEquals(3, PlayStats.level(2, 1, 300, 0, 0))
    }

    @Test
    fun choosesBetterServerOnlyWhenUsualIsBad() {
        assertEquals(v1, PlayStats.choose(all, v1, scores(v2 to smooth)))
        assertEquals(v2, PlayStats.choose(all, v1, scores(v1 to bad, v2 to smooth)))
        // Không tự đổi sang Thuyết minh dù chấm tốt hơn
        assertEquals(v1, PlayStats.choose(all, v1, scores(v1 to bad, tm to smooth)))
    }
}
