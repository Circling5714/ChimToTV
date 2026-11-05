package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** Đồng bộ trong nhà: gộp dữ liệu (cùng ca với SyncMergeTests.swift của bản iPad) + đọc / ghi Prefs + JSON. */
class SyncTest {
    private fun h(film: String, time: Long, ep: String = "e1") = SyncHistory(film, film.uppercase(), "", ep, "$film/$ep", time)
    private fun f(film: String, count: Int, seen: Int, t: Long) = SyncFollow(film, film, "", count, "Tập $count", seen, t)

    @Before
    fun setUp() = Prefs.init(MemoryPrefs())

    @Test
    fun historyNewestWinsAndUnion() {
        val a = SyncProfile("", "Cả nhà", history = listOf(h("x", 100, "e1"), h("y", 50)))
        val b = SyncProfile("", "Cả nhà", history = listOf(h("x", 200, "e2"), h("z", 150)))
        val m = SyncMerge.profile(a, b)
        assertEquals(listOf("x", "z", "y"), m.history.map { it.film })
        assertEquals("e2", m.history[0].ep)
        assertEquals(m.history, SyncMerge.profile(b, a).history)
    }

    @Test
    fun deletedHistoryStaysDeleted() {
        val a = SyncProfile("", "n", histDel = mapOf("x" to 300L))
        val b = SyncProfile("", "n", history = listOf(h("x", 200)),
            pos = listOf(SyncPos("x/e1", 30, "x", 200), SyncPos("y/e1", 5, "y", 10)))
        val m = SyncMerge.profile(a, b)
        assertTrue(m.history.isEmpty())
        assertEquals(listOf("y/e1"), m.pos.map { it.ep })
        assertEquals(300L, m.histDel["x"])
        val c = SyncProfile("", "n", history = listOf(h("x", 400)))
        assertEquals(listOf("x"), SyncMerge.profile(m, c).history.map { it.film })
    }

    @Test
    fun positionsNewestWinsOldestFirst() {
        val a = SyncProfile("", "n", pos = listOf(SyncPos("old", 1, "f", 0), SyncPos("e", 100, "f", 500)))
        val b = SyncProfile("", "n", pos = listOf(SyncPos("e", 250, "f", 600), SyncPos("n", 7, "f", 50)))
        val m = SyncMerge.profile(a, b)
        assertEquals(listOf("old", "n", "e"), m.pos.map { it.ep })
        assertEquals(250, m.pos.last().sec)
    }

    @Test
    fun followsMergeAndUnfollow() {
        val a = SyncProfile("", "n", follows = listOf(f("x", 10, 10, 100), f("y", 3, 1, 50)))
        val b = SyncProfile("", "n", follows = listOf(f("x", 12, 9, 100)), followDel = mapOf("y" to 80L))
        val m = SyncMerge.profile(a, b)
        assertEquals(listOf("x"), m.follows.map { it.film })
        assertEquals(12, m.follows[0].count)
        assertEquals(10, m.follows[0].seen)
        assertEquals("Tập 12", m.follows[0].latest)
        val c = SyncProfile("", "n", follows = listOf(f("y", 3, 3, 90)))
        assertEquals(setOf("x", "y"), SyncMerge.profile(m, c).follows.map { it.film }.toSet())
    }

    @Test
    fun matchProfiles() {
        val local = listOf(SyncProfile("", "Cả nhà"), SyncProfile("p1", "Bố"), SyncProfile("p2", "Bé Na"))
        val remote = listOf(SyncProfile("p3", " bé na "), SyncProfile("", "Nhà mình"), SyncProfile("p1", "Mẹ"))
        assertEquals(listOf(0 to 1, 2 to 0), SyncMerge.match(local, remote))
    }

    @Test
    fun sites() {
        val a = SyncSites(listOf("a", "b", "c"), mapOf("c" to 100L), mapOf("d" to 100L))
        val b = SyncSites(listOf("a", "d", "e"), mapOf("d" to 50L, "e" to 200L), mapOf("b" to 300L))
        val m = SyncMerge.sites(a, b)
        assertEquals(listOf("a", "c", "e"), m.list)
        assertEquals(m.list.sorted(), SyncMerge.sites(b, a).list.sorted())
    }

