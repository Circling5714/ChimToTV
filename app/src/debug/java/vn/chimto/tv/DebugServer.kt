package vn.chimto.tv

import android.app.Activity
import android.app.Application
import android.content.ContentProvider
import android.content.ContentValues
import android.content.Intent
import android.content.pm.ActivityInfo
import android.graphics.Bitmap
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.PixelCopy
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.OutputStream
import java.net.ServerSocket
import java.net.Socket
import java.net.URLDecoder
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/** Khởi động cổng gỡ lỗi cùng app (provider chạy trước Application.onCreate xong — không cần sửa mã chính). */
class DebugBoot : ContentProvider() {
    override fun onCreate(): Boolean {
        DebugServer.start(context!!.applicationContext as Application)
        return true
    }

    override fun query(u: Uri, p: Array<String>?, s: String?, a: Array<String>?, o: String?) = null
    override fun getType(u: Uri) = null
    override fun insert(u: Uri, v: ContentValues?) = null
    override fun delete(u: Uri, s: String?, a: Array<String>?) = 0
    override fun update(u: Uri, v: ContentValues?, s: String?, a: Array<String>?) = 0
}

/**
 * Cổng gỡ lỗi của bản gỡ lỗi (giống bản iPad Beta): thử giao diện trên điện thoại thật khi không có adb. Chỉ trong mạng
 * nhà, cổng 8790, HTTP GET:
 *
 *   /info                         màn đang mở, hướng, cỡ màn (dp), khoảng chừa tai thỏ / thanh điều hướng, bản app
 *   /shot?scale=0.5               ảnh màn app (PNG; PixelCopy — chụp được cả WebView / video)
 *   /views                        các chữ đang hiện + toạ độ (điểm ảnh) — để chạm theo chữ
 *   /tap?x=&y=                    chạm (điểm ảnh của cửa sổ app)
 *   /tapText?t=Cài đặt            chạm vào chữ (khớp đúng, không có thì khớp một phần)
 *   /swipe?x1=&y1=&x2=&y2=&ms=300 vuốt
 *   /key?k=BACK|ENTER|DPAD_DOWN…  phím
 *   /orient?o=portrait|landscape|auto   ép hướng màn (auto = theo cảm biến như bản thật)
 *   /open?s=main|settings         mở màn
 *   /js?c=<mã JS>                 chạy JS ở khung trên cùng của WebView đang hiện (trình phát, Duyệt web)
 */
object DebugServer {
    private const val PORT = 8790
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var top: Activity? = null

    fun start(app: Application) {
        app.registerActivityLifecycleCallbacks(object : Application.ActivityLifecycleCallbacks {
            override fun onActivityResumed(a: Activity) { top = a }
            override fun onActivityPaused(a: Activity) = Unit
            override fun onActivityCreated(a: Activity, b: Bundle?) = Unit
            override fun onActivityStarted(a: Activity) = Unit
            override fun onActivityStopped(a: Activity) = Unit
            override fun onActivitySaveInstanceState(a: Activity, b: Bundle) = Unit
            override fun onActivityDestroyed(a: Activity) { if (top === a) top = null }
        })
        thread(name = "debug-8790", isDaemon = true) {
            val server = runCatching { ServerSocket(PORT) }.getOrNull() ?: return@thread
            while (true) {
                val s = runCatching { server.accept() }.getOrNull() ?: continue
                thread(isDaemon = true) { runCatching { serve(s, app) }; runCatching { s.close() } }
            }
        }
    }

