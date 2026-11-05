package vn.chimto.tv

import android.app.Activity
import android.app.AlertDialog
import android.app.Application
import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.SpannableString
import android.text.Spanned
import android.text.TextUtils
import android.text.TextWatcher
import android.text.style.ForegroundColorSpan
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import java.lang.ref.WeakReference

/**
 * Biết người xem quay lại app sau một lúc lâu không dùng (để hỏi lại "Ai đang xem?"): đếm màn hình đang hiện của cả
 * app — phim đang thu nhỏ (hình trong hình) vẫn tính là đang dùng.
 */
object AppUse : Application.ActivityLifecycleCallbacks {
    private const val IDLE = 30 * 60_000L
    private var started = 0
    private var leftAt = 0L
    private var idleReturn: WeakReference<Activity>? = null
    private var returned: WeakReference<Activity>? = null

    /** Lần quay lại app gần nhất là sau ≥ 30 phút không dùng. */
    var wasIdle = false
        private set

    fun init(app: Application) = app.registerActivityLifecycleCallbacks(this)

    /** a là màn hình hiện lên đầu tiên khi quay lại app sau ≥ 30 phút không dùng (mỗi lần quay lại chỉ báo một lần). */
    fun idleReturn(a: Activity): Boolean {
        if (idleReturn?.get() !== a) return false
        idleReturn = null
        return true
    }

    /** a là màn hình hiện lên đầu tiên khi quay lại app (sau khi đã rời app — không tính lần mở đầu tiên). */
    fun returnedTo(a: Activity): Boolean {
        if (returned?.get() !== a) return false
        returned = null
        return true
    }

    override fun onActivityStarted(activity: Activity) {
        if (started++ == 0 && leftAt > 0) {
            wasIdle = System.currentTimeMillis() - leftAt >= IDLE
            returned = WeakReference(activity)
            if (wasIdle) idleReturn = WeakReference(activity)
        }
    }

    override fun onActivityStopped(activity: Activity) {
        if (--started == 0) {
            leftAt = System.currentTimeMillis()
            PlayerActivity.appLeft() // phim ở cửa sổ nhỏ trong app không xem được khi đã rời app
        }
    }

    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) = Unit
    override fun onActivityResumed(activity: Activity) = Unit
    override fun onActivityPaused(activity: Activity) = Unit
    override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit
    override fun onActivityDestroyed(activity: Activity) = Unit
}

/** Người xem trên TV: vòng tròn màu + chữ cái đầu, hộp thoại thêm / đổi tên / đổi màu / mã PIN / xoá. */
object ProfileUi {
    /**
     * Vừa xoá người đang xem mà những người còn lại đều có mã PIN: màn chính phải hỏi "Ai đang xem?" (không tự vào hồ
     * sơ có khoá).
     */
    var mustPick = false

    /** Mở app phải chọn người xem: nhà có từ 2 người (và bật "hỏi khi mở app"), hoặc người đang dùng có mã PIN. */
    fun mustAsk() = (Prefs.profiles.size >= 2 && Prefs.askProfile) || Prefs.profile.locked

    fun color(p: Profile) = Prefs.PROFILE_COLORS[p.color.mod(Prefs.PROFILE_COLORS.size)]

