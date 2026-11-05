package vn.chimto.tv

import android.app.Activity
import android.app.UiModeManager
import android.content.Context
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.util.AttributeSet
import android.view.View
import android.view.ViewGroup
import java.io.File
import kotlin.math.max

/**
 * Điện thoại / máy tính bảng: xoay dọc được (TV giữ ngang như cũ). Màn hình hẹp (điện thoại cầm dọc, bề ngang < 600dp)
 * thì các màn đổi bố cục: menu bên trái thành thanh ngang, nút xuống dòng, lề nhỏ lại. Trình phát luôn ngang.
 */
object Screen {
    private var tv: Boolean? = null

    /** TV / TV box: chế độ giao diện TV hoặc có leanback (TV box Android TV). */
    fun isTv(c: Context): Boolean = tv ?: run {
        val ui = (c.getSystemService(Context.UI_MODE_SERVICE) as? UiModeManager)?.currentModeType
        val pm = c.packageManager
        val leanback = pm.hasSystemFeature(PackageManager.FEATURE_LEANBACK) ||
            pm.hasSystemFeature("android.software.leanback_only")
        // Bản gỡ lỗi: tạo tệp files/force_phone (adb shell run-as … touch files/force_phone) để thử giao diện điện
        // thoại trên TV box (kèm `wm size 720x1280` cho màn dọc)
        val force = BuildConfig.DEBUG && File(c.filesDir, "force_phone").exists()
        (!force && (ui == Configuration.UI_MODE_TYPE_TELEVISION || leanback)).also { tv = it }
    }

    /** Màn hẹp: điện thoại cầm dọc. */
    fun narrow(c: Context) = c.resources.configuration.screenWidthDp in 1 until 600

    /** Gọi ở onCreate các màn (trừ trình phát): máy không phải TV thì xoay theo cảm biến / khoá xoay của người dùng. */
    fun allowRotation(a: Activity) {
        if (isTv(a)) return
        // Bản gỡ lỗi: tệp files/force_portrait = ép dọc, force_landscape = ép ngang (TV box không có cảm biến xoay); đổi
        // tệp rồi mở / đóng một màn là màn bên dưới xoay theo — thử được onConfigurationChanged
        val want = when {
            BuildConfig.DEBUG && File(a.filesDir, "force_portrait").exists() -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            BuildConfig.DEBUG && File(a.filesDir, "force_landscape").exists() -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            else -> ActivityInfo.SCREEN_ORIENTATION_FULL_USER
        }
        if (a.requestedOrientation != want) a.requestedOrientation = want
    }
}

/**
 * Hàng nút tự xuống dòng khi không đủ chỗ (màn dọc của điện thoại); đủ chỗ thì y như LinearLayout ngang — remote đi
 * trái / phải như cũ. Con dùng layout_margin* như LinearLayout; rowGap: khoảng cách giữa hai dòng.
 */
class FlowRow @JvmOverloads constructor(c: Context, attrs: AttributeSet? = null) : ViewGroup(c, attrs) {
    var rowGap = c.dp(8)
    var centered = false

    override fun generateDefaultLayoutParams() = MarginLayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT)
    override fun generateLayoutParams(attrs: AttributeSet?) = MarginLayoutParams(context, attrs)
    override fun generateLayoutParams(p: LayoutParams?) = if (p is MarginLayoutParams) MarginLayoutParams(p) else MarginLayoutParams(p)
    override fun checkLayoutParams(p: LayoutParams?) = p is MarginLayoutParams

    override fun onMeasure(widthSpec: Int, heightSpec: Int) {
        val mode = MeasureSpec.getMode(widthSpec)
        val limit = if (mode == MeasureSpec.UNSPECIFIED) Int.MAX_VALUE else MeasureSpec.getSize(widthSpec) - paddingLeft - paddingRight
        var x = 0
        var y = 0
        var lineH = 0
        var widest = 0
        for (i in 0 until childCount) {
            val v = getChildAt(i)
            if (v.visibility == View.GONE) continue
            measureChildWithMargins(v, widthSpec, 0, heightSpec, 0)
            val lp = v.layoutParams as MarginLayoutParams
            val w = v.measuredWidth + lp.leftMargin + lp.rightMargin
            val h = v.measuredHeight + lp.topMargin + lp.bottomMargin
            if (x > 0 && x + w > limit) {
                y += lineH + rowGap
                x = 0
                lineH = 0
            }
            x += w
            lineH = max(lineH, h)
            widest = max(widest, x)
        }
        val h = y + lineH + paddingTop + paddingBottom
        val w = if (mode == MeasureSpec.EXACTLY) MeasureSpec.getSize(widthSpec) else widest + paddingLeft + paddingRight
        setMeasuredDimension(resolveSize(w, widthSpec), resolveSize(h, heightSpec))
    }

    override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
        val limit = r - l - paddingLeft - paddingRight
        // Chia dòng trước (để căn giữa từng dòng khi centered)
        val lines = ArrayList<MutableList<View>>()
        var line = ArrayList<View>()
        var x = 0
        for (i in 0 until childCount) {
            val v = getChildAt(i)
            if (v.visibility == View.GONE) continue
            val lp = v.layoutParams as MarginLayoutParams
            val w = v.measuredWidth + lp.leftMargin + lp.rightMargin
            if (x > 0 && x + w > limit) {
                lines += line
                line = ArrayList()
                x = 0
            }
            line += v
            x += w
        }
        if (line.isNotEmpty()) lines += line
        var y = paddingTop
        for (ln in lines) {
            val used = ln.sumOf { val lp = it.layoutParams as MarginLayoutParams; it.measuredWidth + lp.leftMargin + lp.rightMargin }
            var cx = paddingLeft + if (centered) max(0, (limit - used) / 2) else 0
            var lineH = 0
            for (v in ln) {
                val lp = v.layoutParams as MarginLayoutParams
                val left = cx + lp.leftMargin
                val top = y + lp.topMargin
                v.layout(left, top, left + v.measuredWidth, top + v.measuredHeight)
                cx += v.measuredWidth + lp.leftMargin + lp.rightMargin
                lineH = max(lineH, v.measuredHeight + lp.topMargin + lp.bottomMargin)
            }
            y += lineH + rowGap
        }
    }
}

/** Đổi lề trái / phải (và trên nếu có) của view, giữ lề còn lại. */
fun View.padH(start: Int, top: Int = paddingTop, end: Int = start) = setPaddingRelative(start, top, end, paddingBottom)

/** Cỡ cửa sổ phim nhỏ trong app (khung 16:9) theo hướng màn hình hiện tại. */
internal fun miniSize(a: Activity): Pair<Int, Int> {
    val m = a.resources.displayMetrics
    // Ngang: 30% bề ngang (TV 720p: 384×216); dọc (điện thoại): 55% bề ngang
    val w = (m.widthPixels * if (m.heightPixels > m.widthPixels) 0.55f else 0.3f).toInt()
    return w to w * 9 / 16
}
