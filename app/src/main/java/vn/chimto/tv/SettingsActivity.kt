package vn.chimto.tv

import android.content.res.Configuration
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast

class SettingsActivity : BaseActivity() {
    companion object {
        const val CONTACT_URL = "https://t.me/tivikit"

        /** "profiles": mở Cài đặt ở mục Người xem (nút "Sửa người xem…" của màn "Ai đang xem?"). */
        const val EXTRA_SECTION = "section"
    }

    private lateinit var url: EditText
    private lateinit var result: TextView
    private lateinit var recent: LinearLayout
    private lateinit var profileRow: LinearLayout
    private lateinit var serverBtn: TextView
    private var testToken = 0

    private lateinit var proxyHost: EditText
    private lateinit var proxyPort: EditText
    private lateinit var proxyUser: EditText
    private lateinit var proxyPass: EditText
    private lateinit var proxyBtn: TextView
    private lateinit var proxyDnsBtn: TextView
    private lateinit var proxyScopeBtn: TextView
    private lateinit var proxyResult: TextView
    private var proxyToken = 0

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        applyLayout() // điện thoại xoay ngang / dọc
    }

    /** Điện thoại cầm dọc: lề nhỏ, các ô proxy xếp dọc (hàng nút tự xuống dòng — FlowRow). */
    private fun applyLayout() {
        val narrow = Screen.narrow(this)
        findViewById<View>(R.id.settingsBody).padH(dp(if (narrow) 16 else 48))
        for (id in intArrayOf(R.id.proxyRow1, R.id.proxyRow2)) {
            val row = findViewById<LinearLayout>(id)
            row.orientation = if (narrow) LinearLayout.VERTICAL else LinearLayout.HORIZONTAL
            for (k in 0 until row.childCount) {
                val lp = row.getChildAt(k).layoutParams as LinearLayout.LayoutParams
                if (k > 0) {
                    lp.marginStart = if (narrow) 0 else dp(12)
                    lp.leftMargin = lp.marginStart // app không bật RTL: trái = đầu (marginStart đã giải ra lúc dựng)
                    lp.topMargin = if (narrow) dp(8) else 0
                }
                lp.width = if (narrow) LinearLayout.LayoutParams.MATCH_PARENT else 0
                // Ô máy chủ rộng gấp 3 ô cổng (hàng đầu); hàng tên / mật khẩu chia đôi
                lp.weight = if (narrow) 0f else if (id == R.id.proxyRow1 && k == 0) 3f else 1f
            }
            row.requestLayout()
        }
        // Nút chữ dài (vd «Server ưu tiên (…): …»): màn hẹp cho xuống 2 dòng thay vì cắt «…»
        fun walk(v: View) {
            if (v is TextView && v !is EditText && v.isClickable && v.isFocusable) {
                if (narrow) {
                    v.setSingleLine(false)
                    v.maxLines = 2
                } else {
                    v.setSingleLine(true)
                }
            }
            if (v is ViewGroup) for (i in 0 until v.childCount) walk(v.getChildAt(i))
        }
        walk(findViewById(R.id.settingsBody))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)
        applyLayout()
        url = findViewById(R.id.url)
        result = findViewById(R.id.testResult)
        recent = findViewById(R.id.recent)
        profileRow = findViewById(R.id.profileRow)
        setupProfiles()

        url.setText(Prefs.baseUrl)
        url.leaveWithUpDown()
        url.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                findViewById<View>(R.id.btnSave).requestFocus()
            }
            false
        }
        findViewById<View>(R.id.btnSave).setOnClickListener { save() }
        findViewById<View>(R.id.btnTest).setOnClickListener { test() }
        findViewById<View>(R.id.btnDefault).setOnClickListener {
            url.setText(Prefs.DEFAULT_BASE)
            save()
        }
        findViewById<View>(R.id.btnRemove).setOnClickListener { removeCurrent() }

        val seek = findViewById<TextView>(R.id.btnSeek)
        val steps = listOf(5, 10, 15, 30, 60)
        fun showSeek() {
            seek.text = "Bước tua ◀ ▶: ${Prefs.seekStep} giây"
        }
        showSeek()
        seek.setOnClickListener {
            Prefs.seekStep = steps[(steps.indexOf(Prefs.seekStep) + 1) % steps.size]
            showSeek()
        }

        val autoNext = findViewById<TextView>(R.id.btnAutoNext)
        fun showAutoNext() {
            autoNext.text = "Tự chuyển tập khi hết: ${if (Prefs.autoNext) "Bật" else "Tắt"}"
        }
        showAutoNext()
        autoNext.setOnClickListener {
            Prefs.autoNext = !Prefs.autoNext
            showAutoNext()
        }

        serverBtn = findViewById(R.id.btnServer)
        showServer()
        serverBtn.setOnClickListener {
            Prefs.setServer(Site.hostOf(Prefs.baseUrl), null, null)
            showServer()
            toast("Đã đặt lại. App sẽ nhớ server được chọn lần tới trong lúc xem.")
        }

        val kk = findViewById<TextView>(R.id.btnKkBackup)
        fun showKk() {
            kk.text = "Server dự phòng KKPhim: ${if (Prefs.kkBackup) "Bật" else "Tắt"}"
        }
        showKk()
        kk.setOnClickListener {
            Prefs.kkBackup = !Prefs.kkBackup
            showKk()
        }
        val nc = findViewById<TextView>(R.id.btnNcBackup)
        fun showNc() {
            nc.text = "Server dự phòng NguonC: ${if (Prefs.ncBackup) "Bật" else "Tắt"}"
        }
        showNc()
        nc.setOnClickListener {
            Prefs.ncBackup = !Prefs.ncBackup
            showNc()
        }

        // Thu nhỏ phim: hình trong hình của Android (TV có tính năng này) hoặc cửa sổ nhỏ trong app
        val pipHelp = findViewById<TextView>(R.id.pipHelp)
        pipHelp.visibility = View.VISIBLE
        if (PlayerActivity.pipSupported(this)) {
            // Bấm Home khi đang xem: thu nhỏ phim vào góc (chỉ TV / máy có hình trong hình)
            val pipBtn = findViewById<TextView>(R.id.btnPipHome)
            fun showPip() {
                pipBtn.text = "Bấm Home khi đang xem: ${if (Prefs.pipOnHome) "thu nhỏ phim vào góc" else "dừng phim"}"
            }
            showPip()
            pipBtn.visibility = View.VISIBLE
            pipBtn.setOnClickListener {
                Prefs.pipOnHome = !Prefs.pipOnHome
                showPip()
            }
        } else {
            pipHelp.text = "Thu nhỏ phim: lúc xem nhấn ▼ → «Thu nhỏ vào góc» — phim chạy tiếp ở góc màn hình trong lúc chọn " +
                "phim khác trong app. TV này không có hình trong hình của Android nên phim không nổi lên app khác được; rời " +
                "app thì phim dừng. Đầu menu bên trái có «Phóng to» và «Tắt phim thu nhỏ»."
        }

        // Nhà nhiều người xem: các nút của riêng người đang dùng ghi rõ tên
        val who = if (Prefs.profiles.size >= 2) Prefs.profile.name else null
        findViewById<TextView>(R.id.btnClearHistory).apply {
            text = if (who != null) "Xoá lịch sử xem của $who" else "Xoá lịch sử xem"
            setOnClickListener {
                Prefs.clearHistory()
                toast(if (who != null) "Đã xoá lịch sử xem của $who." else "Đã xoá lịch sử xem.")
            }
        }
        findViewById<View>(R.id.btnClearCache).setOnClickListener {
            ImageLoader.clearDisk()
            BrowsingData.clear(this) { toast("Đã xoá ảnh, cookie và dữ liệu duyệt web.") }
        }
        val dohBtn = findViewById<TextView>(R.id.btnDoh)
        fun showDoh() {
            dohBtn.text = "DNS mã hoá: ${Prefs.dohServer?.label ?: "Tắt"}"
        }
        showDoh()
        dohBtn.setOnClickListener {
            val all = listOf(null) + Doh.Server.values()
            Prefs.dohServer = all[(all.indexOf(Prefs.dohServer) + 1) % all.size]
            AppProxy.apply()
            showDoh()
        }
        val unblockBtn = findViewById<TextView>(R.id.btnUnblock)
        fun showUnblock() {
            unblockBtn.text = "Vượt chặn nhà mạng: ${if (!Prefs.unblock) "Tắt" else if (Prefs.unblockLevel >= 2) "Mức 2 (đảo thứ tự)" else "Mức 1 (chia nhỏ)"}" +
                if (Prefs.unblock && !AppProxy.webViewSupported) " (WebView quá cũ: chỉ phần đọc trang)" else ""
        }
        showUnblock()
        unblockBtn.setOnClickListener {
            // Tắt → Mức 1 → Mức 2 → Tắt
            when {
                !Prefs.unblock -> { Prefs.unblock = true; Prefs.unblockLevel = 1 }
                Prefs.unblockLevel < 2 -> Prefs.unblockLevel = 2
                else -> Prefs.unblock = false
            }
            AppProxy.apply()
            showUnblock()
        }
        val privateBtn = findViewById<TextView>(R.id.btnPrivate)
        fun showPrivate() {
            privateBtn.text = "Xem riêng${who?.let { " của $it" } ?: ""} (không lưu lịch sử): ${if (Prefs.privateMode) "Bật" else "Tắt"}"
        }
        showPrivate()
        privateBtn.setOnClickListener {
            Prefs.privateMode = !Prefs.privateMode
            showPrivate()
        }
        val autoClearBtn = findViewById<TextView>(R.id.btnAutoClear)
        fun showAutoClear() {
            autoClearBtn.text = "Tự xoá dữ liệu web khi rời app: ${if (Prefs.autoClearWeb) "Bật" else "Tắt"}"
        }
        showAutoClear()
        autoClearBtn.setOnClickListener {
            Prefs.autoClearWeb = !Prefs.autoClearWeb
            showAutoClear()
        }

        val updateResult = findViewById<TextView>(R.id.updateResult)
        findViewById<TextView>(R.id.btnUpdateCheck).apply {
            text = "Kiểm tra cập nhật (đang dùng ${BuildConfig.VERSION_NAME})"
            setOnClickListener { Updater.checkNow(this@SettingsActivity) { updateResult.text = it } }
        }
        val autoUpdate = findViewById<TextView>(R.id.btnAutoUpdate)
        fun showAutoUpdate() {
            autoUpdate.text = "Tự động cập nhật: ${if (Prefs.autoUpdate) "Bật" else "Tắt"}"
        }
        showAutoUpdate()
        autoUpdate.setOnClickListener {
            Prefs.autoUpdate = !Prefs.autoUpdate
            showAutoUpdate()
        }

        // Mở trang Telegram ngay trong app (TV thường không có Telegram / trình duyệt riêng)
        findViewById<View>(R.id.btnContact).setOnClickListener { startActivity(WebActivity.intent(this, CONTACT_URL)) }

        val version = packageManager.getPackageInfo(packageName, 0).versionName
        findViewById<TextView>(R.id.about).text =
            "${getString(R.string.app_name)} $version — xem phim từ trang web trên tivi bằng remote / bàn phím."
        showSites()
        setupProxy()
        if (intent.getStringExtra(EXTRA_SECTION) == "profiles") {
            val i = Prefs.profiles.indexOfFirst { it.id == Prefs.profileId }.coerceAtLeast(0)
            profileRow.getChildAt(i)?.requestFocus()
        } else {
            url.requestFocus()
        }
    }

    // ---- Người xem (xem Prefs.Profile, ProfileUi) ----

    private fun setupProfiles() {
        showProfiles()
        findViewById<View>(R.id.btnProfileAdd).setOnClickListener {
            ProfileUi.add(this) { p ->
                toast("Đã thêm ${p.name}. Đổi người xem: mục «Người xem» đầu menu, hoặc chọn lúc mở app.")
                reopenAtProfiles()
            }
        }
        findViewById<View>(R.id.btnLimits).setOnClickListener { ProfileUi.limits(this) { reopenAtProfiles() } }
        findViewById<View>(R.id.btnSync).setOnClickListener { SyncUi.show(this) }
        val ask = findViewById<TextView>(R.id.btnAskProfile)
        fun showAsk() {
            ask.text = "Hỏi «Ai đang xem?» khi mở app: ${if (Prefs.askProfile) "Bật" else "Tắt"}"
        }
        showAsk()
        ask.setOnClickListener {
            Prefs.askProfile = !Prefs.askProfile
            showAsk()
        }
    }

    private fun showProfiles() {
        profileRow.removeAllViews()
        val inflater = LayoutInflater.from(this)
        for (p in Prefs.profiles) {
            val b = inflater.inflate(R.layout.item_button, profileRow, false) as TextView
            ProfileUi.showOn(b, p)
            b.isActivated = p.id == Prefs.profileId
            b.setOnClickListener { ProfileUi.manage(this, p) { reopenAtProfiles() } }
            profileRow.addView(b)
        }
    }

    /** Đổi người xem / sửa người xem: dựng lại cả màn (trang phim, server ưu tiên, Xem riêng… là của người đang dùng). */
    private fun reopenAtProfiles() {
        intent.putExtra(EXTRA_SECTION, "profiles")
        recreate()
    }

    // ---- Proxy SOCKS5 (xem AppProxy) ----

    private fun setupProxy() {
        proxyHost = findViewById(R.id.proxyHost)
        proxyPort = findViewById(R.id.proxyPort)
        proxyUser = findViewById(R.id.proxyUser)
        proxyPass = findViewById(R.id.proxyPass)
        proxyBtn = findViewById(R.id.btnProxy)
        proxyDnsBtn = findViewById(R.id.btnProxyDns)
        proxyScopeBtn = findViewById(R.id.btnProxyScope)
        proxyResult = findViewById(R.id.proxyResult)

        proxyHost.setText(Prefs.proxyHost)
        proxyPort.setText(if (Prefs.proxyHost.isEmpty()) "" else Prefs.proxyPort.toString())
        proxyUser.setText(Prefs.proxyUser)
        proxyPass.setText(Prefs.proxyPass)
        listOf(proxyHost, proxyPort, proxyUser, proxyPass).forEach { it.leaveWithUpDown() }

        proxyBtn.setOnClickListener {
            // Bật thì lưu luôn máy chủ / cổng đang nhập (sai thì báo, không bật)
            if (!Prefs.proxyOn && !saveProxyFields()) return@setOnClickListener
            Prefs.proxyOn = !Prefs.proxyOn
            AppProxy.apply()
            showProxy()
        }
        proxyDnsBtn.setOnClickListener {
            Prefs.proxyRemoteDns = !Prefs.proxyRemoteDns
            AppProxy.apply()
            showProxy()
        }
        proxyScopeBtn.setOnClickListener {
            Prefs.proxySitesOnly = !Prefs.proxySitesOnly
            AppProxy.apply()
            showProxy()
        }
        findViewById<View>(R.id.btnProxySave).setOnClickListener {
            if (saveProxyFields()) {
                AppProxy.apply()
                showProxy()
                toast(if (Prefs.proxyOn) "Đã lưu proxy." else "Đã lưu proxy — nhấn «Proxy: Tắt» để bật.")
            }
        }
        findViewById<View>(R.id.btnProxyTest).setOnClickListener { testProxy() }
        showProxy()
    }

    private fun showProxy() {
        proxyBtn.text = "Proxy: ${if (Prefs.proxyOn) "Bật" else "Tắt"}"
        proxyDnsBtn.text = "DNS: ${if (Prefs.proxyRemoteDns) "của proxy" else "của thiết bị"}"
        proxyScopeBtn.text = "Dùng cho: ${if (Prefs.proxySitesOnly) "chỉ trang phim" else "toàn bộ app"}"
        proxyResult.setTextColor(colorOf(R.color.text_dim))
        val c = Prefs.proxyConfig()
        proxyResult.text = (
            if (c == null) {
                "Đang tắt — app dùng mạng mặc định của TV."
            } else {
                "Đang bật: ${c.host}:${c.port}" + (if (c.user.isNotEmpty()) " · có đăng nhập" else "") +
                    (if (AppProxy.webViewSupported) "" else "\n⚠ WebView của TV quá cũ, không đi qua proxy được: video và Duyệt web vẫn đi thẳng.")
            }
            ) + "\nDNS của proxy: proxy tự tìm địa chỉ trang (vượt chặn tên miền của nhà mạng); DNS của thiết bị: TV tự tìm " +
            "rồi chỉ gửi IP cho proxy. Chỉ trang phim: video, ảnh, cập nhật app đi thẳng mạng nhà (đỡ tốn proxy)."
    }

    /** Đọc ô nhập; nhận cả dạng dán sẵn "socks5://user:pass@host:cổng" hoặc "host:cổng". Sai thì báo, trả null. */
    private fun readProxyFields(): SocksConfig? {
        var host = proxyHost.text.toString().trim().substringAfter("://").trimEnd('/')
        var port = proxyPort.text.toString().trim()
        var user = proxyUser.text.toString()
        var pass = proxyPass.text.toString()
        if ('@' in host) {
            val login = host.substringBeforeLast('@')
            host = host.substringAfterLast('@')
            user = login.substringBefore(':')
            pass = login.substringAfter(':', "")
        }
        if (host.count { it == ':' } == 1) {
            port = host.substringAfter(':')
            host = host.substringBefore(':')
        }
        val p = port.toIntOrNull()
        when {
            host.isEmpty() || ' ' in host -> toast("Nhập máy chủ proxy (vd 103.1.2.3).")
            p == null || p !in 1..65535 -> toast("Cổng proxy không hợp lệ (1–65535, vd 1080).")
            user.toByteArray().size > 255 || pass.toByteArray().size > 255 -> toast("Tên đăng nhập / mật khẩu tối đa 255 ký tự.")
            else -> {
                proxyHost.setText(host)
                proxyPort.setText(p.toString())
                proxyUser.setText(user)
                proxyPass.setText(pass)
                return SocksConfig(host, p, user, pass, Prefs.proxyRemoteDns, Prefs.proxySitesOnly)
            }
        }
        return null
    }

    private fun saveProxyFields(): Boolean {
        val c = readProxyFields() ?: return false
        Prefs.proxyHost = c.host
        Prefs.proxyPort = c.port
        Prefs.proxyUser = c.user
        Prefs.proxyPass = c.pass
        return true
    }

    /** Thử proxy đang nhập (chưa cần lưu / bật): mở HTTPS tới Cloudflare qua proxy, báo IP ra ngoài + thời gian. */
    private fun testProxy() {
        val c = readProxyFields() ?: return
        val t = ++proxyToken
        proxyResult.setTextColor(colorOf(R.color.text_dim))
        proxyResult.text = "Đang kiểm tra proxy ${c.host}:${c.port}…"
        val start = System.currentTimeMillis()
        background({ Socks5.check(c) }) { r ->
            if (t != proxyToken) return@background
            val sec = "%.1f s".format((System.currentTimeMillis() - start) / 1000.0).replace('.', ',')
            r.onSuccess {
                proxyResult.setTextColor(colorOf(R.color.brand))
                proxyResult.text = "✓ Proxy chạy — IP ra ngoài $it, $sec." + if (Prefs.proxyOn) "" else " Nhấn «Proxy: Tắt» để bật."
            }.onFailure {
                proxyResult.setTextColor(0xFFFF6B6B.toInt())
                proxyResult.text = "✗ Proxy không chạy: ${it.message ?: it.javaClass.simpleName}"
            }
        }
    }

    private fun showServer() {
        serverBtn.text = "Server ưu tiên (${Site.hostOf(Prefs.baseUrl)}): ${Prefs.serverLabel(Site.hostOf(Prefs.baseUrl)) ?: "server đầu tiên"}"
    }

    private fun showSites() {
        recent.removeAllViews()
        val inflater = LayoutInflater.from(this)
        for (u in Prefs.sites) {
            val b = inflater.inflate(R.layout.item_button, recent, false) as TextView
            b.text = Site.hostOf(u) ?: u
            b.isActivated = u == Prefs.baseUrl
            b.setOnClickListener {
                url.setText(u)
                save()
            }
            recent.addView(b)
        }
        // Trang mẫu đã xoá khỏi danh sách (vd phimmois.fun): thêm lại một chạm
        for (u in Prefs.missingPresets) {
            val b = inflater.inflate(R.layout.item_button, recent, false) as TextView
            b.text = "＋ ${Site.hostOf(u) ?: u}"
            b.setOnClickListener {
                Prefs.addSite(u)
                showSites()
                // Nút vừa bấm đã biến thành nút trang thường: đưa con trỏ về đúng nút đó
                recent.getChildAt(Prefs.sites.indexOf(u))?.requestFocus()
                toast("Đã thêm lại ${Site.hostOf(u)} vào danh sách trang phim.")
            }
            recent.addView(b)
        }
    }

    private fun removeCurrent() {
        val cur = Prefs.baseUrl
        val rest = Prefs.sites.filter { it != cur }
        if (rest.isEmpty()) {
            toast("Danh sách cần ít nhất một trang.")
            return
        }
        Prefs.sites = rest
        Prefs.baseUrl = rest.first()
        url.setText(rest.first())
        showSites()
        showServer()
        toast("Đã xoá ${Site.hostOf(cur)} — đang dùng ${Site.hostOf(rest.first())}")
    }

    private fun save() {
        val normalized = Prefs.normalizeUrl(url.text.toString())
        if (normalized == null) {
            toast("Link không hợp lệ. Ví dụ đúng: yanhh3d.ee")
            return
        }
        url.setText(normalized)
        if (normalized != Prefs.baseUrl || normalized !in Prefs.sites) {
            Prefs.baseUrl = normalized
            showSites()
            showServer()
        }
        toast("Đã lưu: ${Site.hostOf(normalized)}")
        test()
    }

    private fun test() {
        val base = Prefs.normalizeUrl(url.text.toString())
        if (base == null) {
            toast("Link không hợp lệ.")
            return
        }
        val t = ++testToken
        result.visibility = View.VISIBLE
        result.setTextColor(colorOf(R.color.text_dim))
        result.text = "Đang kiểm tra ${Site.hostOf(base)}…"
        background({ Site.home(base) }) { r ->
            if (t != testToken) return@background
            r.onSuccess { home ->
                val count = home.sections.sumOf { it.films.size }
                val moved = home.movedTo?.let { "\nTrang chuyển hướng sang ${Site.hostOf(it)}." } ?: ""
                if (count > 0) {
                    result.setTextColor(colorOf(R.color.brand))
                    result.text = "✓ Kết nối được — đọc được $count phim, ${home.categories.size} thể loại.$moved"
                } else {
                    result.setTextColor(colorOf(R.color.focus))
                    result.text = "Kết nối được nhưng không đọc được danh sách phim (trang khác giao diện?). " +
                        "Vẫn có thể dùng mục «Duyệt web».$moved"
                }
            }.onFailure {
                result.setTextColor(0xFFFF6B6B.toInt())
                result.text = "✗ Không kết nối được: ${it.friendly()}"
            }
        }
    }

    private fun toast(s: String) = Toast.makeText(this, s, Toast.LENGTH_LONG).show()
}
