package vn.chimto.tv

import android.annotation.SuppressLint
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Base64
import android.util.Log
import android.webkit.ConsoleMessage
import android.webkit.CookieManager
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import org.json.JSONObject
import java.io.IOException
import java.net.URI
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * Đọc hộ trang bật lớp chống bot của Cloudflare (hoathinh3d.pw từ 14/11/2026: 403 «Just a moment…» + `cf-mitigated: challenge`
 * với mọi trình tải không phải trình duyệt — cả trang, API, ảnh). Trình duyệt thật tự qua lớp chặn: một WebView ẩn (không gắn
 * vào màn hình) mở trang chủ, rồi mỗi yêu cầu là fetch() ngay trong trang đó (assets/web-fetch.js — dùng chung với bản Windows),
 * cookie dùng chung với WebView của trình phát (CookieManager). Http.get / post, ImageLoader và khung trình phát gọi tới đây khi
 * trang bị chặn; trang bị chặn được nhớ 12 giờ. Không giả / không giải hộ lớp chặn: trang đòi bấm xác nhận thì báo lỗi.
 * 5 phút không dùng thì huỷ WebView (TV box ít RAM).
 */
object WebFetch {
    class Raw(val code: Int, val url: String, val type: String?, val cf: String?, val data: ByteArray)

    /** Trang bật chống bot mà app chưa đọc hộ được. */
    class ChallengeError(val host: String) :
        IOException("Trang $host đang bật lớp chống bot của Cloudflare — app chưa đọc được (thử lại sau, hoặc xem phim này ở nguồn khác).")

    /** Cloudflare của trang chặn hẳn app («Attention Required! … you have been blocked») — quyết định của chủ trang, không lách. */
    class BlockedError(val host: String) :
        IOException("Trang $host chặn app này (Cloudflare «you have been blocked») — xem phim này ở nguồn khác, vd KKPhim.")

    private const val TAG = "ChimToWeb"
    const val KEEP_MS = 12 * 3600_000L
    private const val PASS_MS = 30_000L
    private const val CALL_MS = 45_000L
    private const val IDLE_MS = 5 * 60_000L
    /** Bị chặn hẳn thì thôi mở WebView lại trong 30 phút (báo lỗi ngay). */
    private const val BLOCKED_MS = 30 * 60_000L
    private const val REPLY_JS = "window.__cbReply=function(m){try{ChimtoWeb.reply(JSON.stringify(m))}catch(e){}};"
    private const val PROBE_JS = "JSON.stringify([location.host, document.readyState, document.title || '', typeof window._cf_chl_opt, " +
        "!!document.querySelector('#challenge-form,#challenge-stage,#cf-challenge-running,.cf-turnstile'), " +
        "!!document.querySelector('#cf-error-details,.cf-error-details') || /you have been blocked/i.test(document.body ? document.body.innerText.slice(0, 400) : '')])"

    private val hosts = ConcurrentHashMap<String, Long>()
    private lateinit var app: Context
    private val main by lazy { Handler(Looper.getMainLooper()) }
    /** Chỉ dùng trên luồng giao diện. */
    private val sessions = HashMap<String, Session>()
    private val fetchJs by lazy { app.assets.open("web-fetch.js").bufferedReader().use { it.readText() } }

    fun init(context: Context) {
        app = context.applicationContext
    }

    fun hostOf(url: String): String = try {
        URI(url).host?.lowercase().orEmpty()
    } catch (e: Exception) {
        ""
    }

    fun remember(host: String, now: Long = System.currentTimeMillis()) {
        if (host.isNotEmpty()) hosts[host.lowercase()] = now
    }

    /** Trang của địa chỉ này đang bị nhớ là chặn (trong 12 giờ). */
    fun needs(url: String, now: Long = System.currentTimeMillis()): Boolean {
        val t = hosts[hostOf(url)] ?: return false
        return now - t in 0 until KEEP_MS
    }

    fun forgetAll() = hosts.clear()

    /**
     * Trả lời này là lớp chặn của Cloudflare (không phải lỗi thật của trang): đầu thư `cf-mitigated: challenge`, hoặc 403 / 503 mà
     * nội dung là trang «Just a moment…» / mã của trang chặn (`_cf_chl_opt`). Cùng luật với lõi Rust (web_fetch.rs).
     */
    fun isChallenge(code: Int, cfMitigated: String?, body: ByteArray?): Boolean {
        if (cfMitigated?.trim().equals("challenge", ignoreCase = true)) return true
        if (code != 403 && code != 503) return false
        val head = body?.let { String(it, 0, minOf(it.size, 8192), Charsets.UTF_8).lowercase() } ?: return false
        return "<title>just a moment" in head || "_cf_chl_opt" in head || "/cdn-cgi/challenge-platform/h/" in head
    }

