package vn.chimto.tv

import android.annotation.SuppressLint
import android.annotation.TargetApi
import android.app.AlertDialog
import android.app.PendingIntent
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.drawable.Icon
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Message
import android.util.Log
import android.util.Rational
import android.view.GestureDetector
import kotlin.math.hypot
import android.view.ScaleGestureDetector
import android.text.style.RelativeSizeSpan
import android.text.SpannableString
import android.media.AudioManager
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
import java.lang.ref.WeakReference
import kotlin.math.abs
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * Phát một tập phim.
 *
 * Trình phát của trang chỉ chạy khi nằm trong iframe của chính trang phim, nên app dựng một trang bọc
 * (assets/player.html) với origin = địa chỉ tập phim, đặt trình phát vào iframe, rồi khi WebView tải
 * tài liệu của iframe thì app tự tải hộ và chèn assets/player-agent.js vào đầu. Agent nhận lệnh
 * (phát / dừng / tua) qua postMessage và báo trạng thái lên trang bọc; app đọc trạng thái mỗi giây.
 * Server cho link HLS trực tiếp (.m3u8, trang Next.js kiểu PhimMoi): app phát bằng trang riêng assets/hls.html
 * (hls.js đóng kèm, bỏ quảng cáo chèn giữa phim), agent nằm ngay trong trang đó.
 *
 * Hình trong hình (Android 7+ có tính năng này): nút trong menu ▼, hoặc bấm Home lúc đang phát (Cài đặt) → phim thu
 * nhỏ vào góc, màn trước đó của app (hoặc màn hình chính TV) hiện bên dưới. Chỉ có một trình phát (singleTask): chọn
 * phim khác lúc đang thu nhỏ thì phim đó mở ngay trong trình phát này (onNewIntent), phóng to lại toàn màn hình.
 */
class PlayerActivity : BaseActivity() {
    // Có thanh điều khiển cảm ứng riêng (có nút Thoát), khỏi nút "‹ Quay lại" chung
    override val touchBackLabel: String? = null

    companion object {
        fun intent(context: Context, film: FilmDetail, ep: Episode): Intent =
            Intent(context, PlayerActivity::class.java).putExtra("film", film.url).putExtra("ep", ep.url)

        private const val ACTION_PIP = "vn.chimto.tv.PIP"

        private var currentRef: WeakReference<PlayerActivity>? = null
        private val current: PlayerActivity? get() = currentRef?.get()?.takeIf { !it.isFinishing && !it.isDestroyed }

        /** Màn khác (màn chính) cập nhật khi phim vào / ra khỏi cửa sổ nhỏ. */
        val pipListeners = ArrayList<() -> Unit>()

        private var pipSupport: Boolean? = null

        /**
         * TV / máy dùng được hình trong hình. Nhiều TV bị hãng tắt hẳn (Skyworth); máy ít RAM (TX3: ro.config.low_ram)
         * hoặc không cho chia màn hình thì vẫn khai báo có, nhưng Android báo "Device doesn't support picture-in-picture
         * mode" khi dùng — xét giống Android (ActivityManager.supportsMultiWindow).
         */
        @SuppressLint("DiscouragedApi") // config_supportsMultiWindow chỉ đọc được bằng tên
        fun pipSupported(context: Context): Boolean = pipSupport ?: run {
            val sys = android.content.res.Resources.getSystem()
            val multiWindow = sys.getIdentifier("config_supportsMultiWindow", "bool", "android").let { it == 0 || sys.getBoolean(it) }
            val am = context.getSystemService(Context.ACTIVITY_SERVICE) as android.app.ActivityManager
            (Build.VERSION.SDK_INT >= 24 && context.packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE) &&
                !am.isLowRamDevice && multiWindow).also { pipSupport = it }
        }

        /** Phim đang thu nhỏ — hình trong hình hoặc cửa sổ nhỏ trong app (vd "Tiên Nghịch — Tập 165"); null = không có. */
        val pipTitle: String?
            get() = current?.takeIf { it.small }?.let { p -> p.osdTitle.text.toString().ifEmpty { "phim đang xem" } }

        /** Phóng phim đang thu nhỏ ra toàn màn hình (mở lại trình phát duy nhất — singleTask, task riêng). */
        fun expandPip(context: Context) {
            if (current?.small != true) return
            context.startActivity(Intent(context, PlayerActivity::class.java).putExtra("expand", true))
        }

        fun closePip() {
            current?.takeIf { it.small }?.finish()
        }

