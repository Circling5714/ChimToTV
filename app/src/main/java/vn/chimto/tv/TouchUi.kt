package vn.chimto.tv

import android.app.Activity
import android.content.res.Configuration
import android.os.Build
import android.os.Bundle
import android.view.Gravity
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets
import android.widget.FrameLayout
import android.widget.TextView

/**
 * Màn hình cảm ứng (TV box nối màn cảm ứng, điện thoại): người xem thỉnh thoảng chạm thay remote. active = lần cuối là
 * chạm (hoặc chuột); bấm phím thật (remote / bàn phím rời) thì false.
 */
object TouchUi {
    @Volatile var active = false
}

/**
 * Đưa tiêu điểm tới view cho remote (requestFocusFromTouch: rời chế độ chạm, hiện khung chọn). Đang dùng cảm ứng thì
 * chỉ requestFocus — khỏi bật khung chọn khi người xem đang chạm, và khỏi làm BaseActivity tưởng người xem vừa bấm phím.
 */
fun View.focusForRemote() {
    if (TouchUi.active) requestFocus() else requestFocusFromTouch()
}

/**
 * Màn hình chung của app: ESC = Back; đang dùng cảm ứng thì hiện nút "‹ Quay lại" ở góc trên bên phải (máy chỉ có cảm
 * ứng thì không có phím Back). Nút không nhận tiêu điểm, người dùng remote không thấy.
 */
abstract class BaseActivity : Activity() {
    /** Chữ trên nút; null = không có nút (trình phát có thanh điều khiển cảm ứng riêng). */
    protected open val touchBackLabel: String? = "‹ Quay lại"

    /** true = tạm không hiện nút (vd màn chính trên điện thoại cầm dọc: đã có ☰ và nút Back của điện thoại). */
    protected open val touchBackHidden get() = false

    /** Chỗ đặt nút (mặc định góc trên bên phải). */
    protected open val touchBackGravity = Gravity.TOP or Gravity.END

    protected open fun onTouchBack() {
        @Suppress("DEPRECATION")
        onBackPressed()
    }

    private var touchBack: TextView? = null

