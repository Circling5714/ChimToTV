package vn.chimto.tv

import org.json.JSONObject
import org.jsoup.Jsoup
import org.jsoup.nodes.Document
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Đọc trang Next.js kiểu PhimMoi (phimmoir.fun) từ dữ liệu self.__next_f.push dựng tay, rút gọn theo trang thật. */
class NextSiteTest {
    private val base = "https://phim.test"

    /** Trang có dữ liệu cắt thành nhiều đoạn push (trang thật cắt ngay giữa đối tượng JSON). */
    private fun page(url: String, data: String, parts: Int = 3): Document {
        val size = (data.length + parts - 1) / parts
        val scripts = data.chunked(size).joinToString("") { "<script>self.__next_f.push([1,${JSONObject.quote(it)}])</script>" }
        return Jsoup.parse("<html><head><script>self.__next_f=self.__next_f||[]</script></head><body>$scripts</body></html>", url)
    }

    private fun movie(id: Int, slug: String, name: String, status: String) =
        """{"id":$id,"name":"$name","origin_name":"","slug":"$slug","description":"${'$'}27","poster_url":"/public/images/posters/$slug.webp",""" +
            """"display_status":"$status","country":{"name":"Trung Quốc","slug":"trung-quoc"},"genres":[{"id":11,"name":"Hoạt Hình","slug":"hoat-hinh","is_active":true}]}"""

    private fun eps(n: Int, lang: String) =
        (1..n).joinToString(",", "[", "]") { """{"id":$it,"episode_label":"Tập $it","slug":"tap-$it","is_embed":false,"language":"$lang","name":"Tập $it"}""" }

    private val sources = """[{"server_name":"KK Phim","server_slug":"kk","language_name":"Vietsub","language_slug":"vietsub","total_episodes":3,"priority_order":2},""" +
        """{"server_name":"NguonC","server_slug":"nc","language_name":"Vietsub","language_slug":"vietsub","total_episodes":3,"priority_order":3},""" +
        """{"server_name":"KK Phim","server_slug":"kk","language_name":"Thuyết Minh","language_slug":"thuyetminh","total_episodes":2,"priority_order":2}]"""

    @Test
    fun filters() {
        fun item(id: Int, name: String, slug: String, active: Boolean = true) =
            """{"id":$id,"name":"$name","slug":"$slug","is_active":$active}"""
        val data = """1:{"results":[${movie(1, "a", "A", "Tập 1")}],"genres":[""" +
            listOf(item(1, "Âm Nhạc", "am-nhac"), item(2, "Cổ Trang", "co-trang"), item(3, "Ẩn", "an", active = false)).joinToString(",") +
            """],"countries":[""" +
            listOf(item(54, "1983", "1983"), item(10, "Anh", "anh"), item(60, "KW", "kw"), item(61, "Hàn Quốc", "han-quoc"), item(62, "hàn quốc", "han-quoc-2"))
                .joinToString(",") + "]}\n"
        val (genres, countries) = NextSite.parseFilters(page("$base/loc-phim", data), data)
        assertEquals(listOf("Âm Nhạc", "Cổ Trang"), genres.map { it.title })
        assertEquals("$base/loc-phim?genre=co-trang&sort=updated_at&order=desc", genres[1].url)
        // Bỏ năm, mã hai chữ, tên trùng (khác hoa / thường)
        assertEquals(listOf("Anh", "Hàn Quốc"), countries.map { it.title })
        assertEquals("$base/loc-phim?country=han-quoc&sort=updated_at&order=desc", countries[1].url)
    }

    @Test
    fun notNextJs() {
        assertNull(NextSite.data(Jsoup.parse("<html><body><script>var a=1</script><div class=flw-item></div></body></html>", "$base/")))
    }

    @Test
    fun home() {
        val data = """0:["${'$'}","div",null,{"children":[""" + movie(1, "slide-a", "Phim Nổi Bật", "Vietsub 4") + "]}]\n" +
            """5:["${'$'}","${'$'}L26","Trung Quốc Hot Nhất",{"movies":[""" + movie(2, "tien-nghich", "Tiên Nghịch", "Thuyết Minh 107, Vietsub 165") + "," +
            movie(3, "phim-le", "Phim Lẻ", "Vietsub Full") + """]}]"""
        val doc = page("$base/", data)
        doc.body().append("""<a href="/the-loai/hanh-dong">Hành Động</a>""")
        val h = Site.parseHome(doc)
        assertEquals(listOf("Nổi bật", "Trung Quốc Hot Nhất"), h.sections.map { it.title })
        val f = h.sections[1].films
        assertEquals(listOf("Tiên Nghịch", "Phim Lẻ"), f.map { it.title })
        assertEquals("$base/phim/tien-nghich", f[0].url)
        assertEquals("$base/public/images/posters/tien-nghich.webp", f[0].poster)
        assertEquals(listOf("Tập 165", "Full"), f.map { it.badge })
        // Mục hoạt hình cố định đứng đầu menu, sau đó thể loại của trang
        assertEquals("Hoạt hình Trung Quốc", h.categories.first().title)
        assertEquals("$base/the-loai/hanh-dong", h.categories.last().url)
    }

    @Test
    fun listPaging() {
        val movies = (1..21).joinToString(",") { movie(it, "phim-$it", "Phim $it", "Vietsub $it") }
        val url = "$base/loc-phim?genre=hoat-hinh&sort=updated_at&order=desc"
        val p = Site.parseList(page(url, """{"movies":[$movies],"total":50,"page":1,"limit":21,"filters":{}}"""), url)
        assertEquals(21, p.films.size)
        assertEquals("$url&page=2", p.nextUrl)
        val last = Site.parseList(page("$url&page=3", """{"movies":[${movie(1, "a", "A", "")}],"total":43,"page":3,"limit":21}"""), "$url&page=3")
        assertNull(last.nextUrl)
    }

