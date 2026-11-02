package vn.chimto.tv

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.os.Bundle
import android.os.Message
import android.os.SystemClock
import android.view.InputDevice
import android.view.KeyEvent
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
import android.widget.ProgressBar
import kotlin.math.min

/**
 * Duyệt nguyên trang web bằng con trỏ ảo — dự phòng khi trang đổi giao diện làm phần đọc dữ liệu không chạy.
 * Mũi tên: di chuyển con trỏ (chạm mép thì cuộn trang) • OK: bấm • CH+/CH−: cuộn nhanh • Back: lùi trang.
 */
class WebActivity : Activity() {
    companion object {
        fun intent(context: Context, url: String): Intent =
            Intent(context, WebActivity::class.java).putExtra("url", url)
    }

    private lateinit var web: WebView
    private lateinit var fullscreen: FrameLayout
    private lateinit var cursor: CursorView
    private lateinit var progress: ProgressBar
    private lateinit var notice: Notice
    private var customView: View? = null
    private var customCallback: WebChromeClient.CustomViewCallback? = null

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContentView(R.layout.activity_web)
        web = findViewById(R.id.web)
        fullscreen = findViewById(R.id.fullscreen)
        progress = findViewById(R.id.progress)
        notice = Notice(findViewById(R.id.notice))
        cursor = CursorView(this)
        // Con trỏ nằm trên WebView và video toàn màn hình, dưới thông báo
        findViewById<FrameLayout>(R.id.root).addView(
            cursor, 2, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )

