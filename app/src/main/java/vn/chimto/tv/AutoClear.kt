package vn.chimto.tv

import android.app.Activity
import android.app.Application
import android.os.Bundle
import android.os.Handler
import android.os.Looper

/**
 * Tự xoá dữ liệu duyệt web (Cài đặt, mặc định tắt): xoá khi rời app (Home, Back thoát, mở app khác) và khi mở app
 * lần đầu sau khi tiến trình bị tắt. Không xoá lúc trình phát / Duyệt web còn sống (vd nhấn Home giữa phim rồi quay
 * lại xem tiếp) — để tới khi màn hình đó đóng. Chỉ xoá khi WebView đã chạy từ lần xoá trước (Prefs.webDirty).
 */
object AutoClear : Application.ActivityLifecycleCallbacks {
    private lateinit var app: Application
    private var started = 0
    private var webScreens = 0

    fun init(application: Application) {
        app = application
        application.registerActivityLifecycleCallbacks(this)
        // Tiến trình mới (lần trước có thể bị tắt trước khi kịp xoá): xoá sau khi màn hình đầu đã hiện
        Handler(Looper.getMainLooper()).postDelayed(::clearIfNeeded, 1500)
    }

    private fun isWeb(a: Activity) = a is PlayerActivity || a is WebActivity

    private fun clearIfNeeded() {
        if (!Prefs.autoClearWeb || !Prefs.webDirty || webScreens > 0) return
        Prefs.webDirty = false
        BrowsingData.clear(app) {}
    }

    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {
        if (!isWeb(activity)) return
        webScreens++
        Prefs.webDirty = true
    }

    override fun onActivityDestroyed(activity: Activity) {
        if (!isWeb(activity)) return
        webScreens--
        if (started == 0) clearIfNeeded()
    }

    override fun onActivityStarted(activity: Activity) {
        started++
    }

    override fun onActivityStopped(activity: Activity) {
        started--
        if (started == 0) clearIfNeeded()
    }

    override fun onActivityResumed(activity: Activity) = Unit
    override fun onActivityPaused(activity: Activity) = Unit
    override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit
}
