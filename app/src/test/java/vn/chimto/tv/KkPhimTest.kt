package vn.chimto.tv

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Server dự phòng KKPhim (11/2026): ghép phim của yanhh3d / HaLim với phimapi.com, ghép tập theo số, chọn ngôn ngữ. */
class KkPhimTest {
    private fun movie(name: String, origin: String = "", type: String = "hoathinh", slug: String = "x") =
        JSONObject().put("name", name).put("origin_name", origin).put("type", type).put("slug", slug)

    @Test
    fun normalizesSeasonsAndAccents() {
        assertEquals(KkPhim.norm("Đấu Phá Thương Khung Phần 5"), KkPhim.norm("Đấu Phá Thương Khung (Phần 5)"))
        assertEquals("dau pha thuong khung 5", KkPhim.norm("Đấu Phá Thương Khung (Phần 5)"))
        assertEquals(KkPhim.norm("Sword of Coming SS2"), KkPhim.norm("Sword of Coming (Season 2)"))
        assertEquals(KkPhim.norm("Fights Break Sphere 5"), KkPhim.norm("Fights Break Sphere (Season 5)"))
        assertEquals(KkPhim.norm("Đấu La Đại Lục 2: Tuyệt Thế Đường Môn"), KkPhim.norm("Đấu La Đại Lục 2 (Tuyệt Thế Đường Môn)"))
        assertEquals(KkPhim.norm("Spare Me, Great Lord! Season 3"), KkPhim.norm("Spare Me, Great Lord! (Season 3)"))
    }

    @Test
    fun matchesByNameOrOriginalName() {
        val mine = setOf(KkPhim.norm("Già Thiên"), KkPhim.norm("Zhe Tian"))
        assertTrue(KkPhim.matches(movie("Già Thiên", "Shrouding The Heavens"), mine))
        assertTrue(KkPhim.matches(movie("Che Trời", "Zhe Tian"), mine))
        assertFalse(KkPhim.matches(movie("Già Thiên Movie", "Shrouding The Heavens Movie"), mine))
        // Tên khác trong ngoặc
        val doctor = movie("Thần Y Cổ Đại Ở Đô Thị (Đô Thị Cổ Tiên Y)", "Urban Miracle Doctor")
        assertTrue(KkPhim.matches(doctor, setOf(KkPhim.norm("Thần Y Cổ Đại Ở Đô Thị"))))
        assertTrue(KkPhim.matches(doctor, setOf(KkPhim.norm("Đô Thị Cổ Tiên Y"))))
        // Trong ngoặc chỉ là số phần: phần 5 không được ghép với phim "Đấu Phá Thương Khung" (phần 1)
        val s5 = movie("Đấu Phá Thương Khung (Phần 5)", "Fights Break Sphere (Season 5)")
        assertFalse(KkPhim.matches(s5, setOf(KkPhim.norm("Đấu Phá Thương Khung"))))
        assertTrue(KkPhim.matches(s5, setOf(KkPhim.norm("Đấu Phá Thương Khung Phần 5"))))
    }

    @Test
    fun prefersAnimationAmongSameNameResults() {
        val mine = setOf(KkPhim.norm("Già Thiên"))
        val live = movie("Già Thiên", type = "series", slug = "gia-thien-live")
        val anim = movie("Già Thiên", type = "hoathinh", slug = "gia-thien")
        assertEquals("gia-thien", KkPhim.choose(listOf(live, anim), mine)?.optString("slug"))
        assertEquals("gia-thien-live", KkPhim.choose(listOf(live), mine)?.optString("slug"))
        assertNull(KkPhim.choose(listOf(movie("Tiên Nghịch")), mine))
    }

    @Test
    fun readsSlugFromFilmUrl() {
        assertEquals("tien-nghich", KkPhim.slugOf("https://yanhh3d.ee/tu-tien/tien-nghich"))
        assertEquals("dau-pha-thuong-khung-phan-5", KkPhim.slugOf("https://hoathinh3d.ee/dau-pha-thuong-khung-phan-5/"))
        assertNull(KkPhim.slugOf("https://example.com/"))
    }

