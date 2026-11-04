package vn.chimto.tv

import android.annotation.SuppressLint
import android.annotation.TargetApi
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Message
import android.util.Log
import android.view.GestureDetector
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.MotionEvent
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
import android.widget.SeekBar
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayInputStream
import kotlin.math.abs
import kotlin.math.min

/**
 * Phát một tập phim.
 *
 * Trình phát của trang chỉ chạy khi nằm trong iframe của chính trang phim, nên app dựng một trang bọc
 * (assets/player.html) với origin = địa chỉ tập phim, đặt trình phát vào iframe, rồi khi WebView tải
 * tài liệu của iframe thì app tự tải hộ và chèn assets/player-agent.js vào đầu. Agent nhận lệnh
 * (phát / dừng / tua) qua postMessage và báo trạng thái lên trang bọc; app đọc trạng thái mỗi giây.
 * Server cho link HLS trực tiếp (.m3u8, trang Next.js kiểu PhimMoi): app phát bằng trang riêng assets/hls.html
 * (hls.js đóng kèm, bỏ quảng cáo chèn giữa phim), agent nằm ngay trong trang đó.
 */
class PlayerActivity : BaseActivity() {
    // Có thanh điều khiển cảm ứng riêng (có nút Thoát), khỏi nút "‹ Quay lại" chung
    override val touchBackLabel: String? = null

    companion object {
        fun intent(context: Context, film: FilmDetail, ep: Episode): Intent =
            Intent(context, PlayerActivity::class.java).putExtra("film", film.url).putExtra("ep", ep.url)

        /** Hàng "Xem tiếp": mở thẳng tập trong lịch sử; tập đó đã xem hết thì mở tập kế tiếp. */
        fun resume(context: Context, item: HistoryItem): Intent =
            Intent(context, PlayerActivity::class.java).putExtra("film", item.filmUrl).putExtra("ep", item.epUrl)
                .putExtra("continue", true)

        private val RATES = listOf(1.0, 1.25, 1.5, 2.0, 0.75)

        /** Hẹn giờ tắt (phút); -1 = hết tập đang xem. */
        private val SLEEP = listOf(0, 30, 60, 90, -1)

        private fun rateText(r: Double) = (if (r % 1.0 == 0.0) "${r.toInt()}" else r.toString().replace('.', ',')) + "×"

        /** hls.html tải hls.js từ đường dẫn này (cùng origin với trang tập), app trả tệp trong assets. */
        private const val HLS_JS = "/__chimto/hls.js"

        private val DROP_HEADERS = setOf(
            "content-security-policy", "content-security-policy-report-only", "content-length",
            "content-encoding", "transfer-encoding", "set-cookie", "content-type",
        )
    }

