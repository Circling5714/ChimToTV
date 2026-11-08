package vn.chimto.tv

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Trang chủ tổng hợp (2.8): đọc danh sách các nguồn, gộp phim trùng, sở thích, dựng hàng, tự báo tập mới. */
class FeedTest {
    private val now = Feed.time("2026-11-08T20:00:00+07:00")
    private val hour = 3_600_000L
    private val day = 24 * hour

    private val kkBody = """{"status":"success","data":{"APP_DOMAIN_CDN_IMAGE":"https://phimimg.com","items":[
        {"name":"Mục Thần Ký","slug":"muc-than-ky","origin_name":"Tales Of Herding Gods","type":"hoathinh",
         "poster_url":"upload/vod/a.jpg","thumb_url":"upload/vod/b.jpg","episode_current":"Tập 107","year":2024,
         "modified":{"time":"2026-11-08T11:55:29.000Z"},
         "category":[{"name":"Phiêu Lưu","slug":"phieu-luu"},{"name":"Viễn Tưởng","slug":"vien-tuong"}],
         "country":[{"name":"Trung Quốc","slug":"trung-quoc"}]},
        {"name":"Đấu Phá Thương Khung (Phần 5)","slug":"dau-pha-thuong-khung-phan-5","origin_name":"Fights Break Sphere (Season 5)",
         "type":"hoathinh","poster_url":"https://img.example/dp5.jpg","episode_current":"Tập 150","year":2022,
         "modified":{"time":"2026-11-08T10:00:00.000Z"},"category":[{"name":"Hành Động","slug":"hanh-dong"}],
         "country":[{"name":"Trung Quốc","slug":"trung-quoc"}]},
        {"name":"Thôn Phệ Tinh Không","slug":"thon-phe-tinh-khong","type":"hoathinh","episode_current":"Tập 248","year":2020,
         "modified":{"time":"2026-11-08T09:00:00.000Z"}},
        {"name":"Thiên Long Bát Bộ","slug":"thien-long-bat-bo-2021","type":"series","episode_current":"Hoàn Tất (50/50)","year":2021,
         "modified":{"time":"2026-11-01T09:00:00.000Z"}}
    ]}}"""

    private val ncBody = """{"status":"success","items":[
        {"name":"Mục Thần Ký","slug":"muc-than-ky","original_name":"Mu Shen Ji, Tales of Herding Gods","year":"2024",
         "current_episode":"Tập 108","thumb_url":"https://phim.nguonc.com/t.jpg","modified":"2026-11-08T12:30:00.000000Z"},
        {"name":"Đấu Phá Thương Khung 5","slug":"dau-pha-5","original_name":"","year":"2022","current_episode":"Tập 150",
         "modified":"2026-11-08T08:00:00.000000Z"},
        {"name":"Thôn Phệ Tinh Không 2","slug":"thon-phe-2","year":"2024","current_episode":"Tập 222",
         "modified":"2026-11-08T12:00:00.000000Z"}
    ]}"""

    private val kk = "https://www.kkphim.com"
    private val nc = "https://phim.nguonc.com"
    private fun kkItems() = Feed.parseKk(kkBody, kk)
    private fun ncItems() = Feed.parseNc(ncBody, nc)
    private fun web(vararg titles: Pair<String, String?>) =
        Feed.fromFilms(titles.map { (t, b) -> Film("https://yanhh3d.ee/" + KkPhim.norm(t).replace(' ', '-'), t, null, b) })

    @Test
    fun readsEpisodeNumbers() {
        assertEquals(108.0, Feed.epOf("Tập 108")!!, 0.0)
        assertEquals(16.0, Feed.epOf("Hoàn Tất (16/16)")!!, 0.0)
        assertEquals(108.0, Feed.epOf("108/120")!!, 0.0)
        assertEquals(7.0, Feed.epOf("7/??")!!, 0.0)
        assertEquals(12.0, Feed.epOf("12")!!, 0.0)
        assertNull(Feed.epOf("Full"))
        assertNull(Feed.epOf("Top 1"))
        assertNull(Feed.epOf("HD"))
        assertNull(Feed.epOf(null))
    }

