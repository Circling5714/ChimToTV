package vn.chimto.tv

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Color
import android.os.Bundle
import android.os.Message
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.ConsoleMessage
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import org.json.JSONObject
import java.io.ByteArrayInputStream
import kotlin.math.min

/**
 * Phát một tập phim.
 *
 * Trình phát của trang chỉ chạy khi nằm trong iframe của chính trang phim, nên app dựng một trang bọc
 * (assets/player.html) với origin = địa chỉ tập phim, đặt trình phát vào iframe, rồi khi WebView tải
 * tài liệu của iframe thì app tự tải hộ và chèn assets/player-agent.js vào đầu. Agent nhận lệnh
 * (phát / dừng / tua) qua postMessage và báo trạng thái lên trang bọc; app đọc trạng thái mỗi giây.
 */
class PlayerActivity : Activity() {
    companion object {
        fun intent(context: Context, film: FilmDetail, ep: Episode): Intent =
            Intent(context, PlayerActivity::class.java).putExtra("film", film.url).putExtra("ep", ep.url)

        private val DROP_HEADERS = setOf(
            "content-security-policy", "content-security-policy-report-only", "content-length",
            "content-encoding", "transfer-encoding", "set-cookie", "content-type",
        )
    }

    private data class PlayState(
        val t: Double, val d: Double, val paused: Boolean, val ended: Boolean, val waiting: Boolean, val ready: Boolean,
    )

    private lateinit var web: WebView
    private lateinit var fullscreen: FrameLayout
    private lateinit var loading: View
    private lateinit var loadingProgress: ProgressBar
    private lateinit var loadingText: TextView
    private lateinit var osd: View
    private lateinit var osdTitle: TextView
    private lateinit var osdState: TextView
    private lateinit var osdTime: TextView
    private lateinit var osdProgress: ProgressBar
    private lateinit var panel: View
    private lateinit var panelTitle: TextView
    private lateinit var serverRow: LinearLayout
    private lateinit var notice: Notice

    private var agentJs = ""
    private var wrapperHtml = ""
    private lateinit var filmUrl: String

    private var detail: FilmDetail? = null
    private var index = 0
    private var source: EpisodeSource? = null
    private var server: Server? = null
    private var token = 0

    // Đọc từ luồng của shouldInterceptRequest
    @Volatile private var embedUrl: String? = null
    @Volatile private var episodeUrl: String = ""

    private var customView: View? = null
    private var customCallback: WebChromeClient.CustomViewCallback? = null

    private var state: PlayState? = null
    private var started = false
    private var resumeAt = 0
    private var endedHandled = false
    private var hintShown = false
    private var loadStartedAt = 0L
    private var lastSave = 0L
    private var seekTarget: Double? = null

    private val poll = object : Runnable {
        override fun run() {
            web.evaluateJavascript("window.__chimtoState||null") { onState(it) }
            mainHandler.postDelayed(this, 1000)
        }
    }
    private val hideOsd = Runnable { osd.visibility = View.GONE }
    private val flushSeek = Runnable {
        val target = seekTarget ?: return@Runnable
        seekTarget = null
        cmd("seekTo", target)
        state = state?.copy(t = target)
        updateOsd()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContentView(R.layout.activity_player)
        web = findViewById(R.id.web)
        fullscreen = findViewById(R.id.fullscreen)
        loading = findViewById(R.id.loading)
        loadingProgress = findViewById(R.id.loadingProgress)
        loadingText = findViewById(R.id.loadingText)
        osd = findViewById(R.id.osd)
        osdTitle = findViewById(R.id.osdTitle)
        osdState = findViewById(R.id.osdState)
        osdTime = findViewById(R.id.osdTime)
        osdProgress = findViewById(R.id.osdProgress)
        panel = findViewById(R.id.panel)
        panelTitle = findViewById(R.id.panelTitle)
        serverRow = findViewById(R.id.serverRow)
        notice = Notice(findViewById(R.id.notice))

        agentJs = assets.open("player-agent.js").use { it.readBytes().toString(Charsets.UTF_8) }
        wrapperHtml = assets.open("player.html").use { it.readBytes().toString(Charsets.UTF_8) }
        setupWebView()
        setupPanel()

        filmUrl = intent.getStringExtra("film")!!
        val epUrl = intent.getStringExtra("ep")!!
        val cached = Site.cachedDetail(filmUrl)
        if (cached != null) {
            start(cached, epUrl)
        } else {
            showLoading("Đang tải thông tin phim…")
            background({ Site.detail(filmUrl) }) { r ->
                r.onSuccess { start(it, epUrl) }.onFailure { showError("Không tải được phim.\n${it.friendly()}") }
            }
        }
    }

