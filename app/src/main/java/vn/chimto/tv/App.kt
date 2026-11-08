package vn.chimto.tv

import android.app.Application
import android.webkit.WebView

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        Prefs.init(this)
        Privacy.init(this)
        AppProxy.apply()
        ImageLoader.init(this)
        Feed.init(this)
        AutoClear.init(this)
        AppUse.init(this)
        SyncNet.init(this) // đồng bộ trong nhà (đã ghép máy thì mở cổng)
        if (BuildConfig.DEBUG) WebView.setWebContentsDebuggingEnabled(true)
    }
}