    private data class PlayState(
        val t: Double, val d: Double, val paused: Boolean, val ended: Boolean, val waiting: Boolean, val ready: Boolean,
        val droppedFrames: Int, val totalFrames: Int,
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
    private lateinit var serverLabel: TextView
    private var serverHint: CharSequence = ""
    private lateinit var groupRow: LinearLayout
    private lateinit var groupLabel: View
    private lateinit var notice: Notice
    private lateinit var introLabel: TextView
    private lateinit var btnIntroStart: TextView
    private lateinit var btnIntroEnd: TextView
    private lateinit var btnIntroClear: TextView
    private lateinit var btnSpeed: TextView
    private lateinit var touchLayer: View
    private lateinit var touchBar: View
    private lateinit var touchTitle: TextView
    private lateinit var touchTime: TextView
    private lateinit var touchDuration: TextView
    private lateinit var touchSeek: SeekBar
    private lateinit var touchPlay: TextView
    private var touchDragging = false
    private val hideTouchBar = Runnable { touchBar.visibility = View.INVISIBLE }
    private lateinit var btnFit: TextView
    private lateinit var btnSleep: TextView

    private var agentJs = ""
    private var focusJs = ""
    private var wrapperHtml = ""
    private var hlsHtml = ""
    private lateinit var filmUrl: String

    private var detail: FilmDetail? = null
    private var group: EpisodeGroup? = null
    private val episodes get() = group?.episodes.orEmpty()
    private var index = 0

    // Chế độ mở thẳng trang tập (trang có trình phát tự xác thực, vd hoathinh3d.pw)
    private var pageMode = false
    private var source: EpisodeSource? = null
    private var server: Server? = null
    private var token = 0

    /**
     * Số thứ tự lần nạp trang phát (gắn vào trang: window.__chimtoLoad). Chuyển tập trên TV chậm, trang cũ còn trả lời
     * vài giây sau khi đã chuyển — trạng thái của nó (thời gian tập cũ) không được tính cho tập mới.
     */
    private var pageLoad = 0

    // Đọc từ luồng của shouldInterceptRequest
    @Volatile private var embedUrl: String? = null
    @Volatile private var episodeUrl: String = ""

    // Đang phát link HLS bằng hls.html (không qua iframe)
    private var hlsUrl: String? = null
    private var errShown = false
    private var lastAdAt = 0L

    private var customView: View? = null
    private var customCallback: WebChromeClient.CustomViewCallback? = null

    private var state: PlayState? = null
    private var started = false
    private var resumeAt = 0

    /**
     * Đã ra lệnh tua tới chỗ xem dở (resumeAt) mà trình phát chưa tua xong — TV chậm có khi mất hàng chục giây. Trong
     * lúc đó thời gian trình phát báo là giây đầu tập: không được lưu đè / mang sang thay cho chỗ xem dở.
     */
    private var resumePending = false
    private var resumeIssuedAt = 0L
    private var endedHandled = false
    private var hintShown = false
    private var loadStartedAt = 0L
    private var lastSave = 0L
    private var seekTarget: Double? = null

    /** Tốc độ phát (cả lần xem này, mọi tập), mặc định 1×. */
    private var rate = 1.0

    /** Hẹn giờ tắt: lựa chọn (chỉ số trong SLEEP), giờ hết hẹn; hết giờ thì dừng phim, chờ 60 giây rồi thoát ra màn hình chính TV. */
    private var sleepChoice = 0
    private var sleepAt = 0L
    private var sleepGraceUntil = 0L

    /**
     * Chỗ bắt đầu báo trước cho trình phát của trang nạp tiếp theo (giây; 0 = đầu tập): chỗ xem dở, hoặc chỗ hết nhạc mở
     * đầu khi nhạc nằm ngay đầu tập. Đọc từ luồng của shouldInterceptRequest (chèn vào agent).
     */
    @Volatile private var startHint = 0

    /** Tập này đã qua đoạn nhạc mở đầu (đã tự tua qua, xem tiếp từ giữa, hoặc tự xem qua). */
    private var introDone = false

    // Chấm điểm server: lần phát hiện tại + gợi ý đổi server khi giật nhiều (OK để chuyển)
    private var session: PlaySession? = null
    private var suggested = false
    private var suggestion: Server? = null
    private var suggestionUntil = 0L

    private val poll = object : Runnable {
        override fun run() {
            web.evaluateJavascript("[window.__chimtoLoad||0,window.__chimtoState||null]") { onState(it) }
            mainHandler.postDelayed(this, 1000)
        }
    }
    private val hideOsd = Runnable { osd.visibility = View.GONE }
    private val flushSeek = Runnable {
        val target = seekTarget ?: return@Runnable
        seekTarget = null
        resumePending = false // người xem tự tua: từ giờ lấy theo trình phát
        session?.seeked()
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
        serverLabel = findViewById(R.id.serverLabel)
        serverHint = serverLabel.text
        groupRow = findViewById(R.id.groupRow)
        groupLabel = findViewById(R.id.groupLabel)
        notice = Notice(findViewById(R.id.notice))
        introLabel = findViewById(R.id.introLabel)
        btnIntroStart = findViewById(R.id.btnIntroStart)
        btnIntroEnd = findViewById(R.id.btnIntroEnd)
        btnIntroClear = findViewById(R.id.btnIntroClear)
        btnSpeed = findViewById(R.id.btnSpeed)
        touchLayer = findViewById(R.id.touchLayer)
        touchBar = findViewById(R.id.touchBar)
        touchTitle = findViewById(R.id.touchTitle)
        touchTime = findViewById(R.id.touchTime)
        touchDuration = findViewById(R.id.touchDuration)
        touchSeek = findViewById(R.id.touchSeek)
        touchPlay = findViewById(R.id.touchPlay)
        btnFit = findViewById(R.id.btnFit)
        btnSleep = findViewById(R.id.btnSleep)

        agentJs = assets.open("player-agent.js").use { it.readBytes().toString(Charsets.UTF_8) }
        focusJs = assets.open("page-focus.js").use { it.readBytes().toString(Charsets.UTF_8) }
        wrapperHtml = assets.open("player.html").use { it.readBytes().toString(Charsets.UTF_8) }
        hlsHtml = assets.open("hls.html").use { it.readBytes().toString(Charsets.UTF_8) }
        setupWebView()
        setupPanel()
        setupTouch()

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
        // Phim đang theo dõi: đã vào xem thì không còn là "tập mới"
        d.latestEpisode?.let { Prefs.updateFollow(d.url, d.episodeCount, it.title, seen = true) }
        val g = d.groupOf(epUrl) ?: d.groups.firstOrNull() ?: return showError("Phim này chưa có tập nào.")
        group = g
        buildGroupButtons(d)
        val i = g.episodes.indexOfFirst { it.url == epUrl }.coerceAtLeast(0)
        val next = intent.getBooleanExtra("continue", false) && Prefs.finished(epUrl) && i + 1 < g.episodes.size
        openEpisode(if (next) i + 1 else i)
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
        }
        Privacy.apply(web)
        CookieManager.getInstance().setAcceptCookie(true)
        CookieManager.getInstance().setAcceptThirdPartyCookies(web, true)

        web.webViewClient = object : WebViewClient() {
            // Chặn mọi điều hướng cả trang do trang tự gây ra (quảng cáo); app tự nạp trang bằng code.
            // Chuyển hướng phía máy chủ (vd http → https của trang tập) thì cho qua.
            @TargetApi(Build.VERSION_CODES.N) // Android 5–6 gọi bản (WebView, String) bên dưới
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest) =
                request.isForMainFrame && !request.isRedirect

            @Deprecated("Deprecated in Java")
            override fun shouldOverrideUrlLoading(view: WebView, url: String) = true

            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                intercept(request)

            // Chế độ trang tập: phóng trình phát ra toàn màn hình + agent ở khung trên cùng
            override fun onPageCommitVisible(view: WebView, url: String) = injectPageScripts(url)

            override fun onPageFinished(view: WebView, url: String) = injectPageScripts(url)
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
        if (req.url.path == HLS_JS) return WebResourceResponse("application/javascript", "utf-8", assets.open("hls.light.min.js"))
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
        trace("bắt đầu tải khung trình phát")
        val headers = HashMap<String, String>()
        for ((k, v) in requestHeaders) {
            if (k.equals("Accept-Encoding", true)) continue
            // Header WebView chép sang có thể còn tên gói app — không gửi (xem Privacy); "XMLHttpRequest" thì giữ
            if (k.equals("X-Requested-With", true) && v == packageName) continue
            headers[k] = v
        }
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
        val tag = "<script>window.__chimtoStart=$startHint;\n$agentJs</script>"
        val at = (Regex("<head(\\s[^>]*)?>", RegexOption.IGNORE_CASE).find(html)
            ?: Regex("<html(\\s[^>]*)?>", RegexOption.IGNORE_CASE).find(html))
            ?.range?.last?.plus(1) ?: 0
        return html.substring(0, at) + tag + html.substring(at)
    }