        /** Thoát app (Back ở màn chính) lúc phim đang ở cửa sổ nhỏ trong app: tắt phim (ra ngoài app không xem được). */
        fun closeMini() {
            current?.takeIf { it.mini }?.finish()
        }

        /** Rời hẳn app (Home, mở app khác) lúc phim ở cửa sổ nhỏ trong app: dừng phim, lưu chỗ xem. */
        fun appLeft() {
            val p = current?.takeIf { it.mini } ?: return
            p.cmd("pause")
            p.savePosition()
        }

        /**
         * Quay lại app (bấm biểu tượng app) mà trình phát toàn màn hình đang nằm ở nền — trình phát chạy task riêng nên
         * Android mở màn chính: đưa trình phát ra lại như trước.
         */
        fun showHidden(context: Context) {
            val p = current ?: return
            if (p.small || !p.stopped) return
            context.startActivity(Intent(context, PlayerActivity::class.java).putExtra("expand", true))
        }

        /** Đổi người xem: lưu chỗ đang xem cho người cũ rồi đóng trình phát (đang thu nhỏ). */
        fun closeForProfileSwitch() {
            val p = current ?: return
            p.savePosition()
            p.session?.save(System.currentTimeMillis())
            p.session = null
            p.saveDisabled = true
            p.finish()
        }

