package vn.chimto.tv

import android.app.Application

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        Prefs.init(this)
        ImageLoader.init(this)
    }
}
