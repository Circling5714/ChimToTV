package vn.chimto.tv

import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.ProgressBar
import android.widget.TextView
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.RecyclerView

/** Lớp phủ đang tải / lỗi (layout view_status). */
class StatusView(private val root: View) {
    private val progress = root.findViewById<ProgressBar>(R.id.statusProgress)
    private val text = root.findViewById<TextView>(R.id.statusText)
    private val retry = root.findViewById<TextView>(R.id.statusRetry)

    fun loading(message: String = root.context.getString(R.string.loading)) {
        root.visibility = View.VISIBLE
        progress.visibility = View.VISIBLE
        text.text = message
        retry.visibility = View.GONE
    }

    /** onRetry = null: chỉ hiện thông báo, không có nút. */
    fun message(message: String, onRetry: (() -> Unit)? = null) {
        root.visibility = View.VISIBLE
        progress.visibility = View.GONE
        text.text = message
        retry.visibility = if (onRetry != null) View.VISIBLE else View.GONE
        retry.setOnClickListener { onRetry?.invoke() }
        if (onRetry != null) retry.focusForRemote()
    }

    fun hide() {
        root.visibility = View.GONE
    }
}

class MenuAdapter<T>(
    private val label: (T) -> String,
    private val onClick: (T) -> Unit,
) : RecyclerView.Adapter<MenuAdapter.Holder>() {
    class Holder(val text: TextView) : RecyclerView.ViewHolder(text)

    var items: List<T> = emptyList()
        private set
    var active: T? = null
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    fun submit(list: List<T>) {
        items = list
        notifyDataSetChanged()
    }

    override fun getItemCount() = items.size

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int) =
        Holder(LayoutInflater.from(parent.context).inflate(R.layout.item_menu, parent, false) as TextView)

    override fun onBindViewHolder(holder: Holder, position: Int) {
        val item = items[position]
        holder.text.text = label(item)
        holder.text.isActivated = item == active
        holder.text.setOnClickListener { onClick(item) }
    }
}

sealed interface GridRow
data class HeaderRow(val title: String) : GridRow

/** Dòng ghi chú chữ mờ chiếm cả hàng (vd tìm mọi trang: trang đang tìm / không có phim / không trả lời). */
data class NoteRow(val text: String) : GridRow
/**
 * resume: bấm là xem tiếp luôn (hàng "Xem tiếp"); removable: giữ OK / nhấn Menu để xoá (khỏi lịch sử, hoặc bỏ theo dõi
 * nếu follow). Trang chủ tổng hợp: chip = nhãn góc trên ("3 nguồn", tên nguồn, "bản lưu"); links = các nguồn có phim này
 * (nguồn mở trước đứng đầu) — trang phim có nút đổi nguồn.
 */
data class FilmRow(
    val film: Film, val resume: Boolean = false, val removable: Boolean = false, val follow: Boolean = false,
    val chip: String? = null, val links: List<FeedItem> = emptyList(),
) : GridRow

/** Ô cuối hàng của trang chủ tổng hợp: mở cả danh sách của hàng (key = id hàng; count = số phim, 0 = không ghi). */
data class MoreRow(val key: String, val title: String, val count: Int) : GridRow

/** Ô chọn chủ đề (người xem chưa có lịch sử); on = đã chọn. */
data class ChipRow(val id: String, val label: String, val on: Boolean) : GridRow

/**
 * Lưới poster; tiêu đề nhóm chiếm cả hàng. onNearEnd: gọi khi duyệt gần cuối để tải trang tiếp. onRemove: giữ OK /
 * nhấn Menu trên phim xoá được (lịch sử, phim theo dõi).
 */
