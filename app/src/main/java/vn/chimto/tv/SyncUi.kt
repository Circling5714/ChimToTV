package vn.chimto.tv

import android.app.Activity
import android.app.AlertDialog
import android.text.InputFilter
import android.text.InputType
import android.view.Gravity
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.widget.ArrayAdapter
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.Toast
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Cài đặt → Đồng bộ trong nhà… (SyncNet): mã ghép của TV (chỉ dùng được lúc hộp thoại đang mở), tự đồng bộ, đồng bộ
 * ngay, máy đã ghép (chọn để bỏ ghép), máy tìm thấy trong mạng (chọn rồi nhập mã ghép đang hiện trên máy đó).
 */
object SyncUi {
    private sealed class Row(val text: String) {
        class Info(text: String) : Row(text)
        class Auto(text: String) : Row(text)
        class Now(text: String) : Row(text)
        class Peer(val peer: SyncPeer, text: String) : Row(text)
        class Found(val found: SyncFound, text: String) : Row(text)
        class Manual(text: String) : Row(text)
    }

    private fun dialog(a: Activity) = AlertDialog.Builder(a, android.R.style.Theme_DeviceDefault_Dialog_Alert)

    private fun toast(a: Activity, s: String) = Toast.makeText(a, s, Toast.LENGTH_LONG).show()

    fun show(a: Activity) {
        SyncNet.openPairing()
        val rows = ArrayList<Row>()
        val adapter = ArrayAdapter<String>(a, android.R.layout.simple_list_item_1)
        val list = ListView(a).apply {
            this.adapter = adapter
            itemsCanFocus = false
        }
        val fmt = SimpleDateFormat("HH:mm dd/MM", Locale.US)

        fun rebuild() {
            rows.clear()
            rows += Row.Info("Mã ghép của TV này: ${SyncNet.pairCode ?: "----"}  —  trên iPad / iPhone: Cài đặt → Đồng bộ " +
                "trong nhà, chọn «${SyncNet.deviceName}» rồi nhập mã")
            rows += Row.Auto("Tự đồng bộ (mở app, xem xong một tập): ${if (SyncNet.auto) "Bật" else "Tắt"}")
            val peers = SyncNet.peers
            if (peers.isNotEmpty()) rows += Row.Now(if (SyncNet.busy) "Đang đồng bộ…" else "⟳ Đồng bộ ngay")
            for (p in peers) {
                val last = if (p.last > 0) " · lần cuối ${fmt.format(Date(p.last))}" else ""
                rows += Row.Peer(p, "✓ ${p.name} — ${p.status.ifEmpty { "chưa đồng bộ" }}$last")
            }
            val others = synchronized(SyncNet.found) { SyncNet.found.values.toList() }.filter { f -> peers.none { it.id == f.id } }
            for (f in others) rows += Row.Found(f, "＋ Ghép với ${f.name}")
            if (others.isEmpty()) rows += Row.Info("Đang tìm iPad / iPhone / TV có Côn Bằng TV trong mạng…")
            rows += Row.Manual("＋ Nhập địa chỉ IP của máy kia…")
            val pos = list.selectedItemPosition
            adapter.clear()
            adapter.addAll(rows.map { it.text })
            if (pos >= 0) list.setSelection(pos.coerceAtMost(rows.size - 1))
        }

        val d = dialog(a).setTitle("Đồng bộ trong nhà")
            .setMessage("Xem dở trên TV thì mở iPad / iPhone xem tiếp đúng chỗ, và ngược lại. Đồng bộ lịch sử xem, chỗ đang " +
                "xem, phim theo dõi (người xem cùng tên, và người xem đầu tiên của mỗi máy) và danh sách trang phim. " +
                "Hai máy phải cùng mạng Wi‑Fi.")
            .setView(list)
            .setNegativeButton("Xong", null)
            .setOnDismissListener {
                SyncNet.onChanged = null
                SyncNet.closePairing()
            }
            .create()

        list.setOnItemClickListener { _, _, i, _ ->
            when (val r = rows.getOrNull(i)) {
                is Row.Auto -> {
                    SyncNet.auto = !SyncNet.auto
                    rebuild()
                }
                is Row.Now -> if (!SyncNet.busy) background(a, { SyncNet.syncAll(manual = true) }) { toast(a, it) }
                is Row.Peer -> dialog(a).setTitle("Bỏ ghép ${r.peer.name}?")
                    .setMessage("TV sẽ thôi đồng bộ với máy này (dữ liệu đã đồng bộ vẫn giữ). Muốn đồng bộ lại thì ghép lại bằng mã.")
                    .setPositiveButton("Bỏ ghép") { _, _ ->
                        SyncNet.unpair(r.peer)
                        rebuild()
                    }
                    .setNegativeButton("Huỷ", null)
                    .show()
                is Row.Found -> askCode(a, r.found.name, r.found.host, r.found.port)
                is Row.Manual -> askHost(a)
                else -> Unit
            }
        }
        SyncNet.onChanged = { rebuild() }
        rebuild()
        d.show()
        list.requestFocus()
    }

    private fun field(a: Activity, hint: String, digits: Boolean): EditText = EditText(a).apply {
        this.hint = hint
        setSingleLine()
        if (digits) {
            inputType = InputType.TYPE_CLASS_NUMBER
            filters = arrayOf(InputFilter.LengthFilter(4))
            gravity = Gravity.CENTER
            textSize = 24f
            letterSpacing = 0.4f
        } else {
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            textSize = 18f
        }
        imeOptions = EditorInfo.IME_ACTION_DONE or EditorInfo.IME_FLAG_NO_EXTRACT_UI
        leaveWithUpDown()
    }

    private fun input(a: Activity, title: String, message: String, f: EditText, okText: String, ok: (String) -> Unit) {
        val box = LinearLayout(a).apply {
            setPadding(a.dp(24), a.dp(4), a.dp(24), 0)
            addView(f, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        val d = dialog(a).setTitle(title).setMessage(message).setView(box)
            .setPositiveButton(okText) { _, _ -> ok(f.text.toString().trim()) }
            .setNegativeButton("Huỷ", null)
            .create()
        d.window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE)
        f.setOnEditorActionListener { _, _, _ ->
            d.dismiss()
            ok(f.text.toString().trim())
            true
        }
        d.setOnShowListener { f.requestFocus() }
        d.show()
    }

    private fun askHost(a: Activity) {
        input(a, "Địa chỉ IP của máy kia", "Xem trên máy kia: Cài đặt → Wi‑Fi (vd 192.168.1.20).", field(a, "vd 192.168.1.20", false),
            "Tiếp") { h ->
            if (h.isNotEmpty()) askCode(a, h, h.substringBefore(':'), h.substringAfter(':', "8791").toIntOrNull() ?: 8791)
        }
    }

    private fun askCode(a: Activity, name: String, host: String, port: Int) {
        val f = field(a, "4 số", true)
        input(a, "Ghép với $name", "Nhập mã ghép đang hiện trên «$name» (Cài đặt → Đồng bộ trong nhà).", f, "Ghép") { code ->
            if (code.length != 4) return@input toast(a, "Mã ghép có 4 số.")
            background(a, { SyncNet.pair(host, port, code) }) { err -> toast(a, err ?: "Đã ghép với $name.") }
        }
    }

    /** Chạy work ở luồng nền rồi trả kết quả về luồng chính (Activity còn mở). */
    private fun <T> background(a: Activity, work: () -> T, done: (T) -> Unit) {
        Thread {
            val r = work()
            mainHandler.post { if (!a.isFinishing) done(r) }
        }.start()
    }
}
