package vn.chimto.tv

import android.app.Activity
import android.app.AlertDialog
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * Cài đặt → Trang chủ → «Các hàng trên trang chủ…» (riêng từng người xem): bật / tắt từng hàng, ▲ ▼ đổi thứ tự, «Mặc định».
 * Đổi là lưu ngay (Prefs.homeRows); quay ra màn chính thì trang chủ xếp lại, tải thêm hàng vừa bật. «Có tập mới» / «Xem
 * tiếp» chỉ bật / tắt (luôn ở đầu trang); hồ sơ trẻ em chỉ có các hàng hoạt hình.
 */
object HomeRowsUi {
    fun show(a: Activity) {
        val list = LinearLayout(a).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(a.dp(22), a.dp(6), a.dp(22), a.dp(6))
        }
        val who = if (Prefs.profiles.size >= 2) " — ${Prefs.profile.name}" else ""
        val d = AlertDialog.Builder(a, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Các hàng trên trang chủ$who")
            .setView(ScrollView(a).apply { addView(list) })
            .setNeutralButton("Mặc định", null)
            .setPositiveButton("Xong", null)
            .create()
        // Nút cần giữ tiêu điểm sau khi vẽ lại (remote: bấm ▲ nhiều lần liền vẫn đứng đúng hàng)
        var focus: String? = null

        fun visible(id: String) = if (Prefs.kids) id !in HomeFeed.ADULT_ROWS else id !in HomeFeed.KIDS_ROWS
        fun movable(id: String) = visible(id) && id !in HomeFeed.HEAD_ROWS
        fun label(id: String) = HomeFeed.ROWS.first { it.first == id }.second.replace("Vì …", "Vì ${Prefs.profile.name}")
        fun save(prefs: List<Pair<String, Boolean>>) {
            Prefs.homeRows = prefs.map { (id, on) -> if (on) id else "-$id" }
        }
        fun button(parent: LinearLayout, text: String, tag: String, onClick: () -> Unit): TextView =
            (LayoutInflater.from(a).inflate(R.layout.item_button, parent, false) as TextView).also {
                it.text = text
                it.tag = tag
                it.setOnClickListener { onClick() }
                parent.addView(it)
            }

        lateinit var render: () -> Unit
        fun toggle(id: String) {
            save(HomeFeed.rowPrefs().map { if (it.first == id) id to !it.second else it })
            focus = "t:$id"
            render()
        }
        fun move(id: String, up: Boolean) {
            val prefs = HomeFeed.rowPrefs().toMutableList()
            val i = prefs.indexOfFirst { it.first == id }
            val range = if (up) (i - 1 downTo 0) else (i + 1 until prefs.size)
            val j = range.firstOrNull { movable(prefs[it].first) } ?: return
            prefs[i] = prefs[j].also { prefs[j] = prefs[i] }
            save(prefs)
            focus = (if (up) "u:" else "d:") + id
            render()
        }

        render = render@{
            list.removeAllViews()
            list.addView(TextView(a).apply {
                text = "Hàng tắt thì không tải. «Có tập mới» và «Xem tiếp» luôn ở đầu trang; hàng nào không đủ phim thì tự ẩn." +
                    if (Prefs.kids) "\nHồ sơ trẻ em: chỉ các hàng hoạt hình hợp trẻ em." else ""
                textSize = 14f
                setTextColor(a.colorOf(R.color.text_dim))
                setPadding(0, 0, 0, a.dp(10))
            })
            val shown = HomeFeed.rowPrefs().filter { visible(it.first) }
            val ordered = shown.filter { it.first in HomeFeed.HEAD_ROWS } + shown.filter { it.first !in HomeFeed.HEAD_ROWS }
            val body = ordered.filter { it.first !in HomeFeed.HEAD_ROWS }
            for ((id, on) in ordered) {
                val row = LinearLayout(a).apply {
                    orientation = LinearLayout.HORIZONTAL
                    gravity = Gravity.CENTER_VERTICAL
                }
                button(row, (if (on) "✓  " else "✗  ") + label(id) + if (on) "" else "  (tắt)", "t:$id") { toggle(id) }.apply {
                    layoutParams = (layoutParams as LinearLayout.LayoutParams).apply { width = 0; weight = 1f }
                    gravity = Gravity.START or Gravity.CENTER_VERTICAL
                    alpha = if (on) 1f else 0.6f
                }
                if (id !in HomeFeed.HEAD_ROWS) {
                    val at = body.indexOfFirst { it.first == id }
                    for ((up, sym) in listOf(true to "▲", false to "▼")) {
                        val edge = if (up) at == 0 else at == body.size - 1
                        button(row, sym, (if (up) "u:" else "d:") + id) { move(id, up) }.apply {
                            isEnabled = !edge
                            isFocusable = !edge
                            alpha = if (edge) 0.3f else 1f
                        }
                    }
                }
                list.addView(row)
            }
            val target = focus?.let { list.findViewWithTag<View>(it) }?.takeIf { it.isFocusable }
                ?: focus?.substringAfter(':')?.let { list.findViewWithTag<View>("t:$it") }
            target?.post { target.focusForRemote() }
        }

        d.setOnShowListener {
            d.getButton(AlertDialog.BUTTON_NEUTRAL).setOnClickListener {
                Prefs.homeRows = emptyList()
                focus = null
                render()
                list.findViewWithTag<View>("t:new-eps")?.focusForRemote()
            }
            d.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { d.dismiss() }
        }
        render()
        d.show()
    }
}
