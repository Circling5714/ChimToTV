package vn.chimto.tv

import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.ArrayAdapter
import android.widget.EditText
import android.widget.TextView
import android.widget.Toast
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import kotlin.math.max

class MainActivity : BaseActivity() {
    // Trang chủ: nút cảm ứng thoát app luôn (remote thì Back hai lần)
    override val touchBackLabel = "Thoát"

    override fun onTouchBack() = finish()

    private enum class Kind { HOME, SEARCH, HISTORY, FOLLOWS, CATEGORY, FILTER, SITE, WEB, CLEAR, SETTINGS }

    private data class Entry(val title: String, val kind: Kind, val url: String? = null)

    private lateinit var menu: RecyclerView
    private lateinit var grid: RecyclerView
    private lateinit var titleView: TextView
    private lateinit var siteLabel: TextView
    private lateinit var search: EditText
    private lateinit var status: StatusView

    private val menuAdapter = MenuAdapter<Entry>({ it.title }, ::onMenu)
    private val gridAdapter = FilmGridAdapter(::openFilm, ::loadMore, ::confirmRemove)

    private var categories: List<Category> = emptyList()
    private var token = 0
    private var nextUrl: String? = null
    private var loadingMore = false
    private var loadedBase: String? = null
    private var loadedProxy: SocksConfig? = null
    private var lastBack = 0L

    /** Hàng "Xem tiếp" đang nằm đầu trang chủ (để cập nhật khi xem xong quay lại); homeToken: trang chủ đang hiện. */
    private var head: List<GridRow> = emptyList()
    private var homeToken = -1

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        menu = findViewById(R.id.menu)
        grid = findViewById(R.id.grid)
        titleView = findViewById(R.id.title)
        siteLabel = findViewById(R.id.siteLabel)
        search = findViewById(R.id.search)
        status = StatusView(findViewById(R.id.status))

        menu.layoutManager = LinearLayoutManager(this)
        menu.adapter = menuAdapter

        // Số cột theo bề ngang màn hình (TV 720p/1080p thường ~960dp → 5 cột)
        val widthDp = resources.displayMetrics.widthPixels / resources.displayMetrics.density - 240
        gridAdapter.spanCount = max(3, (widthDp / 135).toInt())
        grid.layoutManager = GridLayoutManager(this, gridAdapter.spanCount).apply { spanSizeLookup = gridAdapter.spanLookup }
        grid.adapter = gridAdapter
        grid.itemAnimator = null
        // Lưới cố định khung: thêm trang mới không bắt cả màn hình đo lại; giữ sẵn nhiều ô để cuộn qua lại không phải dựng lại
        grid.setHasFixedSize(true)
        grid.setItemViewCacheSize(gridAdapter.spanCount * 3)

        search.leaveWithUpDown()
        search.setOnEditorActionListener { _, actionId, event ->
            val enter = event?.keyCode == KeyEvent.KEYCODE_ENTER && event.action == KeyEvent.ACTION_DOWN
            if (actionId == EditorInfo.IME_ACTION_SEARCH || actionId == EditorInfo.IME_ACTION_DONE || enter) {
                doSearch()
                true
            } else {
                false
            }
        }

