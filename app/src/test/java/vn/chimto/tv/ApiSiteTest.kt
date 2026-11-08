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

    /** Trang KKPhim (2.6): địa chỉ trong app theo www.kkphim.com, dữ liệu gọi thẳng phimapi.com. */
    @Test
    fun kkPhimSiteUsesPublicApi() {
        val kk = Prefs.KK_SITE
        assertTrue(ApiSite.isKnown("$kk/"))
        assertTrue(ApiSite.isKk("$kk/phim/tien-nghich/tap-165?sv=0"))
        assertTrue(ApiSite.isKk("https://kkphim.com"))
        assertTrue(!ApiSite.isKk("$o/phim/tien-nghich"))
        assertEquals("https://phimapi.com/v1/api/danh-sach/phim-moi-cap-nhat?page=1&limit=24", ApiSite.fallback(ApiSite.newestUrl(kk)))
        assertEquals("https://phimapi.com/v1/api/tim-kiem?keyword=gi%C3%A0+thi%C3%AAn&page=1&limit=24", ApiSite.fallback(ApiSite.searchUrl(kk, "già thiên")))
        assertEquals("https://phimapi.com/v1/api/danh-sach/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh&page=1&limit=24",
            ApiSite.fallback("$kk/api/movies/category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh&page=1&limit=24"))
        assertEquals("$kk/phim/tien-nghich", ApiSite.filmUrl("$kk/phim/tien-nghich/tap-165?sv=0"))
        // Danh sách v1 của phimapi.com: ảnh tương đối + APP_DOMAIN_CDN_IMAGE, phim mở theo tên miền kkphim.com
        val json = """{"status":true,"data":{"items":[{"name":"Đấu Phá Thương Khung (Phần 5)","slug":"dau-pha-thuong-khung-phan-5",
            "poster_url":"upload/vod/20240822-1/58a4.jpg","episode_current":"Tập 218"}],
            "params":{"pagination":{"totalItems":462,"totalItemsPerPage":24,"currentPage":1,"totalPages":20}},
            "APP_DOMAIN_CDN_IMAGE":"https://phimimg.com"}}"""
        val p = ApiSite.parseList(json, "$kk/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24")
        assertEquals("https://www.kkphim.com/phim/dau-pha-thuong-khung-phan-5", p.films.single().url)
        assertEquals("https://phimimg.com/upload/vod/20240822-1/58a4.jpg", p.films.single().poster)
        assertEquals("$kk/api/movies/category/hoat-hinh?country=trung-quoc&page=2&limit=24", p.nextUrl)
    }

    @Test
    fun filmUrlFromEpisode() {
        assertEquals("$o/phim/tien-nghich", ApiSite.filmUrl("$o/phim/tien-nghich/tap-01?sv=1"))
        assertEquals("$o/phim/tien-nghich", ApiSite.filmUrl("$o/phim/tien-nghich"))
        assertTrue(ApiSite.isKnown("$o/"))
        assertEquals("$o/api/movies/search?keyword=ti%C3%AAn+ngh%E1%BB%8Bch&page=1&limit=24", ApiSite.searchUrl(o, " tiên nghịch "))
    }

    /** VSMOV (2.7): API vsmov.com/api/… — items + pagination ở gốc, tập chỉ có trình phát nhúng. */
    @Test
    fun vsmovListsAndUrls() {
        val vs = "https://vsmov.com"
        assertTrue(ApiSite.isKnown("$vs/") && ApiSite.isVs("$vs/phim/tien-nghich"))
        assertEquals("$vs/api/danh-sach/phim-moi-cap-nhat?page=1", ApiSite.newestUrl(vs))
        assertEquals("$vs/api/tim-kiem?keyword=ti%C3%AAn+ngh%E1%BB%8Bch&page=1&limit=24", ApiSite.searchUrl(vs, "tiên nghịch"))
        assertEquals("$vs/api/the-loai/kiem-hiep?page=1", ApiSite.filters(vs).first.first { it.title == "Kiếm Hiệp" }.url)
        val json = """{"status":true,"items":[
            {"_id":54302,"name":"Túy Linh Lung","origin_name":"Lost Love in Times","slug":"tuy-linh-lung",
             "poster_url":"https://vsmov.com/storage/images/e6.jpg","thumb_url":"https://vsmov.com/storage/images/zv.jpg","year":2017}],
            "pathImage":"https://nguon.vsphim.com/storage/images/",
            "pagination":{"totalItems":2804,"totalItemsPerPage":20,"currentPage":1,"totalPages":141}}"""
        val p = ApiSite.parseList(json, "$vs/api/the-loai/hoat-hinh?country=trung-quoc&page=1")
        assertEquals("$vs/phim/tuy-linh-lung", p.films.single().url)
        assertEquals("https://vsmov.com/storage/images/e6.jpg", p.films.single().poster)
        assertEquals("$vs/api/the-loai/hoat-hinh?country=trung-quoc&page=2", p.nextUrl)
        // Chi tiết: tập chỉ có link_embed vẫn được giữ
        val d = ApiSite.parseDetail("""{"status":true,"movie":{"name":"Túy Linh Lung","lang":"Lồng tiếng"},"episodes":[
            {"server_name":"Lồng tiếng #1","server_data":[{"name":"1","slug":"tap-1","filename":"1",
             "link_embed":"https://v15.streamvsmov.com/video/fc1d03d9-0667-4a14-8891-7dee0d362b05"}]}]}""", "$vs/phim/tuy-linh-lung")
        assertEquals("$vs/phim/tuy-linh-lung/tap-1?sv=0", d.groups.single().episodes.single().url)
        assertEquals("Tập 1", d.groups.single().episodes.single().title) // VSMOV ghi số trần
    }

    @Test
    fun vsmovHlsFromPlayerPage() {
        val signed = """const playerOptions = { linkVast: "", enableSignedUrl: true,
            signedMasterUrl: "https://v2.streamvsmov.com/stream/55010e78/master.m3u8?expires=1794131570&signature=ca8c", volume: 100 };
            const baseUrl = "https://v2.streamvsmov.com"; const videoHash = "55010e78";"""
        assertEquals("https://v2.streamvsmov.com/stream/55010e78/master.m3u8?expires=1794131570&signature=ca8c", ApiSite.vsHlsFrom(signed))
        // Máy chủ không ký: link không có chữ ký
        val plain = """enableSignedUrl: false, signedMasterUrl: "",
            const baseUrl = "https://v15.streamvsmov.com"; const videoHash = "fc1d03d9-0667-4a14-8891-7dee0d362b05";"""
        assertEquals("https://v15.streamvsmov.com/stream/fc1d03d9-0667-4a14-8891-7dee0d362b05/master.m3u8", ApiSite.vsHlsFrom(plain))
        assertNull(ApiSite.vsHlsFrom("<html>lỗi</html>"))
    }
}