    @Test
    fun tombstoneCap() {
        val big = (0 until SyncMerge.MAX_TOMBSTONES + 20).associate { "k$it" to it.toLong() }
        val m = SyncMerge.tombstones(big, emptyMap())
        assertEquals(SyncMerge.MAX_TOMBSTONES, m.size)
        assertNull(m["k0"])
        assertNotNull(m["k${SyncMerge.MAX_TOMBSTONES + 19}"])
    }

    @Test
    fun jsonRoundTrip() {
        val p = SyncPayload("d", "TV", "android",
            listOf(SyncProfile("", "Cả nhà", listOf(h("x", 1)), mapOf("z" to 5L), listOf(SyncPos("x/e1", 3, "x", 9)),
                listOf(f("x", 2, 1, 7)), mapOf("w" to 6L))),
            SyncSites(listOf("a"), mapOf("a" to 1L), mapOf("b" to 2L)))
        val back = SyncPayload.parse(p.toJson())
        assertEquals(p, back)
        // Cùng tên khoá với bản iPad
        val o = org.json.JSONObject(p.toJson())
        val prof = o.getJSONArray("profiles").getJSONObject(0)
        assertEquals(setOf("id", "name", "history", "histDel", "pos", "follows", "followDel"), prof.keys().asSequence().toSet())
        assertEquals(setOf("film", "title", "poster", "ep", "epUrl", "time"),
            prof.getJSONArray("history").getJSONObject(0).keys().asSequence().toSet())
    }

    /** Hai TV giả (hai kho Prefs): xem / xoá / theo dõi ở máy này, gộp sang máy kia rồi ngược lại. */
    @Test
    fun prefsRoundTrip() {
        val pa = MemoryPrefs()
        val pb = MemoryPrefs()
        fun onA(block: () -> Unit) = Prefs.init(pa).also { block() }
        fun onB(block: () -> Unit) = Prefs.init(pb).also { block() }

        onA {
            Prefs.addHistory(HistoryItem("https://s/f1", "Phim 1", null, "Tập 1", "https://s/f1/e1", 1000))
            Prefs.savePosition("https://s/f1/e1", 120, "https://s/f1")
            Prefs.addHistory(HistoryItem("https://s/f2", "Phim 2", null, "Tập 3", "https://s/f2/e3", 2000))
            Prefs.savePosition("https://s/f2/e3", 0, "https://s/f2")
        }
        val fromA = onA { }.let { SyncPayload(Prefs.deviceId, "A", "android", Prefs.profiles.map { Prefs.syncProfile(it) }, Prefs.syncSites) }
        var changed = false
        onB {
            Prefs.addProfile("Bố")
            changed = SyncNet.absorb(fromA)
        }
        assertTrue(changed)
        onB {
            assertEquals(listOf("https://s/f2", "https://s/f1"), Prefs.history.map { it.filmUrl })
            assertEquals(120, Prefs.position("https://s/f1/e1"))
            assertTrue(Prefs.finished("https://s/f2/e3"))
            // Gộp lại lần nữa: không đổi gì
            assertFalse(SyncNet.absorb(fromA))
            // B xoá phim 1 → A nhận bia mộ, phim 1 biến mất ở A
            Prefs.removeHistory("https://s/f1")
        }
        val fromB = onB { }.let { SyncPayload(Prefs.deviceId, "B", "android", Prefs.profiles.map { Prefs.syncProfile(it) }, Prefs.syncSites) }
        onA {
            assertTrue(SyncNet.absorb(fromB))
            assertEquals(listOf("https://s/f2"), Prefs.history.map { it.filmUrl })
            assertEquals(0, Prefs.position("https://s/f1/e1"))
            assertFalse(Prefs.watchedEpisodes().contains("https://s/f1/e1"))
        }
    }

    @Test
    fun hmacSignatureVerifies() {
        val body = "{\"a\":1}".toByteArray()
        val time = System.currentTimeMillis().toString()
        val sig = Http1.sign("tok", time, body)
        assertTrue(Http1.verify(Http1.Message(headers = mapOf("x-time" to time, "x-sig" to sig), body = body), "tok"))
        assertFalse(Http1.verify(Http1.Message(headers = mapOf("x-time" to time, "x-sig" to sig), body = body), "khac"))
        assertFalse(Http1.verify(Http1.Message(headers = mapOf("x-time" to "1", "x-sig" to Http1.sign("tok", "1", body)), body = body), "tok"))
    }
}
