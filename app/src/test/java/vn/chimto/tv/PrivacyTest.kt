package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/** User-Agent rút gọn: giữ số phiên bản chính, bỏ model / mã build / bản Android / "; wv" (UA thật của TX3). */
class PrivacyTest {
    @Test
    fun reducesTx3WebViewUa() {
        val ua = Privacy.reduce(
            "Mozilla/5.0 (Linux; Android 9; TX3 Mini Build/PPR1.180610.011; wv) AppleWebKit/537.36 (KHTML, like Gecko) " +
                "Version/4.0 Chrome/138.0.7204.179 Mobile Safari/537.36",
        )
        assertEquals("Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Mobile Safari/537.36", ua)
        assertFalse("TX3" in ua || "Build/" in ua || "wv" in ua || "7204" in ua)
    }

    @Test
    fun keepsNonMobileAndFallsBack() {
        assertEquals(
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
            Privacy.reduce("Mozilla/5.0 (Linux; Android 11; SWTV; wv) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.1.2 Safari/537.36"),
        )
        // Không đọc được số phiên bản → dùng số dự phòng, vẫn đúng dạng
        assertEquals(
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36",
            Privacy.reduce("lạ"),
        )
    }
}