        /** Phím phát / dừng / tua / chuyển tập (remote, bàn phím) bấm ở màn khác lúc phim đang thu nhỏ: điều khiển phim. */
        fun mediaKey(e: KeyEvent): Boolean {
            val p = current?.takeIf { it.small } ?: return false
            val what = when (e.keyCode) {
                KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.KEYCODE_HEADSETHOOK -> "toggle"
                KeyEvent.KEYCODE_MEDIA_PLAY -> "play"
                KeyEvent.KEYCODE_MEDIA_PAUSE, KeyEvent.KEYCODE_MEDIA_STOP -> "pause"
                KeyEvent.KEYCODE_MEDIA_REWIND -> "back10"
                KeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> "fwd10"
                KeyEvent.KEYCODE_MEDIA_NEXT -> "next"
                KeyEvent.KEYCODE_MEDIA_PREVIOUS -> "prev"
                else -> return false
            }
            if (e.action == KeyEvent.ACTION_DOWN && e.repeatCount == 0) p.pipCommand(what)
            return true
        }

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
        val droppedFrames: Int, val totalFrames: Int, val videoWidth: Int = 0, val videoHeight: Int = 0,
    )

    // ---- Hình trong hình ----

    /** Đang thu nhỏ trong cửa sổ hình trong hình. */
    private var inPip = false

    /** Đang ở cửa sổ nhỏ trong app (MiniPlayer) — TV không có hình trong hình. */
    private var mini = false
    private val small get() = inPip || mini

    /** Chỗ của WebView trong màn trình phát (lấy lại từ cửa sổ nhỏ khi phóng to). */
    private lateinit var webHost: ViewGroup

    /** Vừa ở cửa sổ nhỏ mà chưa quay lại toàn màn hình: dừng hẳn (onStop) lúc này = người xem đã đóng cửa sổ nhỏ. */
    private var leftPip = false
    private var stopped = false
    private var pipReceiverOn = false

    /** Nút trong cửa sổ nhỏ đang vẽ cho trạng thái này (đổi phát ↔ dừng / khung hình thì vẽ lại). */
    private var pipShownPaused: Boolean? = null
    private var pipShownWidth = -1

    /** Đã lưu chỗ xem cho người xem cũ rồi đóng (đổi người xem): không ghi gì nữa. */
    private var saveDisabled = false

    /** Số lần mở phim (mở phim khác trong lúc đang thu nhỏ thì bỏ kết quả tải của phim cũ). */
    private var loadToken = 0

    private val pipReceiver = object : BroadcastReceiver() {
        override fun onReceive(c: Context, i: Intent) {
            i.getStringExtra("do")?.let { pipCommand(it) }
        }
    }

    private val pipClosedCheck = Runnable { if (stopped && !isFinishing) finish() }

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
    private lateinit var gestureHud: TextView
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
        webHost = web.parent as ViewGroup
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
        gestureHud = findViewById(R.id.gestureHud)
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
        setupPip()

        val film = intent.getStringExtra("film")
        val epUrl = intent.getStringExtra("ep")
        if (film == null || epUrl == null) {
            finish() // vd "Phóng to" mà phim thu nhỏ vừa bị đóng
            return
        }
        currentRef = WeakReference(this)
        load(film, epUrl)
    }

    private fun load(film: String, epUrl: String) {
        filmUrl = film
        val t = ++loadToken
        val cached = Site.cachedDetail(film)
        if (cached != null) {
            start(cached, epUrl)
        } else {
            showLoading("Đang tải thông tin phim…")
            background({ Site.detail(film) }) { r ->
                if (t != loadToken) return@background
                r.onSuccess { start(it, epUrl) }.onFailure { showError("Không tải được phim.\n${it.friendly()}") }
            }
        }
    }

    /**
     * Trình phát đang mở (thường là đang thu nhỏ) mà người xem chọn phim / tập khác, hoặc bấm "Phóng to": hệ thống đưa
     * trình phát duy nhất này ra toàn màn hình; phim khác thì lưu chỗ xem phim cũ rồi mở phim mới.
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        exitMini()
        if (intent.getBooleanExtra("expand", false)) return
        val film = intent.getStringExtra("film") ?: return
        val epUrl = intent.getStringExtra("ep") ?: return
        if (film == filmUrl && epUrl == episodes.getOrNull(index)?.url) return // đúng tập đang phát: phát tiếp
        setIntent(intent)
        hidePanel()
        hideTouch()
        token++ // bỏ kết quả tải trang tập / server của phim cũ
        stopVideo()
        load(film, epUrl)
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
        // Người đang xem đã hết giờ xem (WatchLimit): chưa mở tập, hỏi mã phụ huynh trước
        WatchLimit.block()?.let { b ->
            showLimit(b) { openEpisode(i) }
            return
        }
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
        miniCaption()
        for (j in 0 until groupRow.childCount) groupRow.getChildAt(j).let { it.isActivated = it.tag == group?.name }
        resumeAt = Prefs.position(ep.url)
        introDone = false
        if (!Prefs.privateMode) Prefs.addHistory(HistoryItem(d.url, d.title, d.poster, ep.title, ep.url, System.currentTimeMillis()))
        showLoading("Đang mở ${ep.title}…")
        val g = group
        // Trang tập + server dự phòng KKPhim (tải song song; trang lỗi mà KKPhim có tập này thì vẫn phát được)
        background({ KkPhim.source(d, g, ep) }) { r ->
            trace("trang tập")
            if (t != token) return@background
            r.onSuccess { src ->
                source = src
                buildServerButtons(src)
                if (src.pageOnly) {
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
        // "Trình phát của trang" (trang mở thẳng trang tập, có thêm server KKPhim)
        if (s?.type == PAGE_SERVER) {
            ++token
            playPage(src)
            return
        }
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
        val servers = if (src.pageOnly) emptyList() else src.servers
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
        if (small) return // cửa sổ nhỏ không bấm OK được: phóng to ra rồi mới gợi ý
        suggested = true // mỗi lần phát chỉ gợi ý một lần (không có server khác thì thôi)
        val src = source?.takeIf { !it.pageOnly } ?: return
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
                src.pageOnly -> playPage(src)
                else -> playServer(server)
            }
        }
        findViewById<View>(R.id.btnEpisodes).setOnClickListener { finish() }
        findViewById<View>(R.id.btnWebPage).setOnClickListener {
            hidePanel()
            // NO_USER_ACTION: không tính là người xem rời trình phát (khỏi tự thu nhỏ phim — xem onUserLeaveHint)
            startActivity(WebActivity.intent(this, episodeUrl).addFlags(Intent.FLAG_ACTIVITY_NO_USER_ACTION))
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
        startActivity(
            Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NO_USER_ACTION),
        )
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
            val other = if (source?.pageOnly != false) "▼ → Tải lại / Mở trang Web" else "▼ để đổi server"
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
            o.optInt("df"), o.optInt("tf"), o.optInt("vw"), o.optInt("vh"),
        )
        val wasPaused = state?.paused
        if (s.ready && state?.ready != true && !started) trace("trình phát sẵn sàng t=${s.t.toInt()}")
        state = s
        // Cửa sổ nhỏ: nút Phát ↔ Dừng và khung hình theo đúng phim
        if (inPip && (s.paused != pipShownPaused || s.videoWidth != pipShownWidth)) updatePipParams()
        if (mini && s.paused != wasPaused) miniCaption()
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
        WatchLimit.tick(started && !s.paused && !s.waiting, now)
        checkLimit()
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

    // ---- Giới hạn giờ xem (WatchLimit) ----

    /** Đang hiện màn hết giờ xem (hoặc đang nhập mã phụ huynh từ màn đó). */
    private var limitShowing = false

    /** Đã nhắc "còn 5 phút" trong lần xem này. */
    private var limitWarned = false

    /** Mỗi lần đọc trạng thái: sắp hết giờ thì nhắc, hết giờ thì dừng phim và hiện màn hết giờ xem. */
    private fun checkLimit() {
        if (limitShowing) return
        val b = WatchLimit.block()
        if (b == null) {
            val r = WatchLimit.remaining()
            if (started && !limitWarned && r != null && r in 1..300) {
                limitWarned = true
                notice.show("Còn ${WatchLimit.minutesText(r)} xem hôm nay (${Prefs.profile.name}).", 8000)
            }
            return
        }
        cmd("pause")
        // Đang thu nhỏ: chỉ dừng phim; phóng to lại thì lần đọc trạng thái sau hiện màn hết giờ
        if (inPipNow() || mini) return
        showLimit(b) { cmd("play") }
    }

    /** Màn hết giờ xem: người lớn nhập mã PIN phụ huynh để cho xem thêm (rồi chạy resume), hoặc thoát trình phát. */
    private fun showLimit(b: WatchLimit.Block, resume: () -> Unit) {
        if (isFinishing) return
        limitShowing = true
        hideLoading()
        val who = Prefs.profile.name
        val why = when (b) {
            WatchLimit.Block.BEDTIME -> "Đã tới giờ đi ngủ của $who (từ ${WatchLimit.bedtimeLabel(Prefs.bedtime())} tới 6 giờ sáng)."
            WatchLimit.Block.DAILY -> "$who đã xem hết ${WatchLimit.dailyLabel(Prefs.dailyLimit())} của hôm nay."
        }
        val d = AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Hết giờ xem")
            .setMessage("$why\nNgười lớn nhập mã PIN phụ huynh để cho xem thêm.")
            .setPositiveButton("Cho xem thêm…") { _, _ ->
                ProfileUi.grant(this, cancelled = { showLimit(b, resume) }) {
                    limitShowing = false
                    limitWarned = false
                    resume()
                }
            }
            .setNegativeButton("Thoát") { _, _ ->
                limitShowing = false
                finish()
            }
            .setOnCancelListener {
                limitShowing = false
                finish()
            }
            .show()
        d.getButton(AlertDialog.BUTTON_NEGATIVE)?.requestFocus()
    }

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
        if (!started || episodeUrl.isEmpty() || resumePending || saveDisabled) return
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
        // Phím "cửa sổ" (remote TV có phím hình trong hình): thu nhỏ phim
        if (code == KeyEvent.KEYCODE_WINDOW) {
            if (e.action == KeyEvent.ACTION_UP) shrink()
            return true
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
        if (small) return
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
        if (small) return
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
        val swipes = TouchGestures()
        val pinch = ScaleGestureDetector(this, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            private var total = 1f
            override fun onScaleBegin(d: ScaleGestureDetector): Boolean {
                total = 1f
                return gesturesAllowed
            }

            override fun onScale(d: ScaleGestureDetector): Boolean {
                total *= d.scaleFactor
                return true
            }

            // Chụm / mở hai ngón: vừa màn hình / lấp đầy (như bản iPad)
            override fun onScaleEnd(d: ScaleGestureDetector) {
                if (total > 1.15f) setFill(true) else if (total < 0.87f) setFill(false)
            }
        })
        touchLayer.setOnTouchListener { _, ev ->
            pinch.onTouchEvent(ev)
            swipes.onTouchEvent(ev)
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

    // ---- Cử chỉ chạm như bản iPad / iPhone: giữ tay xem nhanh 2×, vuốt ngang tua, vuốt dọc nửa trái độ sáng / nửa phải
    // âm lượng, chụm hai ngón vừa / lấp đầy. Chạm / chạm đúp vẫn do GestureDetector (setupTouch) lo.

    /** Đang mở menu / phim thu nhỏ thì không nhận cử chỉ (chạm để đóng menu như cũ). */
    private val gesturesAllowed get() = !small && panel.visibility != View.VISIBLE

    private var scrubFrom: Double? = null
    private var holdRate: Double? = null
    private val hideHud = Runnable { gestureHud.visibility = View.GONE }

    private fun showHud(text: String, detail: String? = null, ms: Long = 0) {
        gestureHud.removeCallbacks(hideHud)
        gestureHud.text = if (detail == null) text else SpannableString("$text\n$detail").apply {
            setSpan(RelativeSizeSpan(0.65f), text.length + 1, length, 0)
        }
        gestureHud.visibility = View.VISIBLE
        if (ms > 0) gestureHud.postDelayed(hideHud, ms)
    }

    private fun hudOff(delay: Long = 0) {
        gestureHud.removeCallbacks(hideHud)
        if (delay > 0) gestureHud.postDelayed(hideHud, delay) else gestureHud.visibility = View.GONE
    }

    /** Bắt đầu vuốt ngang để tua; false = chưa tua được (phim chưa chạy / chưa biết độ dài). */
    private fun scrubBegan(): Boolean {
        val s = state ?: return false
        if (!gesturesAllowed || !started || s.d <= 0) return false
        mainHandler.removeCallbacks(flushSeek)
        scrubFrom = seekTarget ?: s.t
        return true
    }

    /** fraction = quãng ngón tay đã vuốt / bề ngang (−1…1). Cả bề ngang = 2 phút (phim ngắn hơn: cả phim). */
    private fun scrubChanged(fraction: Double) {
        val from = scrubFrom ?: return
        val d = state?.d ?: return
        if (d <= 0) return
        val target = (from + fraction * min(d, 120.0)).coerceIn(0.0, d - 1)
        seekTarget = target
        val delta = target - from
        showHud(
            (if (delta < 0) "◀◀  " else "▶▶  ") + "${formatTime(target)} / ${formatTime(d)}",
            (if (delta < 0) "−" else "+") + formatTime(abs(delta)),
        )
        showTouch()
    }

    /** Nhấc tay: tua tới chỗ đã chọn; bị huỷ thì về như cũ. */
    private fun scrubEnded(apply: Boolean) {
        if (scrubFrom == null) return
        scrubFrom = null
        hudOff()
        if (apply) flushSeek.run() else seekTarget = null
        showTouch()
    }

    /** Giữ tay: xem nhanh 2× tới khi nhấc tay (đang phát mới có tác dụng). */
    private fun holdFast(on: Boolean): Boolean {
        if (on) {
            if (!gesturesAllowed || !started || state?.paused != false || holdRate != null || rate >= 2.0) return false
            holdRate = rate
            cmd("rate", 2.0)
            showHud("▶▶  Xem nhanh 2×", "nhấc tay để xem tiếp như thường")
            return true
        }
        val r = holdRate ?: return false
        holdRate = null
        cmd("rate", r)
        hudOff()
        return true
    }

    /** Độ sáng của riêng màn trình phát (0…1) — rời trình phát là màn hình về độ sáng cũ. */
    private var brightness: Float
        get() {
            val w = window.attributes.screenBrightness
            if (w >= 0) return w
            return runCatching {
                android.provider.Settings.System.getInt(contentResolver, android.provider.Settings.System.SCREEN_BRIGHTNESS) / 255f
            }.getOrDefault(0.5f).coerceIn(0f, 1f)
        }
        set(v) {
            window.attributes = window.attributes.apply { screenBrightness = v.coerceIn(0.01f, 1f) }
        }

    private val audio by lazy { getSystemService(Context.AUDIO_SERVICE) as AudioManager }

    /** Âm lượng nhạc / phim của máy (0…1), không hiện thanh âm lượng của hệ thống (che phim). */
    private var volume: Float
        get() = audio.getStreamVolume(AudioManager.STREAM_MUSIC).toFloat() /
            audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC).coerceAtLeast(1)
        set(v) {
            val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
            runCatching { audio.setStreamVolume(AudioManager.STREAM_MUSIC, (v * max).roundToInt().coerceIn(0, max), 0) }
        }

    private fun showLevel(isBrightness: Boolean, v: Float) {
        val pct = (v * 100).roundToInt()
        showHud(if (isBrightness) "☀  Độ sáng $pct%" else (if (pct == 0) "🔇  Âm lượng 0%" else "🔊  Âm lượng $pct%"))
    }

    /** Chụm / mở hai ngón: vừa màn hình / lấp đầy. */
    private fun setFill(fill: Boolean) {
        if (!gesturesAllowed) return
        if (Prefs.videoFill != fill) {
            Prefs.videoFill = fill
            cmd("fit", if (fill) 1 else 0)
            showViewButtons()
        }
        showHud(if (fill) "Khung hình: Lấp đầy" else "Khung hình: Vừa màn hình", ms = 1200)
    }

    /**
     * Vuốt / giữ tay trên lớp chạm (một ngón). Ngón đi quá ~24dp mới tính là vuốt: ngang → tua, dọc → độ sáng (nửa trái)
     * / âm lượng (nửa phải); vuốt dọc bắt đầu sát mép trên / dưới thì để cho điện thoại (kéo thanh thông báo, về màn hình
     * chính). Giữ yên 0,5 giây → xem nhanh 2×. Hai ngón: để cho chụm.
     */
    private inner class TouchGestures {
        private var mode = Mode.NONE
        private var downX = 0f
        private var downY = 0f
        private var nearEdge = false
        private var startLevel = 0f
        private val slop = dp(24).toFloat()
        private val hold = Runnable { if (mode == Mode.TAP && holdFast(true)) mode = Mode.HOLD }

        fun onTouchEvent(ev: MotionEvent) {
            when (ev.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    downX = ev.x
                    downY = ev.y
                    nearEdge = ev.y < dp(40) || ev.y > touchLayer.height - dp(40)
                    mode = Mode.TAP
                    touchLayer.removeCallbacks(hold)
                    touchLayer.postDelayed(hold, 500)
                }
                MotionEvent.ACTION_POINTER_DOWN -> {
                    finish(apply = false)
                    mode = Mode.IGNORED
                }
                MotionEvent.ACTION_MOVE -> move(ev.x - downX, ev.y - downY)
                MotionEvent.ACTION_UP -> finish(apply = true)
                MotionEvent.ACTION_CANCEL -> finish(apply = false)
            }
        }

        private fun move(dx: Float, dy: Float) {
            val w = touchLayer.width.coerceAtLeast(1)
            val h = touchLayer.height.coerceAtLeast(1)
            when (mode) {
                Mode.TAP -> {
                    if (hypot(dx, dy) < slop) return
                    touchLayer.removeCallbacks(hold)
                    mode = when {
                        !gesturesAllowed -> Mode.IGNORED
                        abs(dx) > abs(dy) -> if (scrubBegan()) Mode.SCRUB else Mode.IGNORED
                        nearEdge -> Mode.IGNORED
                        downX < w / 2f -> Mode.BRIGHTNESS.also { startLevel = brightness }
                        else -> Mode.VOLUME.also { startLevel = volume }
                    }
                    if (mode != Mode.IGNORED) move(dx, dy)
                }
                Mode.SCRUB -> scrubChanged((dx / w).toDouble())
                Mode.BRIGHTNESS, Mode.VOLUME -> {
                    // Vuốt hết ~70% chiều cao màn = từ 0 lên 100%
                    val v = (startLevel - dy / (h * 0.7f)).coerceIn(0f, 1f)
                    if (mode == Mode.BRIGHTNESS) brightness = v else volume = v
                    showLevel(mode == Mode.BRIGHTNESS, v)
                }
                Mode.HOLD, Mode.NONE, Mode.IGNORED -> Unit
            }
        }

        private fun finish(apply: Boolean) {
            touchLayer.removeCallbacks(hold)
            val was = mode
            mode = Mode.NONE
            when (was) {
                Mode.HOLD -> holdFast(false)
                Mode.SCRUB -> scrubEnded(apply)
                Mode.BRIGHTNESS, Mode.VOLUME -> hudOff(600)
                Mode.TAP, Mode.NONE, Mode.IGNORED -> Unit
            }
        }
    }

    private enum class Mode { NONE, TAP, HOLD, SCRUB, BRIGHTNESS, VOLUME, IGNORED }

    private fun showTouch() {
        if (small) return
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
        miniCaption(error = true)
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

    // ---- Hình trong hình ----

    // Dưới Android 13 chưa có cờ RECEIVER_NOT_EXPORTED; tin chỉ gửi trong app (Intent.setPackage)
    @SuppressLint("UnspecifiedRegisterReceiverFlag")
    private fun setupPip() {
        val supported = pipSupported(this)
        findViewById<View>(R.id.btnPip).apply {
            visibility = View.VISIBLE
            setOnClickListener {
                hidePanel()
                shrink()
            }
        }
        if (!supported || Build.VERSION.SDK_INT < 26) return // Android 7 (TV): cửa sổ nhỏ không có nút riêng
        val filter = IntentFilter(ACTION_PIP)
        if (Build.VERSION.SDK_INT >= 33) {
            registerReceiver(pipReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            registerReceiver(pipReceiver, filter)
        }
        pipReceiverOn = true
    }

    private fun inPipNow() = Build.VERSION.SDK_INT >= 24 && isInPictureInPictureMode

    /** "Thu nhỏ vào góc": hình trong hình của Android nếu TV có, không thì cửa sổ nhỏ trong app. */
    private fun shrink() {
        if (pipSupported(this)) enterPip() else enterMini()
    }

    /**
     * Cửa sổ nhỏ trong app: trình phát (task riêng) lùi ra sau, màn trước đó của app hiện lại; WebView chuyển sang nổi
     * ở góc màn đó (MiniPlayer, gắn vào ở BaseActivity.onResume). Phim chạy tiếp, trạng thái vẫn đọc mỗi giây.
     */
    private fun enterMini() {
        if (mini || inPipNow() || isFinishing) return
        if (customView != null) hideCustomView() // video đang xin toàn màn hình nằm ngoài WebView
        hidePanel()
        hideTouch()
        osd.visibility = View.GONE
        dismissSuggestion()
        mini = true
        notice.muted = true
        MiniPlayer.start(web, osdTitle.text.toString()) { a -> expandPip(a) }
        miniCaption()
        moveTaskToBack(true)
        pipListeners.toList().forEach { it() }
    }

    /** Ra khỏi cửa sổ nhỏ (phóng to / mở phim khác): lấy WebView về màn trình phát. */
    private fun exitMini() {
        if (!mini) return
        mini = false
        notice.muted = inPip
        MiniPlayer.release(web)
        if (web.parent == null) {
            webHost.addView(web, 0, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        }
        pipListeners.toList().forEach { it() }
    }

    /** Dòng chữ của cửa sổ nhỏ trong app: ▶ / ⏸ + tên phim — tập; lỗi thì nhắc phóng to để đổi server. */
    private fun miniCaption(error: Boolean = false) {
        if (!mini) return
        val t = osdTitle.text.toString()
        MiniPlayer.caption(
            when {
                error -> "⚠ $t — phóng to để chọn server khác"
                state?.paused == true -> "⏸ $t"
                else -> "▶ $t"
            },
        )
    }

    /** Thu nhỏ phim vào góc màn hình (phim chạy tiếp, màn bên dưới vẫn dùng được). */
    private fun enterPip(): Boolean {
        if (!pipSupported(this) || inPipNow() || isFinishing) return false
        hidePanel()
        hideTouch()
        osd.visibility = View.GONE
        return try {
            val ok = when {
                Build.VERSION.SDK_INT >= 26 -> enterPictureInPictureMode(pipParams())
                Build.VERSION.SDK_INT >= 24 -> {
                    @Suppress("DEPRECATION")
                    enterPictureInPictureMode()
                    true
                }
                else -> false
            }
            if (BuildConfig.DEBUG) Log.d("ChimToPip", "thu nhỏ: $ok")
            ok
        } catch (e: Exception) {
            // vd người dùng tắt quyền hình trong hình của app trong Cài đặt Android
            Log.w("ChimToPip", "không thu nhỏ được: $e")
            notice.show("Không thu nhỏ được phim (TV không cho dùng hình trong hình).", 5000)
            false
        }
    }

    /** Bấm Home lúc đang phát: thu nhỏ phim thay vì dừng (Cài đặt → "Bấm Home khi đang xem"). */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        if (BuildConfig.DEBUG) Log.d("ChimToPip", "rời trình phát: started=$started paused=${state?.paused} custom=${customView != null}")
        if (Prefs.pipOnHome && started && state?.paused == false && customView == null) enterPip()
    }

    /** Khung cửa sổ nhỏ theo đúng tỉ lệ phim (Android giới hạn 1:2,39 – 2,39:1) + nút lùi 10 giây / phát–dừng / tập sau. */
    @TargetApi(26)
    private fun pipParams(): PictureInPictureParams {
        val s = state
        val w = s?.videoWidth ?: 0
        val h = s?.videoHeight ?: 0
        val ratio = (if (w > 0 && h > 0) w.toDouble() / h else 16.0 / 9).coerceIn(1 / 2.38, 2.38)
        val playing = s?.paused == false
        pipShownPaused = s?.paused
        pipShownWidth = w
        return PictureInPictureParams.Builder()
            .setAspectRatio(Rational((ratio * 1000).roundToInt(), 1000))
            .setActions(
                listOf(
                    pipAction("back10", R.drawable.ic_pip_rewind, "Lùi 10 giây", 1),
                    if (playing) pipAction("pause", R.drawable.ic_pip_pause, "Dừng", 2)
                    else pipAction("play", R.drawable.ic_pip_play, "Phát", 2),
                    pipAction("next", R.drawable.ic_pip_next, "Tập sau", 3),
                ),
            )
            .build()
    }

    @TargetApi(26)
    private fun pipAction(what: String, icon: Int, title: String, request: Int): RemoteAction {
        val i = Intent(ACTION_PIP).setPackage(packageName).putExtra("do", what)
        val pi = PendingIntent.getBroadcast(this, request, i, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        return RemoteAction(Icon.createWithResource(this, icon), title, title, pi)
    }

    private fun updatePipParams() {
        if (Build.VERSION.SDK_INT < 26) return
        try {
            setPictureInPictureParams(pipParams())
        } catch (e: Exception) {
            Log.w("ChimToPip", "không cập nhật được cửa sổ nhỏ: $e")
        }
    }

    /** Nút của cửa sổ nhỏ / phím media bấm ở màn khác. */
    private fun pipCommand(what: String) {
        when (what) {
            "toggle" -> cmd(if (started) "toggle" else "play")
            "play" -> cmd("play")
            "pause" -> cmd("pause")
            "back10" -> seekBy(-10.0)
            "fwd10" -> seekBy(10.0)
            "next" -> next()
            "prev" -> prev()
        }
    }

    @Deprecated("Deprecated in Java")
    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean) {
        @Suppress("DEPRECATION")
        super.onPictureInPictureModeChanged(isInPictureInPictureMode)
        inPip = isInPictureInPictureMode
        notice.muted = inPip
        // Cửa sổ nhỏ: chỉ còn phim (vòng xoay khi đang tải tập kế); chữ / thanh điều khiển để lúc phóng to
        val text = if (inPip) View.GONE else View.VISIBLE
        loadingText.visibility = text
        findViewById<View>(R.id.loadingHint).visibility = text
        touchLayer.visibility = text
        if (inPip) {
            leftPip = false
            hidePanel()
            hideTouch()
            osd.visibility = View.GONE
            dismissSuggestion()
        } else {
            // Ra khỏi cửa sổ nhỏ: phóng to (onResume ngay sau) hay đóng cửa sổ (✕ / TV đóng khi app khác phát video)
            leftPip = true
            mainHandler.removeCallbacks(pipClosedCheck)
            mainHandler.postDelayed(pipClosedCheck, 800)
        }
        pipListeners.toList().forEach { it() }
    }

    /** Điện thoại: xem phim toàn màn hình — ẩn thanh điều hướng (vuốt từ mép để hiện lại một lúc). TV không có thanh này. */
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || Screen.isTv(this) || inPip) return
        @Suppress("DEPRECATION")
        window.decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
            View.SYSTEM_UI_FLAG_FULLSCREEN or View.SYSTEM_UI_FLAG_LAYOUT_STABLE
    }

    // ---- Vòng đời ----

    override fun onStart() {
        super.onStart()
        stopped = false
    }

    override fun onResume() {
        super.onResume()
        leftPip = false
        exitMini() // phóng to từ cửa sổ nhỏ trong app
        web.onResume()
        mainHandler.removeCallbacks(poll)
        mainHandler.post(poll)
    }

    override fun onPause() {
        WatchLimit.flush()
        savePosition()
        if (!saveDisabled) session?.save(System.currentTimeMillis())
        // Thu nhỏ (hình trong hình / cửa sổ nhỏ trong app): phim chạy tiếp, vẫn đọc trạng thái để lưu chỗ xem / tự sang
        // tập. Rời hẳn app lúc ở cửa sổ nhỏ trong app thì AppUse báo appLeft() → dừng phim.
        if (inPipNow() || mini) {
            super.onPause()
            return
        }
        cmd("pause")
        mainHandler.removeCallbacks(poll)
        web.onPause()
        super.onPause()
    }

    override fun onStop() {
        super.onStop()
        stopped = true
        // Người xem đóng cửa sổ nhỏ: thoát hẳn trình phát (không để phim nằm dừng ở nền)
        if (leftPip && !isFinishing) finish()
    }

    override fun onDestroy() {
        WatchLimit.flush()
        mainHandler.removeCallbacks(poll)
        mainHandler.removeCallbacks(flushSeek)
        mainHandler.removeCallbacks(pipClosedCheck)
        if (pipReceiverOn) unregisterReceiver(pipReceiver)
        if (currentRef?.get() === this) currentRef = null
        MiniPlayer.release(web) // đang nổi ở màn khác: gỡ ra trước khi huỷ WebView
        mini = false
        inPip = false
        token++
        web.stopLoading()
        web.destroy()
        super.onDestroy()
        pipListeners.toList().forEach { it() }
    }
}