    // ---- Mở tập / server ----

    /** Mốc thời gian lúc mở tập (chỉ bản debug, logcat ChimToOpen) — đo xem mở phim chậm ở bước nào. */
    private var openAt = 0L

    private fun trace(step: String) {
        if (BuildConfig.DEBUG) Log.d("ChimToOpen", "$step ${System.currentTimeMillis() - openAt} ms")
    }

    private fun openEpisode(i: Int) {
        openAt = System.currentTimeMillis()
        val d = detail ?: return
        val ep = episodes.getOrNull(i) ?: return
        index = i
        val t = ++token
        stopVideo()
        episodeUrl = ep.url
        source = null
        serverRow.removeAllViews()
        val groupName = group?.name?.takeIf { d.groups.size > 1 }?.let { " ($it)" } ?: ""
        osdTitle.text = "${d.title} — ${ep.title}$groupName"
        panelTitle.text = osdTitle.text
        for (j in 0 until groupRow.childCount) groupRow.getChildAt(j).let { it.isActivated = it.tag == group?.name }
        resumeAt = Prefs.position(ep.url)
        introDone = false
        if (!Prefs.privateMode) Prefs.addHistory(HistoryItem(d.url, d.title, d.poster, ep.title, ep.url, System.currentTimeMillis()))
        showLoading("Đang mở ${ep.title}…")
        background({ Site.source(ep.url) }) { r ->
            trace("trang tập")
            if (t != token) return@background
            r.onSuccess { src ->
                source = src
                buildServerButtons(src)
                if (src.pageMode) {
                    playPage(src)
                } else {
                    val host = Site.hostOf(src.epUrl)
                    val stats = AppProxy.statsHost(host)
                    val usual = src.servers.firstOrNull { it.type == Prefs.serverType(host) } ?: src.servers.firstOrNull()
                    val pick = PlayStats.choose(stats, src.servers, usual)
                    if (usual != null && pick != usual) {
                        val why = PlayStats.score(stats, usual.type).label ?: "hay giật"
                        notice.show("${usual.label} $why gần đây — mở ${pick?.label} (▼ để đổi server).", 6000)
                    }
                    playServer(pick)
                }
            }.onFailure { showError("Không mở được ${ep.title}.\n${it.friendly()}") }
        }
    }

    /** Mở thẳng trang tập: trang tự nạp trình phát, injectPageScripts() phóng nó ra toàn màn hình. */
    private fun playPage(src: EpisodeSource) {
        stopVideo()
        pageMode = true
        loadStartedAt = System.currentTimeMillis()
        session = PlaySession(AppProxy.statsHost(Site.hostOf(src.epUrl)), "page", loadStartedAt)
        showLoading("Đang mở trang tập phim…")
        startHint = startFor()
        web.loadUrl(src.epUrl)
    }

    private fun injectPageScripts(url: String) {
        if (!pageMode || !url.startsWith("http")) return
        web.evaluateJavascript("window.__chimtoLoad=$pageLoad;\n" + focusJs + "\n" + agentJs, null)
    }

    private fun playServer(s: Server?) {
        val src = source ?: return
        server = s
        for (i in 0 until serverRow.childCount) serverRow.getChildAt(i).let { it.isActivated = it.tag == s?.type }
        val t = ++token
        stopVideo()
        showLoading("Đang tải ${s?.label ?: "trình phát"}…")
        background({ Site.video(src, s) }) { r ->
            trace("link trình phát")
            if (t != token) return@background
            r.onSuccess { v ->
                val url = v.url
                loadStartedAt = System.currentTimeMillis()
                session = PlaySession(AppProxy.statsHost(Site.hostOf(src.epUrl)), s?.type ?: "embed", loadStartedAt)
                if (Uri.parse(url).path.orEmpty().endsWith(".m3u8")) {
                    hlsUrl = url
                    startHint = startFor()
                    val page = hlsHtml.replace("{{SRC}}", JSONObject.quote(url)).replace("{{START}}", startHint.toString())
                        .replace("{{SUB}}", v.subtitle?.let { JSONObject.quote(it) } ?: "null")
                        .replace("{{AGENT}}", agentJs)
                    web.loadDataWithBaseURL(src.epUrl, marked(page), "text/html", "utf-8", null)
                } else {
                    embedUrl = url
                    startHint = startFor()
                    web.loadDataWithBaseURL(src.epUrl, marked(wrapperHtml.replace("{{SRC}}", htmlAttr(url))), "text/html", "utf-8", null)
                }
            }.onFailure {
                showError("${it.friendly()}\nNhấn ▼ để chọn server khác.")
                showPanel()
            }
        }
    }