    private fun start(d: FilmDetail, epUrl: String) {
        detail = d
        openEpisode(d.episodes.indexOfFirst { it.url == epUrl }.coerceAtLeast(0))
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun setupWebView() {
        web.setBackgroundColor(Color.BLACK)
        web.isFocusable = false
        web.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            mediaPlaybackRequiresUserGesture = false
            setSupportMultipleWindows(true)
            javaScriptCanOpenWindowsAutomatically = false
            mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
            useWideViewPort = true
            loadWithOverviewMode = true
            userAgentString = AdBlock.browserUserAgent(userAgentString)
        }
        CookieManager.getInstance().setAcceptCookie(true)
        CookieManager.getInstance().setAcceptThirdPartyCookies(web, true)

        web.webViewClient = object : WebViewClient() {
            // Chặn mọi điều hướng cả trang do trang tự gây ra (quảng cáo); app tự nạp trang bằng code.
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest) = request.isForMainFrame

            @Deprecated("Deprecated in Java")
            override fun shouldOverrideUrlLoading(view: WebView, url: String) = true

            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                intercept(request)
        }
        web.webChromeClient = object : WebChromeClient() {
            override fun onCreateWindow(view: WebView, isDialog: Boolean, isUserGesture: Boolean, resultMsg: Message) = false

            override fun onShowCustomView(view: View, callback: CustomViewCallback) {
                if (customView != null) {
                    callback.onCustomViewHidden()
                    return
                }
                customView = view
                customCallback = callback
                fullscreen.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
                fullscreen.visibility = View.VISIBLE
            }

            override fun onHideCustomView() = hideCustomView()

            // Bỏ biểu tượng "play" xám mặc định của WebView
            override fun getDefaultVideoPoster(): Bitmap = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)

