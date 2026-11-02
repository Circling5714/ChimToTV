package vn.chimto.tv

import android.app.Application
import android.webkit.WebView

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        Prefs.init(this)
        ImageLoader.init(this)
        if (BuildConfig.DEBUG) WebView.setWebContentsDebuggingEnabled(true)
    }
}
