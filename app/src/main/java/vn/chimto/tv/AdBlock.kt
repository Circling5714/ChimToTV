package vn.chimto.tv

import android.webkit.WebResourceResponse
import java.io.ByteArrayInputStream

/**
 * Chặn tên miền quảng cáo / theo dõi (đỡ nặng cho TV box, bớt popup, trang phim bớt biết người xem). Không chặn thứ trình
 * phát cần: JW Player tải từ ssl.p.jwpcdn.com và hỏi giấy phép ở entitlements.jwplayer.com, video nằm trên CDN
 * (tiktokcdn, ibyteimg…) — chặn là hỏng phim.
 */
object AdBlock {
    private val hosts = listOf(
        "doubleclick.net", "googlesyndication.com", "googletagmanager.com", "google-analytics.com",
        "adservice.google.com", "popads.net", "popcash.net", "propellerads.com", "adsterra.com",
        "exoclick.com", "juicyads.com", "hilltopads.net", "onclickads.net", "clickadu.com",
        "a-ads.com", "histats.com", "adnxs.com", "taboola.com", "outbrain.com",
        // Đo trên 5 trang (11/2026): thống kê của Cloudflare (trình phát streamfree), JW Player (NguonC), Ahrefs
        // (hhkungfu); quảng cáo Adcash (phimmoir) và tên miền quảng cáo bật cửa sổ của hoathinh3d
        "cloudflareinsights.com", "jwpltx.com", "analytics.ahrefs.com", "acscdn.com", "excavatenearbywand.com",
        // Thống kê / theo dõi phổ biến khác (không trang nào cần để phát phim)
        "connect.facebook.net", "clarity.ms", "mc.yandex.ru", "statcounter.com", "hotjar.com",
    )

    fun isAd(host: String?): Boolean {
        val h = host?.lowercase() ?: return false
        return hosts.any { h == it || h.endsWith(".$it") }
    }

    fun empty() = WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(ByteArray(0)))
}