    /** Đọc tối đa 8 KB đầu của luồng lỗi (đủ để nhận ra trang chặn). */
    fun head(input: java.io.InputStream?): ByteArray? = input?.use {
        val buf = ByteArray(8192)
        var n = 0
        while (n < buf.size) {
            val r = it.read(buf, n, buf.size - n)
            if (r < 0) break
            n += r
        }
        buf.copyOf(n)
    }

    /**
     * Đọc qua WebView ẩn — gọi từ luồng nền (chờ tới khi xong). Lớp chặn quay lại giữa chừng (cookie hết hạn) → qua lại lớp
     * chặn, thử thêm một lần.
     */
    fun fetch(url: String, method: String = "GET", body: String? = null, headers: Map<String, String> = emptyMap()): Raw {
        check(Looper.myLooper() != Looper.getMainLooper()) { "WebFetch.fetch không được chạy trên luồng giao diện" }
        val host = hostOf(url)
        repeat(2) { attempt ->
            val r = await(CALL_MS + PASS_MS + 5_000) { done -> session(host).run(url, method, body, headers, done) }
            if (!r.cf.equals("challenge", ignoreCase = true)) return r
            if (attempt == 0) main.post { sessions[host]?.invalidate() }
        }
        throw ChallengeError(host)
    }

    /** Http.Response như Http.get / post (lỗi HTTP ≥ 400 → Http.HttpError; vẫn là trang chặn → ChallengeError). */
    fun response(url: String, method: String = "GET", body: String? = null, headers: Map<String, String> = emptyMap()): Http.Response {
        val r = fetch(url, method, body, headers)
        Log.d(TAG, "$url → ${r.code} qua WebView, ${r.data.size} byte")
        if (isChallenge(r.code, r.cf, r.data)) throw ChallengeError(hostOf(url))
        if (r.code >= 400) throw Http.HttpError(r.code)
        return Http.Response(r.url.ifEmpty { url }, r.code, r.data.toString(Http.charsetOf(r.type)))
    }

    private fun <T> await(timeoutMs: Long, start: (done: (Result<T>) -> Unit) -> Unit): T {
        val latch = CountDownLatch(1)
        val out = AtomicReference<Result<T>>()
        main.post {
            try {
                start { r ->
                    if (out.compareAndSet(null, r)) latch.countDown()
                }
            } catch (e: Exception) {
                if (out.compareAndSet(null, Result.failure(e))) latch.countDown()
            }
        }
        if (!latch.await(timeoutMs, TimeUnit.MILLISECONDS)) throw IOException("WebView không trả lời")
        return out.get().getOrThrow()
    }

    private fun session(host: String): Session = sessions.getOrPut(host) { Session(host) }

    /** Một trang bị chặn = một WebView ẩn. Mọi hàm chạy trên luồng giao diện. */
    private class Session(val host: String) {
        private var web: WebView? = null
        private var ready = false
        private var opening = false
        private val waiters = ArrayList<(Throwable?) -> Unit>()
        private val calls = HashMap<Long, (Result<Raw>) -> Unit>()
        private var next = 0L
        private var onFinished: (() -> Unit)? = null
        private var blockedUntil = 0L
        private val idle = Runnable { close("không dùng 5 phút") }

        fun invalidate() {
            ready = false
        }

        fun run(url: String, method: String, body: String?, headers: Map<String, String>, done: (Result<Raw>) -> Unit) {
            main.removeCallbacks(idle)
            main.postDelayed(idle, IDLE_MS)
            whenReady { err ->
                if (err != null) done(Result.failure(err)) else call(url, method, body, headers, done)
            }
        }

        private fun whenReady(cb: (Throwable?) -> Unit) {
            if (System.currentTimeMillis() < blockedUntil) return cb(BlockedError(host))
            if (ready && web != null) return cb(null)
            waiters += cb
            if (!opening) open()
        }

        private fun finishOpen(err: Throwable?) {
            opening = false
            ready = err == null
            val list = ArrayList(waiters)
            waiters.clear()
            list.forEach { it(err) }
        }

        @SuppressLint("SetJavaScriptEnabled", "JavascriptInterface")
        private fun create(): WebView {
            web?.let { return it }
            val w = WebView(app)
            w.settings.apply {
                javaScriptEnabled = true
                domStorageEnabled = true
                blockNetworkImage = true
                mediaPlaybackRequiresUserGesture = true
            }
            Privacy.apply(w)
            CookieManager.getInstance().setAcceptCookie(true)
            CookieManager.getInstance().setAcceptThirdPartyCookies(w, true)
            w.webViewClient = object : WebViewClient() {
                override fun shouldInterceptRequest(view: WebView, req: WebResourceRequest): WebResourceResponse? =
                    if (AdBlock.isAd(req.url.host)) AdBlock.empty() else null

                override fun onPageFinished(view: WebView, url: String) {
                    onFinished?.let { onFinished = null; it() }
                }
            }
            w.webChromeClient = object : WebChromeClient() {
                override fun onConsoleMessage(consoleMessage: ConsoleMessage) = true
            }
            w.addJavascriptInterface(Bridge(this), "ChimtoWeb")
            web = w
            return w
        }