class FilmGridAdapter(
    private val onClick: (FilmRow) -> Unit,
    private val onNearEnd: () -> Unit,
    private val onRemove: (FilmRow) -> Unit,
    private val onMore: (MoreRow) -> Unit = {},
    private val onChip: (ChipRow) -> Unit = {},
) : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
    private val rows = ArrayList<GridRow>()
    private var focusPosition = -1
    var spanCount = 5

    class HeaderHolder(val text: TextView) : RecyclerView.ViewHolder(text)
    class CardHolder(view: View) : RecyclerView.ViewHolder(view) {
        val poster: ImageView = view.findViewById(R.id.poster)
        val badge: TextView = view.findViewById(R.id.badge)
        val name: TextView = view.findViewById(R.id.name)
        val chip: TextView = view.findViewById(R.id.chip)
        val more: TextView = view.findViewById(R.id.more)
    }

    class ChipHolder(val text: TextView) : RecyclerView.ViewHolder(text)

    private fun isCell(r: GridRow?) = r is FilmRow || r is MoreRow || r is ChipRow

    val spanLookup = object : GridLayoutManager.SpanSizeLookup() {
        override fun getSpanSize(position: Int) = if (isCell(rows.getOrNull(position))) 1 else spanCount
    }

    val isEmpty get() = rows.isEmpty()

    /** Các hàng đang hiện (trang chủ tổng hợp: tìm lại ô đang chọn sau khi làm mới). */
    val items: List<GridRow> get() = rows

    /** focusFirst: đưa tiêu điểm vào poster đầu tiên khi nó hiện ra. */
    fun submit(list: List<GridRow>, focusFirst: Boolean) {
        rows.clear()
        rows.addAll(list)
        focusPosition = if (focusFirst) rows.indexOfFirst(::isCell) else -1
        notifyDataSetChanged()
    }

    fun append(list: List<GridRow>) {
        val start = rows.size
        rows.addAll(list)
        notifyItemRangeInserted(start, list.size)
    }

    /** Thay các hàng từ vị trí from tới cuối bằng list (hàng phía trên giữ nguyên — tiêu điểm đang đứng không nhảy). */
    fun replaceTail(from: Int, list: List<GridRow>) {
        val removed = rows.size - from
        if (removed > 0) {
            rows.subList(from, rows.size).clear()
            notifyItemRangeRemoved(from, removed)
        }
        rows.addAll(list)
        notifyItemRangeInserted(from, list.size)
    }

    fun clear() = submit(emptyList(), false)

    /** Thay count hàng đầu (hàng "Xem tiếp") bằng list, giữ phần còn lại và tiêu điểm đang đứng. */
    fun replaceHead(count: Int, list: List<GridRow>) {
        rows.subList(0, count).clear()
        rows.addAll(0, list)
        val common = minOf(count, list.size)
        if (common > 0) notifyItemRangeChanged(0, common)
        if (list.size > count) notifyItemRangeInserted(count, list.size - count)
        if (count > list.size) notifyItemRangeRemoved(list.size, count - list.size)
    }

    fun positionOf(row: GridRow) = rows.indexOf(row)

    fun firstFilmPosition() = rows.indexOfFirst(::isCell)

    override fun getItemCount() = rows.size
    override fun getItemViewType(position: Int) = when (rows[position]) {
        is HeaderRow -> 0
        is FilmRow -> 1
        is NoteRow -> 2
        is MoreRow -> 3
        is ChipRow -> 4
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): RecyclerView.ViewHolder {
        val inflater = LayoutInflater.from(parent.context)
        return when (viewType) {
            0 -> HeaderHolder(inflater.inflate(R.layout.item_header, parent, false) as TextView)
            2 -> HeaderHolder(
                TextView(parent.context).apply {
                    textSize = 15f
                    setTextColor(context.colorOf(R.color.text_dim))
                    setPadding(context.dp(4), context.dp(14), context.dp(4), context.dp(10))
                    layoutParams = RecyclerView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                },
            )
            4 -> ChipHolder(
                (inflater.inflate(R.layout.item_button, parent, false) as TextView).apply {
                    layoutParams = RecyclerView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                        val m = context.dp(6)
                        setMargins(m, m, m, m)
                    }
                    // Ô hẹp (1 cột lưới): chữ nhỏ hơn nút thường, lề hẹp, xuống được hai dòng
                    gravity = android.view.Gravity.CENTER
                    isSingleLine = false
                    maxLines = 2
                    ellipsize = android.text.TextUtils.TruncateAt.END
                    textSize = 15f
                    minHeight = context.dp(58)
                    setPadding(context.dp(8), context.dp(8), context.dp(8), context.dp(8))
                },
            )
            else -> CardHolder(inflater.inflate(R.layout.item_card, parent, false)).also { it.itemView.focusZoom() }
        }
    }

    override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
        when (val row = rows[position]) {
            is HeaderRow -> (holder as HeaderHolder).text.text = row.title
            is NoteRow -> (holder as HeaderHolder).text.text = row.text
            is ChipRow -> (holder as ChipHolder).text.apply {
                text = if (row.on) "✓ ${row.label}" else row.label
                isActivated = row.on
                setOnClickListener { onChip(row) }
            }
            is MoreRow -> {
                val h = holder as CardHolder
                h.name.text = row.title
                h.badge.visibility = View.GONE
                h.chip.visibility = View.GONE
                h.more.visibility = View.VISIBLE
                h.more.text = if (row.count > 0) "Xem thêm ›\n${row.count} phim" else "Xem thêm ›"
                ImageLoader.load(h.poster, null)
                h.itemView.setOnClickListener { onMore(row) }
                h.itemView.setOnLongClickListener(null)
                h.itemView.isLongClickable = false
                h.itemView.setOnKeyListener(null)
            }
            is FilmRow -> {
                val h = holder as CardHolder
                val f = row.film
                h.name.text = f.title
                h.badge.text = f.badge
                h.badge.visibility = if (f.badge.isNullOrEmpty()) View.GONE else View.VISIBLE
                h.chip.text = row.chip
                h.chip.visibility = if (row.chip.isNullOrEmpty()) View.GONE else View.VISIBLE
                h.more.visibility = View.GONE
                ImageLoader.load(h.poster, f.poster)
                h.itemView.setOnClickListener { onClick(row) }
                if (row.removable) {
                    h.itemView.setOnLongClickListener {
                        onRemove(row)
                        true
                    }
                    h.itemView.setOnKeyListener { _, code, e ->
                        if (code != KeyEvent.KEYCODE_MENU) return@setOnKeyListener false
                        if (e.action == KeyEvent.ACTION_UP) onRemove(row)
                        true
                    }
                } else {
                    h.itemView.setOnLongClickListener(null)
                    h.itemView.isLongClickable = false
                    h.itemView.setOnKeyListener(null)
                }
            }
        }
        if (position == focusPosition) {
            focusPosition = -1
            holder.itemView.post { holder.itemView.focusForRemote() }
        }
        if (position >= rows.size - spanCount * 2) holder.itemView.post(onNearEnd)
    }
}
