package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Test

/** Các hàng trang chủ người xem bật / tắt, sắp thứ tự (Cài đặt → Trang chủ → Các hàng). */
class HomeRowsTest {
    private val defaults = listOf("new-eps", "continue", "similar", "interest", "dominant", "newest", "movies")

    @Test
    fun emptyMeansDefaultAllOn() {
        assertEquals(defaults.map { it to true }, HomeFeed.mergeRows(emptyList(), defaults))
    }

    @Test
    fun keepsSavedOrderAndOff() {
        val m = HomeFeed.mergeRows(listOf("new-eps", "continue", "newest", "-similar", "interest", "dominant", "movies"), defaults)
        assertEquals(listOf("new-eps", "continue", "newest", "similar", "interest", "dominant", "movies"), m.map { it.first })
        assertEquals(false, m.first { it.first == "similar" }.second)
        assertEquals(true, m.first { it.first == "newest" }.second)
    }

    @Test
    fun dropsUnknownAndDuplicates() {
        val m = HomeFeed.mergeRows(listOf("bogus", "movies", "-movies", "similar"), defaults)
        assertEquals(1, m.count { it.first == "movies" })
        assertEquals(true, m.first { it.first == "movies" }.second)
        assertEquals(defaults.size, m.size)
    }

    /** Hàng mới ở bản sau (chưa có trong thứ tự đã lưu) chèn ngay sau hàng đứng trước nó trong thứ tự mặc định. */
    @Test
    fun newRowsInsertedAfterTheirDefaultPredecessor() {
        val m = HomeFeed.mergeRows(listOf("movies", "newest", "similar", "new-eps", "continue"), defaults)
        assertEquals(listOf("movies", "newest", "similar", "interest", "dominant", "new-eps", "continue"), m.map { it.first })
    }

    @Test
    fun arrangeFiltersAndOrders() {
        fun row(id: String, slot: String = "") = FeedRow(id, id, emptyList(), null, slot)
        val rows = listOf(row("topic-cn", "topics"), row("similar", "similar"), row("interest-a", "interest"),
            row("newest", "newest"), row("movies", "movies"))
        val prefs = listOf("new-eps" to true, "continue" to true, "movies" to true, "similar" to false, "newest" to true,
            "interest" to true)
        assertEquals(listOf("topic-cn", "movies", "newest", "interest-a"), HomeFeed.arrange(rows, prefs).map { it.id })
    }

    /** Hàng tự thêm đứng ngay sau «Xem tiếp»; hàng vừa thêm (chưa có trong thứ tự đã lưu) cũng vào đó. */
    @Test
    fun customRowsDefaultAfterHead() {
        val custom = listOf(CustomRow("a", type = "hoathinh"), CustomRow("t-kr", topic = "kr"))
        val d = HomeFeed.rowDefaults(custom)
        assertEquals(listOf("new-eps", "continue", "custom-a", "custom-t-kr", "similar"), d.take(5))
        val m = HomeFeed.mergeRows(listOf("new-eps", "continue", "custom-a", "movies", "similar"), d)
        assertEquals(listOf("new-eps", "continue", "custom-a", "custom-t-kr", "movies", "similar"), m.take(6).map { it.first })
    }

    @Test
    fun customTitles() {
        assertEquals("Hoạt hình Trung Quốc · phiêu lưu · thuyết minh",
            HomeFeed.customTitle(CustomRow("x", type = "hoathinh", country = "trung-quoc", genre = "phieu-luu", lang = "thuyet-minh")))
        assertEquals("Phim bộ Hàn Quốc", HomeFeed.customTitle(CustomRow("y", type = "series", country = "han-quoc")))
        assertEquals("Phim lẻ · hành động", HomeFeed.customTitle(CustomRow("z", type = "single", genre = "hanh-dong")))
        assertEquals("Phim Hàn", HomeFeed.customTitle(CustomRow("t-kr", topic = "kr")))
    }

    /** Trang chủ lưu từ bản cũ (chưa ghi slot): đoán theo id — hai hàng «Vì … hay xem» theo thứ tự xuất hiện. */
    @Test
    fun arrangeLegacyRows() {
        fun row(id: String) = FeedRow(id, id, emptyList())
        val rows = listOf(row("similar"), row("interest-hanh-dong"), row("dominant"), row("interest-tien-hiep"), row("newest"))
        val prefs = listOf("interest2" to true, "similar" to true, "interest" to false, "dominant" to true, "newest" to true)
        assertEquals(listOf("interest-tien-hiep", "similar", "dominant", "newest"), HomeFeed.arrange(rows, prefs).map { it.id })
    }
}