    /** Vòng tròn màu có chữ cái đầu tên; p = null: ô "＋" (thêm người). */
    fun avatar(context: Context, p: Profile?, sizeDp: Int): TextView = TextView(context).apply {
        text = p?.initial ?: "＋"
        gravity = Gravity.CENTER
        setTextColor(Color.WHITE)
        textSize = sizeDp * 0.42f
        typeface = Typeface.DEFAULT_BOLD
        includeFontPadding = false
        background = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(if (p != null) color(p) else context.colorOf(R.color.surface2))
        }
        layoutParams = LinearLayout.LayoutParams(context.dp(sizeDp), context.dp(sizeDp))
    }

    /** Nút người xem trong Cài đặt: vòng tròn màu viền trắng (thấy rõ cả trên nút đang chọn) + "Bố  🔒". */
    fun showOn(b: TextView, p: Profile) {
        b.text = p.name + if (p.locked) "  🔒" else ""
        val size = b.context.dp(16)
        val dot = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(color(p))
            setStroke(b.context.dp(2), Color.WHITE)
            setSize(size, size)
            setBounds(0, 0, size, size)
        }
        b.setCompoundDrawablesRelative(dot, null, null, null)
        b.compoundDrawablePadding = b.context.dp(10)
    }

    private fun dialog(a: Activity) = AlertDialog.Builder(a, android.R.style.Theme_DeviceDefault_Dialog_Alert)

    private fun toast(a: Activity, s: String) = Toast.makeText(a, s, Toast.LENGTH_LONG).show()

    private fun field(a: Activity, hint: String, pin: Boolean): EditText = EditText(a).apply {
        this.hint = hint
        setSingleLine()
        if (pin) {
            inputType = InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_VARIATION_PASSWORD
            filters = arrayOf(InputFilter.LengthFilter(Prefs.PIN_LENGTH))
            gravity = Gravity.CENTER
            textSize = 24f
            letterSpacing = 0.4f
        } else {
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_CAP_WORDS
            filters = arrayOf(InputFilter.LengthFilter(20))
            textSize = 18f
        }
        imeOptions = EditorInfo.IME_ACTION_DONE or EditorInfo.IME_FLAG_NO_EXTRACT_UI
        leaveWithUpDown()
    }

    private fun onText(f: EditText, block: (String) -> Unit) = f.addTextChangedListener(object : TextWatcher {
        override fun afterTextChanged(s: Editable) = block(s.toString())
        override fun beforeTextChanged(s: CharSequence, start: Int, count: Int, after: Int) = Unit
        override fun onTextChanged(s: CharSequence, start: Int, before: Int, count: Int) = Unit
    })

    private fun hideKeyboard(v: View) =
        (v.context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).hideSoftInputFromWindow(v.windowToken, 0)

    /**
     * Hộp thoại có ô nhập. ok kiểm dữ liệu: sai thì báo ngay trên ô và trả null (giữ hộp thoại); đúng thì trả việc cần
     * làm tiếp — chạy sau khi đã ẩn bàn phím và đóng hộp thoại (việc tiếp có thể dựng lại cả màn hình).
     */
    private fun ask(a: Activity, title: String, message: String?, fields: List<EditText>, okText: String, ok: () -> (() -> Unit)?): AlertDialog {
        val box = LinearLayout(a).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(a.dp(24), a.dp(4), a.dp(24), 0)
            fields.forEach {
                addView(it, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                    .apply { topMargin = a.dp(8) })
            }
        }
        val b = dialog(a).setTitle(title).setView(box).setPositiveButton(okText, null)
            .setNegativeButton("Huỷ") { _, _ -> hideKeyboard(fields.first()) }
        message?.let { b.setMessage(it) }
        val d = b.create()
        d.window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE)
        fun submit() {
            if (!d.isShowing) return
            val then = ok() ?: return
            hideKeyboard(fields.first())
            d.dismiss()
            then()
        }
        d.setOnShowListener {
            d.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { submit() }
            fields.first().requestFocus()
        }
        fields.last().setOnEditorActionListener { _, _, _ ->
            submit()
            true
        }
        d.show()
        return d
    }

    /** Người xem có mã PIN: hỏi mã (gõ đủ 4 số là thử luôn), đúng thì chạy then; không khoá thì chạy luôn. */
    fun unlock(a: Activity, p: Profile, title: String = "Mã PIN của ${p.name}", then: () -> Unit) {
        if (!p.locked) return then()
        val pin = field(a, "${Prefs.PIN_LENGTH} số", pin = true)
        val d = ask(a, title, null, listOf(pin), "Mở") {
            if (Prefs.checkPin(p, pin.text.toString())) {
                then
            } else {
                pin.text.clear()
                pin.error = "Sai mã PIN"
                null
            }
        }
        onText(pin) { if (it.length == Prefs.PIN_LENGTH) d.getButton(AlertDialog.BUTTON_POSITIVE)?.performClick() }
    }

    /** Đặt / đổi mã PIN (nhập hai lần). */
    private fun setPin(a: Activity, p: Profile, done: () -> Unit) {
        val f1 = field(a, "Mã PIN mới (${Prefs.PIN_LENGTH} số)", pin = true)
        val f2 = field(a, "Nhập lại mã PIN", pin = true)
        ask(
            a, "Mã PIN của ${p.name}",
            "Mở hồ sơ này (khi mở app, khi đổi người xem) và sửa / xoá nó phải nhập mã. Quên mã thì chỉ xoá dữ liệu app " +
                "(Cài đặt của TV → Ứng dụng) mới mở lại được — mất cả lịch sử, cài đặt.",
            listOf(f1, f2), "Lưu",
        ) {
            val pin = f1.text.toString()
            when {
                pin.length != Prefs.PIN_LENGTH || pin.any { !it.isDigit() } -> {
                    f1.error = "Cần đủ ${Prefs.PIN_LENGTH} số"
                    null
                }
                pin != f2.text.toString() -> {
                    f2.text.clear()
                    f2.error = "Hai lần nhập không khớp"
                    null
                }
                else -> {
                    Prefs.setProfilePin(p.id, pin)
                    done
                }
            }
        }
        onText(f1) { if (it.length == Prefs.PIN_LENGTH && f1.hasFocus()) f2.requestFocus() }
    }

    private fun editName(a: Activity, title: String, current: String, okText: String, done: (String) -> Unit) {
        val f = field(a, "Tên (vd Bố, Mẹ, Bé Na)", pin = false)
        f.setText(current)
        f.setSelection(current.length)
        ask(a, title, null, listOf(f), okText) {
            val name = f.text.toString().trim()
            when {
                name.isEmpty() -> {
                    f.error = "Nhập tên người xem"
                    null
                }
                Prefs.profiles.any { it.name.equals(name, true) && !it.name.equals(current, true) } -> {
                    f.error = "Đã có người tên này"
                    null
                }
                else -> ({ done(name) })
            }
        }
    }

    private fun pickColor(a: Activity, p: Profile, done: () -> Unit) {
        val items = Prefs.PROFILE_COLORS.indices.map { i ->
            SpannableString("●   ${Prefs.PROFILE_COLOR_NAMES[i]}").apply {
                setSpan(ForegroundColorSpan(Prefs.PROFILE_COLORS[i]), 0, 1, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            }
        }
        dialog(a).setTitle("Màu của ${p.name}")
            .setSingleChoiceItems(items.toTypedArray<CharSequence>(), p.color.mod(items.size)) { d, which ->
                Prefs.setProfileColor(p.id, which)
                d.dismiss()
                done()
            }
            .show()
    }

    /** Thêm người xem (không chuyển sang người mới). */
    fun add(a: Activity, done: (Profile) -> Unit) {
        if (Prefs.profiles.size >= Prefs.MAX_PROFILES) {
            toast(a, "Tối đa ${Prefs.MAX_PROFILES} người xem.")
            return
        }
        editName(a, "Thêm người xem", "", "Thêm") { done(Prefs.addProfile(it)) }
    }

    /** Đổi sang người xem khác: lưu chỗ đang xem cho người cũ, đóng phim đang thu nhỏ. */
    fun switchTo(p: Profile) {
        if (p.id == Prefs.profileId) return
        PlayerActivity.closeForProfileSwitch()
        Prefs.switchProfile(p.id)
    }

    /**
     * Sửa một người xem (Cài đặt): chuyển sang, đổi tên / màu, đặt / đổi / tắt mã PIN, xoá. Người có mã PIN thì hỏi mã
     * trước. changed: xong một thay đổi (kể cả đổi người đang xem).
     */
    fun manage(a: Activity, p: Profile, changed: () -> Unit) {
        unlock(a, p, "Nhập mã PIN của ${p.name} để sửa") {
            val actions = ArrayList<Pair<String, () -> Unit>>()
            if (p.id != Prefs.profileId) actions += "Chuyển sang ${p.name}" to {
                switchTo(p)
                changed()
            }
            actions += "Đổi tên" to { editName(a, "Đổi tên", p.name, "Lưu") { Prefs.renameProfile(p.id, it); changed() } }
            actions += "Đổi màu" to { pickColor(a, p, changed) }
            if (p.locked) {
                actions += "Đổi mã PIN" to { setPin(a, p) { toast(a, "Đã đổi mã PIN của ${p.name}."); changed() } }
                actions += "Tắt mã PIN" to {
                    Prefs.setProfilePin(p.id, null)
                    toast(a, "Đã tắt mã PIN của ${p.name}.")
                    changed()
                }
            } else {
                actions += "Đặt mã PIN" to { setPin(a, p) { toast(a, "Đã đặt mã PIN cho ${p.name}."); changed() } }
            }
            if (Prefs.profiles.size > 1) actions += "Xoá người xem này" to { confirmDelete(a, p, changed) }
            dialog(a).setTitle(p.name).setItems(actions.map { it.first }.toTypedArray()) { _, i -> actions[i].second() }.show()
        }
    }

    private fun confirmDelete(a: Activity, p: Profile, changed: () -> Unit) {
        dialog(a).setTitle("Xoá «${p.name}»?")
            .setMessage("Lịch sử xem, hàng Xem tiếp, chỗ đang xem, phim theo dõi và cài đặt riêng của ${p.name} sẽ bị xoá.")
            .setPositiveButton("Xoá") { _, _ ->
                val wasCurrent = p.id == Prefs.profileId
                if (wasCurrent) PlayerActivity.closeForProfileSwitch()
                Prefs.deleteProfile(p.id)
                if (wasCurrent) {
                    // Sang người không khoá; ai cũng có mã PIN thì màn chính hỏi "Ai đang xem?"
                    val open = Prefs.profiles.firstOrNull { !it.locked }
                    if (open != null) Prefs.switchProfile(open.id) else mustPick = true
                }
                toast(a, "Đã xoá ${p.name}.")
                changed()
            }
            .setNegativeButton("Huỷ", null)
            .show()
            .getButton(AlertDialog.BUTTON_NEGATIVE)?.requestFocus() // chọn sẵn Huỷ cho khỏi xoá nhầm
    }
}

