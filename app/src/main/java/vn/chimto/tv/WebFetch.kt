package vn.chimto.tv

import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import org.mozilla.geckoview.GeckoResult
import org.mozilla.geckoview.GeckoRuntime
import org.mozilla.geckoview.GeckoRuntimeSettings
import org.mozilla.geckoview.GeckoSession
import org.mozilla.geckoview.GeckoSessionSettings
import org.mozilla.geckoview.GeckoWebExecutor
import org.mozilla.geckoview.WebRequest
import org.mozilla.geckoview.WebResponse
import java.io.File
import java.io.IOException
import java.net.URI
import java.nio.ByteBuffer
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * Đọc trang bật lớp chống bot của Cloudflare (hoathinh3d.pw từ 14/11/2026: 403 «Just a moment…» với mọi trình tải không phải
 * trình duyệt — cả trang, API, ảnh; còn WebView của Android thì bị chặn hẳn «you have been blocked»). Chỉ những trang này đi qua
 * engine **Gecko** (GeckoView — engine của Firefox, gói kèm app); trang bình thường vẫn như cũ (HttpURLConnection + WebView).
 *
 * Cách đọc: một GeckoSession mở trang chủ cho trình duyệt tự qua lớp chặn (nhận ra qua tiêu đề trang: «Just a moment…» = đang
 * chờ, «Attention Required! | Cloudflare» = trang chặn hẳn), rồi đóng session; mọi yêu cầu sau đó (trang, API, ảnh, POST) đi qua
 * GeckoWebExecutor — mạng của Gecko, chung cookie với session. Gecko đi qua cổng SOCKS nội bộ riêng của app (Socks5Bridge) nên
 * proxy / Shadowsocks / DNS mã hoá / vượt chặn vẫn áp dụng. Không giả / không giải hộ: trang đòi bấm xác nhận / chặn hẳn cả Gecko
 * thì báo lỗi. Trang bị chặn được nhớ 12 giờ (Http.get / post, ImageLoader, khung trình phát đi thẳng qua đây).
 */
object WebFetch {
    class Raw(val code: Int, val url: String, val type: String?, val cf: String?, val data: ByteArray)

    /** Trang bật chống bot mà app chưa đọc được. */
    class ChallengeError(val host: String) :
        IOException("Trang $host đang bật lớp chống bot của Cloudflare — app chưa đọc được (thử lại sau, hoặc xem phim này ở nguồn khác).")

    /** Cloudflare của trang chặn hẳn cả engine Gecko — quyết định của chủ trang. */
    class BlockedError(val host: String) :
        IOException("Trang $host chặn app này (Cloudflare «you have been blocked») — xem phim này ở nguồn khác, vd KKPhim.")

    private const val TAG = "ChimToWeb"
    const val KEEP_MS = 12 * 3600_000L
    private const val PASS_MS = 30_000L
    private const val CALL_MS = 45_000L
    private const val BLOCKED_MS = 30 * 60_000L
    /** GeckoView 141 chạy từ Android 5 (như app); để sẵn chỗ chặn nếu bản Gecko sau đòi cao hơn. */
    val supported: Boolean get() = Build.VERSION.SDK_INT >= 21

    private val hosts = ConcurrentHashMap<String, Long>()
    private lateinit var app: Context
    private val main by lazy { Handler(Looper.getMainLooper()) }

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

