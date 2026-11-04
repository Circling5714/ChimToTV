package vn.chimto.tv

import android.app.Activity
import android.os.Bundle
import android.view.Gravity
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
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

    /** Chỗ đặt nút (mặc định góc trên bên phải). */
    protected open val touchBackGravity = Gravity.TOP or Gravity.END

    protected open fun onTouchBack() {
        @Suppress("DEPRECATION")
        onBackPressed()
    }

    private var touchBack: TextView? = null

    override fun onPostCreate(savedInstanceState: Bundle?) {
        super.onPostCreate(savedInstanceState)
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

    override fun onResume() {
        super.onResume()
        showTouchBack() // màn trước có thể vừa đổi giữa chạm / remote
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
        return escapeAsBack(e) || super.dispatchKeyEvent(e)
    }

    /** Bấm phím thật (không tính phím của bàn phím ảo) → thôi chế độ cảm ứng, ẩn nút. */
    protected fun keyUsed(e: KeyEvent) {
        if (e.action != KeyEvent.ACTION_DOWN || e.flags and KeyEvent.FLAG_SOFT_KEYBOARD != 0 || !TouchUi.active) return
        TouchUi.active = false
        showTouchBack()
    }

    private fun showTouchBack() {
        touchBack?.visibility = if (TouchUi.active) View.VISIBLE else View.GONE
        onTouchUiChanged(TouchUi.active)
    }

    /** Đổi giữa chạm / remote (vd Duyệt web ẩn con trỏ của remote khi đang chạm). */
    protected open fun onTouchUiChanged(active: Boolean) = Unit
}
