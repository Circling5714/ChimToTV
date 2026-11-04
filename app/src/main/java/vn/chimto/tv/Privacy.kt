package vn.chimto.tv

import android.annotation.SuppressLint
import android.content.Context
import android.webkit.WebSettings
import android.webkit.WebView
import androidx.webkit.UserAgentMetadata
import androidx.webkit.WebSettingsCompat
import androidx.webkit.WebViewFeature

/**
 * Riêng tư khi truy cập trang phim: app khai ra ngoài ít thông tin nhất, như nhau cho WebView lẫn Http.kt.
 * - X-Requested-With: WebView mặc định gắn tên gói app vào mọi yêu cầu. Http.kt không gửi; trang iframe app tải hộ
 *   (PlayerActivity) bỏ header này; với yêu cầu WebView tự gửi thì đặt danh sách cho phép rỗng (WebView 138 vẫn gửi).
 * - User-Agent dạng rút gọn chuẩn của Chromium ("Android 10; K", "Chrome/<số chính>.0.0.0"): không lộ model máy, mã
 *   build, bản Android, số phiên bản đầy đủ; không có "; wv" (một số trình phát chặn WebView). Http.kt dùng cùng chuỗi.
 * - Client hints (sec-ch-ua…): thương hiệu "Chromium" (WebView chính là Chromium) thay cho "Android WebView",
 *   không kèm model / bản Android / số phiên bản đầy đủ.
 */
object Privacy {
    /** Dùng khi chưa biết số phiên bản Chromium của máy (chưa từng nạp WebView). */
    private const val FALLBACK_MAJOR = "138"

    @Volatile var userAgent: String = reduce("Chrome/$FALLBACK_MAJOR.0.0.0 Mobile")
        private set

    /** Lúc mở app: lấy User-Agent đã lưu; lần đầu (mới cài / cập nhật) thì hỏi WebView ngay. */
    fun init(context: Context) {
        Prefs.userAgent?.let {
            userAgent = it
            return
        }
        runCatching { learn(WebSettings.getDefaultUserAgent(context)) }
    }

    /** UA mặc định của WebView → UA rút gọn, lưu cho lần sau (WebView tự cập nhật thì số phiên bản đổi theo). */
    private fun learn(webViewUa: String) {
        val ua = reduce(webViewUa)
        userAgent = ua
        if (Prefs.userAgent != ua) Prefs.userAgent = ua
    }

    /**
     * "Mozilla/5.0 (Linux; Android 9; TX3 Mini Build/…; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0
     * Chrome/138.0.7204.179 Mobile Safari/537.36" → "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML,
     * like Gecko) Chrome/138.0.0.0 Mobile Safari/537.36"
     */
    fun reduce(webViewUa: String): String {
        val major = Regex("""Chrome/(\d+)""").find(webViewUa)?.groupValues?.get(1) ?: FALLBACK_MAJOR
        val mobile = if (" Mobile" in webViewUa) " Mobile" else ""
        return "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/$major.0.0.0$mobile Safari/537.36"
    }

    /** Gọi khi dựng mỗi WebView của app (trình phát, Duyệt web). */
    @SuppressLint("RestrictedApi") // REQUESTED_WITH_HEADER_ALLOW_LIST đánh dấu nội bộ nhưng tài liệu androidx.webkit dùng đúng cách này
    fun apply(web: WebView) {
        val s = web.settings
        learn(s.userAgentString)
        s.userAgentString = userAgent
        // WebView 138 (TX3, 11/2026) nhận lệnh này nhưng vẫn gửi tên gói; giữ lại cho bản WebView làm theo
        if (WebViewFeature.isFeatureSupported(WebViewFeature.REQUESTED_WITH_HEADER_ALLOW_LIST)) {
            WebSettingsCompat.setRequestedWithHeaderOriginAllowList(s, emptySet())
        }
        if (WebViewFeature.isFeatureSupported(WebViewFeature.USER_AGENT_METADATA)) {
            val major = Regex("""Chrome/(\d+)""").find(userAgent)?.groupValues?.get(1) ?: FALLBACK_MAJOR
            val brand = UserAgentMetadata.BrandVersion.Builder()
                .setBrand("Chromium").setMajorVersion(major).setFullVersion("$major.0.0.0").build()
            WebSettingsCompat.setUserAgentMetadata(
                s,
                UserAgentMetadata.Builder()
                    .setBrandVersionList(listOf(brand)).setFullVersion("$major.0.0.0")
                    .setPlatform("Android").setPlatformVersion("10.0.0").setModel("")
                    .setMobile(" Mobile" in userAgent)
                    .build(),
            )
        }
    }
}
