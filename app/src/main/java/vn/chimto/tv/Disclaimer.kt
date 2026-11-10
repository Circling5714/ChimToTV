package vn.chimto.tv

import android.content.Context
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.style.ForegroundColorSpan
import android.text.style.RelativeSizeSpan
import android.text.style.StyleSpan
import android.view.KeyEvent
import android.view.View
import android.widget.ScrollView
import android.widget.TextView

/**
 * Miễn trừ trách nhiệm & quyền riêng tư — cùng nội dung trên TV, iPad (Disclaimer.swift), Windows (Disclaimer.cs) và README
 * (mục «Miễn trừ trách nhiệm»). Sửa nội dung đáng kể thì tăng VERSION: mọi máy hiện lại màn «Trước khi bắt đầu» một lần.
 */
object Disclaimer {
    const val VERSION = 1
    const val TITLE = "Miễn trừ trách nhiệm & quyền riêng tư"
    const val DATE = "11/2026"

    val SECTIONS = listOf(
        "1. App chỉ là trình xem" to "Côn Bằng TV là ứng dụng miễn phí giúp xem các trang phim công khai trên Internet bằng remote hoặc " +
            "màn hình cảm ứng. App không lưu trữ, không tải lên, không sở hữu và không phát tán phim, hình ảnh hay phụ đề nào. Mọi nội " +
            "dung do các trang web bên thứ ba cung cấp và nằm trên máy chủ của họ.",
        "2. Bản quyền thuộc chủ sở hữu" to "Tên phim, hình ảnh, video và nhãn hiệu thuộc về chủ sở hữu hợp pháp. Côn Bằng TV không liên " +
            "kết với, không được tài trợ hay bảo trợ bởi trang web, hãng phim hay nhà phát hành nào. Chủ sở hữu muốn app ngừng hỗ trợ " +
            "một trang, xin liên hệ ở mục 9.",
        "3. Người dùng tự chịu trách nhiệm" to "Bạn tự chọn trang để xem và tự tuân thủ pháp luật về bản quyền, sử dụng Internet nơi " +
            "bạn sinh sống. Khi có thể, hãy ủng hộ nhà sản xuất bằng cách xem trên nền tảng chính thức.",
        "4. Trang bên thứ ba" to "Nhà phát triển không kiểm soát và không chịu trách nhiệm về nội dung, quảng cáo, đường dẫn hay mức " +
            "độ an toàn của các trang bên thứ ba. App có lọc quảng cáo nhưng không bảo đảm chặn hết.",
        "5. Tính năng mạng" to "Proxy SOCKS5 / Shadowsocks, Vượt chặn nhà mạng và DNS mã hoá là công cụ kết nối nhằm bảo vệ quyền " +
            "riêng tư và khắc phục lỗi kết nối. App không cung cấp máy chủ proxy. Bạn tự chịu trách nhiệm dùng các tính năng này đúng " +
            "pháp luật và đúng điều khoản của nhà mạng / nhà cung cấp máy chủ.",
        "6. Dữ liệu và quyền riêng tư" to "App không có tài khoản, không thu thập hay bán dữ liệu cá nhân, không gắn công cụ theo dõi. " +
            "Lịch sử xem, hồ sơ người xem và cài đặt chỉ lưu trên thiết bị; «Đồng bộ trong nhà» truyền thẳng giữa các máy trong mạng " +
            "nhà. App chỉ kết nối tới: các trang phim bạn chọn; GitHub để kiểm tra bản mới; dịch vụ DNS mã hoá (NextDNS, Cloudflare, " +
            "Google — tắt được trong Cài đặt); Cloudflare khi bạn bấm «Kiểm tra proxy». DNS mã hoá mặc định dùng hồ sơ NextDNS riêng " +
            "của app, không lưu nhật ký truy cập.",
        "7. Không bảo đảm" to "App được cung cấp miễn phí, «nguyên trạng», không kèm bảo đảm nào. Trang phim có thể đổi, ngừng hoạt " +
            "động hoặc bị chặn bất cứ lúc nào. Ở mức tối đa pháp luật cho phép, nhà phát triển không chịu trách nhiệm cho thiệt hại " +
            "phát sinh từ việc dùng app.",
        "8. Trẻ em" to "Hồ sơ trẻ em và giới hạn giờ xem chỉ hỗ trợ cha mẹ, không thay thế sự giám sát của người lớn. Nội dung từ " +
            "trang bên thứ ba có thể không phù hợp với trẻ.",
        "9. Liên hệ" to "Góp ý, báo lỗi hoặc yêu cầu về bản quyền: Telegram t.me/tivikit.",
    )

