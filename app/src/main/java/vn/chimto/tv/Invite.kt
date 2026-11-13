package vn.chimto.tv

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.Bundle
import android.util.AttributeSet
import android.util.TypedValue
import android.view.View
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast

/**
 * Cài app lên tivi khác bằng app Downloader (AFTVnews): người mới chỉ cần cài Downloader trên tivi rồi nhập CODE. APK_URL:
 * link cố định luôn tải bản TV mới nhất (mỗi release TV đính kèm tệp tên ConBangTV.apk, xem release.ps1). Mã do aftv.news cấp
 * trỏ vào link tên cũ …/ChimToTV.apk (tạo trước khi đổi tên tệp, 13/11/2026) — mọi release TV vẫn đính kèm cả tệp đó, link nào
 * cũng tải cùng một tệp. Đổi mã / link thì sửa đủ năm chỗ: ở đây, bản iPad (Invite.swift), bản Windows (Invite.cs),
 * release.ps1 và README (mục «Cài trên Android TV bằng Downloader»).
 */
object Invite {
    const val TITLE = "Mời bạn bè cài app"
    const val CODE = "9192365"
    const val SHORT_URL = "aftv.news/$CODE"
    const val APK_URL = "https://github.com/Circling5714/ChimToTV/releases/latest/download/ConBangTV.apk"

    /** Trang hướng dẫn tiếng Việt (mục Downloader của README) — mã QR trên màn mở trang này. */
    const val GUIDE_URL = "https://github.com/Circling5714/ChimToTV/blob/main/README.md#downloader"

    /** Tin nhắn gửi cho người quen (nút «Gửi hướng dẫn…» trên điện thoại / máy tính bảng). */
    val MESSAGE = """
        Cài Côn Bằng TV trên tivi Android:
        1. Trên tivi mở CH Play, cài app Downloader (của AFTVnews).
        2. Mở Downloader, nhập $CODE rồi bấm Go.
        3. Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.
        Cài xong app tự cập nhật các bản sau.
        Hướng dẫn: $GUIDE_URL
    """.trimIndent()

    /**
     * Mã QR của GUIDE_URL, vẽ sẵn (mức sửa lỗi M; mỗi chuỗi một hàng, '1' = ô đen) — app không cần thư viện QR. Đổi GUIDE_URL
     * thì chạy InviteTest: test dựng lại bằng ZXing, báo lệch và in ra các hàng mới để dán vào đây; test thứ hai đọc lại mã
     * như điện thoại.
     */
    val QR: List<String> = listOf(
        "1111111010001001011010010000101111111",
        "1000001011001011110110011110001000001",
        "1011101001001001100010011111001011101",
        "1011101010101110010100111110001011101",
        "1011101001001000010100001111101011101",
        "1000001001101110000110100110101000001",
        "1111111010101010101010101010101111111",
        "0000000010100000111110111001100000000",
        "1011011100001011100111010011101001011",
        "0011110001010111110000010100011101010",
        "1000111001010101010001110100100100100",
        "0110100101001001001100101110011010100",
        "0010001101110100010000111111011011111",
        "0100100010110001100000001011011111011",
        "0001011101011010011110001110100010110",
        "1100110011001000111111011000000110010",
        "0001101011000111111111110110000001101",
        "1010110001010010110110111000000101011",
        "1100101110110001100100101011100010111",
        "0001000100011000011100101000100101010",
        "0111101100001010101001110001110110001",
        "1100110101100010000101110000110001000",
        "0100111010111100010010110110100100000",
        "0001100101101101001110010110010011100",
        "1010101110001011001010101111011010110",
        "0101010011000100111010101110110111111",
        "0101011011111101101110001010110001110",
        "1011010101000100101001110011101010010",
        "0000011100111011101001011000111110100",
        "0000000011011010010000011101100010101",
        "1111111010110111010010000010101010111",
        "1000001010110000100010010011100010011",
        "1011101001100000001100111010111111000",
        "1011101010001111011011010001011011001",
        "1011101010111010101111111110111011000",
        "1000001001110000101010011110100111100",
        "1111111011011001100110011110011011111",
    )
}

/** Vẽ Invite.QR: nền trắng, lề trắng 4 ô quanh mã (máy đọc cần), mỗi ô một số nguyên điểm ảnh cho nét. */
class QrView @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null) : View(context, attrs) {
    private val dark = Paint().apply { color = Color.BLACK; isAntiAlias = false }
    private val rows = Invite.QR

    override fun onMeasure(widthSpec: Int, heightSpec: Int) {
        super.onMeasure(widthSpec, heightSpec)
        val side = minOf(measuredWidth, measuredHeight)
        setMeasuredDimension(side, side)
    }