    private val detail = JSONObject(
        """{"status":true,"movie":{"name":"Tiên Nghịch","slug":"tien-nghich","type":"hoathinh"},"episodes":[
        {"server_name":"Vietsub #Hà Nội","is_ai":false,"server_data":[
          {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://a.kvp726.com/164/index.m3u8"},
          {"name":"Tập 165","slug":"tap-165","link_m3u8":"https://a.kvp726.com/165/index.m3u8"}]},
        {"server_name":"Thuyết Minh","is_ai":false,"server_data":[
          {"name":"Tập 08","slug":"tap-08","link_m3u8":"https://s6.kkphimplayer6.com/8/index.m3u8"},
          {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://s6.kkphimplayer6.com/164/index.m3u8"}]},
        {"server_name":"Thuyết Minh","is_ai":true,"server_data":[
          {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://ai.example/164/index.m3u8"}]},
        {"server_name":"Lồng Tiếng","is_ai":false,"server_data":[
          {"name":"Tập 164","slug":"tap-164","link_m3u8":"","link_embed":"https://player.phimapi.com/player/?url=x"}]}
        ]}""",
    )

    @Test
    fun picksSameEpisodeNumberInEveryLanguage() {
        val all = KkPhim.pick(detail, 164.0, null)
        assertEquals(listOf("kk-vs", "kk-tm", "kk-tm-ai"), all.map { it.first.type })
        assertEquals(listOf("KKPhim Vietsub", "KKPhim Thuyết Minh", "KKPhim Thuyết Minh (AI)"), all.map { it.first.label })
        assertEquals("https://a.kvp726.com/164/index.m3u8", all[0].second.url)
        // "Tập 08" = tập 8; tập chỉ có ở Vietsub; bản Lồng Tiếng không có link HLS thì bỏ
        assertEquals(listOf("kk-tm"), KkPhim.pick(detail, 8.0, null).map { it.first.type })
        assertEquals(listOf("kk-vs"), KkPhim.pick(detail, 165.0, null).map { it.first.type })
        assertTrue(KkPhim.pick(detail, 166.0, null).isEmpty())
    }

    @Test
    fun onlyLanguageOfTheVersionBeingWatched() {
        // hoathinh3d.ee / hhkungfu chia bản "Việt Sub" / "Thuyết Minh" (tên server của trang không ghi ngôn ngữ)
        assertEquals(listOf("kk-vs"), KkPhim.pick(detail, 164.0, PlayStats.lang("Việt Sub")).map { it.first.type })
        assertEquals(listOf("kk-tm", "kk-tm-ai"), KkPhim.pick(detail, 164.0, PlayStats.lang("Thuyết Minh")).map { it.first.type })
        assertEquals("vs", PlayStats.lang("4K Việt Sub"))
        assertNull(PlayStats.lang("Tập phim"))
    }

    @Test
    fun attachesAfterSiteServersAndPlaysFromLinks() {
        val kk = KkPhim.pick(detail, 164.0, null)
        val yan = EpisodeSource(
            "https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html", listOf(Server("VIETSUB 1080 V1", "tiktik")), "1010", "tap-164",
            "1", "https://yanhh3d.ee/player/player.php", null, false,
        )
        val s = KkPhim.attach(yan, kk)
        assertEquals(listOf("tiktik", "kk-vs", "kk-tm", "kk-tm-ai"), s.servers.map { it.type })
        assertFalse(s.pageOnly)
        assertEquals("https://s6.kkphimplayer6.com/164/index.m3u8", Site.video(s, s.servers[2]).url)
        assertEquals(yan, KkPhim.attach(yan, emptyList()))

        // Trang mở thẳng trang tập (hoathinh3d.pw): trình phát của trang thành server đầu tiên
        val pw = EpisodeSource("https://hoathinh3d.pw/xem-phim-gia-thien/tap-2-sv1.html", emptyList(), "", "", "", null, null, true)
        assertTrue(pw.pageOnly)
        val p = KkPhim.attach(pw, kk)
        assertEquals(PAGE_SERVER, p.servers.first().type)
        assertFalse(p.pageOnly)
        assertTrue(p.pageMode)
    }

