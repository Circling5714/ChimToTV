package vn.chimto.tv

import android.graphics.Typeface
import android.os.Parcelable
import android.view.Gravity
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.LinearLayoutManager
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

/** Tiêu đề nhóm; more ≠ null: có chữ «Xem thêm ›» ở bên phải (dải vuốt ngang trên điện thoại cầm dọc). */
data class HeaderRow(val title: String, val more: MoreRow? = null) : GridRow

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
 * Dải vuốt ngang chiếm cả hàng (trang chủ tổng hợp trên điện thoại cầm dọc): cells = các ô phim, ô cuối có thể là
 * «Xem thêm» (hẹp). key = id hàng (giữ chỗ đã vuốt tới khi vẽ lại).
 */
data class StripRow(val key: String, val cells: List<GridRow>) : GridRow

/** Dải vuốt ngang: ◄ / ► ở đầu / cuối dải đứng yên (mặc định Android nhảy sang ô gần nhất của dải khác). */
class StripList(context: android.content.Context) : RecyclerView(context) {
    override fun focusSearch(focused: View?, direction: Int): View? {
        val found = super.focusSearch(focused, direction)
        if (direction != View.FOCUS_LEFT && direction != View.FOCUS_RIGHT) return found
        var v: View? = found
        while (v != null && v !== this) v = v.parent as? View
        return if (v === this) found else focused
    }
}

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

    /** > 0: ô của một dải vuốt ngang — bề ngang cố định (px), ô «Xem thêm» hẹp hơn, cao bằng poster. */
    var cellWidth = 0

    /** Bề ngang ô phim (px) của các dải vuốt ngang trong lưới này. */
    var stripCellWidth = 0

    /** Dải cần chọn một ô khi được gắn vào lưới (key dải → vị trí ô): quay về từ «Xem thêm» bằng remote, xoay máy. */
    var stripFocus: Pair<String, Int>? = null

    /** Chỗ đã vuốt tới của từng dải (theo key) — vẽ lại / cuộn đi rồi quay lại không về đầu dải. */
    private val stripStates = HashMap<String, Parcelable?>()
    private val stripPool = RecyclerView.RecycledViewPool()

    class HeaderHolder(val text: TextView) : RecyclerView.ViewHolder(text)
    class HeaderMoreHolder(view: LinearLayout, val text: TextView, val link: TextView) : RecyclerView.ViewHolder(view)
    class StripHolder(val list: StripList, val inner: FilmGridAdapter) : RecyclerView.ViewHolder(list) {
        var key: String? = null
    }
    class TileHolder(val text: TextView) : RecyclerView.ViewHolder(text)
    class CardHolder(view: View) : RecyclerView.ViewHolder(view) {
        val poster: ImageView = view.findViewById(R.id.poster)
        val badge: TextView = view.findViewById(R.id.badge)
        val name: TextView = view.findViewById(R.id.name)
        val chip: TextView = view.findViewById(R.id.chip)
        val more: TextView = view.findViewById(R.id.more)
    }

    class ChipHolder(val text: TextView) : RecyclerView.ViewHolder(text)

    private fun isCell(r: GridRow?) = r is FilmRow || r is MoreRow || r is ChipRow

    /** Ô đầu tiên chọn được (dải vuốt ngang: chính dải). */
    private fun isFocusRow(r: GridRow?) = isCell(r) || r is StripRow

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
        focusPosition = if (focusFirst) rows.indexOfFirst(::isFocusRow) else -1
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

    fun firstFilmPosition() = rows.indexOfFirst(::isFocusRow)

    /** Ghi lại chỗ đã vuốt tới của một dải đang hiện (trước khi vẽ lại cả lưới). */
    fun saveStrip(holder: RecyclerView.ViewHolder?) {
        val h = holder as? StripHolder ?: return
        h.key?.let { stripStates[it] = h.list.layoutManager?.onSaveInstanceState() }
    }

    override fun getItemCount() = rows.size
    override fun getItemViewType(position: Int) = when (val r = rows[position]) {
        is HeaderRow -> if (r.more != null) 6 else 0
        is FilmRow -> 1
        is NoteRow -> 2
        is MoreRow -> if (cellWidth > 0) 7 else 3
        is ChipRow -> 4
        is StripRow -> 5
    }

    override fun onViewRecycled(holder: RecyclerView.ViewHolder) {
        saveStrip(holder)
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
            5 -> {
                val ctx = parent.context
                val inner = FilmGridAdapter(onClick, {}, onRemove, onMore, onChip)
                val list = StripList(ctx).apply {
                    layoutParams = RecyclerView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                    layoutManager = LinearLayoutManager(ctx, LinearLayoutManager.HORIZONTAL, false)
                    adapter = inner
                    itemAnimator = null
                    setRecycledViewPool(stripPool)
                    // Ô phóng to khi chọn bằng remote không bị cắt
                    clipToPadding = false
                    clipChildren = false
                    setPadding(0, ctx.dp(4), 0, ctx.dp(2))
                    isFocusable = false
                    descendantFocusability = ViewGroup.FOCUS_AFTER_DESCENDANTS
                }
                StripHolder(list, inner)
            }
            6 -> {
                val ctx = parent.context
                val title = (inflater.inflate(R.layout.item_header, parent, false) as TextView).apply {
                    layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                    isSingleLine = true
                    ellipsize = android.text.TextUtils.TruncateAt.END
                }
                // Chỉ để chạm (remote / bàn phím đã có ô «Xem thêm» cuối dải)
                val link = TextView(ctx).apply {
                    text = "Xem thêm ›"
                    textSize = 15f
                    setTypeface(typeface, Typeface.BOLD)
                    setTextColor(ctx.colorOf(R.color.brand))
                    setPadding(ctx.dp(12), ctx.dp(14), ctx.dp(6), ctx.dp(6))
                    isFocusable = false
                    isClickable = true
                }
                val row = LinearLayout(ctx).apply {
                    orientation = LinearLayout.HORIZONTAL
                    gravity = Gravity.BOTTOM
                    layoutParams = RecyclerView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                    addView(title)
                    addView(link, LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT))
                }
                HeaderMoreHolder(row, title, link)
            }
            7 -> TileHolder(
                TextView(parent.context).apply {
                    layoutParams = RecyclerView.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                    background = context.getDrawable(R.drawable.bg_card)
                    gravity = Gravity.CENTER
                    textSize = 14f
                    setTypeface(typeface, Typeface.BOLD)
                    setTextColor(context.colorOf(R.color.text))
                    isFocusable = true
                    isClickable = true
                    setPadding(context.dp(4), 0, context.dp(4), 0)
                    focusZoom()
                },
            )
            else -> CardHolder(inflater.inflate(R.layout.item_card, parent, false)).also { it.itemView.focusZoom() }
        }
    }

    /** Ô của dải vuốt ngang: bề ngang cố định; ô «Xem thêm» hẹp, cao bằng poster (ô phim: viền 6dp, đệm 5dp, tỉ lệ 2:3). */
    private fun sizeCell(v: View, row: GridRow) {
        if (cellWidth <= 0) return
        val lp = v.layoutParams as? RecyclerView.LayoutParams ?: return
        val m = v.context.dp(6)
        if (row is MoreRow) {
            val poster = (cellWidth - 2 * m - 2 * v.context.dp(5)) * 3 / 2
            lp.width = (cellWidth * 0.62f).toInt() - 2 * m
            lp.height = poster + 2 * v.context.dp(5)
        } else {
            lp.width = cellWidth - 2 * m
        }
        lp.setMargins(m, m, m, m)
        v.layoutParams = lp
    }

    override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
        when (val row = rows[position]) {
            is HeaderRow -> if (holder is HeaderMoreHolder) {
                holder.text.text = row.title
                holder.link.setOnClickListener { row.more?.let(onMore) }
            } else {
                (holder as HeaderHolder).text.text = row.title
            }
            is StripRow -> {
                val h = holder as StripHolder
                if (h.key != null && h.key != row.key) saveStrip(h)
                h.inner.cellWidth = stripCellWidth
                h.inner.submit(row.cells, focusFirst = false)
                // Dải khác vừa gắn vào ô này: về chỗ đã vuốt tới lần trước (chưa có thì đầu dải)
                if (h.key != row.key) {
                    val state = stripStates[row.key]
                    if (state != null) h.list.layoutManager?.onRestoreInstanceState(state) else h.list.scrollToPosition(0)
                }
                h.key = row.key
                stripFocus?.takeIf { it.first == row.key }?.let { (_, at) ->
                    stripFocus = null
                    h.inner.focusPosition = at // ô đó gắn vào dải thì nhận tiêu điểm
                    h.list.scrollToPosition(at)
                }
            }
            is NoteRow -> (holder as HeaderHolder).text.text = row.text
            is ChipRow -> (holder as ChipHolder).text.apply {
                text = if (row.on) "✓ ${row.label}" else row.label
                isActivated = row.on
                setOnClickListener { onChip(row) }
            }
            is MoreRow -> if (holder is TileHolder) {
                sizeCell(holder.text, row)
                holder.text.text = android.text.SpannableString(if (row.count > 0) "›\nXem thêm\n${row.count} phim" else "›\nXem thêm").apply {
                    setSpan(android.text.style.RelativeSizeSpan(2.2f), 0, 1, 0) // dấu › to
                }
                holder.text.setOnClickListener { onMore(row) }
            } else {
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
                sizeCell(h.itemView, row)
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
