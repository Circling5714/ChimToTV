package vn.chimto.tv

import android.app.Activity
import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.TextView
import android.widget.Toast
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import kotlin.math.max

class MainActivity : Activity() {
    private enum class Kind { HOME, SEARCH, HISTORY, CATEGORY, SITE, WEB, SETTINGS }

    private data class Entry(val title: String, val kind: Kind, val url: String? = null)

    private lateinit var menu: RecyclerView
    private lateinit var grid: RecyclerView
    private lateinit var titleView: TextView
    private lateinit var siteLabel: TextView
    private lateinit var search: EditText
    private lateinit var status: StatusView

    private val menuAdapter = MenuAdapter<Entry>({ it.title }, ::onMenu)
    private val gridAdapter = FilmGridAdapter(::openFilm, ::loadMore)

    private var categories: List<Category> = emptyList()
    private var token = 0
    private var nextUrl: String? = null
    private var loadingMore = false
    private var loadedBase: String? = null
    private var lastBack = 0L

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
        siteLabel.text = Site.hostOf(Prefs.baseUrl)
        val active = menuAdapter.active
        if (loadedBase != null && loadedBase != Prefs.baseUrl) {
            // Link trang vừa đổi trong Cài đặt → tải lại từ đầu
            reloadSite()
        } else if (active?.kind == Kind.HISTORY) {
            showHistory(focusFirst = false)
        }
        Updater.autoCheck(this)
    }

    private fun reloadSite() {
        siteLabel.text = Site.hostOf(Prefs.baseUrl)
        categories = emptyList()
        rebuildMenu()
        onMenu(menuAdapter.items.first())
    }

    /** Chọn trang phim trong danh sách (sửa danh sách trong Cài đặt). */
    private fun pickSite() {
        val sites = Prefs.sites
        val names = sites.map { Site.hostOf(it) ?: it }.toTypedArray()
        AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Chọn trang phim")
            .setSingleChoiceItems(names, sites.indexOf(Prefs.baseUrl)) { dialog, which ->
                dialog.dismiss()
                if (sites[which] != Prefs.baseUrl) {
                    Prefs.baseUrl = sites[which]
                    reloadSite()
                }
            }
            .setNeutralButton("Thêm / sửa…") { _, _ -> startActivity(Intent(this, SettingsActivity::class.java)) }
            .show()
    }

    private fun rebuildMenu() {
        val items = mutableListOf(
            Entry("Trang chủ", Kind.HOME),
            Entry("Tìm kiếm", Kind.SEARCH),
            Entry("Lịch sử xem", Kind.HISTORY),
        )
        categories.forEach { items += Entry(it.title, Kind.CATEGORY, it.url) }
        items += Entry("Đổi trang phim", Kind.SITE)
        items += Entry("Duyệt web", Kind.WEB)
        items += Entry("Cài đặt", Kind.SETTINGS)
        menuAdapter.submit(items)
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
                status.message("Gõ tên phim cần tìm rồi nhấn OK / Enter.")
                search.requestFocusFromTouch()
                showKeyboard(search)
            }
            Kind.HISTORY -> showHistory(focusFirst = true)
            Kind.CATEGORY -> showList(entry.url!!)
            else -> Unit
        }
    }

    private fun showHome() {
        val t = ++token
        val base = Prefs.baseUrl
        nextUrl = null
        gridAdapter.clear()
        status.loading("Đang tải trang chủ ${Site.hostOf(base)}…")
        background({ Site.home(base) }) { r ->
            if (t != token) return@background
            r.onSuccess { home ->
                home.movedTo?.let {
                    Prefs.baseUrl = it
                    siteLabel.text = Site.hostOf(it)
                    toast("Trang đã chuyển sang ${Site.hostOf(it)} — đã tự cập nhật link.")
                }
                loadedBase = Prefs.baseUrl
                if (home.categories.isNotEmpty() && home.categories != categories) {
                    categories = home.categories
                    val active = menuAdapter.active
                    rebuildMenu()
                    menuAdapter.active = active
                }
                val rows = home.sections.flatMap { s -> listOf(HeaderRow(s.title)) + s.films.map { FilmRow(it) } }
                if (rows.isEmpty()) {
                    status.message(
                        "Không đọc được danh sách phim. Có thể trang đã đổi giao diện — thử mục «Duyệt web».",
                        ::showHome,
                    )
                } else {
                    status.hide()
                    gridAdapter.submit(rows, focusFirst = true)
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
        titleView.text = "Tìm: $q"
        showList(q) { Site.search(q) }
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
        gridAdapter.submit(items.map { FilmRow(Film(it.filmUrl, it.title, it.poster, it.epTitle)) }, focusFirst)
    }

    private fun openFilm(film: Film) {
        startActivity(DetailActivity.intent(this, film))
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
