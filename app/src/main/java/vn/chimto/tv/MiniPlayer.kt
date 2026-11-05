package vn.chimto.tv

import android.annotation.SuppressLint
import android.app.Activity
import android.graphics.Color
import android.graphics.Outline
import android.graphics.drawable.GradientDrawable
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import android.webkit.WebView
import android.widget.FrameLayout
import android.widget.TextView
import kotlin.math.roundToInt

/**
 * Cửa sổ nhỏ trong app — cho TV không có hình trong hình của Android (TX3: máy ít RAM; Skyworth: hãng tắt). Trình phát
 * (PlayerActivity, chạy trong task riêng) lùi ra sau nhưng vẫn chạy; WebView của nó được chuyển sang nổi ở góc dưới
 * bên phải của màn đang mở trong app (BaseActivity.onResume gọi attach). Không nhận tiêu điểm: remote vẫn dùng màn
 * bên dưới như thường; chạm vào cửa sổ nhỏ thì phóng to.
 *
 * Giữ WebView / khung (gắn với màn đang mở) là có chủ ý: màn mới hiện lên lấy khung sang (attach), trình phát đóng thì
 * release() bỏ hết tham chiếu.
 */
@SuppressLint("StaticFieldLeak")
object MiniPlayer {
    private var web: WebView? = null
    private var frame: FrameLayout? = null
    private var caption: TextView? = null
    private var text = ""
    private var onTap: ((Activity) -> Unit)? = null

    val active get() = web != null

    fun start(w: WebView, title: String, tap: (Activity) -> Unit) {
        web = w
        text = title
        onTap = tap
    }

    /** Dòng chữ dưới cửa sổ nhỏ (tên phim — tập, đang dừng / lỗi). */
    fun caption(s: String) {
        text = s
        caption?.text = s
    }

    /** Đưa cửa sổ nhỏ vào màn a (màn vừa hiện lên trên cùng). */
    fun attach(a: Activity) {
        val w = web ?: return
        val content = a.findViewById<FrameLayout>(android.R.id.content) ?: return
        if (frame?.parent === content) return
        removeFrame()
        val f = makeFrame(a)
        (w.parent as? ViewGroup)?.removeView(w)
        f.addView(w, 0, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        content.addView(f)
        frame = f
    }

    /** Trả WebView về cho trình phát (phóng to / đóng phim). */
    fun release(w: WebView) {
        if (web !== w) return
        removeFrame()
        (w.parent as? ViewGroup)?.removeView(w)
        web = null
        onTap = null
    }

    private fun removeFrame() {
        val f = frame ?: return
        web?.let { f.removeView(it) }
        (f.parent as? ViewGroup)?.removeView(f)
        frame = null
        caption = null
    }

    private fun makeFrame(a: Activity): FrameLayout {
        // Rộng 30% màn hình, khung 16:9 (TV 720p: 384×216 điểm ảnh)
        val width = (a.resources.displayMetrics.widthPixels * 0.3f).roundToInt()
        val radius = a.dp(10).toFloat()
        val f = FrameLayout(a).apply {
            setBackgroundColor(Color.BLACK)
            isFocusable = false
            descendantFocusability = ViewGroup.FOCUS_BLOCK_DESCENDANTS
            outlineProvider = object : ViewOutlineProvider() {
                override fun getOutline(v: View, o: Outline) = o.setRoundRect(0, 0, v.width, v.height, radius)
            }
            clipToOutline = true
            layoutParams = FrameLayout.LayoutParams(width, width * 9 / 16, Gravity.BOTTOM or Gravity.END)
                .apply { setMargins(0, 0, a.dp(20), a.dp(20)) }
        }
        caption = TextView(a).apply {
            text = this@MiniPlayer.text
            textSize = 12f
            setTextColor(Color.WHITE)
            setSingleLine()
            ellipsize = TextUtils.TruncateAt.END
            setPadding(a.dp(10), a.dp(3), a.dp(10), a.dp(4))
            setBackgroundColor(0xB3000000.toInt())
        }
        f.addView(caption, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM))
        // Viền + lớp nhận chạm trên cùng (chạm không lọt vào trang web / quảng cáo của trình phát)
        f.addView(
            View(a).apply {
                background = GradientDrawable().apply {
                    setStroke(a.dp(2), 0x80FFFFFF.toInt())
                    cornerRadius = radius
                }
                isFocusable = false
                setOnClickListener { onTap?.invoke(a) }
            },
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )
        return f
    }
}