    @Test
    fun readsTimesWithZones() {
        val utc = Feed.time("2026-11-08T14:12:36Z")
        assertEquals(utc, Feed.time("2026-11-08T21:12:36+07:00"))
        assertEquals(utc, Feed.time("2026-11-08T14:12:36.000000Z"))
        assertEquals(utc, Feed.time("2026-11-08 21:12:36")) // không ghi múi giờ → giờ Việt Nam
        assertEquals(0L, Feed.time("không phải giờ"))
        assertEquals(0L, Feed.time(null))
    }

    @Test
    fun parsesKkPhimList() {
        val items = kkItems()
        assertEquals(4, items.size)
        val m = items[0]
        assertEquals("$kk/phim/muc-than-ky", m.url)
        assertEquals("https://phimimg.com/upload/vod/a.jpg", m.poster)
        assertEquals(107.0, m.ep!!, 0.0)
        assertEquals(2024, m.year)
        assertEquals(Feed.time("2026-11-08T11:55:29Z"), m.updated)
        assertEquals(listOf("phieu-luu", "vien-tuong"), m.tags!!.genres)
        assertEquals(listOf("trung-quoc"), m.tags!!.countries)
        assertEquals("hoathinh", m.tags!!.type)
        assertTrue(KkPhim.norm("Tales Of Herding Gods") in m.names)
        assertEquals(50.0, items[3].ep!!, 0.0)
    }

    @Test
    fun parsesNguonCList() {
        val items = ncItems()
        assertEquals("$nc/phim/muc-than-ky", items[0].url)
        assertEquals(108.0, items[0].ep!!, 0.0)
        assertEquals(2024, items[0].year)
        assertTrue(KkPhim.norm("Mu Shen Ji") in items[0].names)
        assertNull(items[0].tags)
        assertEquals(Feed.time("2026-11-08T12:30:00Z"), items[0].updated)
    }

    @Test
    fun feedItemRoundTripsThroughCacheJson() {
        val a = kkItems()[0]
        assertEquals(a, FeedItem.of(a.json()))
        val b = web("Tiên Nghịch" to "Tập 168")[0]
        assertEquals(b, FeedItem.of(b.json()))
    }

    @Test
    fun mergesSameFilmAcrossSources() {
        val y = web("Đấu Phá Thương Khung Phần 5" to "Tập 150", "Mục Thần Ký" to "Tập 107", "Thôn Phệ Tinh Không" to "Tập 248")
        val films = Feed.merge(listOf(y, kkItems(), ncItems()), now)
        fun find(n: String) = films.filter { KkPhim.norm(n) in it.names }
        // Ba tên khác nhau của Đấu Phá Thương Khung phần 5 → một phim, ba nguồn
        assertEquals(1, find("Đấu Phá Thương Khung Phần 5").size)
        assertEquals(3, find("Đấu Phá Thương Khung Phần 5")[0].items.size)
        // Mục Thần Ký: tập mới nhất ở NguonC
        val m = find("Mục Thần Ký").single()
        assertEquals(3, m.items.size)
        assertEquals("phim.nguonc.com", m.latest!!.host)
        // Thôn Phệ Tinh Không (KKPhim + yanhh3d) và Thôn Phệ Tinh Không 2 (NguonC) là hai phim
        assertEquals(1, find("Thôn Phệ Tinh Không").size)
        assertEquals(1, find("Thôn Phệ Tinh Không 2").size)
        // Mới cập nhật trước
        assertEquals(films.sortedByDescending { it.updated }, films)
    }

    @Test
    fun doesNotMergeRemakesOrTwoItemsOfOneSite() {
        val old = Feed.parseKk(
            """{"items":[{"name":"Thiên Long Bát Bộ","slug":"tlbb-1997","year":1997,"modified":{"time":"2026-11-01T00:00:00Z"}}]}""",
            "https://vsmov.com",
        )
        val films = Feed.merge(listOf(kkItems(), old), now)
        assertEquals(2, films.count { KkPhim.norm("Thiên Long Bát Bộ") in it.names })
        // Một trang có hai mục cùng tên (hai phần khác nhau) → không gộp
        val two = Feed.fromFilms(listOf(Film("https://yanhh3d.ee/a", "Tiên Nghịch", null, null), Film("https://yanhh3d.ee/b", "Tiên Nghịch", null, null)))
        assertEquals(2, Feed.merge(listOf(two), now).size)
    }

