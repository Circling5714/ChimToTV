package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Phim liên quan (11/2026): tên gốc, số phần, ghép cùng bộ, hàng cùng thể loại — cùng ca với RelatedTests.cs / .swift. */
class RelatedTest {
    private fun f(title: String, url: String = "https://x.test/" + KkPhim.norm(title).replace(' ', '-')) = Film(url, title, null, null)

    @Test
    fun searchNameDropsPartYearAndEpisodes() {
        assertEquals("Tiên Nghịch", Related.searchName("Tiên Nghịch (Phần 2) 2024 – 20 Tập"))
        assertEquals("Đấu Phá Thương Khung", Related.searchName("Đấu Phá Thương Khung Phần 5"))
        assertEquals("Nhất Nguyệt Thần Kiếm", Related.searchName("Nhất Nguyệt Thần Kiếm 1990 (Phần 01) – 20 Tập"))
        assertEquals("Avatar", Related.searchName("Avatar: Dòng Chảy Của Nước"))
        assertEquals("1917", Related.searchName("1917")) // tên chỉ là năm: giữ nguyên
    }

    @Test
    fun partNumbers() {
        assertEquals(5, Related.part("Đấu Phá Thương Khung Phần 5"))
        assertEquals(1, Related.part("Nhất Nguyệt Thần Kiếm 1990 (Phần 01) – 20 Tập"))
        assertEquals(2, Related.part("Squid Game Season 2"))
        assertEquals(3, Related.part("Thế Giới Hoàn Mỹ 3"))
        assertEquals(4, Related.part("Đấu Phá Thương Khung 4: Đào Vong"))
        assertNull(Related.part("Tiên Nghịch"))
        assertNull(Related.part("Trái Tim Quái Thú 2025")) // năm không phải số phần
    }

    @Test
    fun sameSeriesMatchesOtherPartsOnly() {
        assertTrue(Related.sameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung Phần 1"))
        assertTrue(Related.sameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung"))
        assertTrue(Related.sameSeries("Avatar", "Avatar: Dòng Chảy Của Nước"))
        assertFalse(Related.sameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung Phần 5")) // chính phim đó (nguồn khác)
        assertFalse(Related.sameSeries("Tiên Nghịch", "Tiên Nghịch Truyện Kỳ")) // tên khác
        assertFalse(Related.sameSeries("Yêu", "Yêu 2")) // tên gốc quá ngắn — dễ trùng nhầm
    }

    @Test
    fun pickSeriesOrdersByPartOnePerPart() {
        val found = listOf(
            f("Đấu Phá Thương Khung Phần 3"), f("Đấu Phá Thương Khung Phần 5", "https://x.test/me"),
            f("Đấu Phá Thương Khung"), f("Đấu Phá Thương Khung Phần 3", "https://kk.test/dpk3"), f("Kiếm Lai"),
            f("Đấu Phá Thương Khung Phần 2"),
        )
        val got = Related.pickSeries("Đấu Phá Thương Khung Phần 5", "https://x.test/me", found)
        assertEquals(listOf("Đấu Phá Thương Khung", "Đấu Phá Thương Khung Phần 2", "Đấu Phá Thương Khung Phần 3"), got.map { it.title })
        // Phim lẻ ngoại truyện cùng bộ («… 2: Ngừng Chiến», «…: Thức Tỉnh») không bị gộp mất
        val more = Related.pickSeries("Đấu Phá Thương Khung (Phần 5)", "https://x.test/me",
            listOf(f("Đấu Phá Thương Khung: Thức Tỉnh"), f("Đấu Phá Thương Khung 2: Ngừng Chiến"), f("Đấu Phá Thương Khung (Phần 2)")))
        assertEquals(listOf("Đấu Phá Thương Khung: Thức Tỉnh", "Đấu Phá Thương Khung 2: Ngừng Chiến", "Đấu Phá Thương Khung (Phần 2)"),
            more.map { it.title })
    }

    @Test
    fun pickSimilarSkipsSelfSeriesAndPutsWatchedLast() {
        val found = listOf(f("Tiên Nghịch"), f("Phàm Nhân Tu Tiên"), f("Tiên Nghịch Phần 2"), f("Thế Giới Hoàn Mỹ"), f("Phàm Nhân Tu Tiên"))
        val got = Related.pickSimilar("Tiên Nghịch", "https://x.test/me", found, watched = setOf(KkPhim.norm("Phàm Nhân Tu Tiên")))
        assertEquals(listOf("Thế Giới Hoàn Mỹ", "Phàm Nhân Tu Tiên"), got.map { it.title })
    }
}