    const val CONSENT = "Tiếp tục sử dụng app nghĩa là bạn đồng ý với các điều trên."

    /** Chưa đồng ý bản hiện tại (máy mới cài, hoặc cập nhật từ bản chưa có màn này / nội dung vừa đổi). */
    fun needed(): Boolean = Prefs.disclaimerVersion < VERSION

    fun accept() {
        Prefs.disclaimerVersion = VERSION
    }

    /** Nội dung để hiện: tên mục đậm màu nhấn, đoạn chữ thường. */
    fun text(ctx: Context): CharSequence {
        val b = SpannableStringBuilder()
        for ((i, s) in SECTIONS.withIndex()) {
            if (i > 0) b.append("\n\n")
            val start = b.length
            b.append(s.first)
            b.setSpan(StyleSpan(Typeface.BOLD), start, b.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            b.setSpan(ForegroundColorSpan(ctx.getColor(R.color.focus)), start, b.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            b.append("\n").append(s.second)
        }
        b.append("\n\n")
        val start = b.length
        b.append("$CONSENT  (bản $VERSION – $DATE)")
        b.setSpan(StyleSpan(Typeface.ITALIC), start, b.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
        b.setSpan(RelativeSizeSpan(0.92f), start, b.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
        return b
    }
}

/**
 * Màn miễn trừ trách nhiệm. gate = true (mở app lần đầu / sau khi cập nhật lên bản có màn này): «Đồng ý» mới vào app, «Thoát»
 * hoặc Back là đóng app. gate = false (Cài đặt → Giới thiệu): chỉ đọc, nút «Đóng».
 */
class DisclaimerActivity : BaseActivity() {
    companion object {
        const val EXTRA_GATE = "gate"

        fun open(ctx: Context, gate: Boolean) {
            ctx.startActivity(Intent(ctx, DisclaimerActivity::class.java).putExtra(EXTRA_GATE, gate))
        }
    }

    private val gate get() = intent.getBooleanExtra(EXTRA_GATE, false)

    override val touchBackLabel: String? get() = if (gate) null else "‹ Quay lại"

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_disclaimer)
        if (Screen.narrow(this)) {
            // Điện thoại cầm dọc: lề nhỏ
            val pad = (16 * resources.displayMetrics.density).toInt()
            findViewById<View>(R.id.disclaimerRoot).setPadding(pad, pad, pad, pad)
        }
        findViewById<TextView>(R.id.disclaimerTitle).text = if (gate) "Trước khi bắt đầu" else Disclaimer.TITLE
        findViewById<TextView>(R.id.disclaimerSub).text =
            if (gate) "Côn Bằng TV — ${Disclaimer.TITLE}. Đọc hết rồi chọn «Đồng ý» để vào app." else "Côn Bằng TV · bản ${Disclaimer.VERSION} – ${Disclaimer.DATE}"
        findViewById<TextView>(R.id.disclaimerBody).text = Disclaimer.text(this)
        val agree = findViewById<TextView>(R.id.btnAgree)
        val exit = findViewById<TextView>(R.id.btnExit)
        if (gate) {
            agree.text = "Đồng ý"
            agree.setOnClickListener {
                Disclaimer.accept()
                finish()
            }
            exit.setOnClickListener { finishAffinity() }
        } else {
            agree.text = "Đóng"
            agree.setOnClickListener { finish() }
            exit.visibility = View.GONE
        }
        findViewById<TextView>(R.id.disclaimerHint).text =
            if (Screen.isTv(this)) "▲ ▼: đọc tiếp  •  OK: ${agree.text}" else ""
        agree.requestFocus()
        // Remote: ▲ từ nút lên vùng chữ rồi ▲▼ cuộn; hết chữ thì ▼ xuống lại nút. OK ở vùng chữ = nút chính (như dòng nhắc):
        // mở app ở chế độ chạm (chuột bay / màn cảm ứng) thì lần bấm phím đầu chọn vùng chữ chứ không chọn nút
        findViewById<ScrollView>(R.id.disclaimerScroll).setOnKeyListener { v, code, e ->
            when {
                e.action == KeyEvent.ACTION_DOWN && code == KeyEvent.KEYCODE_DPAD_DOWN && !v.canScrollVertically(1) -> {
                    agree.requestFocus()
                    true
                }
                code == KeyEvent.KEYCODE_DPAD_CENTER || code == KeyEvent.KEYCODE_ENTER || code == KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                    if (e.action == KeyEvent.ACTION_UP) agree.performClick()
                    true
                }
                else -> false
            }
        }
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        if (gate) finishAffinity() else super.onBackPressed()
    }
}