    @Test
    fun sameFilmInTwoListsOfOneSourceKeepsItsOtherSources() {
        // «Mục Thần Ký» của KKPhim có trong danh sách mới cập nhật và danh sách lọc theo thể loại: vẫn một phim, đủ 3 nguồn
        val y = web("Mục Thần Ký" to "Tập 107")
        val films = Feed.merge(listOf(y, kkItems(), ncItems(), kkItems().take(1)), now)
        val m = films.filter { KkPhim.norm("Mục Thần Ký") in it.names }
        assertEquals(1, m.size)
        assertEquals(3, m[0].items.size)
    }

    @Test
    fun pseudoTimesKeepPageOrderForSitesWithoutTimes() {
        val y = web("A" to null, "B" to null, "C" to null)
        val films = Feed.merge(listOf(y), now)
        assertEquals(listOf("A", "B", "C"), films.map { it.items[0].title })
        assertTrue(films.all { it.updated in (now - 3 * hour)..now })
    }

    @Test
    fun ordersSourcesByPreference() {
        val f = Feed.merge(listOf(kkItems(), ncItems(), web("Mục Thần Ký" to "Tập 107")), now).first { KkPhim.norm("Mục Thần Ký") in it.names }
        assertEquals(listOf("yanhh3d.ee", "kkphim.com", "phim.nguonc.com"), f.ordered(listOf("yanhh3d.ee", "kkphim.com")).items.map { it.host })
    }

    @Test
    fun tasteFavoursRecentAndMuchWatchedFilms() {
        val tags = mapOf(
            "Tiên Nghịch" to Tags(listOf("vo-thuat", "hoat-hinh", "chinh-kich"), listOf("trung-quoc"), "hoathinh"),
            "Hạ Cánh Nơi Anh" to Tags(listOf("tinh-cam"), listOf("han-quoc"), "series"),
        )
        val history = listOf(
            HistoryItem("https://yanhh3d.ee/tien-nghich", "Tiên Nghịch", null, "Tập 166", "e1", now - day),
            HistoryItem("https://www.kkphim.com/phim/ha-canh", "Hạ Cánh Nơi Anh", null, "Tập 3", "e2", now - 60 * day),
        )
        val t = HomeFeed.taste(history, { tags[it] }, { if (it.contains("tien")) 20 else 3 }, now)
        assertTrue(t.genre.getValue("vo-thuat") > 15)
        assertNull(t.genre["hoat-hinh"]) // quá chung
        assertEquals(t.genre.getValue("vo-thuat") / 2, t.genre.getValue("chinh-kich"), 1e-9)
        assertTrue(t.genre.getValue("vo-thuat") > 50 * t.genre.getValue("tinh-cam"))
        val top = t.combo.maxByOrNull { it.value }!!.key
        assertEquals(HomeFeed.Combo("hoathinh", "trung-quoc", "vo-thuat"), top)
        assertTrue(t.affinity(tags["Tiên Nghịch"]) > t.affinity(tags["Hạ Cánh Nơi Anh"]))
    }

    private fun part(role: HomeFeed.Role, key: String, host: String, items: List<FeedItem>, title: String? = null) =
        HomeFeed.Part(role, Feed.Req(key, host) { Feed.Loaded(items) }, title) to
            Feed.Result(Feed.Req(key, host) { Feed.Loaded(items) }, Feed.State.OK, items, null, null, false, 0, now)

    private fun plan(parts: List<Pair<HomeFeed.Part, Feed.Result>>) = HomeFeed.Plan(
        parts.map { it.first }, HomeFeed.Sources(kk, nc, null, null, listOf("https://yanhh3d.ee")), null, null, null, emptySet(),
    )