    private fun stopVideo() {
        savePosition()
        session?.save(System.currentTimeMillis())
        session = null
        suggested = false
        dismissSuggestion()
        pageMode = false
        embedUrl = null
        hlsUrl = null
        errShown = false
        lastAdAt = 0
        state = null
        started = false
        resumePending = false
        endedHandled = false
        hintShown = false
        seekTarget = null
        mainHandler.removeCallbacks(flushSeek)
        pageLoad++
        web.loadUrl("about:blank")
    }

    private fun marked(html: String) = html.replaceFirst("<head>", "<head><script>window.__chimtoLoad=$pageLoad</script>")

    private fun buildServerButtons(src: EpisodeSource) {
        serverRow.removeAllViews()
        val servers = if (src.pageMode) emptyList() else src.servers
        serverLabel.visibility = if (servers.isEmpty()) View.GONE else View.VISIBLE
        val inflater = LayoutInflater.from(this)
        val host = AppProxy.statsHost(Site.hostOf(src.epUrl))
        for (s in servers) {
            val b = inflater.inflate(R.layout.item_button, serverRow, false) as TextView
            b.text = serverText(host, s)
            b.tag = s.type
            b.setOnClickListener { switchServer(s) }
            serverRow.addView(b)
        }
    }

    /** Tên server kèm điểm các lần xem trước, vd "VIP  ·  mượt, mở 4 s". */
    private fun serverText(host: String?, s: Server) =
        PlayStats.score(host, s.type).summary?.let { "${s.label}  ·  $it" } ?: s.label

    /** Người xem chọn server khác (nút trong menu ▼ hoặc bấm OK ở gợi ý): nhớ lựa chọn, xem tiếp từ chỗ đang xem. */
    private fun switchServer(s: Server) {
        val src = source ?: return
        Prefs.setServer(Site.hostOf(src.epUrl), s.type, s.label)
        currentTime()?.let { resumeAt = it }
        hidePanel()
        playServer(s)
    }

    /** Server đang giật nhiều → gợi ý server khác (chấm tốt hơn hoặc chưa thử), người xem bấm OK mới chuyển. */
    private fun suggestServer(now: Long) {
        suggested = true // mỗi lần phát chỉ gợi ý một lần (không có server khác thì thôi)
        val src = source?.takeIf { !it.pageMode } ?: return
        val cur = server ?: return
        val (alt, score) = PlayStats.alternative(AppProxy.statsHost(Site.hostOf(src.epUrl)), src.servers, cur) ?: return
        suggestion = alt
        suggestionUntil = now + 15_000
        val why = score.summary?.let { " ($it)" } ?: ""
        notice.show("${cur.label} đang giật nhiều.\nNhấn OK để chuyển sang ${alt.label}$why  •  Back: bỏ qua", 15_000)
    }

    private fun dismissSuggestion() {
        if (suggestion == null) return
        suggestion = null
        notice.hide()
    }

    /** Nút đổi bản (Việt Sub / Thuyết Minh…) khi trang chia tập theo nhóm. */
    private fun buildGroupButtons(d: FilmDetail) {
        groupRow.removeAllViews()
        groupLabel.visibility = if (d.groups.size > 1) View.VISIBLE else View.GONE
        if (d.groups.size < 2) return
        val inflater = LayoutInflater.from(this)
        for (g in d.groups) {
            val b = inflater.inflate(R.layout.item_button, groupRow, false) as TextView
            b.text = g.name
            b.tag = g.name
            b.setOnClickListener {
                hidePanel()
                if (g == group) return@setOnClickListener
                // Sang bản khác: mở tập cùng tên (vd "Tập 165") và xem tiếp đúng chỗ đang xem. Bản đó chưa có tập này
                // (vd Thuyết Minh mới tới Tập 107) thì mở tập cùng vị trí / tập cuối của bản đó, không mang giờ sang
                val cur = episodes.getOrNull(index)
                val same = g.episodes.indexOfFirst { it.title == cur?.title }
                val target = same.takeIf { it >= 0 } ?: index.coerceAtMost(g.episodes.size - 1)
                if (same >= 0) {
                    currentTime()?.let { Prefs.savePosition(g.episodes[target].url, it, d.url) }
                } else {
                    notice.show("${g.name} chưa có ${cur?.title} — mở ${g.episodes[target].title}.", 6000)
                }
                group = g
                openEpisode(target)
            }
            groupRow.addView(b)
        }
    }

