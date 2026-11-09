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

    /** Sửa nhanh trên trang chủ: chuyển qua hàng đang hiện kề nó (hàng tắt / không hiện nằm giữa bị vượt qua). */
    @Test
    fun moveShownSkipsHiddenRows() {
        val prefs = listOf("new-eps" to true, "continue" to true, "similar" to true, "interest" to false, "dominant" to true,
            "newest" to true, "movies" to true)
        val shown = listOf("similar", "dominant", "movies") // «Mới cập nhật» không đủ phim nên không hiện
        val up = HomeFeed.moveShown(prefs, shown, "dominant", up = true)!!
        assertEquals(listOf("new-eps", "continue", "dominant", "similar", "interest", "newest", "movies"), up.map { it.first })
        val down = HomeFeed.moveShown(prefs, shown, "dominant", up = false)!!
        assertEquals(listOf("new-eps", "continue", "similar", "interest", "newest", "movies", "dominant"), down.map { it.first })
        assertEquals(false, down.first { it.first == "interest" }.second)
        // Hàng đầu không lên được, hàng cuối không xuống được; «Xem tiếp» không chuyển
        assertEquals(null, HomeFeed.moveShown(prefs, shown, "similar", up = true))
        assertEquals(null, HomeFeed.moveShown(prefs, shown, "movies", up = false))
        assertEquals(null, HomeFeed.moveShown(prefs, listOf("continue", "similar"), "continue", up = false))
        assertEquals(null, HomeFeed.moveShown(prefs, shown, "newest", up = true))
    }

    @Test
    fun hideTurnsOffOnlyThatRow() {
        val prefs = listOf("similar" to true, "custom-a" to true, "movies" to false)
        assertEquals(listOf("similar" to true, "custom-a" to false, "movies" to false), HomeFeed.hide(prefs, "custom-a"))
    }

    @Test
    fun arrangeSlotsKeepsSlotOfLegacyRows() {
        fun row(id: String, slot: String = "") = FeedRow(id, id, emptyList(), slot = slot)
        val rows = listOf(row("interest-a"), row("custom-x", "custom-x"), row("interest-b"))
        val prefs = listOf("custom-x" to true, "interest2" to true, "interest" to true)
        assertEquals(listOf("custom-x" to "custom-x", "interest-b" to "interest2", "interest-a" to "interest"),
            HomeFeed.arrangeSlots(rows, prefs).map { it.first.id to it.second })
    }

    /** Nguồn tắt thì bỏ; tắt hết nguồn đang có thì coi như bật hết (trang chủ không trống). */
    @Test
    fun sourcesOff() {
        val all = HomeFeed.Sources("https://kk", "https://nc", null, "https://k20", listOf("https://web1", "https://web2"))
        assertEquals(listOf("kk", "nc", "k20", "web"), all.ids)
        val s = HomeFeed.withoutOff(all, setOf("kk", "web", "vs"))
        assertEquals(listOf("nc", "k20"), s.ids)
        assertEquals(null, s.kk)
        assertEquals(emptyList<String>(), s.web)
        assertEquals(all.ids, HomeFeed.withoutOff(all, setOf("kk", "nc", "k20", "web")).ids)
        // Nguồn đã tắt không còn trong danh sách trang: nguồn còn lại vẫn dùng
        assertEquals(listOf("nc"), HomeFeed.withoutOff(HomeFeed.Sources(null, "https://nc", null, null, emptyList()), setOf("kk")).ids)
    }
}
