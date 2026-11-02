package vn.chimto.tv

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.TextView
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.RecyclerView
import kotlin.math.max

class DetailActivity : Activity() {
    companion object {
        fun intent(context: Context, film: Film): Intent = Intent(context, DetailActivity::class.java)
            .putExtra("url", film.url).putExtra("title", film.title).putExtra("poster", film.poster)
    }

    private lateinit var url: String
    private lateinit var episodes: RecyclerView
    private lateinit var status: StatusView
    private lateinit var btnPlay: TextView
    private lateinit var btnLatest: TextView
    private lateinit var btnFirst: TextView
    private lateinit var btnOrder: TextView
    private lateinit var epLabel: TextView
    private lateinit var groupRow: LinearLayout

    private val adapter = EpisodeAdapter(::play)
    private var detail: FilmDetail? = null
    private var group: EpisodeGroup? = null
    private var ascending = true

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_detail)
        url = intent.getStringExtra("url")!!
        episodes = findViewById(R.id.episodes)
        status = StatusView(findViewById(R.id.status))
        btnPlay = findViewById(R.id.btnPlay)
        btnLatest = findViewById(R.id.btnLatest)
        btnFirst = findViewById(R.id.btnFirst)
        btnOrder = findViewById(R.id.btnOrder)
        epLabel = findViewById(R.id.epLabel)
        groupRow = findViewById(R.id.groupRow)

        findViewById<TextView>(R.id.title).text = intent.getStringExtra("title")
        intent.getStringExtra("poster")?.let {
            ImageLoader.load(findViewById(R.id.poster), it)
            ImageLoader.load(findViewById(R.id.backdrop), it, maxWidth = 200)
        }

        val widthDp = resources.displayMetrics.widthPixels / resources.displayMetrics.density - 72
        episodes.layoutManager = GridLayoutManager(this, max(4, (widthDp / 104).toInt()))
        episodes.adapter = adapter
        episodes.itemAnimator = null

        btnOrder.setOnClickListener {
            ascending = !ascending
            showEpisodes()
        }
        findViewById<View>(R.id.btnWeb).setOnClickListener { startActivity(WebActivity.intent(this, url)) }
        load()
    }

    override fun onResume() {
        super.onResume()
        // Quay lại từ trình phát: cập nhật "Xem tiếp" và đánh dấu tập đã xem
        if (detail != null) {
            updateButtons()
            adapter.refreshWatched()
            scrollToResume()
        }
    }

    private fun load() {
        status.loading()
        background({ Site.detail(url) }) { r ->
            r.onSuccess { bind(it) }.onFailure { status.message("Không tải được thông tin phim.\n${it.friendly()}", ::load) }
        }
    }

    private fun bind(d: FilmDetail) {
        detail = d
        findViewById<TextView>(R.id.title).text = d.title
        findViewById<TextView>(R.id.other).apply {
            text = d.otherTitle
            visibility = if (d.otherTitle == null) View.GONE else View.VISIBLE
        }
        findViewById<TextView>(R.id.info).text = d.info
        findViewById<TextView>(R.id.desc).text = d.description
        d.poster?.let {
            ImageLoader.load(findViewById(R.id.poster), it)
            ImageLoader.load(findViewById(R.id.backdrop), it, maxWidth = 200)
        }
        if (d.groups.isEmpty()) {
            status.message("Phim này chưa có tập nào.")
        } else {
            status.hide()
        }
        // Mở sẵn bản (Việt Sub / Thuyết Minh…) đang xem dở
        group = Prefs.historyFor(d.url)?.let { d.groupOf(it.epUrl) } ?: d.groups.firstOrNull()
        buildGroupButtons(d)
        showEpisodes()
        updateButtons()
        btnPlay.requestFocusFromTouch()
    }

    private fun buildGroupButtons(d: FilmDetail) {
        groupRow.removeAllViews()
        if (d.groups.size < 2) return
        val inflater = LayoutInflater.from(this)
        for (g in d.groups) {
            val b = inflater.inflate(R.layout.item_button, groupRow, false) as TextView
            b.text = "${g.name} (${g.episodes.size})"
            b.isActivated = g == group
            b.setOnClickListener {
                group = g
                for (i in 0 until groupRow.childCount) groupRow.getChildAt(i).isActivated = groupRow.getChildAt(i) == b
                showEpisodes()
                updateButtons()
            }
            groupRow.addView(b)
        }
    }

    private fun showEpisodes() {
        val eps = group?.episodes.orEmpty()
        adapter.submit(if (ascending) eps else eps.reversed())
        epLabel.text = "Danh sách tập (${eps.size})"
        btnOrder.text = if (ascending) "Thứ tự: cũ → mới" else "Thứ tự: mới → cũ"
        btnOrder.visibility = if (eps.size > 1) View.VISIBLE else View.GONE
        scrollToResume()
    }

    /** Cuộn lưới tới tập đang xem dở để bấm ▼ là vào đúng chỗ. */
    private fun scrollToResume() {
        val d = detail ?: return
        val h = Prefs.historyFor(d.url) ?: return
        val i = adapter.indexOf(h.epUrl)
        if (i >= 0) (episodes.layoutManager as GridLayoutManager).scrollToPositionWithOffset(i, 0)
    }

    private fun updateButtons() {
        val d = detail ?: return
        val eps = group?.episodes.orEmpty()
        if (eps.isEmpty()) return
        val latest = eps.last()
        // Tập xem dở có thể thuộc bản khác với bản đang hiển thị
        val resume = Prefs.historyFor(d.url)?.let { h -> d.groups.firstNotNullOfOrNull { g -> g.episodes.firstOrNull { it.url == h.epUrl } } }

        btnPlay.visibility = View.VISIBLE
        btnPlay.text = when {
            resume != null -> "▶  Xem tiếp ${resume.title}"
            eps.size == 1 -> "▶  Xem phim"
            else -> "▶  Xem ${latest.title} (mới nhất)"
        }
        btnPlay.setOnClickListener { play(resume ?: latest) }

        btnLatest.visibility = if (resume != null && resume != latest) View.VISIBLE else View.GONE
        btnLatest.text = "Mới nhất: ${latest.title}"
        btnLatest.setOnClickListener { play(latest) }

        btnFirst.visibility = if (eps.size > 1) View.VISIBLE else View.GONE
        btnFirst.text = "Xem từ ${eps.first().title}"
        btnFirst.setOnClickListener { play(eps.first()) }
    }

    private fun play(ep: Episode) {
        val d = detail ?: return
        startActivity(PlayerActivity.intent(this, d, ep))
    }
}

class EpisodeAdapter(private val onClick: (Episode) -> Unit) : RecyclerView.Adapter<EpisodeAdapter.Holder>() {
    class Holder(val text: TextView) : RecyclerView.ViewHolder(text)

    private var items: List<Episode> = emptyList()
    private var watched: Set<String> = emptySet()

    fun submit(list: List<Episode>) {
        items = list
        watched = Prefs.watchedEpisodes()
        notifyDataSetChanged()
    }

    fun refreshWatched() {
        watched = Prefs.watchedEpisodes()
        notifyDataSetChanged()
    }

    fun indexOf(url: String) = items.indexOfFirst { it.url == url }

    override fun getItemCount() = items.size

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int) =
        Holder(LayoutInflater.from(parent.context).inflate(R.layout.item_episode, parent, false) as TextView)
            .also { it.text.focusZoom(1.08f) }

    override fun onBindViewHolder(holder: Holder, position: Int) {
        val ep = items[position]
        holder.text.text = ep.title
        holder.text.isActivated = ep.url in watched
        holder.text.setOnClickListener { onClick(ep) }
    }
}