/**
 * Màn "Ai đang xem?" phủ kín màn chính: mỗi người xem một ô (vòng tròn màu + tên, 🔒 nếu có mã PIN), ô "＋ Thêm người"
 * và nút sửa (mở Cài đặt). Lúc hiện thì chặn tiêu điểm của màn bên dưới.
 */
class ProfilePicker(
    private val activity: Activity,
    private val onPick: (Profile) -> Unit,
    private val onManage: () -> Unit,
) {
    private val root = FrameLayout(activity)
    private val row = LinearLayout(activity)

    /** Hiện ra để chọn người khi mở app / quay lại sau lâu không dùng (Back không được bỏ qua mã PIN). */
    var gate = false
        private set

    val shown get() = root.parent != null

    init {
        root.setBackgroundColor(activity.colorOf(R.color.bg))
        root.isClickable = true // chạm không lọt xuống màn chính
        val column = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            clipChildren = false
            clipToPadding = false
        }
        column.addView(
            TextView(activity).apply {
                text = "Ai đang xem?"
                textSize = 30f
                setTextColor(activity.colorOf(R.color.text))
                typeface = Typeface.DEFAULT_BOLD
            },
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT),
        )
        column.addView(
            TextView(activity).apply {
                text = "Mỗi người có lịch sử xem, Xem tiếp, phim theo dõi và cài đặt riêng."
                textSize = 15f
                setTextColor(activity.colorOf(R.color.text_dim))
            },
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                .apply { topMargin = activity.dp(8) },
        )
        row.orientation = LinearLayout.HORIZONTAL
        row.gravity = Gravity.CENTER
        row.clipChildren = false
        row.clipToPadding = false
        row.setPadding(activity.dp(24), activity.dp(18), activity.dp(24), activity.dp(18))
        val scroll = HorizontalScrollView(activity).apply {
            isHorizontalScrollBarEnabled = false
            clipChildren = false
            clipToPadding = false
            addView(row)
        }
        column.addView(
            scroll,
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                .apply { topMargin = activity.dp(18) },
        )
        val manage = LayoutInflater.from(activity).inflate(R.layout.item_button, column, false) as TextView
        manage.text = "Sửa người xem…"
        manage.setOnClickListener { onManage() }
        column.addView(manage)
        root.addView(
            column,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.CENTER),
        )
    }

    /** Màn chính bên dưới (khối đầu tiên trong content; nút cảm ứng "Thoát" của BaseActivity vẫn ở trên cùng). */
    private val under: ViewGroup? get() = activity.findViewById<FrameLayout>(android.R.id.content).getChildAt(0) as? ViewGroup

    fun show(gate: Boolean) {
        this.gate = gate
        if (!shown) {
            under?.descendantFocusability = ViewGroup.FOCUS_BLOCK_DESCENDANTS
            activity.findViewById<FrameLayout>(android.R.id.content).addView(
                root, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
            )
        }
        refresh()
    }

    /** Vẽ lại các ô (vừa thêm / sửa người xem), tiêu điểm vào người focusId (mặc định người đang dùng). */
    fun refresh(focusId: String = Prefs.profileId) {
        root.bringToFront() // trên cả cửa sổ phim nhỏ (MiniPlayer gắn vào sau, ở onResume)
        row.removeAllViews()
        val list = Prefs.profiles
        list.forEach { row.addView(tile(it)) }
        if (list.size < Prefs.MAX_PROFILES) row.addView(tile(null))
        val i = list.indexOfFirst { it.id == focusId }.coerceAtLeast(0)
        row.getChildAt(i)?.let { v -> v.post { v.focusForRemote() } }
    }

    fun hide() {
        (root.parent as? ViewGroup)?.removeView(root)
        under?.descendantFocusability = ViewGroup.FOCUS_AFTER_DESCENDANTS
    }

    private fun tile(p: Profile?): View {
        val a = activity
        val t = LinearLayout(a).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            isFocusable = true
            isClickable = true
            setBackgroundResource(R.drawable.bg_profile_tile)
            setPadding(a.dp(18), a.dp(16), a.dp(18), a.dp(12))
            focusZoom(1.08f)
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                .apply { setMargins(a.dp(10), 0, a.dp(10), 0) }
        }
        t.addView(ProfileUi.avatar(a, p, 88))
        t.addView(
            TextView(a).apply {
                text = p?.let { it.name + if (it.locked) "  🔒" else "" } ?: "Thêm người"
                textSize = 18f
                setTextColor(a.colorOf(if (p != null) R.color.text else R.color.text_dim))
                gravity = Gravity.CENTER
                setSingleLine()
                ellipsize = TextUtils.TruncateAt.END
                maxWidth = a.dp(150)
            },
            LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                .apply { topMargin = a.dp(10) },
        )
        t.setOnClickListener {
            if (p == null) ProfileUi.add(a) { added -> refresh(focusId = added.id) }
            else ProfileUi.unlock(a, p) { onPick(p) }
        }
        return t
    }
}
