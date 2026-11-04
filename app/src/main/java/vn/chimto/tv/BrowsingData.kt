package vn.chimto.tv

import android.content.Context
import android.webkit.CookieManager
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewDatabase

/**
 * Dữ liệu duyệt web của app: cookie, bộ nhớ của trang (localStorage, IndexedDB…), bộ nhớ đệm và mật khẩu HTTP đã nhớ
 * của WebView. Không đụng lịch sử xem, vị trí đang xem, điểm server, cài đặt, proxy (SharedPreferences của app).
 * Http.kt không lưu cookie riêng (chỉ mượn cookie của WebView khi tải hộ trình phát), nên xoá ở đây là đủ.
 */
object BrowsingData {
    /** Gọi ở luồng chính; done chạy ở luồng chính khi cookie đã xoá xong. */
    fun clear(context: Context, done: () -> Unit) {
        WebStorage.getInstance().deleteAllData()
        // Bộ nhớ đệm HTTP của WebView dùng chung cả app: xoá qua một WebView tạm
        WebView(context.applicationContext).apply {
            clearCache(true)
            destroy()
        }
        WebViewDatabase.getInstance(context).clearHttpAuthUsernamePassword()
        val cookies = CookieManager.getInstance()
        cookies.removeAllCookies {
            cookies.flush()
            done()
        }
    }
}
