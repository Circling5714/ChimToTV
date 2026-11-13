package vn.chimto.tv

import com.google.zxing.BinaryBitmap
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import com.google.zxing.qrcode.encoder.Encoder
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Màn «Mời bạn bè cài app»: mã Downloader và mã QR vẽ sẵn (Invite.QR) phải đúng với link trong mã nguồn. */
class InviteTest {
    @Test
    fun codeAndLinks() {
        assertTrue("mã Downloader là dãy số", Invite.CODE.matches(Regex("\\d{5,9}")))
        assertEquals("aftv.news/${Invite.CODE}", Invite.SHORT_URL)
        // Link cố định: release đang là Latest, tệp tên cố định (release.ps1 đính kèm ở mọi bản TV)
        assertTrue(Invite.APK_URL.endsWith("/releases/latest/download/ConBangTV.apk"))
        assertTrue(Invite.GUIDE_URL.startsWith("https://github.com/Circling5714/ChimToTV/"))
    }

    /** Ba bản (TV, iPad, Windows) gửi cùng một tin nhắn — sửa câu chữ thì sửa cả ba bản và cả ba test. */
    @Test
    fun messageIsTheSameOnEveryPlatform() {
        assertEquals(
            "Cài Côn Bằng TV trên tivi Android:\n" +
                "1. Trên tivi mở CH Play, cài app Downloader (của AFTVnews).\n" +
                "2. Mở Downloader, nhập 9192365 rồi bấm Go.\n" +
                "3. Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.\n" +
                "Cài xong app tự cập nhật các bản sau.\n" +
                "Hướng dẫn: https://github.com/Circling5714/ChimToTV/blob/main/README.md#downloader",
            Invite.MESSAGE,
        )
    }

    /** Dựng lại mã QR của GUIDE_URL bằng ZXing: phải giống hệt Invite.QR. Lệch (vừa đổi link) thì dán các hàng in ra dưới đây. */
    @Test
    fun qrIsTheCodeOfGuideUrl() {
        val m = Encoder.encode(Invite.GUIDE_URL, ErrorCorrectionLevel.M).matrix
        val rows = (0 until m.height).map { y -> (0 until m.width).joinToString("") { x -> if (m.get(x, y).toInt() == 1) "1" else "0" } }
        assertEquals(
            "Invite.QR không phải mã QR của GUIDE_URL — dán lại:\n" + rows.joinToString("\n") { "        \"$it\"," } + "\n",
            rows, Invite.QR,
        )
    }

    /** Đọc lại như điện thoại: vẽ Invite.QR kiểu QrView (nền trắng, lề 4 ô) rồi giải mã — phải ra đúng GUIDE_URL. */
    @Test
    fun qrReadsBackAsGuideUrl() {
        val quiet = 4
        val scale = 5
        val n = Invite.QR.size
        assertTrue("mã QR vuông", n >= 21 && Invite.QR.all { it.length == n })
        val side = (n + 2 * quiet) * scale
        val white = 0xFFFFFFFF.toInt()
        val black = 0xFF000000.toInt()
        val px = IntArray(side * side) { white }
        for ((y, row) in Invite.QR.withIndex()) {
            for ((x, c) in row.withIndex()) {
                if (c != '1') continue
                for (dy in 0 until scale) for (dx in 0 until scale) px[((y + quiet) * scale + dy) * side + (x + quiet) * scale + dx] = black
            }
        }
        val read = QRCodeReader().decode(BinaryBitmap(HybridBinarizer(RGBLuminanceSource(side, side, px))))
        assertEquals(Invite.GUIDE_URL, read.text)
    }
}
