package vn.chimto.tv

import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.recyclerview.widget.ConcatAdapter
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.RecyclerView
import kotlin.math.max

class DetailActivity : BaseActivity() {
    companion object {
        /** links: các nguồn có cùng phim (trang chủ tổng hợp) — có từ 2 nguồn thì hiện nút «Nguồn: … ▾» để đổi. */
        fun intent(context: Context, film: Film, links: List<FeedItem> = emptyList()): Intent = Intent(context, DetailActivity::class.java)
            .putExtra("url", film.url).putExtra("title", film.title).putExtra("poster", film.poster)
            .putStringArrayListExtra("alt_urls", ArrayList(links.map { it.url }))
            .putStringArrayListExtra("alt_titles", ArrayList(links.map { it.title }))
            .putStringArrayListExtra("alt_labels", ArrayList(links.map { it.badge.orEmpty() }))
            .putStringArrayListExtra("alt_posters", ArrayList(links.map { it.poster.orEmpty() }))
    }

    private lateinit var url: String
    private lateinit var episodes: RecyclerView
    private lateinit var status: StatusView
    private lateinit var btnPlay: TextView
    private lateinit var btnLatest: TextView
    private lateinit var btnFirst: TextView
    private lateinit var btnOrder: TextView
    private lateinit var btnJump: TextView
    private lateinit var btnFollow: TextView
    private lateinit var btnRelated: TextView
    private lateinit var epLabel: TextView
    private lateinit var groupRow: LinearLayout

    private val adapter = EpisodeAdapter(::play)

    /** Các hàng «Phim liên quan» (Related): nối sau danh sách tập trong cùng lưới — phim lẻ thấy ngay dưới nút Xem. */
    private val related = FilmGridAdapter(onClick = { openRelated(it.film) }, onNearEnd = {}, onRemove = {})
    private var relatedRows: List<Related.Row> = emptyList()
    private var relatedLoaded = false
    private var detail: FilmDetail? = null
    private var group: EpisodeGroup? = null
    private var ascending = true

    /** Số tập đang gõ bằng phím số trên remote (nhảy tới sau khi ngừng gõ). */
    private var typed = ""
    private val handler = Handler(Looper.getMainLooper())
    private val jumpTyped = Runnable {
        val n = typed.toIntOrNull()
        typed = ""
        showEpisodeLabel()
        n?.let(::jumpTo)
    }

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
        btnJump = findViewById(R.id.btnJump)
        btnFollow = findViewById(R.id.btnFollow)
        btnRelated = findViewById(R.id.btnRelated)
        epLabel = findViewById(R.id.epLabel)
        groupRow = findViewById(R.id.groupRow)

        findViewById<TextView>(R.id.title).text = intent.getStringExtra("title")
        intent.getStringExtra("poster")?.let {
            ImageLoader.load(findViewById(R.id.poster), it)
            ImageLoader.load(findViewById(R.id.backdrop), it, maxWidth = 200)
        }

        episodes.layoutManager = GridLayoutManager(this, 4).apply {
            spanSizeLookup = object : GridLayoutManager.SpanSizeLookup() {
                override fun getSpanSize(position: Int): Int {
                    val n = adapter.itemCount
                    return if (position < n) 1 else related.spanLookup.getSpanSize(position - n)
                }
            }
        }
        applyLayout()
        episodes.adapter = ConcatAdapter(adapter, related)
        episodes.itemAnimator = null
        btnRelated.setOnClickListener { jumpToRelated() }
        // Màn TV thấp: chọn phim trong hàng liên quan thì tiêu đề hàng (trong lưới) bị cuộn khuất → dòng nhãn trên lưới ghi tên hàng
        episodes.viewTreeObserver.addOnGlobalFocusChangeListener { _, now -> showSection(now) }

