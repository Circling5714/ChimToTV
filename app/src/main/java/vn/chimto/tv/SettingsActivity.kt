package vn.chimto.tv

import android.app.Activity
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.inputmethod.EditorInfo
import android.webkit.CookieManager
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast

class SettingsActivity : Activity() {
    private lateinit var url: EditText
    private lateinit var result: TextView
    private lateinit var recent: LinearLayout
    private lateinit var serverBtn: TextView
    private var testToken = 0

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
            CookieManager.getInstance().removeAllCookies(null)
            toast("Đã xoá ảnh và cookie đã lưu.")
        }

        val version = packageManager.getPackageInfo(packageName, 0).versionName
        findViewById<TextView>(R.id.about).text =
            "ChimTo TV $version — xem phim từ trang web trên tivi bằng remote / bàn phím."
        showSites()
        url.requestFocus()
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