        rebuildMenu()
        onMenu(menuAdapter.items.first())
    }

    override fun onResume() {
        super.onResume()
        showSiteLabel()
        val active = menuAdapter.active
        if (loadedBase != null && (loadedBase != Prefs.baseUrl || loadedProxy != Prefs.proxyConfig())) {
            // Link trang hoặc proxy vừa đổi trong Cài đặt → tải lại từ đầu
            reloadSite()
        } else if (active?.kind == Kind.HISTORY) {
            showHistory(focusFirst = false)
        } else if (active?.kind == Kind.FOLLOWS) {
            showFollows(focusFirst = false)
        } else if (homeToken == token) {
            refreshHead()
        }
        // Mở phim có tập mới rồi quay ra: số "mới" trên menu đổi theo
        if (menuAdapter.items.firstOrNull { it.kind == Kind.FOLLOWS }?.title != followsTitle()) rebuildMenu()
        if (homeToken == token || active?.kind == Kind.FOLLOWS) Follows.check(this, done = ::onFollowsChecked)
        Updater.autoCheck(this)
    }

    /** Tên trang đang dùng, kèm "proxy" khi đang bật proxy SOCKS5 (để nhớ là app đang đi qua proxy). */
    private fun showSiteLabel() {
        siteLabel.text = Site.hostOf(Prefs.baseUrl) + (if (AppProxy.active) "  ·  proxy" else "") +
            (if (AppProxy.dohActive) "  ·  DoH" else "") +
            if (Prefs.privateMode) "  ·  xem riêng" else ""
    }

    private fun reloadSite() {
        showSiteLabel()
        categories = emptyList()
        rebuildMenu()
        onMenu(menuAdapter.items.first())
    }

    /**
     * Chọn trang phim trong danh sách (sửa danh sách trong Cài đặt). Mở ra là đo cùng lúc mọi trang: tải trang chủ
     * mất bao lâu, đọc được bao nhiêu phim, kèm điểm video của các lần xem trước.
     */
    private fun pickSite() {
        val sites = Prefs.sites
        val names = sites.map { Site.hostOf(it) ?: it }
        val checks = arrayOfNulls<Result<SiteCheck>>(sites.size)
        val labels = names.indices.map { siteLabel(names[it], null, false) }.toMutableList()
        val adapter = ArrayAdapter(this, android.R.layout.simple_list_item_single_choice, labels)
        val dialog = AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Chọn trang phim")
            .setSingleChoiceItems(adapter, sites.indexOf(Prefs.baseUrl)) { dialog, which ->
                dialog.dismiss()
                if (sites[which] != Prefs.baseUrl) {
                    Prefs.baseUrl = sites[which]
                    reloadSite()
                }
            }
            .setNeutralButton("Thêm / sửa…") { _, _ -> startActivity(Intent(this, SettingsActivity::class.java)) }
            .show()
        for (i in sites.indices) {
            background({ Site.check(sites[i]) }) { r ->
                if (!dialog.isShowing) return@background
                checks[i] = r
                val ok = checks.withIndex().filter { (it.value?.getOrNull()?.films ?: 0) > 0 }
                val fastest = ok.minByOrNull { it.value!!.getOrThrow().ms }?.index.takeIf { ok.size >= 2 }
                for (j in sites.indices) labels[j] = siteLabel(names[j], checks[j], j == fastest)
                adapter.notifyDataSetChanged()
            }
        }
    }

    /** Thể loại / quốc gia đã tải của trang đang dùng (đổi trang thì tải lại). */
    private var filters: Pair<String, Pair<List<Category>, List<Category>>>? = null

    private fun pickFilter(entry: Entry) {
        val base = Prefs.baseUrl
        val cached = filters?.takeIf { it.first == base }?.second
        if (cached == null) toast("Đang tải danh sách…")
        background({ cached ?: NextSite.filters(base) }) { r ->
            r.onSuccess { f ->
                filters = base to f
                val list = if (entry.url == "genre") f.first else f.second
                if (list.isEmpty()) {
                    toast("Trang không có danh sách này.")
                    return@onSuccess
                }
                AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                    .setTitle(entry.title.trimEnd('…'))
                    .setItems(list.map { it.title }.toTypedArray()) { _, which ->
                        val c = list[which]
                        menuAdapter.active = entry
                        titleView.text = c.title
                        search.visibility = View.GONE
                        showList(c.url)
                    }
                    .show()
            }.onFailure { toast("Không tải được danh sách: ${it.friendly()}") }
        }
    }

    /** Xoá nhanh cookie + dữ liệu duyệt web ngay từ menu (Cài đặt có nút tương tự, kèm xoá ảnh đã lưu). */
    private fun confirmClearBrowsingData() {
        AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Xoá dữ liệu duyệt web?")
            .setMessage(
                "Xoá cookie, dữ liệu các trang đã lưu và bộ nhớ đệm của trình duyệt trong app.\n" +
                    "Lịch sử xem, chỗ đang xem dở, cài đặt và proxy giữ nguyên.",
            )
            .setPositiveButton("Xoá") { _, _ -> BrowsingData.clear(this) { toast("Đã xoá cookie và dữ liệu duyệt web.") } }
            .setNegativeButton("Huỷ", null)
            .show()
            .getButton(AlertDialog.BUTTON_POSITIVE)?.requestFocus()
    }

    /** vd "yanhh3d.ee  —  0,4 s · 36 phim · video mượt  ★ nhanh nhất" */
    private fun siteLabel(host: String, check: Result<SiteCheck>?, fastest: Boolean): String {
        val parts = ArrayList<String>()
        when {
            check == null -> parts += "đang đo…"
            check.isFailure -> parts += "✗ không vào được"
            else -> check.getOrThrow().let {
                parts += "%.1f s".format(it.ms / 1000.0).replace('.', ',')
                parts += if (it.films > 0) "${it.films} phim" else "không đọc được phim"
            }
        }
        PlayStats.siteSummary(host)?.let { parts += it }
        return "$host  —  " + parts.joinToString(" · ") + if (fastest) "  ★ nhanh nhất" else ""
    }

    private fun rebuildMenu() {
        val items = mutableListOf(
            Entry("Trang chủ", Kind.HOME),
            Entry("Tìm kiếm", Kind.SEARCH),
            Entry("Lịch sử xem", Kind.HISTORY),
            Entry(followsTitle(), Kind.FOLLOWS),
        )
        categories.forEach { items += Entry(it.title, Kind.CATEGORY, it.url) }
        // Trang kiểu PhimMoi: đủ thể loại / quốc gia của bộ lọc (menu trang chỉ có vài mục)
        if (NextSite.isKnown(Prefs.baseUrl)) {
            items += Entry("Thể loại…", Kind.FILTER, "genre")
            items += Entry("Quốc gia…", Kind.FILTER, "country")
        }
        items += Entry("Đổi trang phim", Kind.SITE)
        items += Entry("Duyệt web", Kind.WEB)
        items += Entry("Xoá dữ liệu web", Kind.CLEAR)
        items += Entry("Cài đặt", Kind.SETTINGS)
        // Giữ mục đang chọn (tên mục "Phim theo dõi" có thể đổi theo số tập mới)
        val active = menuAdapter.active
        menuAdapter.submit(items)
        menuAdapter.active = items.firstOrNull { it.kind == active?.kind && it.url == active?.url }
    }

    private fun followsTitle(): String {
        val n = Prefs.follows.count { it.newCount > 0 }
        return if (n > 0) "Theo dõi ($n mới)" else "Phim theo dõi"
    }

    /** Vừa kiểm xong phim theo dõi: phim nào có tập mới thì báo, cập nhật menu / hàng "Có tập mới". */
    private fun onFollowsChecked(fresh: List<Follow>) {
        rebuildMenu()
        when {
            menuAdapter.active?.kind == Kind.FOLLOWS -> showFollows(focusFirst = false)
            homeToken == token -> refreshHead()
        }
        if (fresh.isNotEmpty()) {
            toast("Có tập mới: " + fresh.take(3).joinToString(", ") { "${it.title} (${it.latest})" } +
                if (fresh.size > 3) "…" else "")
        }
    }

    private fun onMenu(entry: Entry) {
        when (entry.kind) {
            Kind.WEB -> {
                startActivity(Intent(this, WebActivity::class.java))
                return
            }
            Kind.SETTINGS -> {
                startActivity(Intent(this, SettingsActivity::class.java))
                return
            }
            Kind.SITE -> {
                pickSite()
                return
            }
            Kind.CLEAR -> {
                confirmClearBrowsingData()
                return
            }
            Kind.FILTER -> {
                pickFilter(entry)
                return
            }
            else -> Unit
        }
        menuAdapter.active = entry
        titleView.text = entry.title
        search.visibility = if (entry.kind == Kind.SEARCH) View.VISIBLE else View.GONE
        when (entry.kind) {
            Kind.HOME -> showHome()
            Kind.SEARCH -> {
                token++
                gridAdapter.clear()
                status.message("Gõ tên phim cần tìm rồi nhấn OK / Enter — app tìm cùng lúc trên mọi trang trong danh sách.")
                search.focusForRemote()
                showKeyboard(search)
            }
            Kind.HISTORY -> showHistory(focusFirst = true)
            Kind.FOLLOWS -> {
                showFollows(focusFirst = true)
                Follows.check(this, done = ::onFollowsChecked)
            }
            Kind.CATEGORY -> showList(entry.url!!)
            else -> Unit
        }
    }

    private fun showHome() {
        val t = ++token
        val base = Prefs.baseUrl
        loadedProxy = Prefs.proxyConfig()
        nextUrl = null
        gridAdapter.clear()
        status.loading("Đang tải trang chủ ${Site.hostOf(base)}…")
        background({ Site.home(base) }) { r ->
            if (t != token) return@background
            r.onSuccess { home ->
                home.movedTo?.let {
                    Prefs.baseUrl = it
                    showSiteLabel()
                    toast("Trang đã chuyển sang ${Site.hostOf(it)} — đã tự cập nhật link.")
                }
                loadedBase = Prefs.baseUrl
                if (home.categories.isNotEmpty() && home.categories != categories) {
                    categories = home.categories
                    rebuildMenu()
                }
                val rows = home.sections.flatMap { s -> listOf(HeaderRow(s.title)) + s.films.map { FilmRow(it) } }
                if (rows.isEmpty()) {
                    status.message(
                        "Không đọc được danh sách phim. Có thể trang đã đổi giao diện — thử mục «Duyệt web».",
                        ::showHome,
                    )
                } else {
                    status.hide()
                    head = headRows()
                    homeToken = t
                    gridAdapter.submit(head + rows, focusFirst = true)
                    Follows.check(this, done = ::onFollowsChecked)
                }
            }.onFailure {
                loadedBase = base
                status.message("Không tải được trang chủ.\n${it.friendly()}", ::showHome)
            }
        }
    }

    private fun showList(url: String, loader: () -> FilmPage = { Site.list(url) }) {
        val t = ++token
        nextUrl = null
        gridAdapter.clear()
        status.loading()
        background(loader) { r ->
            if (t != token) return@background
            r.onSuccess { page ->
                nextUrl = page.nextUrl
                val rows = if (page.sections.isNotEmpty()) {
                    page.sections.flatMap { s -> listOf(HeaderRow(s.title)) + s.films.map { FilmRow(it) } }
                } else {
                    page.films.map { FilmRow(it) }
                }
                if (rows.isEmpty()) {
                    status.message("Không có phim nào.")
                } else {
                    status.hide()
                    gridAdapter.submit(rows, focusFirst = true)
                }
            }.onFailure { status.message("Không tải được danh sách.\n${it.friendly()}") { showList(url, loader) } }
        }
    }

    private fun loadMore() {
        val url = nextUrl ?: return
        if (loadingMore) return
        loadingMore = true
        val t = token
        background({ Site.list(url) }) { r ->
            loadingMore = false
            if (t != token) return@background
            r.onSuccess {
                nextUrl = it.nextUrl
                gridAdapter.append(it.films.map { f -> FilmRow(f) })
            }.onFailure {
                nextUrl = null
                toast("Không tải thêm được: ${it.friendly()}")
            }
        }
    }

    private fun doSearch() {
        val q = search.text.toString().trim()
        if (q.isEmpty()) return
        hideKeyboard(search)
        titleView.text = "Tìm: $q (mọi trang)"
        showList(q) { Site.searchAll(q) }
    }

    /** Đầu trang chủ: "Có tập mới" (phim theo dõi vừa ra tập) rồi "Xem tiếp" (phim xem gần nhất, mọi trang). */
    private fun headRows(): List<GridRow> {
        val rows = ArrayList<GridRow>()
        val fresh = Prefs.follows.filter { it.newCount > 0 }
        if (fresh.isNotEmpty()) {
            rows += HeaderRow("Có tập mới")
            fresh.forEach { rows += FilmRow(followFilm(it), removable = true, follow = true) }
        }
        val items = Prefs.history.take(gridAdapter.spanCount)
        if (items.isNotEmpty()) {
            rows += HeaderRow("Xem tiếp")
            items.forEach { rows += FilmRow(historyFilm(it), resume = true, removable = true) }
        }
        return rows
    }

    /** vd badge "Tập 166 mới", "+3 · Tập 168", hoặc tập mới nhất khi đã xem hết. */
    private fun followFilm(f: Follow): Film {
        val badge = when (f.newCount) {
            0 -> f.latest
            1 -> "${f.latest} mới"
            else -> "+${f.newCount} · ${f.latest}"
        }
        return Film(f.filmUrl, f.title, f.poster, badge.ifEmpty { null })
    }

    private fun showFollows(focusFirst: Boolean) {
        token++
        nextUrl = null
        val items = Prefs.follows.sortedByDescending { it.newCount > 0 }
        if (items.isEmpty()) {
            gridAdapter.clear()
            status.message("Chưa theo dõi phim nào.\nMở một phim rồi chọn «＋ Theo dõi» — trang chủ sẽ báo khi phim có tập mới.")
            return
        }
        status.hide()
        gridAdapter.submit(items.map { FilmRow(followFilm(it), removable = true, follow = true) }, focusFirst)
    }

    /** Phim trong lịch sử; phim của trang khác ghi thêm tên trang (vd hai bản "Tiên Nghịch" ở hai trang). */
    private fun historyFilm(h: HistoryItem): Film {
        val host = Site.hostOf(h.filmUrl)
        val title = if (host != null && host != Site.hostOf(Prefs.baseUrl)) "${h.title} · ${host.substringBefore('.')}" else h.title
        return Film(h.filmUrl, title, h.poster, h.epTitle)
    }

    private fun refreshHead() {
        val rows = headRows()
        if (rows == head) return
        val focused = grid.focusedChild?.let { grid.getChildAdapterPosition(it) } ?: -1
        // Đang đứng trong hàng "Xem tiếp": tiêu điểm đi theo phim (phim vừa xem lên đầu); phim bị xoá thì sang ô kế,
        // hết phim thì về phim đầu của trang
        val focusedFilm = (head.getOrNull(focused) as? FilmRow)?.film?.url
        val target = when {
            focusedFilm == null -> -1
            else -> rows.indexOfFirst { (it as? FilmRow)?.film?.url == focusedFilm }.takeIf { it >= 0 }
                ?: focused.coerceAtMost(rows.size - 1).takeIf { it >= 1 }
        }
        // Đang đứng gần đầu trang (hàng đầu của trang phim) → cuộn lên cho thấy hàng "Xem tiếp" vừa đổi / vừa có
        val nearTop = focused < head.size + gridAdapter.spanCount + 1
        gridAdapter.replaceHead(head.size, rows)
        head = rows
        grid.post {
            if (nearTop) grid.scrollToPosition(0)
            grid.post {
                val pos = target ?: gridAdapter.firstFilmPosition().takeIf { focusedFilm != null } ?: return@post
                if (pos >= 0) grid.findViewHolderForAdapterPosition(pos)?.itemView?.requestFocus()
            }
        }
    }

    private fun confirmRemove(row: FilmRow) {
        if (row.follow) confirmUnfollow(row) else confirmRemoveHistory(row)
    }

    private fun confirmUnfollow(row: FilmRow) {
        AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Bỏ theo dõi?")
            .setMessage("«${row.film.title}» — app sẽ không báo tập mới của phim này nữa.")
            .setPositiveButton("Bỏ theo dõi") { _, _ ->
                val pos = gridAdapter.positionOf(row)
                Prefs.unfollow(row.film.url)
                rebuildMenu()
                if (menuAdapter.active?.kind == Kind.FOLLOWS) {
                    showFollows(focusFirst = false)
                    val i = minOf(pos, Prefs.follows.size - 1)
                    if (i >= 0) grid.post { grid.findViewHolderForAdapterPosition(i)?.itemView?.requestFocus() }
                } else if (homeToken == token) {
                    refreshHead()
                }
            }
            .setNegativeButton("Huỷ", null)
            .show()
            .getButton(AlertDialog.BUTTON_POSITIVE)?.requestFocus()
    }

    private fun confirmRemoveHistory(row: FilmRow) {
        AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Xoá khỏi lịch sử xem?")
            .setMessage("«${row.film.title}» và chỗ đang xem dở các tập của phim này sẽ bị xoá.")
            .setPositiveButton("Xoá") { _, _ ->
                val pos = gridAdapter.positionOf(row)
                Prefs.removeHistory(row.film.url)
                if (menuAdapter.active?.kind == Kind.HISTORY) {
                    showHistory(focusFirst = false)
                    val i = minOf(pos, Prefs.history.size - 1)
                    if (i >= 0) grid.post { grid.findViewHolderForAdapterPosition(i)?.itemView?.requestFocus() }
                } else if (homeToken == token) {
                    refreshHead()
                }
            }
            .setNegativeButton("Huỷ", null)
            .show()
            .getButton(AlertDialog.BUTTON_POSITIVE)?.requestFocus()
    }

    private fun showHistory(focusFirst: Boolean) {
        token++
        nextUrl = null
        val items = Prefs.history
        if (items.isEmpty()) {
            gridAdapter.clear()
            status.message("Chưa có phim nào trong lịch sử xem.")
            return
        }
        status.hide()
        gridAdapter.submit(items.map { FilmRow(historyFilm(it), removable = true) }, focusFirst)
    }

    private fun openFilm(row: FilmRow) {
        val item = if (row.resume) Prefs.historyFor(row.film.url) else null
        startActivity(if (item != null) PlayerActivity.resume(this, item) else DetailActivity.intent(this, row.film))
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        // Đang ở lưới / ô tìm kiếm → quay về menu; đang ở menu → nhấn 2 lần để thoát
        if (grid.hasFocus() || search.hasFocus()) {
            val i = menuAdapter.items.indexOf(menuAdapter.active)
            menu.scrollToPosition(max(0, i))
            menu.post { menu.findViewHolderForAdapterPosition(i)?.itemView?.requestFocus() }
            return
        }
        val now = System.currentTimeMillis()
        if (now - lastBack < 2000) {
            super.onBackPressed()
        } else {
            lastBack = now
            toast("Nhấn Back lần nữa để thoát")
        }
    }

    private fun showKeyboard(v: View) {
        v.post { (getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).showSoftInput(v, 0) }
    }

    private fun hideKeyboard(v: View) {
        (getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).hideSoftInputFromWindow(v.windowToken, 0)
    }

    private fun toast(s: String) = Toast.makeText(this, s, Toast.LENGTH_LONG).show()
}
