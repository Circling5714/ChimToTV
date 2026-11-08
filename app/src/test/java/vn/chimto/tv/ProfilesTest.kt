package vn.chimto.tv

import android.content.SharedPreferences
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** SharedPreferences trong bộ nhớ cho test JVM (cùng cách áp thay đổi với Android: xoá trước, ghi sau). */
class MemoryPrefs : SharedPreferences {
    val map = HashMap<String, Any>()

    override fun getAll(): Map<String, *> = HashMap(map)
    override fun getString(key: String, defValue: String?): String? = map[key] as String? ?: defValue
    override fun getStringSet(key: String, defValues: MutableSet<String>?): MutableSet<String>? = defValues
    override fun getInt(key: String, defValue: Int): Int = map[key] as Int? ?: defValue
    override fun getLong(key: String, defValue: Long): Long = map[key] as Long? ?: defValue
    override fun getFloat(key: String, defValue: Float): Float = map[key] as Float? ?: defValue
    override fun getBoolean(key: String, defValue: Boolean): Boolean = map[key] as Boolean? ?: defValue
    override fun contains(key: String): Boolean = map.containsKey(key)
    override fun registerOnSharedPreferenceChangeListener(l: SharedPreferences.OnSharedPreferenceChangeListener) = Unit
    override fun unregisterOnSharedPreferenceChangeListener(l: SharedPreferences.OnSharedPreferenceChangeListener) = Unit

    override fun edit(): SharedPreferences.Editor = object : SharedPreferences.Editor {
        val puts = HashMap<String, Any?>()
        val removes = HashSet<String>()
        var clear = false

        private fun put(key: String, value: Any?): SharedPreferences.Editor {
            puts[key] = value
            return this
        }

        override fun putString(key: String, value: String?) = put(key, value)
        override fun putStringSet(key: String, values: MutableSet<String>?) = put(key, values)
        override fun putInt(key: String, value: Int) = put(key, value)
        override fun putLong(key: String, value: Long) = put(key, value)
        override fun putFloat(key: String, value: Float) = put(key, value)
        override fun putBoolean(key: String, value: Boolean) = put(key, value)

        override fun remove(key: String): SharedPreferences.Editor {
            removes += key
            return this
        }

        override fun clear(): SharedPreferences.Editor {
            clear = true
            return this
        }

        override fun commit(): Boolean {
            apply()
            return true
        }

        override fun apply() {
            if (clear) map.clear()
            removes.forEach { map.remove(it) }
            puts.forEach { (k, v) -> if (v == null) map.remove(k) else map[k] = v }
        }
    }
}

class ProfilesTest {
    private lateinit var sp: MemoryPrefs

    private val film = FilmDetail(
        "https://yanhh3d.ee/tien-nghich", "Tiên Nghịch", null, null, null, null,
        listOf(EpisodeGroup("Vietsub", (1..3).map { Episode("Tập $it", "https://yanhh3d.ee/tien-nghich/tap-$it") })),
    )

    private fun history(title: String) =
        HistoryItem("https://yanhh3d.ee/$title", title, null, "Tập 1", "https://yanhh3d.ee/$title/tap-1", 1L)

    @Before
    fun setUp() {
        sp = MemoryPrefs()
    }

    /** Bản cũ (chưa có người xem): dữ liệu là của người xem đầu tiên "Cả nhà", khoá giữ nguyên. */
    @Test
    fun oldDataBelongsToFirstProfile() {
        Prefs.init(sp)
        Prefs.addHistory(history("tien-nghich"))
        Prefs.savePosition("https://yanhh3d.ee/tien-nghich/tap-1", 120, "https://yanhh3d.ee/tien-nghich")
        Prefs.baseUrl = "https://hhkungfu.ee"
        assertTrue(sp.contains("history") && sp.contains("positions") && sp.contains("base_url"))

        Prefs.init(sp) // mở lại app (bản mới)
        assertEquals(listOf(Profile("", "Cả nhà", 0)), Prefs.profiles)
        assertEquals("", Prefs.profileId)
        assertEquals(1, Prefs.history.size)
        assertEquals(120, Prefs.position("https://yanhh3d.ee/tien-nghich/tap-1"))
        assertEquals("https://hhkungfu.ee", Prefs.baseUrl)
    }

