package vn.chimto.tv

import android.app.Activity
import android.app.AlertDialog
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast

/**
 * Cài đặt → Trang chủ → «Các hàng trên trang chủ…» (riêng từng người xem): bật / tắt từng hàng, ▲ ▼ đổi thứ tự, «＋ Thêm
 * hàng…» (chủ đề có sẵn hoặc loại phim + nước + thể loại + ngôn ngữ, tối đa HomeFeed.CUSTOM_MAX), ✕ xoá hàng tự thêm,
 * «Mặc định» (thứ tự / bật tắt như ban đầu, giữ hàng tự thêm). Đổi là lưu ngay (Prefs.homeRows / homeCustom); quay ra màn chính
 * thì trang chủ xếp lại, tải hàng vừa bật / vừa thêm. «Có tập mới» / «Xem tiếp» chỉ bật / tắt (luôn ở đầu trang); hồ sơ trẻ em
 * chỉ có các hàng hoạt hình.
 */
object HomeRowsUi {
    private fun dialog(a: Activity) = AlertDialog.Builder(a, android.R.style.Theme_DeviceDefault_Dialog_Alert)

    fun show(a: Activity) {
        val list = LinearLayout(a).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(a.dp(22), a.dp(6), a.dp(22), a.dp(6))
        }
        val who = if (Prefs.profiles.size >= 2) " — ${Prefs.profile.name}" else ""
        val d = dialog(a)
            .setTitle("Các hàng trên trang chủ$who")
            .setView(ScrollView(a).apply { addView(list) })
            .setNeutralButton("Mặc định", null)
            .setPositiveButton("Xong", null)
            .create()
        // Nút cần giữ tiêu điểm sau khi vẽ lại (remote: bấm ▲ nhiều lần liền vẫn đứng đúng hàng)
        var focus: String? = null