    @Test
    fun siteDownStillPlaysFromKkAndSlowKkIsSkipped() {
        val ep = "https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html"
        val kk = KkPhim.pick(detail, 164.0, null)
        val site = EpisodeSource(ep, listOf(Server("VIETSUB 1080 V1", "tiktik")), "1", "tap-164", "1", "https://yanhh3d.ee/player/player.php", null, false)
        // Trang lỗi (vd tên miền chết / nhà mạng chặn) → chỉ còn server KKPhim, phát được
        val only = KkPhim.combine(ep, { throw java.io.IOException("Trang trả về lỗi HTTP 522") }, { kk })
        assertEquals(listOf("kk-vs", "kk-tm", "kk-tm-ai"), only.servers.map { it.type })
        assertEquals("https://a.kvp726.com/164/index.m3u8", Site.video(only, only.servers[0]).url)
        // Trang lỗi và KKPhim cũng không có → báo lỗi của trang
        val err = runCatching { KkPhim.combine(ep, { throw java.io.IOException("HTTP 522") }, { emptyList() }) }.exceptionOrNull()
        assertEquals("HTTP 522", err?.message)
        // KKPhim lỗi / quá chậm → phát bằng server của trang như cũ, không chờ mãi
        assertEquals(site, KkPhim.combine(ep, { site }, { throw java.io.IOException("KK lỗi") }))
        val t0 = System.currentTimeMillis()
        val slow = KkPhim.combine(ep, { site }, { Thread.sleep(3_000); kk }, waitMs = 300)
        assertEquals(site, slow)
        assertTrue(System.currentTimeMillis() - t0 < 2_000)
        assertEquals(4, KkPhim.combine(ep, { site }, { kk }).servers.size)
    }

    @Test
    fun slowBackupDoesNotHideTheFastOne() {
        // KKPhim chậm quá thời gian chờ nhưng NguonC đã xong → vẫn có server NguonC; trang lỗi → chỉ còn NguonC
        val ep = "https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html"
        val site = EpisodeSource(ep, listOf(Server("VIETSUB 1080 V1", "tiktik")), "1", "tap-164", "1", "https://yanhh3d.ee/player/player.php", null, false)
        val kk = KkPhim.pick(detail, 164.0, null)
        val nc = listOf(Server("NguonC Vietsub", "nc-vs") to Video("https://embed1.streamc.xyz/embed.php?hash=x"))
        val t0 = System.currentTimeMillis()
        val s = KkPhim.combineAll(ep, { site }, listOf({ Thread.sleep(3_000); kk }, { nc }), waitMs = 400)
        assertEquals(listOf("tiktik", "nc-vs"), s.servers.map { it.type })
        assertTrue(System.currentTimeMillis() - t0 < 2_000)
        val both = KkPhim.combineAll(ep, { site }, listOf({ kk }, { nc }))
        assertEquals(listOf("tiktik", "kk-vs", "kk-tm", "kk-tm-ai", "nc-vs"), both.servers.map { it.type })
        val down = KkPhim.combineAll(ep, { throw java.io.IOException("HTTP 522") }, listOf({ throw java.io.IOException("KK lỗi") }, { nc }))
        assertEquals(listOf("nc-vs"), down.servers.map { it.type })
        assertEquals("https://embed1.streamc.xyz/embed.php?hash=x", Site.video(down, down.servers[0]).url)
    }

    @Test
    fun suggestsKkWhenEveryMirrorStalls() {
        // Server của yanhh3d (streamfree) đều giật / lỗi → gợi ý KKPhim cùng ngôn ngữ, không sang Thuyết Minh
        val v1 = Server("VIETSUB 1080 V1", "tiktik")
        val v2 = Server("VIETSUB 1080 V2", "pro")
        val tm = Server("Thuyết minh 1080 V1", "tiktm")
        val kk = KkPhim.pick(detail, 164.0, null).map { it.first }
        val bad = PlayStats.Score(2, 20, 0.12)
        val unknown = PlayStats.Score(-1, null, 0.0)
        val all = listOf(v1, v2, tm) + kk
        val alt = PlayStats.alternative(all, v2) { if (it == v1) bad else unknown }
        assertEquals("kk-vs", alt?.first?.type)
        // Còn server cùng trang chưa thử thì thử nó trước
        assertEquals(v2, PlayStats.alternative(all, v1) { unknown }?.first)
    }

    @Test
    fun bareKkDomainBecomesTheWwwSite() {
        // kkphim.com trần bị nhà mạng chặn, và trùng với trang mẫu www.kkphim.com trong danh sách
        assertEquals(Prefs.KK_SITE, Prefs.normalizeUrl("kkphim.com"))
        assertEquals(Prefs.KK_SITE, Prefs.normalizeUrl("https://KKPhim.com/"))
        assertEquals(Prefs.KK_SITE, Prefs.normalizeUrl("www.kkphim.com"))
        assertEquals("https://yanhh3d.ee", Prefs.normalizeUrl("yanhh3d.ee/"))
    }
}
