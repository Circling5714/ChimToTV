package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** hoathinh3d.pw (11/2026): danh sách nạp bằng API JSON (cùng ca với HalimApiTests.swift của bản iPad). */
class HalimApiTest {
    private val pageHtml = """<main id="main-contents"><div class="mv-grid" id="mvGrid"></div></main>
        <script>(function(){ const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/halim\/v1\/movies"; const PER_PAGE = 20;
        const SORTBY = "default"; })();</script>"""

    @Test
    fun findsApiInPage() {
        assertEquals("https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=1&per_page=20", Site.HalimApi.firstPage(pageHtml))
        assertEquals("https://hoathinh3d.pw/wp-json/halim/v1/ongoing?page=1&per_page=20",
            Site.HalimApi.firstPage("""const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/halim\/v1\/ongoing";"""))
        assertNull(Site.HalimApi.firstPage("<div class=\"halim-item\"></div>"))
        assertEquals("https://hoathinh3d.pw/wp-json/movie-schedule/v1/daily",
            Site.HalimApi.scheduleApi("""const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/movie-schedule\/v1\/daily";"""))
    }

    @Test
    fun parsesItemsAndNextPage() {
        val json = """{"success":true,"page":1,"per_page":20,"total":33,"has_more":true,"items":[
            {"id":1,"title":"Tiên Nghịch","permalink":"https:\/\/hoathinh3d.pw\/tien-nghich","poster_url":"https:\/\/x\/a.jpg","average_rating":4.6,"latest_episode":"Tập 165"},
            {"id":2,"rank":2,"title":"Mục Thần Ký","permalink":"https:\/\/hoathinh3d.pw\/muc-than-ky","poster_url":"https:\/\/x\/b.jpg"},
            {"id":3,"title":"Phim","permalink":"https:\/\/hoathinh3d.pw\/phim","poster_url":"","latest_episode":"12"}]}"""
        val url = "https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=1&per_page=20"
        val p = Site.HalimApi.parse(json, url)
        assertEquals(listOf("Tiên Nghịch", "Mục Thần Ký", "Phim"), p.films.map { it.title })
        assertEquals(listOf("Tập 165", "Top 2", "Tập 12"), p.films.map { it.badge })
        assertNull(p.films[2].poster)
        assertEquals("https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=2&per_page=20", p.nextUrl)
        assertNull(Site.HalimApi.parse(json.replace("\"has_more\":true", "\"has_more\":false"), url).nextUrl)
    }

    @Test
    fun parsesScheduleDay() {
        val json = """{"success":true,"day":"thu-nam","early_movies":[{"title":"Tru Tiên","permalink":"https:\/\/h\/tru-tien",
            "thumbnail":"https:\/\/x\/t.jpg","latest_episode":"Tập 91","is_early_screening":true,"early_screening_time":"18:00"}],
            "regular_movies":[{"title":"A","permalink":"https:\/\/h\/a","thumbnail":"","latest_episode":"Tập 17"}]}"""
        val films = Site.HalimApi.parseDay(json)
        assertEquals(listOf("Tru Tiên", "A"), films.map { it.title })
        assertEquals("18:00 · Tập 91", films[0].badge)
        assertEquals("https://x/t.jpg", films[0].poster)
    }
}
