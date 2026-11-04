package vn.chimto.tv

import androidx.webkit.ProxyConfig
import androidx.webkit.ProxyController
import androidx.webkit.WebViewFeature
import java.net.InetSocketAddress
import java.net.Proxy

/**
 * Proxy SOCKS5 cho toàn app (Cài đặt → Proxy SOCKS5).
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

    /** Đang bật proxy SOCKS5. */
    val active: Boolean get() = bridgeConfig?.socks != null

    /** Đang bật DNS mã hoá (DoH) — cổng nội bộ chạy cả khi không có proxy. */
    val dohActive: Boolean get() = bridgeConfig?.doh != null

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
        val c = RouteConfig(Prefs.proxyConfig(), Prefs.dohServer).takeIf { it.socks != null || it.doh != null }
        if (c == bridgeConfig && (c == null) == (bridge == null)) return
        bridge?.close()
        Doh.clearCache()
        bridge = c?.let { cfg -> runCatching { Socks5Bridge({ cfg }, ::siteHosts) }.getOrNull() }
        bridgeConfig = c
        setWebView(localPort())
    }

    /** Lỗi proxy trong 30 giây gần đây, để ghép vào thông báo lỗi (vd "proxy báo sai tên đăng nhập / mật khẩu"). */
    fun recentError(): String? = if (bridgeConfig != null && bridge == null) "không mở được cổng nội bộ"
    else bridge?.takeIf { System.currentTimeMillis() - it.lastErrorAt < 30_000 }?.lastError

    /** Câu nhắc thêm vào thông báo lỗi khi đang bật proxy. */
    fun note(): String = when {
        active -> "\nĐang bật proxy" + (recentError()?.let { ": $it" } ?: "") + " — kiểm tra Cài đặt → Proxy SOCKS5, hoặc tắt proxy."
        dohActive -> "\nĐang bật DNS mã hoá" + (recentError()?.let { ": $it" } ?: "") +
            " — thử máy chủ DNS khác hoặc tắt trong Cài đặt → DNS mã hoá."
        else -> ""
    }

    /** Video cũng đi qua proxy (chế độ "toàn bộ app"). */
    private val videoViaProxy get() = bridgeConfig?.socks?.sitesOnly == false

    /**
     * Câu nhắc cho lỗi video: máy chủ video hay chặn IP của proxy (vd CDN của KK Phim trả 404 với IP của proxy dù
     * trang phim vẫn vào được) → gợi ý cho video đi thẳng.
     */
    fun videoNote(): String = if (!videoViaProxy || recentError() != null) note()
    else "\nĐang bật proxy cho toàn bộ app: máy chủ video có thể chặn IP của proxy — thử Cài đặt → Proxy SOCKS5 → " +
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
