package vn.chimto.tv

import android.app.Activity
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.drawable.Drawable
import android.os.Build
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

/** Context.getColorStateList chỉ có từ Android 6. */
fun Context.colorStateOf(id: Int): ColorStateList =
    if (Build.VERSION.SDK_INT >= 23) getColorStateList(id) else @Suppress("DEPRECATION") resources.getColorStateList(id)

/** Context.getColor chỉ có từ Android 6; app chạy từ Android 5. */
fun Context.colorOf(id: Int): Int =
    if (Build.VERSION.SDK_INT >= 23) getColor(id) else @Suppress("DEPRECATION") resources.getColor(id)

val mainHandler = Handler(Looper.getMainLooper())
private val io = Executors.newFixedThreadPool(3)

/** Chạy work ở luồng nền rồi trả kết quả về luồng chính (bỏ qua nếu màn hình đã đóng). */
/** Như background nhưng luôn gọi done (việc nền không gắn với một màn hình, vd kiểm phim theo dõi). */
fun <T> inBackground(work: () -> T, done: (Result<T>) -> Unit) {
    io.execute {
        val r = runCatching(work)
        mainHandler.post { done(r) }
    }
}

fun <T> Activity.background(work: () -> T, done: (Result<T>) -> Unit) {
    io.execute {
        val r = runCatching(work)
        mainHandler.post { if (!isFinishing && !isDestroyed) done(r) }
    }
}

/**
 * Phím ESC của bàn phím rời (điện thoại / TV box cắm bàn phím) = Back. Android chỉ tự đổi ESC thành Back khi không ai
 * nhận ESC trước — WebView chuyển ESC cho trang web / trình phát, bàn phím ảo cũng có thể nhận — nên app tự làm. Gọi ở
 * đầu dispatchKeyEvent; true = đã xử lý.
 */
fun Activity.escapeAsBack(e: KeyEvent): Boolean {
    if (e.keyCode != KeyEvent.KEYCODE_ESCAPE) return false
    @Suppress("DEPRECATION")
    if (e.action == KeyEvent.ACTION_UP && !e.isCanceled) onBackPressed()
    return true
}

fun Context.dp(v: Int): Int = (v * resources.displayMetrics.density).roundToInt()

fun Throwable.friendly(): String {
    // Đang bật proxy mà proxy vừa báo lỗi (sai mật khẩu, proxy chết…): nói rõ là do proxy
    if (AppProxy.active && this !is Http.HttpError) {
        AppProxy.recentError()?.let { return "Proxy: $it.\nKiểm tra Cài đặt → Proxy SOCKS5, hoặc tắt proxy." }
    }
    // DNS mã hoá vừa lỗi (máy chủ DoH bị chặn / không trả lời): nói rõ
    if (AppProxy.dohActive && !AppProxy.active && this !is Http.HttpError) {
        AppProxy.recentError()?.let { return "DNS mã hoá: $it.\nThử máy chủ DNS khác hoặc tắt trong Cài đặt → DNS mã hoá." }
    }
    // Bắt tay HTTPS hỏng / treo: hay là nhà mạng chặn theo tên trang → gợi ý bật Vượt chặn nhà mạng
    val hint = if (!Prefs.unblock && (this is SocketTimeoutException || this is SSLException))
        "\nNếu nhà mạng chặn trang này: bật Cài đặt → Vượt chặn nhà mạng." else ""
    return when (this) {
        is UnknownHostException -> "Không tìm thấy trang. Kiểm tra mạng hoặc đổi link trong Cài đặt."
        is SocketTimeoutException -> "Trang phản hồi quá chậm, thử lại sau."
        is SSLException -> "Lỗi kết nối bảo mật (SSL) tới trang."
        else -> message ?: javaClass.simpleName
    } + AppProxy.note() + hint
}

/**
 * Phóng to nhẹ khi được chọn bằng remote. Không có bóng đổ (outlineProvider = null) vì GPU của TV yếu vẽ bóng rất tốn;
 * translationZ nhỏ chỉ để ô được chọn vẽ đè lên ô bên cạnh khi phóng to.
 */
fun View.focusZoom(scale: Float = 1.07f) {
    outlineProvider = null
    setOnFocusChangeListener { v, has ->
        v.animate().scaleX(if (has) scale else 1f).scaleY(if (has) scale else 1f).setDuration(120).start()
        v.translationZ = if (has) 1f else 0f
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

    /** Không hiện gì (phim đang thu nhỏ — hình trong hình: cửa sổ quá nhỏ cho chữ). */
    var muted = false
        set(value) {
            field = value
            if (value) hide()
        }

    fun show(text: String, ms: Long = 3000) {
        if (muted) return
        view.text = text
        view.visibility = View.VISIBLE
        view.removeCallbacks(hide)
        view.postDelayed(hide, ms)
    }

    fun hide() {
        view.removeCallbacks(hide)
        view.visibility = View.GONE
    }
}

/**
 * Poster tỉ lệ 2:3, cao theo chiều rộng ô lưới.
 * Kích thước không phụ thuộc ảnh, nên khi ảnh tải xong không cần đo lại cả lưới
 * (mặc định ImageView gọi requestLayout → RecyclerView bố cục lại toàn bộ, gây khựng trên TV yếu).
 */
class PosterView @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null) : ImageView(context, attrs) {
    private var settingImage = false

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val w = MeasureSpec.getSize(widthMeasureSpec)
        setMeasuredDimension(w, w * 3 / 2)
    }

    override fun setImageDrawable(drawable: Drawable?) {
        settingImage = true
        super.setImageDrawable(drawable)
        // Nền giữ chỗ chỉ cần khi chưa có ảnh (đỡ vẽ chồng một lớp dưới ảnh)
        setBackgroundColor(if (drawable == null) context.colorOf(R.color.surface2) else 0)
        settingImage = false
    }

    override fun requestLayout() {
        if (!settingImage) super.requestLayout()
    }
}
