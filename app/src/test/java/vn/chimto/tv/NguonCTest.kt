package vn.chimto.tv

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** NguonC (11/2026): API phim.nguonc.com/api — trang riêng + server dự phòng thứ hai cạnh KKPhim. */
class NguonCTest {
    private val o = "https://phim.nguonc.com"

    private val detail = JSONObject(
        """{"status":"success","movie":{"name":"Mục Thần Ký","original_name":"Mu Shen Ji, Tales of Herding Gods",
        "slug":"muc-than-ky","description":"<p>Tần Mục lớn lên ở <b>Tàn Lão Thôn</b>.</p>","total_episodes":120,
        "current_episode":"Tập 108","time":"21 phút/tập","quality":"HD","language":"Vietsub","year":2023,
        "thumb_url":"https://img.nguonc.com/images/a.jpg","poster_url":"https://img.nguonc.com/images/b.jpg",
        "category":{"1":{"group":{"name":"Định dạng"},"list":[{"name":"Phim bộ"}]},
                    "2":{"group":{"name":"Thể loại"},"list":[{"name":"Hành động"},{"name":"Hoạt hình"}]},
                    "4":{"group":{"name":"Quốc gia"},"list":[{"name":"Trung Quốc"}]}},
        "episodes":[
          {"server_name":"Vietsub #1","items":[
            {"name":"1","slug":"tap-1","embed":"https://embed1.streamc.xyz/embed.php?hash=a1"},
            {"name":"107","slug":"tap-107","embed":"https://embed14.streamc.xyz/embed.php?hash=a107"},
            {"name":"108","slug":"tap-108","embed":"https://embed10.streamc.xyz/embed.php?hash=a108"},
            {"name":"109","slug":"tap-109","embed":""}]},
          {"server_name":"Thuyết Minh #1","items":[
            {"name":"108","slug":"tap-108","embed":"https://embed10.streamc.xyz/embed.php?hash=t108"}]}]}}""",
    )

    @Test
    fun parsesListWithPortraitPostersAndPaging() {
        val json = """{"status":"success","paginate":{"current_page":2,"total_page":3365,"total_items":33647,"items_per_page":10},
            "items":[
             {"name":"Mục Thần Ký","slug":"muc-than-ky","current_episode":"Tập 108","language":"Vietsub",
              "thumb_url":"https://img.nguonc.com/images/thumb.jpg","poster_url":"https://img.nguonc.com/images/wide.jpg"},
             {"name":"Doraemon","slug":"doraemon","current_episode":"Hoàn tất (26/26)","poster_url":"https://img.nguonc.com/images/d.jpg"},
             {"name":"","slug":"rong"}]}"""
        val url = "$o/api/films/danh-sach/hoat-hinh?page=2"
        val p = NguonC.parseList(json, url)
        assertEquals(listOf("Mục Thần Ký", "Doraemon"), p.films.map { it.title })
        assertEquals("$o/phim/muc-than-ky", p.films[0].url)
        assertEquals("https://img.nguonc.com/images/thumb.jpg", p.films[0].poster) // thumb_url là ảnh dọc
        assertEquals("https://img.nguonc.com/images/d.jpg", p.films[1].poster)
        assertEquals(listOf("Tập 108", "Tập 26"), p.films.map { it.badge })
        assertEquals("$o/api/films/danh-sach/hoat-hinh?page=3", p.nextUrl)
        assertNull(NguonC.parseList(json.replace("\"current_page\":2", "\"current_page\":3365"), url).nextUrl)
    }

    @Test
    fun parsesDetailGroupsAndInfo() {
        val d = NguonC.parseDetail(detail.toString(), "$o/phim/muc-than-ky")
        assertEquals("Mục Thần Ký", d.title)
        assertEquals("Mu Shen Ji, Tales of Herding Gods", d.otherTitle)
        assertEquals("Tần Mục lớn lên ở Tàn Lão Thôn.", d.description)
        assertEquals("Tập 108  •  21 phút/tập  •  Vietsub  •  Trung Quốc  •  2023  •  Hành động, Hoạt hình", d.info)
        assertEquals(listOf("Vietsub #1", "Thuyết Minh #1"), d.groups.map { it.name })
        assertEquals(listOf("Tập 1", "Tập 107", "Tập 108"), d.groups[0].episodes.map { it.title }) // tập 109 chưa có video
        assertEquals("$o/phim/muc-than-ky/tap-108?sv=0", d.groups[0].episodes[2].url)
        assertEquals("$o/phim/muc-than-ky/tap-108?sv=1", d.groups[1].episodes[0].url)
        assertEquals("$o/phim/muc-than-ky", NguonC.filmUrl("$o/phim/muc-than-ky/tap-108?sv=1"))
    }

    @Test
    fun backupPicksSameEpisodeInLanguageBeingWatched() {
        val m = detail.getJSONObject("movie")
        val all = NguonC.pick(m, 108.0, null)
        assertEquals(listOf("nc-vs", "nc-tm"), all.map { it.first.type })
        assertEquals(listOf("NguonC Vietsub", "NguonC Thuyết Minh"), all.map { it.first.label })
        assertEquals("https://embed10.streamc.xyz/embed.php?hash=a108", all[0].second.url)
        assertEquals(listOf("nc-tm"), NguonC.pick(m, 108.0, "tm").map { it.first.type })
        assertTrue(NguonC.pick(m, 109.0, null).isEmpty()) // tập chưa có video
        assertTrue(NguonC.pick(m, 200.0, null).isEmpty())
    }

    @Test
    fun matchesEveryCommaSeparatedOriginalName() {
        val m = detail.getJSONObject("movie")
        assertTrue(KkPhim.matches(m, setOf(KkPhim.norm("Tales of Herding Gods"))))
        assertTrue(KkPhim.matches(m, setOf(KkPhim.norm("Mục Thần Ký"))))
        assertTrue(!KkPhim.matches(m, setOf(KkPhim.norm("Tiên Nghịch"))))
    }

    @Test
    fun backupServerPlaysFromLinks() {
        val ep = "https://yanhh3d.ee/tu-tien/muc-than-ky/tap-108.html"
        val site = EpisodeSource(ep, listOf(Server("VIETSUB 1080 V1", "tiktik")), "1", "tap-108", "1", "https://yanhh3d.ee/player/player.php", null, false)
        val nc = NguonC.pick(detail.getJSONObject("movie"), 108.0, null)
        val s = KkPhim.attach(site, nc)
        assertEquals(listOf("tiktik", "nc-vs", "nc-tm"), s.servers.map { it.type })
        assertEquals("https://embed10.streamc.xyz/embed.php?hash=t108", Site.video(s, s.servers[2]).url)
    }
}