    private fun ctx(history: List<HistoryItem>, seen: Map<String, Double> = emptyMap(), follows: Set<String> = emptySet()) = HomeFeed.Ctx(
        "Bố", history, follows, { seen[it] }, "yanhh3d.ee", listOf("https://yanhh3d.ee", kk, nc), now,
    )

    @Test
    fun tellsNewEpisodesOfFilmsBeingWatchedOnAnySource() {
        val y = web("Mục Thần Ký" to "Tập 107", "Tiên Nghịch" to "Tập 168", "Thiên Long Bát Bộ" to "Tập 45")
        val parts = listOf(
            part(HomeFeed.Role.WEB, "web|y", "yanhh3d.ee", y),
            part(HomeFeed.Role.NEW, "new|kk", "kkphim.com", kkItems()),
            part(HomeFeed.Role.NEW, "new|nc", "phim.nguonc.com", ncItems()),
        )
        val history = listOf(
            HistoryItem("https://yanhh3d.ee/muc-than-ky", "Mục Thần Ký", null, "Tập 107", "e1", now - day),
            HistoryItem("https://yanhh3d.ee/tien-nghich", "Tiên Nghịch", null, "Tập 166", "e2", now - 2 * day),
            // Phim cũ đang xem dở (tập 30 / 45): không phải «tập mới»
            HistoryItem("https://yanhh3d.ee/thien-long-bat-bo", "Thiên Long Bát Bộ", null, "Tập 30", "e3", now - day),
        )
        val built = HomeFeed.buildRows(plan(parts), parts, ctx(history, seen = mapOf(KkPhim.norm("Thiên Long Bát Bộ") to 45.0)))
        val fresh = built.fresh.associateBy { it.card.film.title }
        assertEquals(setOf("Mục Thần Ký", "Tiên Nghịch"), fresh.keys)
        // Tập 108 chỉ có ở NguonC → mở NguonC
        assertEquals("Tập 108 mới", fresh.getValue("Mục Thần Ký").card.film.badge)
        assertEquals("có ở nguonc", fresh.getValue("Mục Thần Ký").card.chip)
        assertEquals("$nc/phim/muc-than-ky", fresh.getValue("Mục Thần Ký").card.film.url)
        // Trang đang xem có 2 tập mới → mở đúng trang đó
        assertEquals("+2 · Tập 168", fresh.getValue("Tiên Nghịch").card.film.badge)
        assertEquals("https://yanhh3d.ee/tien-nghich", fresh.getValue("Tiên Nghịch").card.film.url)

        // Xem từ trước 2.8, đang ở tập 31 mà phim đã tới 108: đang xem dở, không phải «tập mới»
        val behind = listOf(HistoryItem("https://yanhh3d.ee/muc-than-ky", "Mục Thần Ký", null, "Tập 31", "e1", now - 3 * day))
        assertTrue(HomeFeed.buildRows(plan(parts), parts, ctx(behind)).fresh.isEmpty())

        // Đã mở trang phim thấy tập 108 (seen) → thôi báo
        val again = HomeFeed.buildRows(plan(parts), parts, ctx(history, seen = mapOf(KkPhim.norm("Mục Thần Ký") to 108.0)))
        assertFalse(again.fresh.any { it.card.film.title == "Mục Thần Ký" })
        // Phim đang theo dõi: để phần «theo dõi» báo, không báo hai lần
        val followed = HomeFeed.buildRows(plan(parts), parts, ctx(history, follows = setOf("https://yanhh3d.ee/tien-nghich")))
        assertFalse(followed.fresh.any { it.card.film.title == "Tiên Nghịch" })
    }