    /** Trang chặn hẳn của Cloudflare («Attention Required! … Sorry, you have been blocked»). */
    fun isBlockPage(code: Int, body: ByteArray?): Boolean {
        if (code != 403) return false
        val head = body?.let { String(it, 0, minOf(it.size, 8192), Charsets.UTF_8).lowercase() } ?: return false
        return "<title>attention required! | cloudflare" in head || "cf-error-details" in head
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
     * Đọc qua Gecko — gọi từ luồng nền (chờ tới khi xong). Lớp chặn quay lại giữa chừng (cookie hết hạn) → qua lại lớp chặn,
     * thử thêm một lần.
     */
    fun fetch(url: String, method: String = "GET", body: String? = null, headers: Map<String, String> = emptyMap()): Raw {
        check(Looper.myLooper() != Looper.getMainLooper()) { "WebFetch.fetch không được chạy trên luồng giao diện" }
        val host = hostOf(url)
        if (!supported) throw ChallengeError(host)
        repeat(2) { attempt ->
            Gecko.pass(host)
            val r = Gecko.request(url, method, body, headers)
            if (!isChallenge(r.code, r.cf, r.data)) {
                if (isBlockPage(r.code, r.data)) throw BlockedError(host)
                return r
            }
            if (attempt == 0) Gecko.invalidate(host)
        }
        throw ChallengeError(host)
    }

    /** Http.Response như Http.get / post (lỗi HTTP ≥ 400 → Http.HttpError). */
    fun response(url: String, method: String = "GET", body: String? = null, headers: Map<String, String> = emptyMap()): Http.Response {
        val r = fetch(url, method, body, headers)
        Log.d(TAG, "$url → ${r.code} qua Gecko, ${r.data.size} byte")
        if (r.code >= 400) throw Http.HttpError(r.code)
        return Http.Response(r.url.ifEmpty { url }, r.code, r.data.toString(Http.charsetOf(r.type)))
    }

    /** Chạy trên luồng giao diện, chờ ở luồng nền. */
    private fun <T> onMain(timeoutMs: Long, start: (done: (Result<T>) -> Unit) -> Unit): T {
        val latch = CountDownLatch(1)
        val out = AtomicReference<Result<T>>()
        main.post {
            try {
                start { r -> if (out.compareAndSet(null, r)) latch.countDown() }
            } catch (e: Throwable) {
                if (out.compareAndSet(null, Result.failure(e))) latch.countDown()
            }
        }
        if (!latch.await(timeoutMs, TimeUnit.MILLISECONDS)) throw IOException("Gecko không trả lời")
        return out.get().getOrThrow()
    }

    /** Engine Gecko: tạo lần đầu cần (một lần mỗi tiến trình — GeckoRuntime không tạo lại được), giữ tới khi tắt app. */
    private object Gecko {
        private var runtime: GeckoRuntime? = null
        private var executor: GeckoWebExecutor? = null
        /** Cổng SOCKS riêng cho Gecko (cố định suốt phiên; đi theo cài đặt mạng hiện tại của app lúc mở từng kết nối). */
        private var bridge: Socks5Bridge? = null
        /** Chỉ dùng trên luồng giao diện. */
        private val passed = HashSet<String>()
        private val blockedUntil = HashMap<String, Long>()
        private val opening = HashMap<String, MutableList<(Throwable?) -> Unit>>()

        private fun runtime(): GeckoRuntime {
            runtime?.let { return it }
            val b = bridge ?: runCatching {
                Socks5Bridge({ AppProxy.currentConfig() ?: RouteConfig(null, false) }, { (Prefs.sites + Prefs.baseUrl).mapNotNull { Site.hostOf(it) } })
            }.getOrNull().also { bridge = it }
            // Tuỳ chọn của Gecko (prefs) đọc lúc khởi động: đi qua cổng SOCKS nội bộ, để cổng phân giải tên miền (DNS mã hoá / của proxy)
            val cfg = File(app.filesDir, "geckoview-config.yaml")
            cfg.writeText(
                buildString {
                    append("prefs:\n")
                    if (b != null) {
                        append("  network.proxy.type: 1\n")
                        append("  network.proxy.socks: \"127.0.0.1\"\n")
                        append("  network.proxy.socks_port: ${b.port}\n")
                        append("  network.proxy.socks_version: 5\n")
                        append("  network.proxy.socks_remote_dns: true\n")
                        append("  network.proxy.allow_hijacking_localhost: false\n")
                    }
                    append("  media.autoplay.default: 5\n") // không tự phát âm thanh / video
                    append("  permissions.default.image: 2\n") // trang chủ mở để qua lớp chặn: khỏi tải ảnh
                    append("  browser.cache.disk.enable: false\n")
                    // Gecko ở đây không vẽ gì lên màn hình: khỏi tiến trình GPU, tiến trình chạy sẵn — nhẹ RAM cho TV box
                    append("  layers.gpu-process.enabled: false\n")
                    append("  dom.ipc.processPrelaunch.enabled: false\n")
                    append("  dom.ipc.processCount: 1\n")
                    append("  dom.ipc.keepProcessesAlive.web: 0\n") // đóng session (đã qua lớp chặn) là trả RAM của tiến trình trang
                    append("  browser.sessionhistory.max_total_viewers: 0\n")
                    append("env:\n")
                    append("  MOZ_CRASHREPORTER_DISABLE: \"1\"\n") // không cần tiến trình báo lỗi (crashhelper)
                },
            )
            val settings = GeckoRuntimeSettings.Builder()
                .configFilePath(cfg.absolutePath)
                .consoleOutput(false)
                .javaScriptEnabled(true)
                .remoteDebuggingEnabled(BuildConfig.DEBUG)
                .build()
            val r = GeckoRuntime.create(app, settings)
            runtime = r
            executor = GeckoWebExecutor(r)
            Log.d(TAG, "Gecko khởi động (cổng SOCKS riêng ${b?.port ?: "không có"})")
            return r
        }

        fun invalidate(host: String) {
            main.post { passed.remove(host) }
        }

        /** Chờ tới khi trình duyệt Gecko đã qua lớp chặn của trang (lần đầu mở trang chủ; các lần sau trả ngay). */
        fun pass(host: String) = onMain<Unit>(PASS_MS + 10_000) { done ->
            val until = blockedUntil[host] ?: 0
            when {
                System.currentTimeMillis() < until -> done(Result.failure(BlockedError(host)))
                host in passed -> done(Result.success(Unit))
                else -> {
                    val waiting = opening[host]
                    val cb: (Throwable?) -> Unit = { e -> done(if (e == null) Result.success(Unit) else Result.failure(e)) }
                    if (waiting != null) waiting += cb else {
                        opening[host] = mutableListOf(cb)
                        open(host)
                    }
                }
            }
        }

        private fun finish(host: String, e: Throwable?) {
            if (e == null) passed += host
            opening.remove(host)?.forEach { it(e) }
        }

        private fun open(host: String) {
            val r = try {
                runtime()
            } catch (e: Throwable) {
                Log.d(TAG, "Không khởi động được Gecko: $e")
                return finish(host, IOException("Không khởi động được engine Gecko: ${e.message}"))
            }
            val t0 = System.currentTimeMillis()
            var title = ""
            var titleAt = t0
            var stopped = false
            val s = GeckoSession(GeckoSessionSettings.Builder().usePrivateMode(false).build())
            s.contentDelegate = object : GeckoSession.ContentDelegate {
                override fun onTitleChange(session: GeckoSession, t: String?) {
                    if (t.orEmpty() != title) titleAt = System.currentTimeMillis()
                    title = t.orEmpty()
                }
            }
            s.progressDelegate = object : GeckoSession.ProgressDelegate {
                override fun onPageStart(session: GeckoSession, url: String) {
                    stopped = false
                }

                override fun onPageStop(session: GeckoSession, success: Boolean) {
                    stopped = true
                }
            }
            s.open(r)
            s.setActive(true)
            s.loadUri("https://$host/")
            fun done(e: Throwable?) {
                try {
                    s.close()
                } catch (x: Throwable) {
                }
                finish(host, e)
            }
            fun probe() {
                val ms = System.currentTimeMillis() - t0
                when {
                    title.startsWith("Attention Required", ignoreCase = true) -> {
                        Log.d(TAG, "$host: Cloudflare chặn hẳn cả Gecko")
                        blockedUntil[host] = System.currentTimeMillis() + BLOCKED_MS
                        done(BlockedError(host))
                    }
                    // Tiêu đề của trang thật đã hiện (cookie qua lớp chặn đặt trước khi trang chặn nạp lại): tải xong, hoặc tiêu
                    // đề đứng yên 1,5 giây (trang còn tải quảng cáo / mã khác thì onPageStop có khi không tới) — yêu cầu đầu tiên
                    // vẫn kiểm lại (còn lớp chặn thì qua lại một lần)
                    title.isNotEmpty() && !title.startsWith("Just a moment", ignoreCase = true) && !title.startsWith("http") &&
                        !title.equals(host, ignoreCase = true) && (stopped || System.currentTimeMillis() - titleAt > 1500) -> {
                        Log.d(TAG, "$host: Gecko đã qua lớp chống bot ($ms ms, «$title»)")
                        done(null)
                    }
                    ms > PASS_MS -> {
                        Log.d(TAG, "$host: Gecko chưa qua lớp chặn sau ${PASS_MS / 1000} giây («$title»)")
                        done(ChallengeError(host))
                    }
                    else -> main.postDelayed({ probe() }, 400)
                }
            }
            main.postDelayed({ probe() }, 400)
        }

        /** Một yêu cầu qua mạng của Gecko (chung cookie với session đã qua lớp chặn). */
        fun request(url: String, method: String, body: String?, headers: Map<String, String>): Raw {
            val result = onMain<GeckoResult<WebResponse>>(10_000) { done ->
                val b = WebRequest.Builder(url).method(method)
                for ((k, v) in headers) {
                    when {
                        k.equals("Referer", true) -> b.referrer(v)
                        k.equals("User-Agent", true) || k.equals("Cookie", true) || k.equals("Accept-Encoding", true) -> {}
                        else -> b.addHeader(k, v)
                    }
                }
                if (body != null) {
                    val bytes = body.toByteArray(Charsets.UTF_8)
                    b.body(ByteBuffer.allocateDirect(bytes.size).put(bytes).also { it.flip() })
                }
                done(Result.success(executor!!.fetch(b.build(), GeckoWebExecutor.FETCH_FLAGS_NONE)))
            }
            val resp = try {
                result.poll(CALL_MS)!!
            } catch (e: Throwable) {
                throw IOException("Gecko đọc $url lỗi: ${e.message ?: e.javaClass.simpleName}")
            }
            val data = resp.body?.use { it.readBytes() } ?: ByteArray(0)
            fun header(name: String) = resp.headers.entries.firstOrNull { it.key.equals(name, true) }?.value
            return Raw(resp.statusCode, resp.uri, header("Content-Type"), header("cf-mitigated"), data)
        }
    }
}
