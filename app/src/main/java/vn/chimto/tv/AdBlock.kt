package vn.chimto.tv

import android.webkit.WebResourceResponse
import java.io.ByteArrayInputStream

/** Chặn vài tên miền quảng cáo / theo dõi phổ biến (đỡ nặng cho TV box, bớt popup). */
object AdBlock {
    private val hosts = listOf(
        "doubleclick.net", "googlesyndication.com", "googletagmanager.com", "google-analytics.com",
        "adservice.google.com", "popads.net", "popcash.net", "propellerads.com", "adsterra.com",
        "exoclick.com", "juicyads.com", "hilltopads.net", "onclickads.net", "clickadu.com",
        "a-ads.com", "histats.com", "adnxs.com", "taboola.com", "outbrain.com",
    )

    fun isAd(host: String?): Boolean {
        val h = host?.lowercase() ?: return false
        return hosts.any { h == it || h.endsWith(".$it") }
    }

    fun empty() = WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(ByteArray(0)))

    /** UA của WebView có "; wv" — một số trình phát chặn WebView, nên giả làm Chrome thường. */
    fun browserUserAgent(webViewUa: String): String =
        webViewUa.replace("; wv", "").replace(Regex("""Version/\d+(\.\d+)* """), "")
}
