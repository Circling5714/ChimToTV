package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** CLB Phim Xưa qua add-on Stremio k-20 (11/2026): catalog / meta / stream. */
class StremioTest {
    private val o = "https://sc.k-20.xyz"

    @Test
    fun buildsCatalogUrls() {
        assertEquals("$o/catalog/movie/clbpx-movie.json", Stremio.catalog(o))
        assertEquals(
            "$o/catalog/movie/clbpx-movie/genre=Th%E1%BB%83%20lo%E1%BA%A1i%3A%20Ki%E1%BA%BFm%20Hi%E1%BB%87p&skip=30.json",
            Stremio.catalog(o, genre = "Kiếm Hiệp", skip = 30),
        )
        assertEquals("$o/catalog/movie/clbpx-movie/search=thi%C3%AAn%20long.json", Stremio.searchUrl(o, " thiên long "))
    }

    @Test
    fun parsesCatalogAndPagesBySkip() {
        val json = """{"metas":[
            {"id":"clbpx:nch1993","type":"movie","name":"Nguyên Chấn Hiệp 1993 – 20 Tập","poster":"https://clbphimxua.com/a-400x400.jpg"},
            {"id":"clbpx:harry-5","type":"movie","name":"Harry Bẩn Thỉu 5: Mồ Chôn Tội Ác 1988","poster":"https://clbphimxua.com/b.jpg"},
            {"id":"nguonc:khac","type":"movie","name":"Không phải CLBPX"}]}"""
        val url = Stremio.catalog(o, genre = "Kiếm Hiệp", skip = 30)
        val p = Stremio.parseList(json, url)
        assertEquals(listOf("$o/clbpx/nch1993", "$o/clbpx/harry-5"), p.films.map { it.url })
        assertEquals(listOf("20 tập", null), p.films.map { it.badge })
        assertEquals(Stremio.catalog(o, genre = "Kiếm Hiệp", skip = 32), p.nextUrl)
        assertEquals(Stremio.catalog(o, skip = 2), Stremio.parseList(json, Stremio.catalog(o)).nextUrl)
        assertNull(Stremio.parseList(json, Stremio.searchUrl(o, "x")).nextUrl) // tìm kiếm chỉ một trang
        assertNull(Stremio.parseList("""{"metas":[]}""", url).nextUrl)
    }

    @Test
    fun parsesSeriesAndMovieDetail() {
        val series = """{"meta":{"id":"clbpx:nch1993","type":"series","name":"Nguyên Chấn Hiệp 1993 – 20 Tập",
            "poster":"https://clbphimxua.com/p.jpg","releaseInfo":"1993","description":"Phim kiếm hiệp.",
            "videos":[{"id":"clbpx:nch1993:HFvsf0tTx","title":"Tập 02","season":1,"episode":2},
                      {"id":"clbpx:nch1993:LM4bPc-3a","title":"Tập 01","season":1,"episode":1}]}}"""
        val d = Stremio.parseDetail(series, "$o/clbpx/nch1993")
        assertEquals("Nguyên Chấn Hiệp 1993 – 20 Tập", d.title)
        assertEquals("1993", d.info)
        assertEquals(listOf("Tập 1", "Tập 2"), d.groups.single().episodes.map { it.title })
        assertEquals("$o/clbpx/nch1993/LM4bPc-3a", d.groups.single().episodes[0].url)
        assertEquals("$o/clbpx/nch1993", Stremio.filmUrl("$o/clbpx/nch1993/LM4bPc-3a"))

        // Phim lẻ: meta không có videos → một tập "Xem phim" (mã tập = mã phim)
        val movie = """{"meta":{"id":"clbpx:harry-5","type":"movie","name":"Harry Bẩn Thỉu 5","releaseInfo":"1988"}}"""
        val m = Stremio.parseDetail(movie, "$o/clbpx/harry-5")
        assertEquals(listOf("$o/clbpx/harry-5/full"), m.groups.single().episodes.map { it.url })
    }

    @Test
    fun eachMp4IsAServer() {
        val json = """{"streams":[
            {"name":"CLB Phim Xưa","title":"1080p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=5&size=3069863340"},
            {"name":"CLB Phim Xưa","title":"1080p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=5&size=1938835536"},
            {"name":"CLB Phim Xưa","title":"360p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=2&size=395465856"},
            {"name":"Hỏng","title":"720p","url":""}]}"""
        val s = Stremio.parseSource(json, "$o/clbpx/harry-5/full")
        assertEquals(listOf("1080p · 3,1 GB", "1080p · 1,9 GB", "360p"), s.servers.map { it.label })
        val v = Site.video(s, s.servers[2])
        assertEquals("https://sc.k-20.xyz/hx-mp4?embed=x&res=2&size=395465856", v.url)
        assertTrue(v.file)
    }
}