        web.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            mediaPlaybackRequiresUserGesture = false
            setSupportMultipleWindows(true)
            javaScriptCanOpenWindowsAutomatically = false
            mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
            useWideViewPort = true
            loadWithOverviewMode = true
            setSupportZoom(false)
            userAgentString = AdBlock.browserUserAgent(userAgentString)
        }
        CookieManager.getInstance().setAcceptThirdPartyCookies(web, true)

        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                if (!request.isForMainFrame || request.isRedirect) return false
                val host = request.url.host
                if (isSiteHost(host)) return false
                notice.show("Đã chặn chuyển sang trang ngoài: $host")
                return true
            }

            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                if (AdBlock.isAd(request.url.host)) AdBlock.empty() else null
        }
        web.webChromeClient = object : WebChromeClient() {
            override fun onProgressChanged(view: WebView, newProgress: Int) {
                progress.progress = newProgress
                progress.visibility = if (newProgress >= 100) View.GONE else View.VISIBLE
            }

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

            override fun getDefaultVideoPoster(): Bitmap = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)

            override fun onConsoleMessage(consoleMessage: ConsoleMessage) = true
        }

        web.loadUrl(intent.getStringExtra("url") ?: (Prefs.baseUrl + "/"))
        notice.show("Mũi tên: di chuyển con trỏ  •  OK: bấm  •  CH+/CH−: cuộn trang  •  Back: quay lại", 7000)
    }

    /** Cho phép đi trong cùng trang phim (kể cả tên miền con), chặn các trang ngoài (thường là quảng cáo). */
    private fun isSiteHost(host: String?): Boolean {
        val h = host?.lowercase()?.removePrefix("www.") ?: return false
        val site = Site.hostOf(Prefs.baseUrl) ?: return true
        val current = web.url?.let { Site.hostOf(it) }
        return h == site || h.endsWith(".$site") || h == current
    }

    private fun hideCustomView() {
        val v = customView ?: return
        fullscreen.removeView(v)
        fullscreen.visibility = View.GONE
        customView = null
        customCallback?.onCustomViewHidden()
        customCallback = null
    }

    override fun dispatchKeyEvent(e: KeyEvent): Boolean {
        val down = e.action == KeyEvent.ACTION_DOWN
        when (e.keyCode) {
            KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN -> {
                if (down) move(e.keyCode, e.repeatCount)
                return true
            }
            KeyEvent.KEYCODE_DPAD_CENTER -> {
                if (down && e.repeatCount == 0) tap()
                return true
            }
            KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                if (down && e.repeatCount == 0) enter()
                return true
            }
            KeyEvent.KEYCODE_CHANNEL_UP, KeyEvent.KEYCODE_PAGE_UP -> {
                if (down) web.evaluateJavascript("window.scrollBy(0,-innerHeight*0.8)", null)
                return true
            }
            KeyEvent.KEYCODE_CHANNEL_DOWN, KeyEvent.KEYCODE_PAGE_DOWN -> {
                if (down) web.evaluateJavascript("window.scrollBy(0,innerHeight*0.8)", null)
                return true
            }
            KeyEvent.KEYCODE_MENU -> {
                if (down && e.repeatCount == 0) {
                    web.reload()
                    notice.show("Đang tải lại trang…")
                }
                return true
            }
        }
        return super.dispatchKeyEvent(e)
    }

    private fun move(keyCode: Int, repeat: Int) {
        // Giữ phím thì con trỏ chạy nhanh dần
        val speed = min(dp(9) * (1 + repeat * 0.45f), dp(46).toFloat())
        val (dx, dy) = when (keyCode) {
            KeyEvent.KEYCODE_DPAD_LEFT -> -speed to 0f
            KeyEvent.KEYCODE_DPAD_RIGHT -> speed to 0f
            KeyEvent.KEYCODE_DPAD_UP -> 0f to -speed
            else -> 0f to speed
        }
        val (restX, restY) = cursor.moveBy(dx, dy)
        if (restX != 0f || restY != 0f) scrollAtCursor(restX * 3, restY * 3)
        hover()
    }

    /**
     * Cuộn khung nằm dưới con trỏ nếu khung đó còn cuộn được theo hướng đang đi, không thì cuộn cả trang.
     * Toạ độ CSS = px / mật độ màn hình.
     */
    private fun scrollAtCursor(dx: Float, dy: Float) {
        val d = resources.displayMetrics.density
        val js = """(function(x,y,dx,dy){var e=document.elementFromPoint(x,y);
            while(e&&e!==document.body&&e!==document.documentElement){var s=getComputedStyle(e);
            var canY=dy&&/(auto|scroll)/.test(s.overflowY)&&(dy>0?e.scrollTop+e.clientHeight<e.scrollHeight-1:e.scrollTop>0);
            var canX=dx&&/(auto|scroll)/.test(s.overflowX)&&(dx>0?e.scrollLeft+e.clientWidth<e.scrollWidth-1:e.scrollLeft>0);
            if(canX||canY){e.scrollBy(dx,dy);return}e=e.parentElement}window.scrollBy(dx,dy)})""".trimIndent()
        web.evaluateJavascript("$js(${cursor.cx / d},${cursor.cy / d},${dx / d},${dy / d})", null)
    }

    private fun touchTarget(): View = if (customView != null) fullscreen else web

    private fun tap() {
        val target = touchTarget()
        val x = cursor.cx
        val y = cursor.cy
        val t = SystemClock.uptimeMillis()
        val downEv = MotionEvent.obtain(t, t, MotionEvent.ACTION_DOWN, x, y, 0).apply { source = InputDevice.SOURCE_TOUCHSCREEN }
        target.dispatchTouchEvent(downEv)
        downEv.recycle()
        target.postDelayed({
            val upEv = MotionEvent.obtain(t, SystemClock.uptimeMillis(), MotionEvent.ACTION_UP, x, y, 0)
                .apply { source = InputDevice.SOURCE_TOUCHSCREEN }
            target.dispatchTouchEvent(upEv)
            upEv.recycle()
        }, 60)
    }

    /** Enter: đang gõ trong ô nhập của trang (vd ô tìm kiếm) thì gửi Enter cho trang, còn lại thì bấm như OK. */
    private fun enter() {
        val js = "(function(){var a=document.activeElement;" +
            "return !!a&&(a.tagName=='INPUT'||a.tagName=='TEXTAREA'||a.isContentEditable)})()"
        web.evaluateJavascript(js) { editing ->
            if (editing == "true") {
                web.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER))
                web.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER))
            } else {
                tap()
            }
        }
    }

    /** Báo vị trí "chuột" cho trang để menu thả xuống khi rê chuột cũng hoạt động. */
    private fun hover() {
        val t = SystemClock.uptimeMillis()
        val ev = MotionEvent.obtain(t, t, MotionEvent.ACTION_HOVER_MOVE, cursor.cx, cursor.cy, 0)
            .apply { source = InputDevice.SOURCE_MOUSE }
        touchTarget().dispatchGenericMotionEvent(ev)
        ev.recycle()
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        when {
            customView != null -> hideCustomView()
            web.canGoBack() -> web.goBack()
            else -> super.onBackPressed()
        }
    }

    override fun onResume() {
        super.onResume()
        web.onResume()
    }

    override fun onPause() {
        web.onPause()
        super.onPause()
    }

    override fun onDestroy() {
        web.stopLoading()
        web.destroy()
        super.onDestroy()
    }
}