        private fun open() {
            opening = true
            val w = try {
                create()
            } catch (e: Exception) {
                return finishOpen(IOException("Không mở được WebView: ${e.message}"))
            }
            val t0 = System.currentTimeMillis()
            w.loadUrl("https://$host/")
            // Trang chặn («Just a moment…», window._cf_chl_opt) tự giải rồi nạp lại trang thật
            fun probe() {
                if (web !== w) return
                w.evaluateJavascript(PROBE_JS) { raw ->
                    if (web !== w) return@evaluateJavascript
                    val st = state(raw)
                    if (st == "blocked") {
                        Log.d(TAG, "$host: Cloudflare của trang chặn hẳn app — không đọc được")
                        blockedUntil = System.currentTimeMillis() + BLOCKED_MS
                        finishOpen(BlockedError(host))
                        main.post { close("trang chặn app") }
                    } else if (st == "passed") {
                        val ms = System.currentTimeMillis() - t0
                        Log.d(TAG, "$host: WebView đã qua lớp chống bot ($ms ms)")
                        // Đứng ở trang nhẹ cùng gốc (khỏi chạy quảng cáo / mã của trang chủ) — fetch() vẫn mang cookie của trang
                        onFinished = {
                            w.evaluateJavascript(REPLY_JS + fetchJs, null)
                            finishOpen(null)
                        }
                        w.loadUrl("https://$host/robots.txt")
                    } else if (System.currentTimeMillis() - t0 > PASS_MS) {
                        Log.d(TAG, "$host: chưa qua lớp chặn sau ${PASS_MS / 1000} giây")
                        finishOpen(ChallengeError(host))
                    } else {
                        main.postDelayed({ probe() }, 500)
                    }
                }
            }
            main.postDelayed({ probe() }, 500)
        }

        /** "passed" = trang thật đã mở; "blocked" = Cloudflare chặn hẳn; "wait" = còn ở trang thử thách / đang nạp. */
        private fun state(raw: String?): String = try {
            val s = org.json.JSONTokener(raw ?: "null").nextValue() as? String ?: return "wait"
            val a = org.json.JSONArray(s)
            val title = a.optString(2)
            when {
                a.optString(0).equals(host, ignoreCase = true) && a.optString(1) != "loading" &&
                    (a.optBoolean(5) || title.startsWith("Attention Required", ignoreCase = true)) -> "blocked"
                a.optString(0).equals(host, ignoreCase = true) && a.optString(1) != "loading" && a.optString(3) == "undefined" &&
                    !a.optBoolean(4) && !title.startsWith("Just a moment", ignoreCase = true) -> "passed"
                else -> "wait"
            }
        } catch (e: Exception) {
            "wait"
        }

        private fun call(url: String, method: String, body: String?, headers: Map<String, String>, done: (Result<Raw>) -> Unit) {
            val w = web ?: return done(Result.failure(IOException("WebView của $host đã đóng")))
            val id = ++next
            calls[id] = done
            main.postDelayed({ calls.remove(id)?.invoke(Result.failure(IOException("WebView không trả lời ($url)"))) }, CALL_MS)
            val js = "${REPLY_JS}${fetchJs}\nwindow.__cbFetch($id,${JSONObject.quote(url)},${JSONObject.quote(method)}," +
                "${if (body == null) "null" else JSONObject.quote(body)},${JSONObject(headers)});0"
            w.evaluateJavascript(js, null)
        }

        /** Trả lời của web-fetch.js (luồng JavaBridge → chuyển về luồng giao diện). */
        fun reply(json: String) = main.post {
            val m = try {
                JSONObject(json)
            } catch (e: Exception) {
                return@post
            }
            val cb = calls.remove(m.optLong("id")) ?: return@post
            if (m.has("err")) {
                cb(Result.failure(IOException("WebView đọc lỗi: ${m.optString("err")}")))
                return@post
            }
            val data = try {
                Base64.decode(m.optString("b64"), Base64.DEFAULT)
            } catch (e: Exception) {
                ByteArray(0)
            }
            cb(
                Result.success(
                    Raw(m.optInt("code"), m.optString("url"), m.text("type"), m.text("cf"), data),
                ),
            )
        }

        fun close(why: String) {
            if (calls.isNotEmpty() || opening) {
                main.postDelayed(idle, IDLE_MS)
                return
            }
            web?.let {
                it.stopLoading()
                it.destroy()
            }
            web = null
            ready = false
            Log.d(TAG, "$host: huỷ WebView ẩn ($why)")
        }
    }

    /** Chuỗi hoặc null (optString trả chữ "null" cho giá trị null của JSON). */
    private fun JSONObject.text(k: String): String? = if (isNull(k)) null else optString(k).ifEmpty { null }

    private class Bridge(val s: Session) {
        @JavascriptInterface
        fun reply(json: String) {
            s.reply(json)
        }
    }
}