        btnOrder.setOnClickListener {
            ascending = !ascending
            showEpisodes()
        }
        btnJump.setOnClickListener { askEpisode() }
        findViewById<View>(R.id.btnWeb).setOnClickListener { startActivity(WebActivity.intent(this, url)) }
        showSources()
        load()
    }

    /** Phim có ở nhiều nguồn (mở từ trang chủ tổng hợp): nút «Nguồn: <trang> ▾» → chọn nguồn khác (mở lại trang phim). */
    private fun showSources() {
        val urls = intent.getStringArrayListExtra("alt_urls").orEmpty()
        if (urls.size < 2) return
        val titles = intent.getStringArrayListExtra("alt_titles").orEmpty()
        val labels = intent.getStringArrayListExtra("alt_labels").orEmpty()
        val posters = intent.getStringArrayListExtra("alt_posters").orEmpty()
        val sites = Prefs.sites
        fun name(u: String) = Site.hostOf(u)?.let { siteTag(it, sites) } ?: u
        val btn = findViewById<TextView>(R.id.btnSource)
        btn.visibility = View.VISIBLE
        btn.text = "Nguồn: ${name(url)} ▾"
        btn.setOnClickListener {
            val items = urls.indices.map { i ->
                name(urls[i]) + (labels.getOrNull(i)?.takeIf { it.isNotEmpty() }?.let { " — $it" } ?: "") +
                    if (urls[i] == url) "  ✓" else ""
            }
            AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Mở phim ở nguồn khác")
                .setItems(items.toTypedArray()) { _, i ->
                    if (urls[i] == url) return@setItems
                    val links = urls.indices.map { j ->
                        FeedItem(urls[j], titles.getOrElse(j) { "" }, posters.getOrNull(j)?.ifEmpty { null },
                            labels.getOrNull(j)?.ifEmpty { null }, emptySet(), null, 0, null, null)
                    }
                    val film = Film(urls[i], titles.getOrElse(i) { intent.getStringExtra("title").orEmpty() },
                        posters.getOrNull(i)?.ifEmpty { null } ?: intent.getStringExtra("poster"), null)
                    startActivity(intent(this, film, links))
                    finish()
                }
                .show()
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        applyLayout() // điện thoại xoay ngang / dọc
    }

    /**
     * Ngang (TV, điện thoại xoay ngang): poster bên trái, chữ và hàng nút bên phải. Điện thoại cầm dọc: poster nhỏ, hàng
     * nút xuống dưới rộng cả màn (cuộn ngang), lề nhỏ.
     */
    private fun applyLayout() {
        val narrow = Screen.narrow(this)
        val body = findViewById<LinearLayout>(R.id.body)
        val head = findViewById<View>(R.id.headRow)
        val info = findViewById<LinearLayout>(R.id.infoCol)
        val buttons = findViewById<View>(R.id.btnScroll)
        body.padH(dp(if (narrow) 14 else 36), top = dp(if (narrow) 8 else 24)) // dọc: chỗ cho nút "‹ Quay lại" do BaseActivity chừa
        findViewById<View>(R.id.poster).layoutParams.apply {
            width = dp(if (narrow) 96 else 124)
            height = dp(if (narrow) 144 else 186)
        }
        (info.layoutParams as LinearLayout.LayoutParams).marginStart = dp(if (narrow) 14 else 24)
        findViewById<TextView>(R.id.title).textSize = if (narrow) 21f else 27f
        findViewById<TextView>(R.id.desc).maxLines = if (narrow) 5 else 3
        val inInfo = buttons.parent === info
        if (narrow && inInfo) {
            info.removeView(buttons)
            body.addView(buttons, body.indexOfChild(head) + 1)
        } else if (!narrow && !inInfo) {
            body.removeView(buttons)
            info.addView(buttons)
        }
        (buttons.layoutParams as LinearLayout.LayoutParams).apply {
            width = LinearLayout.LayoutParams.MATCH_PARENT
            topMargin = dp(if (narrow) 10 else 14)
        }
        head.requestLayout()
        // Số cột tập: TV ~ ô 104dp; điện thoại dọc ô nhỏ hơn (~84dp) cho đủ 4 cột
        val widthDp = resources.configuration.screenWidthDp.takeIf { it > 0 }
            ?: (resources.displayMetrics.widthPixels / resources.displayMetrics.density).toInt()
        val span = if (narrow) max(4, (widthDp - 28) / 84) else max(4, (widthDp - 72) / 104)
        (episodes.layoutManager as GridLayoutManager).spanCount = span
        // Phim liên quan: dải vuốt ngang chiếm cả hàng, thấy ~5 phim (điện thoại dọc ~3) và một phần phim kế
        related.spanCount = span
        related.stripCellWidth = (dp(widthDp - if (narrow) 28 else 72) / ((if (narrow) 3 else 5) + 0.35f)).toInt()
        related.notifyDataSetChanged()
    }

    override fun onResume() {
        super.onResume()
        SyncNet.syncSoon() // xem xong quay ra: đồng bộ chỗ xem với iPad / iPhone (đã ghép)
        // Quay lại từ trình phát: cập nhật "Xem tiếp" và đánh dấu tập đã xem
        if (detail != null) {
            updateButtons()
            adapter.refreshWatched()
            scrollToResume()
        }
    }

    override fun onDestroy() {
        handler.removeCallbacks(jumpTyped)
        super.onDestroy()
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
        // Đang theo dõi: mở phim là đã thấy các tập mới
        d.latestEpisode?.let { Prefs.updateFollow(d.url, d.episodeCount, it.title, seen = true) }
        HomeFeed.markSeen(d)
        showFollow(d)
        btnPlay.focusForRemote()
        if (!relatedLoaded) loadRelated(d)
    }

    /** Phim lẻ (một bản, một tập): không cần ô tập — nút Xem là đủ, hàng phim liên quan lên ngay dưới. */
    private val isMovie get() = detail?.let { it.groups.size == 1 && it.episodeCount == 1 } == true

    /** Tải các hàng phim liên quan sau khi trang phim đã hiện (không làm chậm danh sách tập). */
    private fun loadRelated(d: FilmDetail) {
        relatedLoaded = true
        background({ Related.load(d.title, d.url) }) { r ->
            val rows = r.getOrNull().orEmpty()
            if (isFinishing || isDestroyed || rows.isEmpty()) return@background
            relatedRows = rows
            val host = Site.hostOf(d.url)
            val sites = Prefs.sites
            related.submit(rows.flatMap { row ->
                listOf(
                    HeaderRow(row.title),
                    StripRow("rel-${row.id}", row.films.map { f ->
                        // Phim ở trang khác: ghi tên trang ở góc (như trang chủ tổng hợp)
                        FilmRow(f, chip = Site.hostOf(f.url)?.takeIf { it != host }?.let { siteTag(it, sites) })
                    }),
                )
            }, focusFirst = false)
            btnRelated.visibility = if (isMovie) View.GONE else View.VISIBLE
        }
    }

    /** Hàng liên quan đang có tiêu điểm (null = danh sách tập / nút) → dòng nhãn trên lưới ghi tên hàng đó. */
    private var section: Related.Row? = null

    private fun showSection(focused: View?) {
        val h = focused?.let { v -> if (v === episodes) null else episodes.findContainingViewHolder(v) }
        val row = h?.takeIf { it.bindingAdapter === related }?.bindingAdapterPosition?.takeIf { it >= 0 }
            ?.let { relatedRows.getOrNull(it / 2) }
        if (row == section) return
        section = row
        (groupRow.parent as View).visibility = if (row != null) View.GONE else View.VISIBLE
        (epLabel.parent as View).visibility = if (row == null && isMovie) View.GONE else View.VISIBLE
        showEpisodeLabel()
    }

    /** Nút «Liên quan ▼» (phim bộ): cuộn xuống hàng phim liên quan đầu tiên, chọn phim đầu. */
    private fun jumpToRelated() {
        val first = relatedRows.firstOrNull() ?: return
        related.stripFocus = "rel-${first.id}" to 0
        related.notifyItemChanged(1)
        (episodes.layoutManager as GridLayoutManager).scrollToPositionWithOffset(adapter.itemCount, 0)
    }

    private fun openRelated(film: Film) {
        startActivity(intent(this, film))
    }

    private fun showFollow(d: FilmDetail) {
        if (d.groups.isEmpty()) return
        btnFollow.visibility = View.VISIBLE
        btnFollow.text = if (Prefs.followOf(d.url) != null) "✓ Đang theo dõi" else "＋ Theo dõi"
        btnFollow.setOnClickListener {
            if (Prefs.followOf(d.url) != null) {
                Prefs.unfollow(d.url)
                Toast.makeText(this, "Đã bỏ theo dõi.", Toast.LENGTH_SHORT).show()
            } else {
                Prefs.follow(d)
                Toast.makeText(this, "Đã theo dõi — trang chủ sẽ báo khi phim có tập mới.", Toast.LENGTH_LONG).show()
            }
            SyncNet.syncSoon()
            showFollow(d)
        }
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
        val movie = isMovie
        adapter.submit(if (movie) emptyList() else if (ascending) eps else eps.reversed())
        (epLabel.parent as View).visibility = if (movie) View.GONE else View.VISIBLE
        showEpisodeLabel()
        btnOrder.text = if (ascending) "Thứ tự: cũ → mới" else "Thứ tự: mới → cũ"
        btnOrder.visibility = if (eps.size > 1) View.VISIBLE else View.GONE
        btnJump.visibility = if (eps.size > 20) View.VISIBLE else View.GONE
        scrollToResume()
    }

    private fun showEpisodeLabel() {
        section?.let {
            epLabel.text = it.title
            return
        }
        val n = group?.episodes?.size ?: 0
        epLabel.text = if (typed.isEmpty()) "Danh sách tập ($n)" else "Danh sách tập ($n) — đến tập $typed…"
    }

    /** Phím số trên remote / bàn phím: gõ số tập, ngừng 1,2 giây là nhảy tới tập đó (không cần bấm nút "Đến tập…"). */
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        val digit = when (keyCode) {
            in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 -> keyCode - KeyEvent.KEYCODE_0
            in KeyEvent.KEYCODE_NUMPAD_0..KeyEvent.KEYCODE_NUMPAD_9 -> keyCode - KeyEvent.KEYCODE_NUMPAD_0
            else -> return super.onKeyDown(keyCode, event)
        }
        if (group?.episodes.isNullOrEmpty() || typed.length >= 5) return true
        typed += digit
        showEpisodeLabel()
        handler.removeCallbacks(jumpTyped)
        handler.postDelayed(jumpTyped, 1200)
        return true
    }

    private fun askEpisode() {
        val input = EditText(this).apply {
            inputType = InputType.TYPE_CLASS_NUMBER
            imeOptions = EditorInfo.IME_ACTION_GO or EditorInfo.IME_FLAG_NO_EXTRACT_UI
            isSingleLine = true
            hint = "Số tập, vd 120"
        }
        val pad = (20 * resources.displayMetrics.density).toInt()
        val box = FrameLayout(this).apply {
            setPadding(pad, pad / 2, pad, 0)
            addView(input)
        }
        fun go() = input.text.toString().trim().toIntOrNull()?.let(::jumpTo)
        val dialog = AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Đến tập số")
            .setView(box)
            .setPositiveButton("Đến") { _, _ -> go() }
            .setNegativeButton("Huỷ", null)
            .show()
        // Phím ✓ / Enter trên bàn phím số: nhảy luôn
        input.setOnEditorActionListener { _, _, _ ->
            dialog.dismiss()
            go()
            true
        }
        input.requestFocus()
    }

    /** Đưa tiêu điểm tới tập có số n (số đầu tiên trong tên tập: "Tập 120", "Tập 01", "120 - Vietsub"). */
    private fun jumpTo(n: Int) {
        val i = adapter.indexOfNumber(n)
        if (i < 0) {
            Toast.makeText(this, "Không có tập $n trong danh sách này.", Toast.LENGTH_SHORT).show()
            return
        }
        (episodes.layoutManager as GridLayoutManager).scrollToPositionWithOffset(i, 0)
        episodes.post { episodes.findViewHolderForAdapterPosition(i)?.itemView?.focusForRemote() }
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
    companion object {
        private val NUMBER = Regex("""\d+""")
    }

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

    fun indexOfNumber(n: Int) = items.indexOfFirst { NUMBER.find(it.title)?.value?.toIntOrNull() == n }

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