    override fun onDraw(canvas: Canvas) {
        canvas.drawColor(Color.WHITE)
        if (rows.isEmpty()) return
        val cells = rows.size + 2 * QUIET
        val cell = minOf(width, height) / cells
        if (cell <= 0) return
        val left = (width - cell * rows.size) / 2f
        val top = (height - cell * rows.size) / 2f
        for ((y, row) in rows.withIndex()) {
            for ((x, c) in row.withIndex()) {
                if (c == '1') canvas.drawRect(left + x * cell, top + y * cell, left + (x + 1) * cell, top + (y + 1) * cell, dark)
            }
        }
    }

    private companion object {
        const val QUIET = 4
    }
}

/**
 * Màn «Mời bạn bè cài app» (menu bên trái và Cài đặt → Liên hệ): mã Downloader thật to, ba bước cài, mã QR mở trang hướng
 * dẫn để khách chụp bằng điện thoại. Chỉ có nút «Đóng»; Back cũng đóng.
 */
class InviteActivity : BaseActivity() {
    companion object {
        fun open(ctx: Context) {
            ctx.startActivity(Intent(ctx, InviteActivity::class.java))
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_invite)
        findViewById<TextView>(R.id.inviteCode).text = Invite.CODE.chunked(1).joinToString(" ")
        findViewById<TextView>(R.id.inviteLinks).text =
            "Trên trình duyệt: ${Invite.SHORT_URL}\nLink tải trực tiếp: ${Invite.APK_URL.removePrefix("https://")}"
        findViewById<TextView>(R.id.btnInviteShare).apply {
            visibility = if (Screen.isTv(this@InviteActivity)) View.GONE else View.VISIBLE
            setOnClickListener { share() }
        }
        findViewById<TextView>(R.id.btnInviteClose).apply {
            setOnClickListener { finish() }
            requestFocus()
        }
        arrange()
    }

    /** Điện thoại / máy tính bảng: gửi cách cài (Invite.MESSAGE) qua app nhắn tin người xem chọn. */
    private fun share() {
        val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, Invite.MESSAGE)
        try {
            startActivity(Intent.createChooser(send, "Gửi hướng dẫn cài app"))
        } catch (e: ActivityNotFoundException) {
            Toast.makeText(this, "Máy không có app nào để gửi — đọc mã ${Invite.CODE} cho bạn bè.", Toast.LENGTH_LONG).show()
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        arrange()
    }

    /** Màn ngang (TV, máy tính bảng): các bước bên trái, mã QR bên phải. Điện thoại cầm dọc: xếp dọc, lề và chữ nhỏ hơn. */
    private fun arrange() {
        val narrow = Screen.narrow(this)
        val d = resources.displayMetrics.density
        val pad = ((if (narrow) 16 else 48) * d).toInt()
        findViewById<View>(R.id.inviteRoot).setPadding(pad, ((if (narrow) 16 else 28) * d).toInt(), pad, ((if (narrow) 16 else 20) * d).toInt())
        findViewById<LinearLayout>(R.id.inviteContent).orientation = if (narrow) LinearLayout.VERTICAL else LinearLayout.HORIZONTAL
        findViewById<TextView>(R.id.inviteCode).setTextSize(TypedValue.COMPLEX_UNIT_SP, if (narrow) 44f else 64f)
        // Màn hẹp: dòng link dài chiếm hết đáy màn — bỏ (tin nhắn «Gửi hướng dẫn…» đã có link); nút «Đóng» co lại cho nút
        // «Gửi hướng dẫn…» đủ chỗ hiện hết chữ
        findViewById<View>(R.id.inviteLinks).visibility = if (narrow) View.GONE else View.VISIBLE
        findViewById<TextView>(R.id.btnInviteClose).apply {
            // android:minWidth trong layout đặt cả hai mức tối thiểu (của TextView và của View) — phải hạ cả hai
            val w = ((if (narrow) 0 else 140) * d).toInt()
            minWidth = w
            minimumWidth = w
        }
        findViewById<View>(R.id.inviteSteps).layoutParams = LinearLayout.LayoutParams(
            if (narrow) LinearLayout.LayoutParams.MATCH_PARENT else 0, LinearLayout.LayoutParams.WRAP_CONTENT, if (narrow) 0f else 1f,
        )
        findViewById<View>(R.id.inviteQrBox).layoutParams = LinearLayout.LayoutParams(
            if (narrow) LinearLayout.LayoutParams.MATCH_PARENT else (290 * d).toInt(), LinearLayout.LayoutParams.WRAP_CONTENT,
        ).apply {
            if (narrow) topMargin = (20 * d).toInt() else marginStart = (40 * d).toInt()
        }
    }
}