    @Test
    fun newestRowMergesSourcesAndSkipsWatchedFilms() {
        val y = web("Đấu Phá Thương Khung Phần 5" to "Tập 150", "Mục Thần Ký" to "Tập 107")
        val parts = listOf(
            part(HomeFeed.Role.WEB, "web|y", "yanhh3d.ee", y),
            part(HomeFeed.Role.NEW, "new|kk", "kkphim.com", kkItems()),
            part(HomeFeed.Role.NEW, "new|nc", "phim.nguonc.com", ncItems()),
        )
        val history = listOf(HistoryItem("https://yanhh3d.ee/muc-than-ky", "Mục Thần Ký", null, "Tập 107", "e1", now - day))
        val rows = HomeFeed.buildRows(plan(parts), parts, ctx(history)).rows
        val newest = rows.single { it.id == "newest" }
        assertFalse(newest.cards.any { it.film.title == "Mục Thần Ký" })
        val dp = newest.cards.single { KkPhim.norm(it.film.title) == KkPhim.norm("Đấu Phá Thương Khung Phần 5") }
        assertEquals("3 nguồn", dp.chip)
        // Trang đang dùng mở trước
        assertEquals("https://yanhh3d.ee/dau-pha-thuong-khung-5", dp.film.url)
        assertEquals(3, dp.links.size)
        assertEquals("Tập 150", dp.film.badge)
    }

    @Test
    fun brokenSourceShowsSavedCopyLabelOrFallsBackToOtherSources() {
        val y = web("Đấu Phá Thương Khung Phần 5" to "Tập 150")
        val ok = part(HomeFeed.Role.WEB, "web|y", "yanhh3d.ee", y)
        val nc0 = part(HomeFeed.Role.NEW, "new|nc", "phim.nguonc.com", ncItems())
        // KKPhim lỗi, dùng bản lưu
        val kkCached = HomeFeed.Part(HomeFeed.Role.NEW, Feed.Req("new|kk", "kkphim.com") { Feed.Loaded(emptyList()) }) to
            Feed.Result(Feed.Req("new|kk", "kkphim.com") { Feed.Loaded(emptyList()) }, Feed.State.CACHED, kkItems(), null, "lỗi 502", false, 500, now - hour)
        val parts = listOf(ok, kkCached, nc0)
        val newest = HomeFeed.buildRows(plan(parts), parts, ctx(emptyList())).rows.single { it.id == "newest" }
        // Phim chỉ có ở KKPhim (bản lưu): ghi «bản lưu»; phim có cả nguồn khác: không tính KKPhim
        assertEquals("bản lưu", newest.cards.single { it.film.title == "Thiên Long Bát Bộ" }.chip)
        val dp = newest.cards.single { KkPhim.norm(it.film.title) == KkPhim.norm("Đấu Phá Thương Khung Phần 5") }
        assertEquals("2 nguồn", dp.chip)
        // Mất hết (không bản lưu): trang chủ không có hàng nào
        val none = listOf(
            HomeFeed.Part(HomeFeed.Role.NEW, Feed.Req("x", "a") { Feed.Loaded(emptyList()) }) to
                Feed.Result(Feed.Req("x", "a") { Feed.Loaded(emptyList()) }, Feed.State.NO_DATA, emptyList(), null, "không có mạng", false, 0, 0),
        )
        assertTrue(HomeFeed.buildRows(plan(none), none, ctx(emptyList())).rows.isEmpty())
    }

