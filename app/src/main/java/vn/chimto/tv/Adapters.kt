package vn.chimto.tv

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
        if (onRetry != null) retry.requestFocusFromTouch()
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
data class FilmRow(val film: Film) : GridRow

/** Lưới poster; tiêu đề nhóm chiếm cả hàng. onNearEnd: gọi khi duyệt gần cuối để tải trang tiếp. */
class FilmGridAdapter(
    private val onClick: (Film) -> Unit,
    private val onNearEnd: () -> Unit,
) : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
    private val rows = ArrayList<GridRow>()
    private var focusPosition = -1
    var spanCount = 5

    class HeaderHolder(val text: TextView) : RecyclerView.ViewHolder(text)
    class CardHolder(view: View) : RecyclerView.ViewHolder(view) {
        val poster: ImageView = view.findViewById(R.id.poster)
        val badge: TextView = view.findViewById(R.id.badge)
        val name: TextView = view.findViewById(R.id.name)
    }

    val spanLookup = object : GridLayoutManager.SpanSizeLookup() {
        override fun getSpanSize(position: Int) = if (rows.getOrNull(position) is HeaderRow) spanCount else 1
    }

    val isEmpty get() = rows.isEmpty()

    /** focusFirst: đưa tiêu điểm vào poster đầu tiên khi nó hiện ra. */
    fun submit(list: List<GridRow>, focusFirst: Boolean) {
        rows.clear()
        rows.addAll(list)
        focusPosition = if (focusFirst) rows.indexOfFirst { it is FilmRow } else -1
        notifyDataSetChanged()
    }

    fun append(list: List<GridRow>) {
        val start = rows.size
        rows.addAll(list)
        notifyItemRangeInserted(start, list.size)
    }

    fun clear() = submit(emptyList(), false)

    override fun getItemCount() = rows.size
    override fun getItemViewType(position: Int) = if (rows[position] is HeaderRow) 0 else 1

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): RecyclerView.ViewHolder {
        val inflater = LayoutInflater.from(parent.context)
        return if (viewType == 0) {
            HeaderHolder(inflater.inflate(R.layout.item_header, parent, false) as TextView)
        } else {
            CardHolder(inflater.inflate(R.layout.item_card, parent, false)).also { it.itemView.focusZoom() }
        }
    }

    override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
        when (val row = rows[position]) {
            is HeaderRow -> (holder as HeaderHolder).text.text = row.title
            is FilmRow -> {
                val h = holder as CardHolder
                val f = row.film
                h.name.text = f.title
                h.badge.text = f.badge
                h.badge.visibility = if (f.badge.isNullOrEmpty()) View.GONE else View.VISIBLE
                ImageLoader.load(h.poster, f.poster)
                h.itemView.setOnClickListener { onClick(f) }
            }
        }
        if (position == focusPosition) {
            focusPosition = -1
            holder.itemView.post { holder.itemView.requestFocusFromTouch() }
        }
        if (position >= rows.size - spanCount * 2) holder.itemView.post(onNearEnd)
    }
}