    /** Ô nhập ở cuối màn (Duyệt web): chừa chỗ cho bàn phím ảo (Android 15 vẽ tràn màn, adjustResize không tự co). */
    protected open val padForKeyboard = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Điện thoại: xoay dọc được (trình phát tự giữ ngang); TV giữ ngang theo manifest
        if (this !is PlayerActivity) Screen.allowRotation(this)
    }

    override fun onPostCreate(savedInstanceState: Bundle?) {
        super.onPostCreate(savedInstanceState)
        fitSystemBars()
        val label = touchBackLabel ?: return
        touchBack = TextView(this).apply {
            text = label
            textSize = 16f
            setTextColor(colorStateOf(R.color.text_on_focus))
            setBackgroundResource(R.drawable.bg_button)
            setPadding(dp(18), dp(8), dp(18), dp(8)) // thấp, khỏi đụng ô nhập ngay dưới tiêu đề màn hình
            isFocusable = false
            elevation = dp(6).toFloat()
            setOnClickListener { onTouchBack() }
        }
        // Phím điều hướng đầu tiên sau khi chạm (▲▼◀▶, OK) bị Android dùng để rời chế độ chạm, không tới
        // dispatchKeyEvent → nghe việc rời chế độ chạm (app không tự rời khi đang dùng cảm ứng, xem focusForRemote)
        window.decorView.viewTreeObserver.addOnTouchModeChangeListener { inTouchMode ->
            if (!inTouchMode && TouchUi.active) {
                TouchUi.active = false
                showTouchBack()
            }
        }
        findViewById<FrameLayout>(android.R.id.content).addView(
            touchBack,
            FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT, touchBackGravity)
                .apply { setMargins(dp(18), dp(10), dp(18), dp(14)) },
        )
        showTouchBack()
    }

    /**
     * Điện thoại Android 15 (app vẽ tràn cả màn): chừa chỗ tai thỏ / lỗ camera và thanh điều hướng — nội dung không nằm
     * dưới chúng. Android cũ / TV: các khoảng này bằng 0 (hệ thống đã tự chừa).
     */
    private fun fitSystemBars() {
        val content = findViewById<FrameLayout>(android.R.id.content)
        content.setOnApplyWindowInsetsListener { v, ins ->
            var l: Int
            var t: Int
            var r: Int
            var b: Int
            if (Build.VERSION.SDK_INT >= 30) {
                var types = WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout()
                if (padForKeyboard) types = types or WindowInsets.Type.ime()
                val i = ins.getInsets(types)
                l = i.left; t = i.top; r = i.right; b = i.bottom
            } else {
                @Suppress("DEPRECATION")
                run { l = ins.systemWindowInsetLeft; t = ins.systemWindowInsetTop; r = ins.systemWindowInsetRight; b = ins.systemWindowInsetBottom }
                if (Build.VERSION.SDK_INT >= 28) ins.displayCutout?.let {
                    l = maxOf(l, it.safeInsetLeft); t = maxOf(t, it.safeInsetTop)
                    r = maxOf(r, it.safeInsetRight); b = maxOf(b, it.safeInsetBottom)
                }
            }
            v.setPadding(l, t, r, b)
            ins
        }
        content.requestApplyInsets()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        MiniPlayer.relayout(this) // điện thoại xoay: cửa sổ phim nhỏ đổi cỡ theo hướng mới
        showTouchBack()
    }

    override fun onResume() {
        super.onResume()
        if (BuildConfig.DEBUG && this !is PlayerActivity) Screen.allowRotation(this) // thử xoay trên TV box (xem Screen)
        showTouchBack() // màn trước có thể vừa đổi giữa chạm / remote
        if (this is PlayerActivity) return
        // Vừa quay lại app (bấm biểu tượng app) mà trình phát toàn màn hình đang ở nền: đưa ra lại (trình phát chạy task
        // riêng nên Android mở màn chính). Quay lại sau lâu mà phải hỏi "Ai đang xem?" thì thôi.
        if (AppUse.returnedTo(this) && this !is WebActivity && !(AppUse.wasIdle && ProfileUi.mustAsk())) {
            PlayerActivity.showHidden(this)
        }
        MiniPlayer.attach(this) // phim đang ở cửa sổ nhỏ trong app: nổi ở góc màn này
    }

    override fun dispatchTouchEvent(ev: MotionEvent): Boolean {
        if (ev.actionMasked == MotionEvent.ACTION_DOWN &&
            (ev.isFromSource(InputDevice.SOURCE_TOUCHSCREEN) || ev.isFromSource(InputDevice.SOURCE_MOUSE))
        ) {
            TouchUi.active = true
            showTouchBack()
        }
        return super.dispatchTouchEvent(ev)
    }

    override fun dispatchKeyEvent(e: KeyEvent): Boolean {
        keyUsed(e)
        // Phim đang thu nhỏ (hình trong hình): phím phát / dừng / chuyển tập của remote điều khiển phim đó
        if (PlayerActivity.mediaKey(e)) return true
        return escapeAsBack(e) || super.dispatchKeyEvent(e)
    }

    /** Bấm phím thật (không tính phím của bàn phím ảo) → thôi chế độ cảm ứng, ẩn nút. */
    protected fun keyUsed(e: KeyEvent) {
        if (e.action != KeyEvent.ACTION_DOWN || e.flags and KeyEvent.FLAG_SOFT_KEYBOARD != 0 || !TouchUi.active) return
        TouchUi.active = false
        showTouchBack()
    }

    private fun showTouchBack() {
        touchBack?.visibility = if (TouchUi.active && !touchBackHidden) View.VISIBLE else View.GONE
        onTouchUiChanged(TouchUi.active)
    }

    /** Đổi giữa chạm / remote (vd Duyệt web ẩn con trỏ của remote khi đang chạm). */
    protected open fun onTouchUiChanged(active: Boolean) = Unit
}
