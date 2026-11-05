package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** mephim.ddns.net (11/2026): ứng dụng React, API JSON /api/movies/… (định dạng KKPhim / phimapi.com). */
class ApiSiteTest {
    private val o = "https://mephim.ddns.net"

    @Test
    fun parsesListWithRelativePostersAndPaging() {
        val json = """{"status":true,"msg":"done","data":{"items":[
            {"name":"Tiên Nghịch","slug":"tien-nghich","poster_url":"uploads/movies/20260720/tien-nghich-poster.webp",
             "thumb_url":"uploads/movies/20260720/tien-nghich-thumb.webp","episode_current":"Tập 165"},
            {"name":"Avatar","slug":"avatar","poster_url":"https://phimimg.com/upload/vod/a.jpg","episode_current":"Full"},
            {"name":"Hết","slug":"het","thumb_url":"uploads/movies/x-thumb.webp","episode_current":"Hoàn Tất (12/12)"}],
            "params":{"pagination":{"totalItems":3629,"totalItemsPerPage":24,"currentPage":2,"totalPages":152}},
            "APP_DOMAIN_CDN_IMAGE":"https://phimimg.com"}}"""
        val url = "$o/api/movies/category/hoat-hinh?country=trung-quoc&page=2&limit=24"
        val p = ApiSite.parseList(json, url)
        assertEquals(listOf("Tiên Nghịch", "Avatar", "Hết"), p.films.map { it.title })
        assertEquals("$o/phim/tien-nghich", p.films[0].url)
        assertEquals("https://phimimg.com/uploads/movies/20260720/tien-nghich-poster.webp", p.films[0].poster)
        assertEquals("https://phimimg.com/upload/vod/a.jpg", p.films[1].poster)
        assertEquals("https://phimimg.com/uploads/movies/x-thumb.webp", p.films[2].poster)
        assertEquals(listOf("Tập 165", "Full", "Tập 12"), p.films.map { it.badge })
        assertEquals("$o/api/movies/category/hoat-hinh?country=trung-quoc&page=3&limit=24", p.nextUrl)
        assertNull(ApiSite.parseList(json.replace("\"currentPage\":2", "\"currentPage\":152"), url).nextUrl)
    }

    @Test
    fun parsesTrendingItemsAtRoot() {
        val json = """{"status":true,"cdnDomain":"https://phimimg.com/uploads/movies","items":[
            {"name":"VisionQuest","slug":"vision","poster_url":"https://image.tmdb.org/t/p/w500/x.jpg","episode_current":"Tập 6"}]}"""
        val films = ApiSite.parseItems(json, o)
        assertEquals("$o/phim/vision", films.single().url)
        assertEquals("https://image.tmdb.org/t/p/w500/x.jpg", films.single().poster)
    }

    @Test
    fun parsesDetailGroupsPerLanguage() {
        val json = """{"status":true,"movie":{"name":"Tiên Nghịch","origin_name":"Renegade Immortal",
            "content":"<p>Phim kể về <b>Vương Lâm</b>, một thiếu niên bình thường.</p>",
            "poster_url":"https://phimimg.com/uploads/movies/20260720/tien-nghich-poster.webp","episode_current":"Tập 165",
            "time":"23 phút/tập","lang":"Vietsub + Thuyết Minh","year":2023,
            "country":[{"name":"Trung Quốc"}],"category":[{"name":"Cổ Trang"},{"name":"Hoạt Hình"}]},
            "episodes":[
             {"server_name":"Vietsub","server_data":[
               {"name":"Tập 01","slug":"tap-01","link_embed":"https://player.phimapi.com/player/?url=a","link_m3u8":"https://s4.phim1280.tv/a/index.m3u8"},
               {"name":"Tập 02","slug":"tap-02","link_embed":"","link_m3u8":"https://s4.phim1280.tv/b/index.m3u8"},
               {"name":"Tập 03","slug":"tap-03","link_embed":"","link_m3u8":""}]},
             {"server_name":"Thuyết Minh","server_data":[
               {"name":"Tập 01","slug":"tap-01","link_embed":"","link_m3u8":"https://s5.phim1280.tv/c/index.m3u8"}]}]}"""
        val d = ApiSite.parseDetail(json, "$o/phim/tien-nghich")
        assertEquals("Tiên Nghịch", d.title)
        assertEquals("Renegade Immortal", d.otherTitle)
        assertEquals("Phim kể về Vương Lâm, một thiếu niên bình thường.", d.description)
        assertEquals("Tập 165  •  23 phút/tập  •  Vietsub + Thuyết Minh  •  Trung Quốc  •  2023  •  Cổ Trang, Hoạt Hình", d.info)
        assertEquals(listOf("Vietsub", "Thuyết Minh"), d.groups.map { it.name })
        assertEquals(listOf("Tập 1", "Tập 2"), d.groups[0].episodes.map { it.title }) // tập 3 chưa có video → bỏ
        assertEquals("$o/phim/tien-nghich/tap-01?sv=0", d.groups[0].episodes[0].url)
        assertEquals("$o/phim/tien-nghich/tap-01?sv=1", d.groups[1].episodes[0].url)
        assertTrue(d.groups[0].episodes.map { it.url } != d.groups[1].episodes.map { it.url })
    }

    @Test
    fun fallsBackToKkPhimApi() {
        assertEquals("https://phimapi.com/v1/api/danh-sach/phim-moi-cap-nhat?page=2&limit=24", ApiSite.fallback("$o/api/movies/newest?page=2&limit=24"))
        assertEquals("https://phimapi.com/v1/api/danh-sach/hoat-hinh?country=trung-quoc&page=1&limit=24",
            ApiSite.fallback("$o/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24"))
        assertEquals("https://phimapi.com/v1/api/the-loai/co-trang?page=1&limit=24", ApiSite.fallback("$o/api/movies/the-loai/co-trang?page=1&limit=24"))
        assertEquals("https://phimapi.com/v1/api/tim-kiem?keyword=ti%C3%AAn&page=1&limit=24", ApiSite.fallback("$o/api/movies/search?keyword=ti%C3%AAn&page=1&limit=24"))
        assertEquals("https://phimapi.com/phim/tien-nghich", ApiSite.fallback("$o/api/movies/detail/tien-nghich"))
        assertNull(ApiSite.fallback("$o/api/movies/trending"))
    }

    @Test
    fun filmUrlFromEpisode() {
        assertEquals("$o/phim/tien-nghich", ApiSite.filmUrl("$o/phim/tien-nghich/tap-01?sv=1"))
        assertEquals("$o/phim/tien-nghich", ApiSite.filmUrl("$o/phim/tien-nghich"))
        assertTrue(ApiSite.isKnown("$o/"))
        assertEquals("$o/api/movies/search?keyword=ti%C3%AAn+ngh%E1%BB%8Bch&page=1&limit=24", ApiSite.searchUrl(o, " tiên nghịch "))
    }
}