    private fun setupPanel() {
        findViewById<View>(R.id.btnPrev).setOnClickListener { hidePanel(); prev() }
        findViewById<View>(R.id.btnNext).setOnClickListener { hidePanel(); next() }
        findViewById<View>(R.id.btnReload).setOnClickListener {
            hidePanel()
            currentTime()?.let { resumeAt = it }
            val src = source
            when {
                src == null -> openEpisode(index)
                src.pageMode -> playPage(src)
                else -> playServer(server)
            }
        }
        findViewById<View>(R.id.btnEpisodes).setOnClickListener { finish() }
        findViewById<View>(R.id.btnWebPage).setOnClickListener {
            hidePanel()
            startActivity(WebActivity.intent(this, episodeUrl))
        }
        findViewById<View>(R.id.btnClose).setOnClickListener { hidePanel() }
        btnSpeed.setOnClickListener {
            rate = RATES[(RATES.indexOf(rate) + 1) % RATES.size]
            cmd("rate", rate)
            showViewButtons()
        }
        btnFit.setOnClickListener {
            Prefs.videoFill = !Prefs.videoFill
            cmd("fit", if (Prefs.videoFill) 1 else 0)
            showViewButtons()
        }
        btnSleep.setOnClickListener {
            sleepChoice = (sleepChoice + 1) % SLEEP.size
            val min = SLEEP[sleepChoice]
            sleepAt = if (min > 0) System.currentTimeMillis() + min * 60_000L else 0
            showViewButtons()
        }
    }

    private fun showViewButtons() {
        btnSpeed.text = "Tốc độ: ${rateText(rate)}"
        btnFit.text = if (Prefs.videoFill) "Khung hình: Lấp đầy" else "Khung hình: Vừa màn hình"
        btnSleep.text = "Hẹn giờ tắt: " + when {
            sleepAt > 0 -> "còn ${((sleepAt - System.currentTimeMillis()) / 60_000 + 1).coerceAtLeast(1)} phút"
            SLEEP[sleepChoice] < 0 -> "hết tập này"
            else -> "Tắt"
        }
    }

    /** Hết giờ hẹn: dừng phim, cho 60 giây bấm phím bất kỳ để xem tiếp. */
    private fun sleepNow() {
        sleepAt = 0
        sleepChoice = 0
        cmd("pause")
        sleepGraceUntil = System.currentTimeMillis() + 60_000
        notice.show("Hết giờ hẹn tắt. Nhấn phím bất kỳ trong 60 giây để xem tiếp, không thì app tự thoát.", 60_000)
    }

