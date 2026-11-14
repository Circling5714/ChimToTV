package vn.chimto.tv

import android.app.Application
import android.os.Build
import android.webkit.WebView
import java.io.File

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        // Engine Gecko (WebFetch — trang bật chống bot) chạy thêm tiến trình con (vn.chimto.tv:tab0, :gpu, :crashhelper…) và Android
        // tạo lại App trong từng tiến trình đó: chỉ khởi tạo app ở tiến trình chính (WebView không cho hai tiến trình dùng chung
        // thư mục dữ liệu — tiến trình con chạm vào WebView là văng).
        if (!isMainProcess()) return
        Prefs.init(this)
        Prefs.applyNetDefaults()
        Privacy.init(this)
        WebFetch.init(this) // trang bật chống bot của Cloudflare → engine Gecko
        AppProxy.apply()
        ImageLoader.init(this)
        ImdbTop.cacheDir = cacheDir
        Feed.init(this)
        AutoClear.init(this)
        AppUse.init(this)
        SyncNet.init(this) // đồng bộ trong nhà (đã ghép máy thì mở cổng)
        if (BuildConfig.DEBUG) WebView.setWebContentsDebuggingEnabled(true)
    }

    private fun isMainProcess(): Boolean {
        val name = if (Build.VERSION.SDK_INT >= 28) {
            getProcessName()
        } else {
            runCatching { File("/proc/self/cmdline").readText().trimEnd('\u0000') }.getOrDefault(packageName)
        }
        return name == packageName
    }
}