    @Test
    fun eachProfileHasOwnHistoryFollowsAndSettings() {
        Prefs.init(sp)
        Prefs.addHistory(history("tien-nghich"))
        Prefs.savePosition("https://yanhh3d.ee/tien-nghich/tap-1", 120, film.url)
        Prefs.follow(film)
        Prefs.baseUrl = "https://hoathinh3d.ee"
        Prefs.seekStep = 30
        Prefs.setServer("hoathinh3d.ee", "tm", "Thuyết Minh")
        Prefs.setIntro(film.url, 0, 90)
        Prefs.privateMode = true

        val bo = Prefs.addProfile("  Bố ")
        assertEquals("Bố", bo.name)
        assertEquals("p1", bo.id)
        assertEquals("", Prefs.profileId) // thêm chưa chuyển sang
        Prefs.switchProfile(bo.id)

        // Lịch sử, chỗ xem, theo dõi, Xem riêng: trống; trang / bước tua / server ưu tiên: chép từ người cũ
        assertTrue(Prefs.history.isEmpty())
        assertEquals(0, Prefs.position("https://yanhh3d.ee/tien-nghich/tap-1"))
        assertTrue(Prefs.watchedEpisodes().isEmpty())
        assertTrue(Prefs.follows.isEmpty())
        assertFalse(Prefs.privateMode)
        assertEquals("https://hoathinh3d.ee", Prefs.baseUrl)
        assertEquals(30, Prefs.seekStep)
        assertEquals("tm", Prefs.serverType("hoathinh3d.ee"))
        // Mốc nhạc mở đầu của phim: dùng chung
        assertEquals(0 to 90, Prefs.intro(film.url))

        // Người mới đổi gì cũng không đụng tới người cũ
        Prefs.addHistory(history("vu-than"))
        Prefs.baseUrl = "https://hhkungfu.ee"
        Prefs.setServer("hoathinh3d.ee", "vs", "Vietsub")
        Prefs.seekStep = 10
        Prefs.switchProfile("")
        assertEquals(listOf("tien-nghich"), Prefs.history.map { it.title })
        assertEquals(120, Prefs.position("https://yanhh3d.ee/tien-nghich/tap-1"))
        assertEquals(1, Prefs.follows.size)
        assertTrue(Prefs.privateMode)
        assertEquals("https://hoathinh3d.ee", Prefs.baseUrl)
        assertEquals("tm", Prefs.serverType("hoathinh3d.ee"))
        assertEquals(30, Prefs.seekStep)
        // Danh sách trang dùng chung: trang người mới thêm có trong danh sách của cả nhà
        assertTrue("https://hhkungfu.ee" in Prefs.sites)

        // Mở lại app: nhớ người xem lần trước
        Prefs.switchProfile(bo.id)
        Prefs.init(sp)
        assertEquals(bo.id, Prefs.profileId)
        assertEquals(listOf("vu-than"), Prefs.history.map { it.title })
    }

    @Test
    fun deleteRemovesOnlyThatProfileData() {
        Prefs.init(sp)
        Prefs.addHistory(history("tien-nghich"))
        val me = Prefs.addProfile("Mẹ")
        Prefs.switchProfile(me.id)
        Prefs.addHistory(history("vu-than"))
        Prefs.setServer("yanhh3d.ee", "v1", "VIP 1")
        Prefs.follow(film)
        Prefs.setIntro(film.url, 0, 80)
        PlayStats.record("yanhh3d.ee", "v1", 3000, 600, 0, 0, failed = false)

        Prefs.deleteProfile(me.id)
        assertEquals("", Prefs.profileId) // xoá người đang dùng → sang người còn lại
        assertEquals(listOf(""), Prefs.profiles.map { it.id })
        assertTrue(sp.map.keys.none { it.endsWith("#${me.id}") })
        assertEquals(listOf("tien-nghich"), Prefs.history.map { it.title })
        // Dùng chung vẫn còn: mốc nhạc mở đầu, điểm server
        assertEquals(0 to 80, Prefs.intro(film.url))
        assertTrue(sp.contains("play_stats"))
        // Còn một người thì không xoá được
        Prefs.deleteProfile("")
        assertEquals(1, Prefs.profiles.size)
    }

    @Test
    fun deleteFirstProfileKeepsSharedSettings() {
        Prefs.init(sp)
        Prefs.addHistory(history("tien-nghich"))
        Prefs.sites = listOf("https://yanhh3d.ee", "https://hhkungfu.ee")
        Prefs.proxyHost = "103.1.2.3"
        Prefs.setIntro(film.url, 0, 80)
        val be = Prefs.addProfile("Bé")
        Prefs.deleteProfile("")
        assertEquals(be.id, Prefs.profileId)
        assertFalse(sp.contains("history"))
        assertEquals(listOf("https://yanhh3d.ee", "https://hhkungfu.ee"), Prefs.sites)
        assertEquals("103.1.2.3", Prefs.proxyHost)
        assertEquals(0 to 80, Prefs.intro(film.url))
        assertTrue(sp.contains("intro@${film.url}"))
    }

