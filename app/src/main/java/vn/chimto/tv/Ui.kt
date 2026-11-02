package vn.chimto.tv

import android.app.Activity
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.AttributeSet
import android.view.KeyEvent
import android.view.View
import android.widget.EditText
import android.widget.ImageView
import android.widget.TextView
import java.net.SocketTimeoutException
import java.net.UnknownHostException
import java.util.concurrent.Executors
import javax.net.ssl.SSLException
import kotlin.math.roundToInt

val mainHandler = Handler(Looper.getMainLooper())
private val io = Executors.newFixedThreadPool(3)

/** Chạy work ở luồng nền rồi trả kết quả về luồng chính (bỏ qua nếu màn hình đã đóng). */
fun <T> Activity.background(work: () -> T, done: (Result<T>) -> Unit) {
    io.execute {
        val r = runCatching(work)
        mainHandler.post { if (!isFinishing && !isDestroyed) done(r) }
    }
}

fun Context.dp(v: Int): Int = (v * resources.displayMetrics.density).roundToInt()

fun Throwable.friendly(): String = when (this) {
    is UnknownHostException -> "Không tìm thấy trang. Kiểm tra mạng hoặc đổi link trong Cài đặt."
    is SocketTimeoutException -> "Trang phản hồi quá chậm, thử lại sau."
    is SSLException -> "Lỗi kết nối bảo mật (SSL) tới trang."
    else -> message ?: javaClass.simpleName
}

/** Phóng to nhẹ + nổi lên trên các ô bên cạnh khi được chọn bằng remote. */
fun View.focusZoom(scale: Float = 1.07f) {
    val lift = context.dp(6).toFloat()
    setOnFocusChangeListener { v, has ->
        v.animate().scaleX(if (has) scale else 1f).scaleY(if (has) scale else 1f).setDuration(120).start()
        v.translationZ = if (has) lift else 0f
    }
}

/**
 * Ô nhập một dòng: ▲/▼ của bàn phím vật lý luôn chuyển sang mục khác
 * (Android mặc định giữ phím mũi tên của bàn phím trong ô chữ, remote thì không).
 */
fun EditText.leaveWithUpDown() {
    setOnKeyListener { v, keyCode, e ->
        val dir = when (keyCode) {
            KeyEvent.KEYCODE_DPAD_UP -> View.FOCUS_UP
            KeyEvent.KEYCODE_DPAD_DOWN -> View.FOCUS_DOWN
            else -> return@setOnKeyListener false
        }
        if (e.action == KeyEvent.ACTION_DOWN) v.focusSearch(dir)?.requestFocus(dir)
        true
    }
}

fun formatTime(seconds: Double): String {
    val s = seconds.coerceAtLeast(0.0).toLong()
    val h = s / 3600
    val m = s % 3600 / 60
    val sec = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, sec) else "%d:%02d".format(m, sec)
}

/** Thông báo nhỏ tự ẩn (Toast hệ thống trên một số TV box bị ẩn sau WebView toàn màn hình). */
class Notice(private val view: TextView) {
    private val hide = Runnable { view.visibility = View.GONE }
    fun show(text: String, ms: Long = 3000) {
        view.text = text
        view.visibility = View.VISIBLE
        view.removeCallbacks(hide)
        view.postDelayed(hide, ms)
    }
}

/** Poster tỉ lệ 2:3, cao theo chiều rộng ô lưới. */
class PosterView @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null) : ImageView(context, attrs) {
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val w = MeasureSpec.getSize(widthMeasureSpec)
        setMeasuredDimension(w, w * 3 / 2)
    }
}