        fun visible(id: String) = when {
            id.startsWith("custom-") -> true
            Prefs.kids -> id !in HomeFeed.ADULT_ROWS
            else -> id !in HomeFeed.KIDS_ROWS
        }
        fun movable(id: String) = visible(id) && id !in HomeFeed.HEAD_ROWS
        fun label(id: String): String {
            if (id.startsWith("custom-")) {
                return Prefs.homeCustom.firstOrNull { it.key == id }?.let { "＋ " + HomeFeed.customTitle(it) } ?: id
            }
            return HomeFeed.ROWS.first { it.first == id }.second.replace("Vì …", "Vì ${Prefs.profile.name}")
        }
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
        fun remove(id: String) {
            val c = Prefs.homeCustom.firstOrNull { it.key == id } ?: return
            dialog(a).setTitle("Xoá hàng «${HomeFeed.customTitle(c)}»?")
                .setPositiveButton("Xoá") { _, _ ->
                    Prefs.homeCustom = Prefs.homeCustom.filter { it.key != id }
                    save(HomeFeed.rowPrefs().filter { it.first != id })
                    focus = "add"
                    render()
                }
                .setNegativeButton("Huỷ", null)
                .show()
        }
        fun added(c: CustomRow) {
            if (Prefs.homeCustom.any { it.copy(id = "") == c.copy(id = "") }) {
                Toast.makeText(a, "Đã có hàng «${HomeFeed.customTitle(c)}».", Toast.LENGTH_LONG).show()
                return
            }
            Prefs.homeCustom = Prefs.homeCustom + c
            focus = "t:${c.key}"
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
                    // Hàng tự thêm có nút ✕ xoá; hàng khác giữ chỗ trống để cột ▲ ▼ thẳng hàng
                    button(row, "✕", "x:$id") { remove(id) }.apply {
                        if (!id.startsWith("custom-")) {
                            visibility = View.INVISIBLE
                            isFocusable = false
                        }
                    }
                }
                list.addView(row)
            }
            val n = Prefs.homeCustom.size
            button(list, if (n < HomeFeed.CUSTOM_MAX) "＋  Thêm hàng…" else "Đã đủ ${HomeFeed.CUSTOM_MAX} hàng tự thêm", "add") {
                if (Prefs.homeCustom.size < HomeFeed.CUSTOM_MAX) addRow(a, ::added)
            }.apply {
                layoutParams = (layoutParams as LinearLayout.LayoutParams).apply { topMargin = a.dp(8) }
                alpha = if (n < HomeFeed.CUSTOM_MAX) 1f else 0.5f
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

    /** «＋ Thêm hàng…»: chủ đề có sẵn, hoặc tự chọn loại phim / nước / thể loại / ngôn ngữ. */
    private fun addRow(a: Activity, done: (CustomRow) -> Unit) {
        val kids = Prefs.kids
        val topics = HomeFeed.TOPICS.filter { !kids || it.first in HomeFeed.KIDS_TOPICS }
            .filter { t -> Prefs.homeCustom.none { it.topic == t.first } }
        val items = topics.map { it.second } + "Tự chọn loại phim, nước, thể loại…"
        dialog(a).setTitle("Thêm hàng").setItems(items.toTypedArray()) { _, i ->
            if (i < topics.size) {
                val t = topics[i].first
                done(CustomRow("t-$t", topic = t))
            } else {
                pickFilter(a, kids, done)
            }
        }.setNegativeButton("Huỷ", null).show()
    }

    /** Tự chọn: mỗi dòng một nút mở danh sách chọn; «Thêm» thêm hàng. Hồ sơ trẻ em chỉ hoạt hình. */
    private fun pickFilter(a: Activity, kids: Boolean, done: (CustomRow) -> Unit) {
        var row = CustomRow("c${System.currentTimeMillis()}", type = "hoathinh")
        val box = LinearLayout(a).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(a.dp(22), a.dp(6), a.dp(22), a.dp(6))
        }
        val preview = TextView(a).apply {
            textSize = 15f
            setTextColor(a.colorOf(R.color.text_dim))
            setPadding(0, a.dp(8), 0, 0)
        }
        val types = if (kids) HomeFeed.TYPES.take(1) else HomeFeed.TYPES
        val countries = listOf("Mọi nước" to "") + ApiSite.countryList
        val genres = listOf("Mọi thể loại" to "") + ApiSite.genreList.filter { !kids || it.second !in HomeFeed.ADULT_GENRES }
        val langs = HomeFeed.LANGS.map { it.second to it.first }
        lateinit var refresh: () -> Unit
        fun field(title: String, options: List<Pair<String, String>>, set: (String) -> Unit): TextView =
            (LayoutInflater.from(a).inflate(R.layout.item_button, box, false) as TextView).also { b ->
                b.gravity = Gravity.START or Gravity.CENTER_VERTICAL
                b.layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT)
                    .apply { bottomMargin = a.dp(6) }
                b.setOnClickListener {
                    dialog(a).setTitle(title).setItems(options.map { it.first }.toTypedArray()) { _, i ->
                        set(options[i].second)
                        refresh()
                        b.post { b.focusForRemote() }
                    }.show()
                }
                box.addView(b)
            }
        val typeOpts = types.map { it.second to it.first }
        val bType = field("Loại phim", typeOpts) { row = row.copy(type = it) }
        val bCountry = field("Nước", countries) { row = row.copy(country = it) }
        val bGenre = field("Thể loại", genres) { row = row.copy(genre = it) }
        val bLang = field("Ngôn ngữ", langs) { row = row.copy(lang = it) }
        box.addView(preview)
        fun name(options: List<Pair<String, String>>, v: String) = options.firstOrNull { it.second == v }?.first ?: v
        refresh = {
            bType.text = "Loại phim: " + name(typeOpts, row.type)
            bCountry.text = "Nước: " + name(countries, row.country)
            bGenre.text = "Thể loại: " + name(genres, row.genre)
            bLang.text = "Ngôn ngữ: " + name(langs, row.lang)
            preview.text = "Hàng mới: «${HomeFeed.customTitle(row)}»"
        }
        refresh()
        dialog(a).setTitle("Tự chọn hàng").setView(ScrollView(a).apply { addView(box) })
            .setPositiveButton("Thêm") { _, _ -> done(row) }
            .setNegativeButton("Huỷ", null)
            .show()
        bType.post { bType.focusForRemote() } // remote: đứng sẵn ở dòng đầu
    }
}