    @Test
    fun pinLock() {
        Prefs.init(sp)
        val bo = Prefs.addProfile("Bố")
        assertFalse(bo.locked)
        assertTrue(Prefs.checkPin(bo, "")) // không khoá: mã nào cũng qua
        Prefs.setProfilePin(bo.id, "1234")
        val locked = Prefs.profiles.first { it.id == bo.id }
        assertTrue(locked.locked)
        assertFalse(locked.pin!!.contains("1234")) // chỉ lưu mã băm
        assertTrue(Prefs.checkPin(locked, "1234"))
        assertFalse(Prefs.checkPin(locked, "4321"))
        assertFalse(Prefs.checkPin(locked, ""))
        Prefs.setProfilePin(bo.id, null)
        assertNull(Prefs.profiles.first { it.id == bo.id }.pin)
    }

    @Test
    fun newProfilesGetFreeColorsAndIds() {
        Prefs.init(sp)
        val a = Prefs.addProfile("A")
        val b = Prefs.addProfile("B")
        assertEquals(listOf(0, 1, 2), Prefs.profiles.map { it.color })
        assertEquals(listOf("", "p1", "p2"), Prefs.profiles.map { it.id })
        Prefs.deleteProfile(a.id)
        val c = Prefs.addProfile("C")
        assertEquals("p1", c.id) // id trống thì dùng lại (dữ liệu của người cũ đã xoá hết)
        assertEquals(1, c.color)
        Prefs.renameProfile(b.id, "Bé Na")
        Prefs.setProfileColor(b.id, 6)
        assertEquals(Profile("p2", "Bé Na", 6), Prefs.profiles.first { it.id == b.id })
    }

    @Test
    fun deletedPresetSiteCanBeAddedBack() {
        Prefs.init(sp)
        val phimmois = "https://phimmois.fun"
        Prefs.sites = Prefs.PRESET_SITES.filter { it != phimmois }
        assertEquals(listOf(phimmois), Prefs.missingPresets)
        Prefs.addSite(phimmois)
        Prefs.addSite(phimmois)
        assertTrue(Prefs.missingPresets.isEmpty())
        assertEquals(1, Prefs.sites.count { it == phimmois })
        assertEquals(Prefs.PRESET_SITES.size, Prefs.sites.size)
    }

    /** Trang đổi tên miền (phimmoir.fun → phimmois.fun): dữ liệu đã lưu tự đổi sang tên mới lúc mở app. */
    @Test
    fun movedHostRenamedInSavedData() {
        sp.map["sites"] = """["https:\/\/yanhh3d.ee","https:\/\/phimmoir.fun","https:\/\/phimmois.fun"]"""
        sp.map["base_url"] = "https://phimmoir.fun"
        sp.map["server_type@phimmoir.fun"] = "kk"
        sp.map["server_type@phimmois.fun#p1"] = "nguonc"
        sp.map["server_type@phimmoir.fun#p1"] = "ophim" // khoá tên mới đã có → giữ khoá tên mới
        sp.map["preset_added@https://phimmoir.fun"] = true
        sp.map["history"] = """[{"filmUrl":"https:\/\/phimmoir.fun\/phim\/a","title":"A"}]"""
        Prefs.init(sp)
        // mephim.ddns.net (2.5), KKPhim (2.6), NguonC / VSMOV / CLB Phim Xưa (2.7): trang mẫu thêm ở bản sau → thêm một lần
        assertEquals(
            listOf("https://yanhh3d.ee", "https://phimmois.fun", "https://mephim.ddns.net", "https://www.kkphim.com",
                "https://phim.nguonc.com", "https://vsmov.com", "https://sc.k-20.xyz"),
            Prefs.sites,
        )
        assertEquals("https://phimmois.fun", Prefs.baseUrl)
        assertEquals("kk", sp.map["server_type@phimmois.fun"])
        assertEquals("nguonc", sp.map["server_type@phimmois.fun#p1"])
        assertFalse(sp.map.keys.any { "phimmoir" in it })
        assertFalse(sp.map.values.any { it is String && "phimmoir" in it })
        // Trang mẫu mới (phimmois.fun) đã có trong danh sách → không thêm lần nữa
        assertEquals(1, Prefs.sites.count { it == "https://phimmois.fun" })
    }

    /** Vượt chặn nhà mạng: tìm tên trang trong gói bắt tay, không phân biệt hoa / thường. */
    @Test
    fun findHostInHello() {
        val b = "xxPhimMoiS.funyy".toByteArray()
        assertEquals(2, find("phimmois.fun".toByteArray(), b, 1, b.size))
        assertNull(find("yanhh3d.ee".toByteArray(), b, 1, b.size))
    }
}