    @Test
    fun detail() {
        val data = """{"movie":{"id":283,"name":"Tiên Nghịch","origin_name":"Renegade Immortal","slug":"tien-nghich",""" +
            """"description":"Cuộc hành trình của Vương Lâm, một thiếu niên bình thường.","poster_url":"/public/images/posters/p.webp",""" +
            """"status_text":"Thuyết Minh 2, Vietsub 3","year":2023,"country":{"name":"Trung Quốc"},"genres":[{"name":"Hoạt Hình"}]},""" +
            """"episode":{"episodes":${eps(3, "vietsub")},"available_languages":[],"available_sources":$sources,"current_language":"vietsub","current_server":"kk"}}"""
        val d = Site.parseDetail(page("$base/phim/tien-nghich", data), "$base/phim/tien-nghich")
        assertEquals("Tiên Nghịch", d.title)
        assertEquals("Renegade Immortal", d.otherTitle)
        assertTrue(d.info!!.startsWith("Thuyết Minh 2, Vietsub 3  •  Trung Quốc  •  2023"))
        assertEquals(1, d.groups.size) // danh sách Thuyết Minh tải thêm trong NextSite.detail()
        assertEquals("Vietsub", d.groups[0].name)
        assertEquals(listOf("Tập 1", "Tập 2", "Tập 3"), d.groups[0].episodes.map { it.title })
        assertEquals("$base/xem-phim/tien-nghich/tap-3/vietsub", d.groups[0].episodes.last().url)
    }

    @Test
    fun source() {
        val epUrl = "$base/xem-phim/tien-nghich/tap-3/vietsub"
        val data = """{"episode":{"episodes":${eps(3, "vietsub")},"current_language":"vietsub","available_sources":$sources,""" +
            """"episode_sources":[{"server":{"id":3,"name":"KK Phim","slug":"kk"},"language":{"name":"Vietsub","slug":"vietsub"},"is_lock":false},""" +
            """{"server":{"id":2,"name":"NguonC","slug":"nc"},"language":{"name":"Vietsub","slug":"vietsub"},"is_lock":false},""" +
            """{"server":{"id":3,"name":"KK Phim","slug":"kk"},"language":{"name":"Thuyết Minh","slug":"thuyetminh"},"is_lock":false}],""" +
            """"current_server":{"id":3,"name":"KK Phim","slug":"kk"},"current_source":{"server":"${'$'}1f:props:data:episode:current_server"},""" +
            """"video":{"id":1,"link_embed":"https://player.test/?url=x","link_m3u8":"https://cdn.test/a/index.m3u8","is_embed":false}}}"""
        val s = Site.parseSource(page(epUrl, data), epUrl)
        assertTrue(s.next)
        assertTrue(!s.pageMode)
        assertEquals(listOf("kk" to "KK Phim", "nc" to "NguonC"), s.servers.map { it.type to it.label })
        assertEquals(mapOf("kk" to Video("https://cdn.test/a/index.m3u8")), s.links)
        assertEquals("tap-3", s.ep)
        // Server chỉ có trình phát nhúng: dùng link_embed
        val embed = Site.parseSource(page(epUrl, data.replace("https://cdn.test/a/index.m3u8", "")), epUrl)
        assertEquals("https://player.test/?url=x", embed.links["kk"]?.url)
    }

    @Test
    fun ownServerWithSubtitles() {
        // Server HD của trang (bản Song Ngữ): link cùng tên miền, phụ đề rời — chọn bản mặc định
        val epUrl = "$base/xem-phim/bac-si-x/tap-1/song-ngu"
        val data = """{"episode":{"episodes":${eps(1, "song-ngu")},"current_language":"song-ngu",""" +
            """"episode_sources":[{"server":{"id":9,"name":"HD","slug":"hd"},"language":{"name":"Song Ngữ","slug":"song-ngu"},"is_lock":false}],""" +
            """"current_server":{"id":9,"name":"HD","slug":"hd"},"video":{"id":1,"link_embed":"/embed/v2/abc","link_m3u8":"/public/m3u8/job_1.m3u8",""" +
            """"is_embed":false,"subtitles":[{"url":"https://sub.test/zh","lang":"chi","label":"Chinese","isDefault":false},""" +
            """{"url":"https://sub.test/vi","lang":"vie","label":"Vietnamese","isDefault":true}]}}}"""
        val s = Site.parseSource(page(epUrl, data), epUrl)
        assertEquals(Video("$base/public/m3u8/job_1.m3u8", "https://sub.test/vi"), s.links["hd"])
    }

    @Test
    fun oddPosterDoesNotBreakList() {
        // Ảnh bìa dạng lạ (data:…) chỉ làm mất ảnh của thẻ đó, không làm hỏng cả danh sách
        val odd = movie(1, "phim-a", "Phim A", "Vietsub 3").replace("/public/images/posters/phim-a.webp", "data:image/png;base64,AAAA")
        val url = "$base/loc-phim?type=phim-bo"
        val p = Site.parseList(page(url, """{"movies":[$odd,${movie(2, "phim-b", "Phim B", "Full")}],"total":2,"page":1,"limit":21}"""), url)
        assertEquals(listOf("Phim A", "Phim B"), p.films.map { it.title })
        assertNull(p.films[0].poster)
        assertEquals("$base/public/images/posters/phim-b.webp", p.films[1].poster)
    }

    @Test
    fun badge() {
        assertEquals("Tập 8", NextSite.badge("Song Ngữ 4, Thuyết Minh 8, Vietsub 8"))
        assertEquals("Full", NextSite.badge("Thuyết Minh Full, Vietsub Full"))
        assertNull(NextSite.badge(""))
    }
}
