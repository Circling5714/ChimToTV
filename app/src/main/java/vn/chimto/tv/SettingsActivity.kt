package vn.chimto.tv

import android.app.Activity
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast

class SettingsActivity : Activity() {
    companion object {
        const val CONTACT_URL = "https://t.me/tivikit"
    }

    private lateinit var url: EditText
    private lateinit var result: TextView
    private lateinit var recent: LinearLayout
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)
        url = findViewById(R.id.url)
        result = findViewById(R.id.testResult)
        recent = findViewById(R.id.recent)

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

        findViewById<View>(R.id.btnClearHistory).setOnClickListener {
            Prefs.clearHistory()
            toast("Đã xoá lịch sử xem.")
        }
        findViewById<View>(R.id.btnClearCache).setOnClickListener {
            ImageLoader.clearDisk()
            BrowsingData.clear(this) { toast("Đã xoá ảnh, cookie và dữ liệu duyệt web.") }
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
        url.requestFocus()
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
        proxyResult.setTextColor(getColor(R.color.text_dim))
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
        proxyResult.setTextColor(getColor(R.color.text_dim))
        proxyResult.text = "Đang kiểm tra proxy ${c.host}:${c.port}…"
        val start = System.currentTimeMillis()
        background({ Socks5.check(c) }) { r ->
            if (t != proxyToken) return@background
            val sec = "%.1f s".format((System.currentTimeMillis() - start) / 1000.0).replace('.', ',')
            r.onSuccess {
                proxyResult.setTextColor(getColor(R.color.brand))
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
        result.setTextColor(getColor(R.color.text_dim))
        result.text = "Đang kiểm tra ${Site.hostOf(base)}…"
        background({ Site.home(base) }) { r ->
            if (t != testToken) return@background
            r.onSuccess { home ->
                val count = home.sections.sumOf { it.films.size }
                val moved = home.movedTo?.let { "\nTrang chuyển hướng sang ${Site.hostOf(it)}." } ?: ""
                if (count > 0) {
                    result.setTextColor(getColor(R.color.brand))
                    result.text = "✓ Kết nối được — đọc được $count phim, ${home.categories.size} thể loại.$moved"
                } else {
                    result.setTextColor(getColor(R.color.focus))
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
