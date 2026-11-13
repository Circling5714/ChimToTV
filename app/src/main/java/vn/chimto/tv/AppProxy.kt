package vn.chimto.tv

import androidx.webkit.ProxyConfig
import androidx.webkit.ProxyController
import androidx.webkit.WebViewFeature
import java.net.InetSocketAddress
import java.net.Proxy

/**
 * Proxy cho toàn app (Cài đặt → Proxy): SOCKS5, hoặc Shadowsocks (mã hoá — nhà mạng không đọc được tên trang / nội dung,
 * vd máy chủ 3X-UI; xem Shadowsocks.kt).
 *
 * WebView (Chromium) không đăng nhập được SOCKS5 có user / pass và luôn để proxy phân giải tên miền; HttpURLConnection
 * cũng tự để proxy phân giải. Nên app mở một cổng SOCKS5 nội bộ (Socks5Bridge, chỉ nghe 127.0.0.1): WebView
 * (ProxyController) và Http.kt đều đi qua cổng này, cổng nội bộ tự đăng nhập proxy thật, chọn DNS (của proxy / của
 * thiết bị) và ở chế độ "chỉ trang phim" thì cho kết nối tới nơi khác đi thẳng. Tắt proxy → app đi mạng mặc định.
 */
object AppProxy {
    private var bridge: Socks5Bridge? = null
    private var bridgeConfig: RouteConfig? = null
    private var webViewSet = false

    /** WebView của TV không có ProxyController (bản quá cũ) → trình phát / Duyệt web vẫn đi thẳng. */
    var webViewSupported = true
        private set

    /** Đang bật proxy (SOCKS5 hoặc Shadowsocks). */
    val active: Boolean get() = bridgeConfig?.proxied == true

    /** Đang bật DNS mã hoá (DoH / DoT) — cổng nội bộ chạy cả khi không có proxy. */
    val dohActive: Boolean get() = bridgeConfig?.dns == true

    /**
     * Cổng cho Http.kt / WebView; null = tắt (đi mạng mặc định của máy như trước). Bật mà cổng nội bộ không mở được
     * (rất hiếm) thì trỏ vào cổng đóng: kết nối báo lỗi chứ không lén đi thẳng.
     */
    private fun localPort(): Int? = bridge?.port ?: bridgeConfig?.let { 1 }

    fun javaProxy(): Proxy? = localPort()?.let { Proxy(Proxy.Type.SOCKS, InetSocketAddress("127.0.0.1", it)) }

    /**
     * Áp dụng cài đặt trong Prefs — lúc mở app và mỗi lần đổi trong Cài đặt. Gọi ở luồng chính.
     * Cài đặt khác đi thì mở cổng nội bộ mới (cổng khác) và đóng cổng cũ cùng mọi kết nối đang giữ: WebView và
     * Http.kt buộc phải kết nối lại theo cài đặt mới ngay (không dùng lại kết nối keep-alive đi theo cài đặt cũ).
     */
    fun apply() {
        if (Level2Memo.save == null) {
            // Trang đã tự lên Mức 2 (các lần mở app trước): nạp lại, ghi xuống máy mỗi lần nhớ thêm
            Level2Memo.load(Prefs.level2Hosts)
            Level2Memo.save = { Prefs.level2Hosts = it }
        }
        val c = RouteConfig(Prefs.proxyConfig(), Prefs.secureDns, Prefs.unblock, Prefs.unblock && Prefs.unblockLevel >= 2, Prefs.ssConfig())
            .takeIf { it.proxied || it.dns || it.split }
        if (c == bridgeConfig && (c == null) == (bridge == null)) return
        bridge?.close()
        Doh.clearCache()
        bridge = c?.let { cfg -> runCatching { Socks5Bridge({ cfg }, ::siteHosts) }.getOrNull() }
        if (BuildConfig.DEBUG) bridge?.debugLog = { android.util.Log.d("ChimToTtl", it) }
        bridgeConfig = c
        setWebView(localPort())
    }

    /** Số gói bắt tay đã gửi đảo thứ tự (Vượt chặn mức 2) từ lúc mở cổng nội bộ — để thử. */
    val disorderCount: Int get() = bridge?.disorderCount ?: 0

    /** Số lần tự lên Mức 2 (trang không trả lời ở Mức 1) từ lúc mở cổng nội bộ — để thử. */
    val autoLevel2Count: Int get() = bridge?.autoLevel2 ?: 0

    /** Lỗi proxy trong 30 giây gần đây, để ghép vào thông báo lỗi (vd "proxy báo sai tên đăng nhập / mật khẩu"). */
    fun recentError(): String? = if (bridgeConfig != null && bridge == null) "không mở được cổng nội bộ"
    else bridge?.takeIf { System.currentTimeMillis() - it.lastErrorAt < 30_000 }?.lastError

    /** Câu nhắc thêm vào thông báo lỗi khi đang bật proxy. */
    fun note(): String = when {
        active -> "\nĐang bật proxy" + (recentError()?.let { ": $it" } ?: "") +  " — kiểm tra Cài đặt → Proxy, hoặc tắt proxy."
        else -> ""
    }

    /** Video cũng đi qua proxy (chế độ "toàn bộ app"). */
    private val videoViaProxy get() = bridgeConfig?.sitesOnly == false

    /**
     * Câu nhắc cho lỗi video: máy chủ video hay chặn IP của proxy (vd CDN của KK Phim trả 404 với IP của proxy dù
     * trang phim vẫn vào được) → gợi ý cho video đi thẳng.
     */
    fun videoNote(): String = if (!videoViaProxy || recentError() != null) note()
    else "\nĐang bật proxy cho toàn bộ app: máy chủ video có thể chặn IP của proxy — thử Cài đặt → Proxy → " +
        "«Dùng cho: chỉ trang phim», hoặc tắt proxy."

    /**
     * Khoá chấm điểm server (PlayStats): video đi qua proxy thì chấm riêng — server bị chặn IP proxy không bị coi là
     * hỏng khi xem thẳng, và ngược lại.
     */
    fun statsHost(host: String?): String? = if (videoViaProxy) "$host·proxy" else host

    private fun siteHosts(): Set<String> = (Prefs.sites + Prefs.baseUrl).mapNotNull { Site.hostOf(it) }.toSet()

    private fun setWebView(port: Int?) {
        if (port == null && !webViewSet) return // chưa từng đặt: khỏi nạp WebView lúc mở app
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE)) {
            webViewSupported = false
            return
        }
        val pc = ProxyController.getInstance()
        if (port == null) {
            pc.clearProxyOverride({ it.run() }, {})
            webViewSet = false
        } else {
            // Không thêm luật "đi thẳng" dự phòng: proxy lỗi thì báo lỗi, không lén đi thẳng mạng nhà
            pc.setProxyOverride(ProxyConfig.Builder().addProxyRule("socks5://127.0.0.1:$port").build(), { it.run() }, {})
            webViewSet = true
        }
    }
}