    private fun serve(s: Socket, app: Application) {
        s.soTimeout = 15000
        val line = s.getInputStream().bufferedReader().readLine() ?: return
        val target = line.split(' ').getOrNull(1) ?: return
        val path = target.substringBefore('?')
        val q = target.substringAfter('?', "").split('&').filter { '=' in it }
            .associate { URLDecoder.decode(it.substringBefore('='), "UTF-8") to URLDecoder.decode(it.substringAfter('='), "UTF-8") }
        val out = s.getOutputStream()
        try {
            when (path) {
                "/info" -> json(out, onMain { info(app) })
                "/shot" -> {
                    val png = shot(q["scale"]?.toFloatOrNull() ?: 0.5f) ?: return text(out, 503, "không chụp được")
                    send(out, 200, "image/png", png)
                }
                "/views" -> json(out, onMain { views() })
                "/tap" -> { tap(q.f("x"), q.f("y")); text(out, 200, "ok") }
                "/tapText" -> {
                    val r = onMain { findText(q["t"].orEmpty()) } ?: return text(out, 404, "không thấy chữ")
                    tap(r[0], r[1])
                    text(out, 200, "ok ${r[0].toInt()},${r[1].toInt()}")
                }
                "/swipe" -> {
                    swipe(q.f("x1"), q.f("y1"), q.f("x2"), q.f("y2"), q["ms"]?.toLongOrNull() ?: 300)
                    text(out, 200, "ok")
                }
                "/key" -> {
                    val code = KeyEvent.keyCodeFromString("KEYCODE_" + q["k"].orEmpty().uppercase())
                    if (code == KeyEvent.KEYCODE_UNKNOWN) return text(out, 400, "phím?")
                    onMain {
                        val a = top ?: return@onMain
                        // Back: sự kiện tự dựng không được "theo dõi" (startTracking) nên Activity không gọi onBackPressed
                        @Suppress("DEPRECATION")
                        if (code == KeyEvent.KEYCODE_BACK) return@onMain a.onBackPressed()
                        a.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, code))
                        a.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, code))
                    }
                    text(out, 200, "ok")
                }
                "/orient" -> {
                    Screen.debugOrientation = when (q["o"]) {
                        "portrait" -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
                        "landscape" -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
                        else -> null
                    }
                    onMain { top?.let { if (it !is PlayerActivity) Screen.allowRotation(it) } }
                    text(out, 200, "ok")
                }
                "/js" -> {
                    // Chạy JS ở khung trên cùng của WebView trên màn đang mở (vd JSON.stringify(window.__chimtoState))
                    val code = q["c"].orEmpty()
                    val done = CountDownLatch(1)
                    var res: String? = null
                    main.post {
                        val w = top?.window?.decorView?.let(::findWeb)
                        if (w == null) done.countDown() else w.evaluateJavascript(code) { res = it; done.countDown() }
                    }
                    done.await(10, TimeUnit.SECONDS)
                    text(out, if (res == null) 404 else 200, res ?: "không có WebView / không trả lời")
                }
                "/open" -> {
                    val cls = when (q["s"]) {
                        "settings" -> SettingsActivity::class.java
                        else -> MainActivity::class.java
                    }
                    onMain { (top ?: app).startActivity(Intent(top ?: app, cls).addFlags(if (top == null) Intent.FLAG_ACTIVITY_NEW_TASK else 0)) }
                    text(out, 200, "ok")
                }
                "/play" -> {
                    // Mở thẳng trình phát: /play?film=<địa chỉ phim>&ep=<địa chỉ tập>
                    val film = q["film"] ?: return text(out, 400, "thiếu film")
                    val ep = q["ep"] ?: return text(out, 400, "thiếu ep")
                    onMain {
                        val ctx = top ?: app
                        ctx.startActivity(
                            Intent(ctx, PlayerActivity::class.java).putExtra("film", film).putExtra("ep", ep)
                                .addFlags(if (top == null) Intent.FLAG_ACTIVITY_NEW_TASK else 0),
                        )
                    }
                    text(out, 200, "ok")
                }
                else -> text(out, 404, "các lệnh: /info /shot /views /tap /tapText /swipe /key /orient /open /play /js")
            }
        } catch (e: Throwable) {
            runCatching { text(out, 500, e.toString()) }
        }
    }

    private fun findWeb(v: View): android.webkit.WebView? {
        if (v is android.webkit.WebView && v.isShown) return v
        if (v is ViewGroup) for (i in 0 until v.childCount) findWeb(v.getChildAt(i))?.let { return it }
        return null
    }

    private fun Map<String, String>.f(k: String) = this[k]?.toFloatOrNull() ?: throw IllegalArgumentException("thiếu $k")

    private fun info(app: Application): JSONObject {
        val a = top
        val c = (a ?: app).resources.configuration
        val content = a?.findViewById<View>(android.R.id.content)
        return JSONObject()
            .put("app", "${BuildConfig.APPLICATION_ID} ${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})")
            .put("device", "${Build.MANUFACTURER} ${Build.MODEL}, Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})")
            .put("activity", a?.javaClass?.simpleName)
            .put("orientation", if (c.orientation == 1) "portrait" else "landscape")
            .put("requestedOrientation", a?.requestedOrientation)
            .put("screenDp", "${c.screenWidthDp}x${c.screenHeightDp}")
            .put("density", (a ?: app).resources.displayMetrics.density)
            .put("isTv", Screen.isTv(a ?: app))
            .put("narrow", Screen.narrow(a ?: app))
            .put("touchUi", TouchUi.active)
            .put("contentPadding", content?.let { "l=${it.paddingLeft} t=${it.paddingTop} r=${it.paddingRight} b=${it.paddingBottom}" })
            .put("window", content?.rootView?.let { "${it.width}x${it.height}" })
    }

    private fun shot(scale: Float): ByteArray? {
        val a = top ?: return null
        val root = onMain { a.window.decorView } ?: return null
        if (root.width == 0) return null
        val full = Bitmap.createBitmap(root.width, root.height, Bitmap.Config.ARGB_8888)
        if (Build.VERSION.SDK_INT >= 26) {
            val done = CountDownLatch(1)
            var ok = false
            main.post {
                PixelCopy.request(a.window, full, { r -> ok = r == PixelCopy.SUCCESS; done.countDown() }, main)
            }
            done.await(5, TimeUnit.SECONDS)
            if (!ok) return null
        } else {
            onMain { root.draw(android.graphics.Canvas(full)) }
        }
        val bmp = if (scale in 0.05f..0.99f) {
            Bitmap.createScaledBitmap(full, (full.width * scale).toInt(), (full.height * scale).toInt(), true)
        } else {
            full
        }
        return ByteArrayOutputStream().also { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
    }

    /** Các TextView đang hiện: chữ + khung (điểm ảnh trong cửa sổ). */
    private fun views(): JSONArray {
        val arr = JSONArray()
        walk { v, r -> arr.put(JSONObject().put("text", v.text.toString()).put("bounds", r.joinToString(",") { it.toInt().toString() })) }
        return arr
    }

    private fun findText(t: String): FloatArray? {
        var exact: FloatArray? = null
        var part: FloatArray? = null
        walk { v, r ->
            val s = v.text.toString()
            val c = floatArrayOf((r[0] + r[2]) / 2, (r[1] + r[3]) / 2)
            if (exact == null && s == t) exact = c
            if (part == null && s.contains(t, ignoreCase = true)) part = c
        }
        return exact ?: part
    }

    private fun walk(each: (TextView, FloatArray) -> Unit) {
        val root = top?.window?.decorView ?: return
        val loc = IntArray(2)
        fun go(v: View) {
            if (v.visibility != View.VISIBLE || !v.isShown) return
            if (v is TextView && v.text.isNotEmpty()) {
                v.getLocationInWindow(loc)
                if (v.width > 0 && loc[1] + v.height > 0 && loc[1] < root.height) {
                    each(v, floatArrayOf(loc[0].toFloat(), loc[1].toFloat(), (loc[0] + v.width).toFloat(), (loc[1] + v.height).toFloat()))
                }
            }
            if (v is ViewGroup) for (i in 0 until v.childCount) go(v.getChildAt(i))
        }
        go(root)
    }

    private fun tap(x: Float, y: Float) {
        val t = SystemClock.uptimeMillis()
        touch(MotionEvent.ACTION_DOWN, x, y, t, t)
        Thread.sleep(60)
        touch(MotionEvent.ACTION_UP, x, y, t, SystemClock.uptimeMillis())
    }

    private fun swipe(x1: Float, y1: Float, x2: Float, y2: Float, ms: Long) {
        val t = SystemClock.uptimeMillis()
        val steps = (ms / 16).coerceIn(5, 60).toInt()
        touch(MotionEvent.ACTION_DOWN, x1, y1, t, t)
        for (i in 1..steps) {
            Thread.sleep(ms / steps)
            val f = i.toFloat() / steps
            touch(MotionEvent.ACTION_MOVE, x1 + (x2 - x1) * f, y1 + (y2 - y1) * f, t, SystemClock.uptimeMillis())
        }
        touch(MotionEvent.ACTION_UP, x2, y2, t, SystemClock.uptimeMillis())
    }

    private fun touch(action: Int, x: Float, y: Float, down: Long, at: Long) = onMain {
        val a = top ?: return@onMain
        val e = MotionEvent.obtain(down, at, action, x, y, 0)
        e.source = InputDevice.SOURCE_TOUCHSCREEN
        a.dispatchTouchEvent(e)
        e.recycle()
    }

    private fun <T> onMain(block: () -> T): T? {
        if (Looper.myLooper() == Looper.getMainLooper()) return block()
        var r: T? = null
        val done = CountDownLatch(1)
        main.post {
            try { r = block() } finally { done.countDown() }
        }
        done.await(5, TimeUnit.SECONDS)
        return r
    }

    private fun json(out: OutputStream, o: Any?) = send(out, 200, "application/json; charset=utf-8", (if (o is JSONObject) o.toString(2) else if (o is JSONArray) o.toString(1) else "null").toByteArray())
    private fun text(out: OutputStream, code: Int, s: String) = send(out, code, "text/plain; charset=utf-8", (s + "\n").toByteArray())

    private fun send(out: OutputStream, code: Int, type: String, body: ByteArray) {
        out.write("HTTP/1.1 $code X\r\nContent-Type: $type\r\nContent-Length: ${body.size}\r\nConnection: close\r\n\r\n".toByteArray())
        out.write(body)
        out.flush()
    }
}