    /** Không ai bấm: lưu chỗ xem, thoát ra màn hình chính của TV (TV tự tắt màn hình / bảo vệ màn hình như thường). */
    private fun exitForSleep() {
        finish()
        startActivity(Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    private fun next() {
        if (index + 1 < episodes.size) openEpisode(index + 1) else notice.show("Đây là tập mới nhất.")
    }

    private fun prev() {
        if (index > 0) openEpisode(index - 1) else notice.show("Đây là tập đầu tiên.")
    }

    // ---- Trạng thái phát ----

    private fun onState(raw: String?) {
        // [lần nạp trang, trạng thái]: trạng thái của trang tập cũ (còn trả lời ngay sau khi chuyển tập) thì bỏ
        val json = try {
            JSONArray(raw).takeIf { it.optInt(0) == pageLoad }?.optJSONObject(1)?.toString()
        } catch (e: Exception) {
            null
        }
        // Mở 25 giây mà video chưa chạy (chưa nạp, hoặc nạp rồi mà đứng yên). Không tính lúc trình phát chạy quảng
        // cáo (đếm lại từ khi hết quảng cáo), không đè lên thông báo lỗi server đang hiện.
        val inAd = json?.contains("\"ad\":true") == true
        if (inAd) lastAdAt = System.currentTimeMillis()
        val waited = System.currentTimeMillis() - maxOf(loadStartedAt, lastAdAt)
        if (!started && !errShown && !hintShown && !inAd && (embedUrl != null || hlsUrl != null || pageMode) && waited > 25_000) {
            hintShown = true
            hideLoading()
            val other = if (pageMode) "▼ → Tải lại / Mở trang Web" else "▼ để đổi server"
            notice.show("Video chưa chạy? Nhấn OK để thử phát, hoặc $other." + AppProxy.videoNote(), 10_000)
        }
        if (json == null || json == "null") return
        val o = try {
            JSONObject(json)
        } catch (e: Exception) {
            return
        }
        if (inAd) {
            // Trình phát đang chạy quảng cáo (agent tự bỏ qua khi được phép) — chưa phải phim
            session?.ad()
            if (!started) showLoading("Trình phát đang chạy quảng cáo — tự bỏ qua khi được phép…")
            return
        }
        val s = PlayState(
            o.optDouble("t", 0.0), o.optDouble("d", 0.0), o.optBoolean("paused"),
            o.optBoolean("ended"), o.optBoolean("waiting"), o.optBoolean("ready"),
            o.optInt("df"), o.optInt("tf"),
        )
        val wasPaused = state?.paused
        if (s.ready && state?.ready != true && !started) trace("trình phát sẵn sàng t=${s.t.toInt()}")
        state = s
        val now = System.currentTimeMillis()
        val err = o.optString("err")
        if (err.isNotEmpty() && !errShown) {
            errShown = true
            session?.fail()
            if (started) {
                notice.show("Mất kết nối video ($err).\nNhấn ▼ → Tải lại, hoặc chọn server khác." + AppProxy.videoNote(), 10_000)
            } else {
                showError("${server?.label ?: "Server"} không phát được video ($err).\nNhấn ▼ để chọn server khác." + AppProxy.videoNote())
                showPanel()
            }
        }
        session?.let {
            it.update(now, s.paused, s.waiting, s.ready, s.droppedFrames, s.totalFrames)
            if (!suggested && it.struggling(now)) suggestServer(now)
        }
        // Bắt đầu khi video thật sự chạy (không phải lúc mới nạp xong mà còn dừng: trình phát như NguonC còn chạy
        // quảng cáo rồi phát lại từ đầu, tua xem tiếp lúc đó sẽ mất)
        if (!started && s.ready && !s.paused) {
            started = true
            trace("video chạy t=${s.t.toInt()}, báo trước $startHint")
            hideLoading()
            if (resumeAt > 30 && (s.d <= 0 || resumeAt < s.d - 30)) {
                introDone = true // xem tiếp đúng chỗ, không tua thêm qua nhạc mở đầu
                // Trình phát đã bắt đầu đúng chỗ (app báo trước) thì khỏi tua lại
                if (abs(s.t - resumeAt) > 10) {
                    resumePending = true
                    resumeIssuedAt = now
                    session?.seeked()
                    cmd("seekTo", resumeAt)
                }
                notice.show("Xem tiếp từ ${formatTime(resumeAt.toDouble())}")
            } else if (startHint > 0 && s.t >= startHint - 10) {
                // Bắt đầu thẳng ở chỗ hết nhạc mở đầu (app báo trước)
                introDone = true
                notice.show("Đã bỏ qua nhạc mở đầu (tới ${formatTime(startHint.toDouble())}) — ◀ để xem lại.")
            }
            // Tốc độ / khung hình đã chọn: trang tập mới chưa biết
            if (rate != 1.0) cmd("rate", rate)
            if (Prefs.videoFill) cmd("fit", 1)
            showOsd()
        }
        if (sleepAt in 1..now) sleepNow()
        if (sleepGraceUntil in 1..now) {
            sleepGraceUntil = 0
            exitForSleep()
        }
        if (resumePending && (s.t >= resumeAt - 5 || now - resumeIssuedAt > 30_000)) resumePending = false
        if (started && !introDone && !s.paused) skipIntro(s)
        if (osd.visibility == View.VISIBLE) updateOsd()
        if (touchBar.visibility == View.VISIBLE) updateTouchBar()
        if (wasPaused != null && wasPaused != s.paused) showOsd()

        if (started && now - lastSave > 5000) {
            lastSave = now
            savePosition()
        }
        if (s.ended && !endedHandled) {
            endedHandled = true
            if (SLEEP[sleepChoice] < 0) {
                sleepNow()
            } else if (Prefs.autoNext && index + 1 < episodes.size) {
                notice.show("Hết tập — chuyển sang ${episodes[index + 1].title}")
                openEpisode(index + 1)
            }
        }
    }

    private val introKey get() = detail?.url ?: filmUrl

    /**
     * Phim đã đánh dấu nhạc mở đầu: video vừa chạy tới mốc đầu (trễ tối đa 8 giây — mỗi tập lệch vài giây) thì tua tới
     * mốc hết, một lần mỗi tập. Qua mốc mà chưa tua (vd người xem tự tua) thì thôi, ◀ lùi lại xem được.
     */
    private fun skipIntro(s: PlayState) {
        val (start, end) = Prefs.intro(introKey) ?: return
        if (end <= start || seekTarget != null) return
        if (s.t >= start + 8 || s.t >= end - 3) {
            introDone = true
            return
        }
        if (s.t < start - 1 || (s.d > 0 && end > s.d - 30)) return
        introDone = true
        session?.seeked()
        cmd("seekTo", end)
        notice.show("Đã bỏ qua nhạc mở đầu (tới ${formatTime(end.toDouble())}) — ◀ để xem lại.")
    }

    /** Chỗ bắt đầu báo trước cho lần nạp trang này: chỗ xem dở; hoặc hết nhạc mở đầu nếu nhạc nằm ngay đầu tập. */
    private fun startFor(): Int {
        if (resumeAt > 30) return resumeAt
        val (start, end) = Prefs.intro(introKey) ?: return 0
        return if (!introDone && start <= 1 && end > start) end else 0
    }

    /** Hàng "Nhạc mở đầu" trong menu ▼: đánh dấu ở thời điểm đang xem. */
    private fun showIntroRow() {
        val t = (state?.t ?: 0.0).toInt()
        val mark = Prefs.intro(introKey)
        introLabel.text = "Nhạc mở đầu (cả phim): " + when {
            mark == null -> "chưa đánh dấu — đang ở đầu / cuối nhạc thì bấm nút tương ứng"
            mark.second < 0 -> "bắt đầu ${formatTime(mark.first.toDouble())}, chưa đánh dấu chỗ hết"
            else -> "${formatTime(mark.first.toDouble())} → ${formatTime(mark.second.toDouble())}, tự bỏ qua ở các tập"
        }
        btnIntroStart.text = "Bắt đầu ở ${formatTime(t.toDouble())}"
        btnIntroEnd.text = "Hết nhạc ở ${formatTime(t.toDouble())}"
        btnIntroClear.visibility = if (mark != null) View.VISIBLE else View.GONE
        val enabled = started
        btnIntroStart.isEnabled = enabled
        btnIntroEnd.isEnabled = enabled
        btnIntroStart.setOnClickListener {
            hidePanel()
            val end = Prefs.intro(introKey)?.second?.takeIf { it > t } ?: -1
            Prefs.setIntro(introKey, t, end)
            notice.show(
                if (end > t) "Nhạc mở đầu: ${formatTime(t.toDouble())} → ${formatTime(end.toDouble())}, các tập sẽ tự bỏ qua."
                else "Đã đánh dấu đầu nhạc mở đầu. Hết nhạc thì nhấn ▼ → «Hết nhạc ở …».",
                6000,
            )
        }
        btnIntroEnd.setOnClickListener {
            hidePanel()
            val start = Prefs.intro(introKey)?.first?.takeIf { it < t } ?: 0
            Prefs.setIntro(introKey, start, t)
            introDone = true
            notice.show("Nhạc mở đầu: ${formatTime(start.toDouble())} → ${formatTime(t.toDouble())}, các tập sẽ tự bỏ qua.", 6000)
        }
        btnIntroClear.setOnClickListener {
            hidePanel()
            Prefs.clearIntro(introKey)
            notice.show("Đã bỏ mốc nhạc mở đầu của phim này.")
        }
    }

    /** Chỗ đang xem để mang sang (đổi server / tải lại / đổi bản): lệnh tua xem tiếp chưa xong thì vẫn là chỗ cũ. */
    private fun currentTime(): Int? = if (resumePending) resumeAt else state?.takeIf { started && it.t > 5 }?.t?.toInt()

    private fun savePosition() {
        val s = state ?: return
        if (!started || episodeUrl.isEmpty() || resumePending) return
        // Xem gần hết thì coi như xong (lần sau mở lại từ đầu)
        val done = s.ended || (s.d > 0 && s.t > s.d - 90)
        Prefs.savePosition(episodeUrl, if (done) 0 else s.t.toInt(), detail?.url ?: filmUrl)
    }

    private fun cmd(name: String, arg: Number? = null) {
        web.evaluateJavascript("window.chimtoCmd&&chimtoCmd('$name',${arg ?: "null"})", null)
    }

    // ---- Phím remote / bàn phím ----

    override fun dispatchKeyEvent(e: KeyEvent): Boolean {
        val code = e.keyCode
        // Bấm remote / bàn phím: thôi chế độ cảm ứng
        keyUsed(e)
        if (e.action == KeyEvent.ACTION_DOWN && touchBar.visibility == View.VISIBLE) hideTouch()
        // Đang đếm 60 giây sau khi hết giờ hẹn: có người bấm → thôi thoát (phím vẫn làm việc của nó, vd OK để phát)
        if (sleepGraceUntil > 0 && e.action == KeyEvent.ACTION_DOWN) {
            sleepGraceUntil = 0
            notice.show("Đã huỷ hẹn giờ tắt — xem tiếp.")
        }
        if (panel.visibility == View.VISIBLE) {
            if (code == KeyEvent.KEYCODE_BACK || code == KeyEvent.KEYCODE_ESCAPE || code == KeyEvent.KEYCODE_MENU) {
                if (e.action == KeyEvent.ACTION_UP) hidePanel()
                return true
            }
            return super.dispatchKeyEvent(e)
        }
        // Đang hiện gợi ý đổi server: OK = chuyển, Back = bỏ qua
        val alt = suggestion
        if (alt != null && System.currentTimeMillis() < suggestionUntil) {
            when (code) {
                KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                    if (e.action == KeyEvent.ACTION_DOWN && e.repeatCount == 0) {
                        dismissSuggestion()
                        switchServer(alt)
                    }
                    return true
                }
                KeyEvent.KEYCODE_BACK, KeyEvent.KEYCODE_ESCAPE -> {
                    if (e.action == KeyEvent.ACTION_UP) dismissSuggestion()
                    return true
                }
            }
        }
        // ESC = Back (không để WebView chuyển ESC cho trang web / trình phát rồi mất)
        if (escapeAsBack(e)) return true
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
        // Đang hiện thanh cảm ứng (có thời gian riêng): khỏi chồng thêm thanh thời gian của remote; tính lại giờ tự ẩn
        // (vd vừa chạm Dừng: trình phát báo đã dừng sau ~1 giây → thanh ở lại để chạm Phát)
        if (touchBar.visibility == View.VISIBLE) {
            showTouch()
            return
        }
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
        hideTouch()
        // Điểm server mới nhất + tình hình lần xem này
        val host = source?.let { AppProxy.statsHost(Site.hostOf(it.epUrl)) }
        for (i in 0 until serverRow.childCount) {
            val b = serverRow.getChildAt(i) as TextView
            source?.servers?.firstOrNull { it.type == b.tag }?.let { b.text = serverText(host, it) }
        }
        serverLabel.text = session?.describe()?.let { "Server — lần xem này: $it" } ?: serverHint
        showIntroRow()
        showViewButtons()
        panel.visibility = View.VISIBLE
        val active = ((0 until serverRow.childCount).map { serverRow.getChildAt(it) } +
            (0 until groupRow.childCount).map { groupRow.getChildAt(it) }).firstOrNull { it.isActivated }
        (active ?: findViewById(R.id.btnNext)).requestFocus()
    }

    // ---- Màn hình cảm ứng ----

    // Lớp chạm chỉ cho màn cảm ứng: remote / bàn phím (và dịch vụ trợ năng) đã có đường phím riêng
    @SuppressLint("ClickableViewAccessibility")
    private fun setupTouch() {
        val gestures = GestureDetector(this, object : GestureDetector.SimpleOnGestureListener() {
            /** Thanh vừa hiện do lần chạm này (khỏi ẩn ngay khi xác nhận là chạm một lần). */
            private var shownByTap = false

            override fun onDown(e: MotionEvent) = true

            // Hiện thanh ngay khi nhấc tay, không chờ 0,3 giây xem có chạm đúp không: TV chậm mà người xem chạm tiếp
            // vào nút ngay thì nút đã có sẵn (chạm đúp để tua thì thanh hiện luôn cũng tiện xem giờ)
            override fun onSingleTapUp(e: MotionEvent): Boolean {
                if (panel.visibility != View.VISIBLE && touchBar.visibility != View.VISIBLE) {
                    showTouch()
                    shownByTap = true
                }
                return true
            }

            // Chạm một lần khi thanh / menu đang hiện: ẩn đi
            override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
                if (shownByTap) {
                    shownByTap = false
                    return true
                }
                if (panel.visibility == View.VISIBLE) hidePanel() else hideTouch()
                return true
            }

            override fun onDoubleTap(e: MotionEvent): Boolean {
                shownByTap = false
                val back = e.x < touchLayer.width / 2f
                seekBy(if (back) -10.0 else 10.0)
                notice.show(if (back) "◀◀ 10 giây" else "10 giây ▶▶", 1200)
                if (panel.visibility != View.VISIBLE) showTouch()
                return true
            }
        })
        touchLayer.setOnTouchListener { _, ev ->
            gestures.onTouchEvent(ev)
            true
        }
        // Bấm nút nào cũng giữ thanh thêm 5 giây
        fun button(id: Int, action: () -> Unit) = findViewById<View>(id).setOnClickListener {
            action()
            showTouch()
        }
        button(R.id.touchPrev) { prev() }
        button(R.id.touchBack10) { seekBy(-10.0) }
        button(R.id.touchPlay) { cmd(if (started) "toggle" else "play") }
        button(R.id.touchFwd10) { seekBy(10.0) }
        button(R.id.touchNext) { next() }
        findViewById<View>(R.id.touchMenu).setOnClickListener { showPanel() }
        findViewById<View>(R.id.touchExit).setOnClickListener { finish() }
        touchSeek.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(bar: SeekBar, p: Int, fromUser: Boolean) {
                val d = state?.d ?: 0.0
                if (fromUser && d > 0) touchTime.text = formatTime(d * p / 1000)
            }

            override fun onStartTrackingTouch(bar: SeekBar) {
                touchDragging = true
                touchBar.removeCallbacks(hideTouchBar)
            }

            override fun onStopTrackingTouch(bar: SeekBar) {
                touchDragging = false
                val d = state?.d ?: 0.0
                if (started && d > 0) {
                    val target = (d * bar.progress / 1000).coerceIn(0.0, d - 1)
                    resumePending = false // người xem tự tua
                    session?.seeked()
                    cmd("seekTo", target)
                    state = state?.copy(t = target)
                }
                showTouch()
            }
        })
    }

    private fun showTouch() {
        osd.visibility = View.GONE
        touchBar.visibility = View.VISIBLE
        updateTouchBar()
        touchBar.removeCallbacks(hideTouchBar)
        // Đang dừng thì giữ thanh trên màn hình (như thanh thời gian khi dùng remote)
        if (state?.paused != true || !started) touchBar.postDelayed(hideTouchBar, 5000)
    }

    private fun hideTouch() {
        touchBar.removeCallbacks(hideTouchBar)
        touchBar.visibility = View.INVISIBLE
    }

    private fun updateTouchBar() {
        val s = state
        touchTitle.text = osdTitle.text
        val d = s?.d ?: 0.0
        val t = seekTarget ?: s?.t ?: 0.0
        if (!touchDragging) {
            touchTime.text = formatTime(t)
            touchSeek.progress = if (d > 0) (t / d * 1000).toInt() else 0
        }
        touchDuration.text = if (d > 0) formatTime(d) else "--:--"
        touchSeek.isEnabled = started && d > 0
        touchPlay.text = if (!started || s?.paused == true) "▶ Phát" else "Dừng"
    }

    private fun hidePanel() {
        panel.visibility = View.GONE
    }

    private fun showLoading(text: String) {
        showLoadingHint()
        loading.visibility = View.VISIBLE
        loadingProgress.visibility = View.VISIBLE
        loadingText.text = text
    }

    private fun showError(text: String) {
        showLoadingHint()
        loading.visibility = View.VISIBLE
        loadingProgress.visibility = View.GONE
        loadingText.text = text
    }

    /** Dòng hướng dẫn dưới màn chờ: phím remote, hoặc cách dùng khi đang dùng cảm ứng. */
    private fun showLoadingHint() {
        findViewById<TextView>(R.id.loadingHint).text = if (TouchUi.active) {
            "Chạm vào phim để hiện nút điều khiển (dừng / phát, tua, đổi tập, menu, thoát)  •  Chạm đúp trái / phải: lùi / tới 10 giây"
        } else {
            "OK: Dừng / Phát   •   ◀ ▶: Tua   •   ▼: Menu (đổi server, đổi tập)   •   Back: Thoát"
        }
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
        session?.save(System.currentTimeMillis())
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
