package vn.chimto.tv

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Điểm IMDb từ dữ liệu KKPhim + xếp hạng «Điểm IMDb cao» (ImdbTop.kt). */
class ImdbTopTest {
    private fun imdb(s: Double, v: Int) = JSONObject().put("id", "tt1").put("vote_average", s).put("vote_count", v)

    @Test
    fun parsesScoreAndIgnoresTinyVoteCounts() {
        assertEquals(Imdb(8.5, 377702), Imdb.of(imdb(8.5, 377702)))
        assertNull(Imdb.of(imdb(10.0, 1))) // 10 điểm từ 1 lượt chấm: bỏ
        assertNull(Imdb.of(imdb(0.0, 0)))
        assertNull(Imdb.of(null))
        assertEquals("IMDb 8.5", Imdb(8.5, 1000).label)
        assertEquals("IMDb 7.0", Imdb(7.0, 1000).label)
        assertEquals("IMDb 8.5 (377 nghìn lượt chấm)", Imdb(8.5, 377702).detail)
        assertEquals("2,4 triệu", Imdb.votesText(2_400_000))
        assertEquals("1 triệu", Imdb.votesText(1_000_000))
        assertEquals("850", Imdb.votesText(850))
    }

    /** Nhãn tập + nhãn IMDb ở đáy ảnh bìa (BadgeRow): chung một hàng khi đủ chỗ, không thì nhãn IMDb lên hàng trên. */
    @Test
    fun imdbBadgeMovesAboveEpisodeBadgeWhenTheCardIsNarrow() {
        // Thẻ rộng 150, khoảng hở 4: «Full» (40) + «IMDb 8.6» (60) vừa một hàng
        assertFalse(BadgeRow.needsTwoRows(150, 40, 60, 4))
        assertFalse(BadgeRow.needsTwoRows(104, 40, 60, 4)) // vừa khít
        // «Hoàn Tất (16/16)» (110) + «IMDb 8.6» (60) không vừa → hai hàng; thẻ hẹp trên điện thoại cầm dọc cũng vậy
        assertTrue(BadgeRow.needsTwoRows(150, 110, 60, 4))
        assertTrue(BadgeRow.needsTwoRows(103, 40, 60, 4))
        // Chỉ có một nhãn: luôn một hàng, kể cả khi nhãn đó dài hơn thẻ (chữ bị cắt bằng dấu …)
        assertFalse(BadgeRow.needsTwoRows(100, 0, 60, 4))
        assertFalse(BadgeRow.needsTwoRows(100, 140, 0, 4))
    }

    @Test
    fun listAndDetailCarryImdb() {
        val body = """{"status":true,"data":{"items":[
            {"name":"Sử Thi Odyssey","slug":"su-thi-odyssey","poster_url":"a.jpg","episode_current":"Full",
             "imdb":{"id":"tt33764258","vote_average":8.5,"vote_count":377702}},
            {"name":"Không điểm","slug":"khong-diem","poster_url":"b.jpg","imdb":{"id":"tt1","vote_average":0,"vote_count":0}}
        ]}}"""
        val films = ApiSite.parseItems(body, "https://www.kkphim.com")
        assertEquals(Imdb(8.5, 377702), films[0].imdb)
        assertNull(films[1].imdb)
        val detail = """{"status":true,"movie":{"name":"Sử Thi Odyssey","slug":"su-thi-odyssey","episode_current":"Full",
            "year":2026,"country":[{"name":"Âu Mỹ"}],"category":[{"name":"Hành Động"}],
            "imdb":{"id":"tt33764258","vote_average":8.5,"vote_count":377702}},"episodes":[]}"""
        val d = ApiSite.parseDetail(detail, "https://www.kkphim.com/phim/su-thi-odyssey")
        assertTrue(d.info, d.info!!.contains("IMDb 8.5 (377 nghìn lượt chấm)"))
    }

    @Test
    fun feedItemKeepsImdbInCache() {
        val f = FeedItem("https://www.kkphim.com/phim/a", "A", null, null, setOf("a"), 2026, 0, null, null, Imdb(7.9, 5000))
        assertEquals(Imdb(7.9, 5000), FeedItem.of(f.json())!!.imdb)
        val g = f.copy(imdb = null)
        assertNull(FeedItem.of(g.json())!!.imdb)
    }

    @Test
    fun weightedRankPullsLowVoteFilmsTowardTheMean() {
        fun film(n: String, s: Double, v: Int) = Film("https://www.kkphim.com/phim/$n", n, null, null, Imdb(s, v))
        val films = listOf(
            film("nhieu-luot", 8.6, 2_000_000), // điểm cao, rất nhiều lượt
            film("it-luot", 9.4, 1_200), // điểm cao hơn nhưng ít lượt → bị kéo về trung bình
            film("trung-binh", 7.0, 300_000),
            film("qua-it", 9.9, 300), // dưới số lượt tối thiểu: bỏ
            Film("https://www.kkphim.com/phim/khong", "không điểm", null, null),
        )
        val r = ImdbTop.rank(films, minVotes = 1000, top = 10).map { it.title }
        assertEquals(listOf("nhieu-luot", "it-luot", "trung-binh"), r)
        assertTrue(ImdbTop.rank(films, minVotes = 5_000_000, top = 10).isEmpty())
        assertEquals(1, ImdbTop.rank(films, minVotes = 1000, top = 1).size)
    }

    @Test
    fun pagingUrls() {
        assertEquals("https://www.kkphim.com/api/movies/the-loai/hanh-dong?page=3&limit=64",
            ImdbTop.withPage("https://www.kkphim.com/api/movies/the-loai/hanh-dong?page=1&limit=24", 3))
        assertEquals("https://www.kkphim.com/api/movies/category/phim-le?page=2&limit=64",
            ImdbTop.withPage("https://www.kkphim.com/api/movies/category/phim-le", 2))
        assertEquals("https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=4&limit=64",
            ImdbTop.withPage("https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24", 4))
        assertTrue(ImdbTop.canRank("https://www.kkphim.com/api/movies/category/phim-le?page=1&limit=24"))
        assertFalse(ImdbTop.canRank("https://vsmov.com/api/the-loai/hoat-hinh?page=1"))
        assertFalse(ImdbTop.canRank("https://phim.nguonc.com/api/films/danh-sach/phim-dang-chieu"))
    }
}