    @Test
    fun kidsProfileKeepsOnlyChildFriendlyAnimation() {
        fun item(slug: String, type: String, vararg genres: String) =
            """{"name":"$slug","slug":"$slug","type":"$type","modified":{"time":"2026-11-08T10:00:00Z"},
               "category":[${genres.joinToString(",") { "{\"slug\":\"$it\"}" }}]}"""
        val body = """{"data":{"items":[${listOf(
            item("doraemon", "hoathinh", "gia-dinh", "hai-huoc"),
            item("chiikawa", "hoathinh", "tre-em"),
            item("conan", "hoathinh", "bi-an"),
            item("bluey", "hoathinh", "gia-dinh"),
            item("edgerunners", "hoathinh", "hanh-dong", "hinh-su"), // hình sự: không hợp trẻ em
            item("phim-ma", "hoathinh", "kinh-di"),
            item("phim-bo-nguoi-dong", "series", "gia-dinh"), // không phải hoạt hình
            item("Horror Collector", "hoathinh", "tre-em", "bi-an"), // tên là phim ma, nguồn không gắn thể loại kinh dị
        ).joinToString(",")}]}}"""
        val list = Feed.parseKk(body, kk)
        val vs = Feed.fromFilms(listOf(Film("https://vsmov.com/phim/na-tra", "Na Tra Truyền Kỳ", null, null))) // «thiếu nhi», chưa rõ thể loại
        val parts = listOf(
            part(HomeFeed.Role.KIDS, "list|kk-young", "kkphim.com", list).let { (p, r) -> HomeFeed.Part(p.role, p.req, "Hoạt hình thiếu nhi", null, "kids-young") to r },
            part(HomeFeed.Role.KIDS, "list|vs-young", "vsmov.com", vs).let { (p, r) -> HomeFeed.Part(p.role, p.req, "Hoạt hình thiếu nhi", null, "kids-young") to r },
        )
        val plan = HomeFeed.Plan(parts.map { it.first }, HomeFeed.Sources(kk, null, "https://vsmov.com", null, emptyList()), null, null, null,
            emptySet(), kids = true)
        val rows = HomeFeed.buildRows(plan, parts, ctx(emptyList())).rows
        assertEquals(1, rows.size) // KKPhim + VSMOV gộp một hàng
        assertEquals(setOf("doraemon", "chiikawa", "conan", "bluey", "Na Tra Truyền Kỳ"), rows[0].cards.map { it.film.title }.toSet())
        // Hồ sơ thường: giữ hết
        val adult = HomeFeed.buildRows(HomeFeed.Plan(plan.parts, plan.src, null, null, null, emptySet()), parts, ctx(emptyList())).rows
        assertEquals(9, adult[0].cards.size)
    }

    @Test
    fun remembersWatchLanguages() {
        Prefs.init(MemoryPrefs())
        repeat(45) { Prefs.recordWatchLang(if (it % 3 == 0) "vs" else "tm") }
        assertEquals(40, Prefs.watchLangs.size)
        assertEquals("tm", Prefs.watchLangs.last())
        assertFalse(Prefs.kids)
        assertTrue(Prefs.homeMixOn)
        Prefs.homeMix = false
        assertFalse(Prefs.homeMixOn)
        Prefs.kids = true
        assertTrue(Prefs.homeMixOn) // hồ sơ trẻ em luôn dùng trang chủ tổng hợp
    }

    @Test
    fun remembersFilmTagsAndSeenEpisodes() {
        Prefs.init(MemoryPrefs())
        assertTrue(FilmTags.needsLookup("Tiên Nghịch"))
        FilmTags.remember(KkPhim.norm("Tiên Nghịch"), Tags(listOf("vo-thuat"), listOf("trung-quoc"), "hoathinh"))
        assertEquals("hoathinh", FilmTags.of("Tiên nghịch")!!.type)
        assertFalse(FilmTags.needsLookup("Tiên Nghịch"))
        FilmTags.remember(KkPhim.norm("Phim Lạ"), null)
        assertNull(FilmTags.of("Phim Lạ"))
        assertFalse(FilmTags.needsLookup("Phim Lạ")) // mới tra không thấy: 3 ngày sau mới tra lại

        assertNull(Prefs.seenEp("muc than ky"))
        Prefs.setSeenEp("muc than ky", 107.0)
        Prefs.setSeenEp("muc than ky", 105.0) // không lùi
        assertEquals(107.0, Prefs.seenEp("muc than ky")!!, 0.0)
        Prefs.savePosition("https://yanhh3d.ee/muc-than-ky/tap-1", 0, "https://yanhh3d.ee/muc-than-ky")
        Prefs.savePosition("https://yanhh3d.ee/muc-than-ky/tap-2", 30, "https://yanhh3d.ee/muc-than-ky")
        assertEquals(2, Prefs.watchedCount("https://yanhh3d.ee/muc-than-ky"))
        assertTrue(Prefs.homeMix)
        Prefs.homeTopics = setOf("cn", "kh")
        assertEquals(setOf("cn", "kh"), Prefs.homeTopics)
        assertNotNull(Prefs.feedHealth)
    }
}
