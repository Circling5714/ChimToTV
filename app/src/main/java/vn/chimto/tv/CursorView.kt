package vn.chimto.tv

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.view.View

/** Con trỏ chuột ảo vẽ đè lên WebView; chỉ để hiển thị, không nhận sự kiện. */
class CursorView(context: Context) : View(context) {
    var cx = -1f
        private set
    var cy = -1f
        private set

    private val radius = context.dp(11).toFloat()
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.argb(110, 255, 255, 255) }
    private val ring = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = context.dp(3).toFloat()
        color = context.colorOf(R.color.focus)
    }
    private val dot = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.BLACK }

    init {
        isFocusable = false
        isClickable = false
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        if (cx < 0) {
            cx = w / 2f
            cy = h / 2f
        } else {
            cx = cx.coerceIn(0f, w - 1f)
            cy = cy.coerceIn(0f, h - 1f)
        }
    }

    /** Dịch con trỏ; trả về phần không đi được vì chạm mép (để cuộn trang). */
    fun moveBy(dx: Float, dy: Float): Pair<Float, Float> {
        if (width <= 0 || height <= 0) return dx to dy // chưa được xếp chỗ
        val nx = (cx + dx).coerceIn(0f, width - 1f)
        val ny = (cy + dy).coerceIn(0f, height - 1f)
        val rest = (cx + dx - nx) to (cy + dy - ny)
        cx = nx
        cy = ny
        invalidate()
        return rest
    }

    override fun onDraw(canvas: Canvas) {
        canvas.drawCircle(cx, cy, radius, fill)
        canvas.drawCircle(cx, cy, radius, ring)
        canvas.drawCircle(cx, cy, radius / 4, dot)
    }
}