            // Trình phát ghi log liên tục; chép ra logcat tốn CPU của TV box
            override fun onConsoleMessage(consoleMessage: ConsoleMessage) = true
        }
    }

    private fun hideCustomView() {
        val v = customView ?: return
        fullscreen.removeView(v)
        fullscreen.visibility = View.GONE
        customView = null
        customCallback?.onCustomViewHidden()
        customCallback = null
    }

    // ---- Chèn agent vào tài liệu iframe của trình phát (chạy ở luồng nền của WebView) ----

    private fun intercept(req: WebResourceRequest): WebResourceResponse? {
        if (AdBlock.isAd(req.url.host)) return AdBlock.empty()
        if (req.isForMainFrame || req.method != "GET") return null
        val url = req.url.toString()
        val accept = req.requestHeaders.entries.firstOrNull { it.key.equals("Accept", true) }?.value.orEmpty()
        if (url != embedUrl && !accept.startsWith("text/html")) return null
        return try {
            fetchWithAgent(url, req.requestHeaders)
        } catch (e: Exception) {
            null // để WebView tự tải: video vẫn phát, chỉ mất điều khiển bằng phím
        }
    }

    private fun fetchWithAgent(url: String, requestHeaders: Map<String, String>): WebResourceResponse {
        val headers = HashMap<String, String>()
        for ((k, v) in requestHeaders) if (!k.equals("Accept-Encoding", true)) headers[k] = v
        if (url == embedUrl || headers.keys.none { it.equals("Referer", true) }) {
            headers.keys.removeAll { it.equals("Referer", true) }
            headers["Referer"] = episodeUrl
        }
        val cookies = CookieManager.getInstance()
        cookies.getCookie(url)?.let { headers["Cookie"] = it }

        val c = Http.open(url, headers)
        try {
            for ((k, values) in c.headerFields) {
                if (k != null && k.equals("Set-Cookie", true)) values.forEach { cookies.setCookie(url, it) }
            }
            val code = c.responseCode
            val type = c.contentType ?: "text/html"
            val mime = type.substringBefore(';').trim().ifEmpty { "text/html" }
            val charset = Http.charsetOf(type)
            val bytes = (if (code >= 400) c.errorStream else c.inputStream)?.use { it.readBytes() } ?: ByteArray(0)
            val isHtml = mime.contains("html")
            val body = if (isHtml) injectAgent(bytes.toString(charset)).toByteArray(Charsets.UTF_8) else bytes

            val outHeaders = HashMap<String, String>()
            for ((k, v) in c.headerFields) {
                if (k == null || v.isNullOrEmpty() || k.lowercase() in DROP_HEADERS || k.startsWith("X-Android", true)) continue
                outHeaders[k] = v.joinToString(", ")
            }
            val status = if (code in 100..299 || code in 400..599) code else 200
            val reason = c.responseMessage?.takeIf { it.isNotBlank() } ?: "OK"
            return WebResourceResponse(
                mime, if (isHtml) "utf-8" else charset.name(), status, reason, outHeaders, ByteArrayInputStream(body),
            )
        } finally {
            c.disconnect()
        }
    }

    private fun injectAgent(html: String): String {
        val tag = "<script>$agentJs</script>"
        val at = (Regex("<head(\\s[^>]*)?>", RegexOption.IGNORE_CASE).find(html)
            ?: Regex("<html(\\s[^>]*)?>", RegexOption.IGNORE_CASE).find(html))
            ?.range?.last?.plus(1) ?: 0
        return html.substring(0, at) + tag + html.substring(at)
    }

    // ---- Mở tập / server ----

    private fun openEpisode(i: Int) {
        val d = detail ?: return
        val ep = d.episodes.getOrNull(i) ?: return
        index = i
        val t = ++token
        stopVideo()
        episodeUrl = ep.url
        source = null
        serverRow.removeAllViews()
        osdTitle.text = "${d.title} — ${ep.title}"
        panelTitle.text = osdTitle.text
        resumeAt = Prefs.position(ep.url)
        Prefs.addHistory(HistoryItem(d.url, d.title, d.poster, ep.title, ep.url, System.currentTimeMillis()))
        showLoading("Đang mở ${ep.title}…")
        background({ Site.source(ep.url) }) { r ->
            if (t != token) return@background
            r.onSuccess { src ->
                source = src
                buildServerButtons(src)
                playServer(src.servers.firstOrNull { it.type == Prefs.serverType } ?: src.servers.firstOrNull())
            }.onFailure { showError("Không mở được ${ep.title}.\n${it.friendly()}") }
        }
    }

    private fun playServer(s: Server?) {
        val src = source ?: return
        server = s
        for (i in 0 until serverRow.childCount) serverRow.getChildAt(i).let { it.isActivated = it.tag == s?.type }
        val t = ++token
        stopVideo()
        showLoading("Đang tải ${s?.label ?: "trình phát"}…")
        background({ Site.embedUrl(src, s) }) { r ->
            if (t != token) return@background
            r.onSuccess { url ->
                embedUrl = url
                loadStartedAt = System.currentTimeMillis()
                web.loadDataWithBaseURL(src.epUrl, wrapperHtml.replace("{{SRC}}", htmlAttr(url)), "text/html", "utf-8", null)
            }.onFailure {
                showError("${it.friendly()}\nNhấn ▼ để chọn server khác.")
                showPanel()
            }
        }
    }

    private fun stopVideo() {
        savePosition()
        embedUrl = null
        state = null
        started = false
        endedHandled = false
        hintShown = false
        seekTarget = null
        mainHandler.removeCallbacks(flushSeek)
        web.loadUrl("about:blank")
    }

    private fun buildServerButtons(src: EpisodeSource) {
        serverRow.removeAllViews()
        val inflater = LayoutInflater.from(this)
        for (s in src.servers) {
            val b = inflater.inflate(R.layout.item_button, serverRow, false) as TextView
            b.text = s.label
            b.tag = s.type
            b.setOnClickListener {
                Prefs.serverType = s.type
                Prefs.serverLabel = s.label
                // Đổi server giữa chừng: xem tiếp từ chỗ đang xem
                state?.takeIf { started && it.t > 5 }?.let { resumeAt = it.t.toInt() }
                hidePanel()
                playServer(s)
            }
            serverRow.addView(b)
        }
    }

    private fun setupPanel() {
        findViewById<View>(R.id.btnPrev).setOnClickListener { hidePanel(); prev() }
        findViewById<View>(R.id.btnNext).setOnClickListener { hidePanel(); next() }
        findViewById<View>(R.id.btnReload).setOnClickListener {
            hidePanel()
            state?.takeIf { started && it.t > 5 }?.let { resumeAt = it.t.toInt() }
            if (source != null) playServer(server) else openEpisode(index)
        }
        findViewById<View>(R.id.btnEpisodes).setOnClickListener { finish() }
        findViewById<View>(R.id.btnWebPage).setOnClickListener {
            hidePanel()
            startActivity(WebActivity.intent(this, episodeUrl))
        }
        findViewById<View>(R.id.btnClose).setOnClickListener { hidePanel() }
    }

    private fun next() {
        val d = detail ?: return
        if (index + 1 < d.episodes.size) openEpisode(index + 1) else notice.show("Đây là tập mới nhất.")
    }

    private fun prev() {
        if (index > 0) openEpisode(index - 1) else notice.show("Đây là tập đầu tiên.")
    }

    // ---- Trạng thái phát ----

    private fun onState(json: String?) {
        if (json == null || json == "null") {
            val waited = System.currentTimeMillis() - loadStartedAt
            if (!started && embedUrl != null && !hintShown && waited > 25_000) {
                hintShown = true
                hideLoading()
                notice.show("Video chưa chạy? Nhấn OK để thử phát, hoặc ▼ để đổi server.", 10_000)
            }
            return
        }
        val o = try {
            JSONObject(json)
        } catch (e: Exception) {
            return
        }
        val s = PlayState(
            o.optDouble("t", 0.0), o.optDouble("d", 0.0), o.optBoolean("paused"),
            o.optBoolean("ended"), o.optBoolean("waiting"), o.optBoolean("ready"),
        )
        val wasPaused = state?.paused
        state = s
        if (!started && s.ready) {
            started = true
            hideLoading()
            if (resumeAt > 30 && (s.d <= 0 || resumeAt < s.d - 30)) {
                cmd("seekTo", resumeAt)
                notice.show("Xem tiếp từ ${formatTime(resumeAt.toDouble())}")
            }
            showOsd()
        }
        if (osd.visibility == View.VISIBLE) updateOsd()
        if (wasPaused != null && wasPaused != s.paused) showOsd()

        val now = System.currentTimeMillis()
        if (started && now - lastSave > 5000) {
            lastSave = now
            savePosition()
        }
        if (s.ended && !endedHandled) {
            endedHandled = true
            val d = detail
            if (Prefs.autoNext && d != null && index + 1 < d.episodes.size) {
                notice.show("Hết tập — chuyển sang ${d.episodes[index + 1].title}")
                openEpisode(index + 1)
            }
        }
    }

    private fun savePosition() {
        val s = state ?: return
        if (!started || episodeUrl.isEmpty()) return
        // Xem gần hết thì coi như xong (lần sau mở lại từ đầu)
        val done = s.ended || (s.d > 0 && s.t > s.d - 90)
        Prefs.savePosition(episodeUrl, if (done) 0 else s.t.toInt())
    }

    private fun cmd(name: String, arg: Number? = null) {
        web.evaluateJavascript("window.chimtoCmd&&chimtoCmd('$name',${arg ?: "null"})", null)
    }

    // ---- Phím remote / bàn phím ----

    override fun dispatchKeyEvent(e: KeyEvent): Boolean {
        val code = e.keyCode
        if (panel.visibility == View.VISIBLE) {
            if (code == KeyEvent.KEYCODE_BACK || code == KeyEvent.KEYCODE_ESCAPE || code == KeyEvent.KEYCODE_MENU) {
                if (e.action == KeyEvent.ACTION_UP) hidePanel()
                return true
            }
            return super.dispatchKeyEvent(e)
        }
        if (!isPlayerKey(code)) return super.dispatchKeyEvent(e)
        if (e.action != KeyEvent.ACTION_DOWN) return true
        val first = e.repeatCount == 0
        when (code) {
            KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.KEYCODE_SPACE,
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.KEYCODE_HEADSETHOOK, KeyEvent.KEYCODE_K,
            -> if (first) {
                cmd(if (started) "toggle" else "play")
                showOsd()
            }
            KeyEvent.KEYCODE_MEDIA_PLAY -> if (first) cmd("play")
            KeyEvent.KEYCODE_MEDIA_PAUSE, KeyEvent.KEYCODE_MEDIA_STOP -> if (first) cmd("pause")
            KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_MEDIA_REWIND, KeyEvent.KEYCODE_J -> seekBy(-step(e.repeatCount))
            KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_MEDIA_FAST_FORWARD, KeyEvent.KEYCODE_L -> seekBy(step(e.repeatCount))
            KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_INFO, KeyEvent.KEYCODE_I -> showOsd(6000)
            KeyEvent.KEYCODE_DPAD_DOWN, KeyEvent.KEYCODE_MENU, KeyEvent.KEYCODE_M -> if (first) showPanel()
            KeyEvent.KEYCODE_CHANNEL_UP, KeyEvent.KEYCODE_PAGE_DOWN, KeyEvent.KEYCODE_MEDIA_NEXT, KeyEvent.KEYCODE_N,
            -> if (first) next()
            KeyEvent.KEYCODE_CHANNEL_DOWN, KeyEvent.KEYCODE_PAGE_UP, KeyEvent.KEYCODE_MEDIA_PREVIOUS, KeyEvent.KEYCODE_P,
            -> if (first) prev()
            in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 -> if (first) {
                val s = state
                if (s != null && s.d > 0) {
                    seekTarget = s.d * (code - KeyEvent.KEYCODE_0) / 10.0
                    showOsd()
                    mainHandler.removeCallbacks(flushSeek)
                    mainHandler.post(flushSeek)
                }
            }
        }
        return true
    }

    private fun isPlayerKey(code: Int) = when (code) {
        KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.KEYCODE_SPACE,
        KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.KEYCODE_HEADSETHOOK, KeyEvent.KEYCODE_K,
        KeyEvent.KEYCODE_MEDIA_PLAY, KeyEvent.KEYCODE_MEDIA_PAUSE, KeyEvent.KEYCODE_MEDIA_STOP,
        KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_MEDIA_REWIND, KeyEvent.KEYCODE_J,
        KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_MEDIA_FAST_FORWARD, KeyEvent.KEYCODE_L,
        KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_INFO, KeyEvent.KEYCODE_I,
        KeyEvent.KEYCODE_DPAD_DOWN, KeyEvent.KEYCODE_MENU, KeyEvent.KEYCODE_M,
        KeyEvent.KEYCODE_CHANNEL_UP, KeyEvent.KEYCODE_PAGE_DOWN, KeyEvent.KEYCODE_MEDIA_NEXT, KeyEvent.KEYCODE_N,
        KeyEvent.KEYCODE_CHANNEL_DOWN, KeyEvent.KEYCODE_PAGE_UP, KeyEvent.KEYCODE_MEDIA_PREVIOUS, KeyEvent.KEYCODE_P,
        -> true
        else -> code in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9
    }

    /** Giữ phím tua thì bước tua tăng dần (tối đa 4 lần). */
    private fun step(repeat: Int) = Prefs.seekStep * min(4.0, 1 + repeat / 8.0)

    /** Gộp các lần bấm tua liên tiếp thành một lệnh tua (đỡ giật khi giữ phím). */
    private fun seekBy(delta: Double) {
        val s = state ?: return
        if (!started) return
        val max = if (s.d > 0) s.d - 1 else Double.MAX_VALUE
        seekTarget = ((seekTarget ?: s.t) + delta).coerceIn(0.0, max)
        showOsd()
        mainHandler.removeCallbacks(flushSeek)
        mainHandler.postDelayed(flushSeek, 500)
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        if (customView != null) {
            hideCustomView()
            return
        }
        super.onBackPressed()
    }

    // ---- Giao diện phủ ----

    private fun showOsd(ms: Long = 4000) {
        osd.visibility = View.VISIBLE
        updateOsd()
        osd.removeCallbacks(hideOsd)
        // Đang dừng thì giữ thanh thời gian trên màn hình
        if (state?.paused != true || seekTarget != null) osd.postDelayed(hideOsd, ms)
    }

    private fun updateOsd() {
        val s = state
        val target = seekTarget
        val t = target ?: s?.t ?: 0.0
        val d = s?.d ?: 0.0
        osdTime.text = if (d > 0) "${formatTime(t)} / ${formatTime(d)}" else formatTime(t)
        osdProgress.progress = if (d > 0) (t / d * 1000).toInt() else 0
        osdState.text = when {
            s == null -> "Đang tải"
            target != null -> if (target >= s.t) "Tua tới" else "Tua lùi"
            s.waiting -> "Đang tải"
            s.paused -> "Tạm dừng"
            else -> "Đang phát"
        }
    }

    private fun showPanel() {
        osd.visibility = View.GONE
        panel.visibility = View.VISIBLE
        val active = (0 until serverRow.childCount).map { serverRow.getChildAt(it) }.firstOrNull { it.isActivated }
        (active ?: findViewById(R.id.btnNext)).requestFocus()
    }

    private fun hidePanel() {
        panel.visibility = View.GONE
    }

    private fun showLoading(text: String) {
        loading.visibility = View.VISIBLE
        loadingProgress.visibility = View.VISIBLE
        loadingText.text = text
    }

    private fun showError(text: String) {
        loading.visibility = View.VISIBLE
        loadingProgress.visibility = View.GONE
        loadingText.text = text
    }

    private fun hideLoading() {
        loading.visibility = View.GONE
    }

    private fun htmlAttr(s: String) =
        s.replace("&", "&amp;").replace("\"", "&quot;").replace("<", "&lt;").replace(">", "&gt;")

    // ---- Vòng đời ----

    override fun onResume() {
        super.onResume()
        web.onResume()
        mainHandler.removeCallbacks(poll)
        mainHandler.post(poll)
    }

    override fun onPause() {
        savePosition()
        cmd("pause")
        mainHandler.removeCallbacks(poll)
        web.onPause()
        super.onPause()
    }

    override fun onDestroy() {
        mainHandler.removeCallbacks(poll)
        mainHandler.removeCallbacks(flushSeek)
        token++
        web.stopLoading()
        web.destroy()
        super.onDestroy()
    }
}
