package vn.chimto.tv

import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.os.Bundle
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.ArrayAdapter
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import kotlin.math.max

/**
 * Tên trang ghi sau tên phim của trang khác (vd "Tiên Nghịch · yanhh3d"): tên ngắn, nhưng hai trang trong danh sách
 * trùng tên ngắn (hoathinh3d.ee / hoathinh3d.pw) thì ghi đủ tên miền cho khỏi lẫn.
 */
fun siteTag(host: String, sites: List<String>): String {
    if (host == Stremio.HOST) return Stremio.NAME
    // "phim.nguonc.com" → "nguonc" (nhãn đầu chỉ là chữ chung)
    val short = host.split('.').let { if (it.size > 2 && it[0] in setOf("phim", "www", "m")) it[1] else it[0] }
    val clash = sites.any { s -> Site.hostOf(s).let { it != null && it != host && it.substringBefore('.') == short } }
    return if (clash) host else short
}

class MainActivity : BaseActivity() {
    // Trang chủ: nút cảm ứng thoát app luôn (remote thì Back hai lần)
    override val touchBackLabel = "Thoát"

    override fun onTouchBack() {
        PlayerActivity.closeMini()
        finish()
    }

    private enum class Kind { PIP_EXPAND, PIP_CLOSE, PROFILE, HOME, SEARCH, HISTORY, FOLLOWS, CATEGORY, FILTER, SITE, WEB, CLEAR, SETTINGS }

    private data class Entry(val title: String, val kind: Kind, val url: String? = null)

    private lateinit var menu: RecyclerView
    private lateinit var grid: RecyclerView
    private lateinit var titleView: TextView
    private lateinit var siteLabel: TextView
    private lateinit var search: EditText
    private lateinit var status: StatusView
    private lateinit var side: View
    private lateinit var scrim: View
    private lateinit var menuBtn: View
    private lateinit var siteTag: TextView

    /** Ngăn kéo menu đang mở (điện thoại cầm dọc). */
    private var drawerOpen = false

    private val menuAdapter = MenuAdapter<Entry>({ it.title }, ::onMenu)
    private val gridAdapter = FilmGridAdapter(::openFilm, ::loadMore, ::confirmRemove, ::openMore, ::toggleTopic)
    private lateinit var homeNote: TextView

    /**
     * Trang chủ tổng hợp: lần dựng gần nhất (feedAt = lúc tải xong); autoFresh = phim đang xem dở vừa có tập mới;
     * moreFrom = đang xem cả một hàng («Xem thêm») — Back quay lại trang chủ đúng ô đó; pendingFocus = ô cần chọn lại sau
     * khi dựng lại (vd chủ đề vừa bấm).
     */
    private var feedSnap: HomeFeed.Snapshot? = null
    private var feedAt = 0L
    private var autoFresh: List<FreshCard> = emptyList()
    private var moreFrom: String? = null
    private var pendingFocus: String? = null
    private var loadedMix: Boolean? = null

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

    /** Màn "Ai đang xem?"; mainStarted: đã chọn xong người xem và tải màn chính lần đầu. */
    private lateinit var picker: ProfilePicker
    private var mainStarted = false
    private var loadedProfile: String? = null

    /** Phim vào / ra khỏi cửa sổ nhỏ (hình trong hình): thêm / bỏ mục "Phóng to", "Tắt phim thu nhỏ" ở đầu menu. */
    private val pipChanged: () -> Unit = { if (!isFinishing && !isDestroyed) refreshMenu() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        menu = findViewById(R.id.menu)
        grid = findViewById(R.id.grid)
        titleView = findViewById(R.id.title)
        siteLabel = findViewById(R.id.siteLabel)
        search = findViewById(R.id.search)
        status = StatusView(findViewById(R.id.status))
        side = findViewById(R.id.side)
        scrim = findViewById(R.id.scrim)
        menuBtn = findViewById(R.id.menuBtn)
        siteTag = findViewById(R.id.siteTag)
        homeNote = findViewById(R.id.homeNote)
        menuBtn.setOnClickListener { openDrawer() }
        scrim.setOnClickListener { closeDrawer() }

        menu.layoutManager = LinearLayoutManager(this)
        menu.adapter = menuAdapter

        grid.layoutManager = GridLayoutManager(this, gridAdapter.spanCount).apply { spanSizeLookup = gridAdapter.spanLookup }
        grid.adapter = gridAdapter
        grid.itemAnimator = null
        // Lưới cố định khung: thêm trang mới không bắt cả màn hình đo lại
        grid.setHasFixedSize(true)
        applyLayout()

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

        picker = ProfilePicker(this, ::onProfilePicked) {
            startActivity(Intent(this, SettingsActivity::class.java).putExtra(SettingsActivity.EXTRA_SECTION, "profiles"))
        }
        PlayerActivity.pipListeners += pipChanged
        // Nhà nhiều người xem: hỏi "Ai đang xem?" trước khi tải (mỗi người một trang phim, lịch sử riêng)
        if (ProfileUi.mustAsk()) picker.show(gate = true) else startMain()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        applyLayout() // điện thoại xoay ngang / dọc
    }

    /**
     * Bố cục theo bề ngang màn hình. Ngang (TV, điện thoại xoay ngang): menu dọc bên trái, lưới bên phải. Điện thoại cầm
     * dọc (giống bản iPhone): thanh trên cùng ☰ · tên mục · tên trang; bấm ☰ thì menu trượt ra từ bên trái, nền tối phía
     * sau (chạm nền / chọn mục / Back là đóng); lưới 3 cột.
     */
    private fun applyLayout() {
        val narrow = Screen.narrow(this)
        val frame = findViewById<FrameLayout>(R.id.mainFrame)
        val root = findViewById<LinearLayout>(R.id.mainRoot)
        val body = findViewById<View>(R.id.body)
        if (narrow && side.parent === root) {
            root.removeView(side)
            frame.addView(side) // trên lớp nền tối (scrim)
        } else if (!narrow && side.parent !== root) {
            frame.removeView(side)
            root.addView(side, 0)
        }
        side.animate().cancel()
        scrim.animate().cancel()
        side.translationX = 0f
        scrim.alpha = 1f
        if (narrow) {
            val width = minOf(dp(300), dp(resources.configuration.screenWidthDp.coerceAtLeast(320) - 56))
            side.layoutParams = FrameLayout.LayoutParams(width, FrameLayout.LayoutParams.MATCH_PARENT, Gravity.START)
            side.elevation = dp(12).toFloat()
            side.visibility = if (drawerOpen) View.VISIBLE else View.GONE
            scrim.visibility = if (drawerOpen) View.VISIBLE else View.GONE
        } else {
            drawerOpen = false
            side.layoutParams = LinearLayout.LayoutParams(dp(200), LinearLayout.LayoutParams.MATCH_PARENT)
            side.elevation = 0f
            side.visibility = View.VISIBLE
            scrim.visibility = View.GONE
        }
        menuBtn.visibility = if (narrow) View.VISIBLE else View.GONE
        siteTag.visibility = if (narrow) View.VISIBLE else View.GONE
        body.padH(dp(if (narrow) 8 else 20), top = dp(if (narrow) 8 else 16))
        titleView.textSize = if (narrow) 22f else 24f

        // Số cột theo bề ngang còn lại (TV 720p/1080p thường ~960dp → 5 cột; điện thoại dọc ~400dp → 3 cột)
        val widthDp = resources.configuration.screenWidthDp.takeIf { it > 0 }
            ?: (resources.displayMetrics.widthPixels / resources.displayMetrics.density).toInt()
        val span = max(3, (widthDp - if (narrow) 16 else 240) / 135)
        if (span != gridAdapter.spanCount || (grid.layoutManager as GridLayoutManager).spanCount != span) {
            gridAdapter.spanCount = span
            (grid.layoutManager as GridLayoutManager).spanCount = span
            if (mainStarted && homeToken == token) refreshHead() // hàng Xem tiếp lấy đúng số phim một hàng
        }
        // Giữ sẵn nhiều ô để cuộn qua lại không phải dựng lại
        grid.setItemViewCacheSize(gridAdapter.spanCount * 3)
    }

    // Màn chính trên điện thoại cầm dọc: không hiện nút "Thoát" (đã có ☰; Back của điện thoại nhấn 2 lần là thoát)
    override val touchBackHidden get() = Screen.narrow(this)

    /** Đang chạm: nút "Thoát" nổi ở góc phải trên — dòng trạng thái trang chủ chừa chỗ cho khỏi bị che. */
    override fun onTouchUiChanged(active: Boolean) {
        if (!::homeNote.isInitialized) return
        (homeNote.layoutParams as? LinearLayout.LayoutParams)?.let { lp ->
            val end = if (active && !Screen.narrow(this)) dp(96) else dp(8)
            if (lp.marginEnd != end) {
                lp.marginEnd = end
                homeNote.layoutParams = lp
            }
        }
    }

    private fun openDrawer() {
        if (!Screen.narrow(this) || drawerOpen) return
        drawerOpen = true
        scrim.animate().cancel()
        scrim.alpha = 0f
        scrim.visibility = View.VISIBLE
        scrim.animate().alpha(1f).setDuration(200).start()
        side.animate().cancel()
        side.visibility = View.VISIBLE
        side.translationX = -side.layoutParams.width.toFloat()
        side.animate().translationX(0f).setDuration(200).start()
        val i = menuAdapter.items.indexOf(menuAdapter.active).coerceAtLeast(0)
        // Mục đang chọn ở gần đầu, vẫn thấy vài mục trước nó (vd đang ở Tìm kiếm vẫn thấy Trang chủ, Người xem)
        (menu.layoutManager as LinearLayoutManager).scrollToPositionWithOffset(max(0, i - 3), 0)
        if (!TouchUi.active) menu.post { menu.findViewHolderForAdapterPosition(i)?.itemView?.focusForRemote() }
    }

    private fun closeDrawer() {
        if (!drawerOpen) return
        drawerOpen = false
        scrim.animate().alpha(0f).setDuration(180).withEndAction { if (!drawerOpen) scrim.visibility = View.GONE }.start()
        side.animate().translationX(-side.width.toFloat()).setDuration(180)
            .withEndAction { if (!drawerOpen) side.visibility = View.GONE }.start()
        if (!TouchUi.active) menuBtn.focusForRemote()
    }

    private fun startMain() {
        mainStarted = true
        loadedProfile = Prefs.profileId
        showSiteLabel()
        rebuildMenu()
        onMenu(homeEntry())
    }

    private fun homeEntry() = menuAdapter.items.first { it.kind == Kind.HOME }

    override fun onDestroy() {
        PlayerActivity.pipListeners -= pipChanged
        searchRun = null
        super.onDestroy()
    }

    override fun onPause() {
        SyncNet.onApplied = null
        super.onPause()
    }

    /** Đồng bộ trong nhà vừa nhận dữ liệu mới từ iPad / iPhone: vẽ lại hàng Xem tiếp / lịch sử / theo dõi. */
    private fun syncApplied() {
        if (!mainStarted || picker.shown) return
        when {
            menuAdapter.active?.kind == Kind.HISTORY -> showHistory(focusFirst = false)
            menuAdapter.active?.kind == Kind.FOLLOWS -> showFollows(focusFirst = false)
            homeToken == token -> refreshHead()
        }
        refreshMenu()
    }

    override fun onResume() {
        super.onResume()
        Updater.autoCheck(this)
        SyncNet.onApplied = ::syncApplied
        SyncNet.syncSoon(1500) // đồng bộ trong nhà (đã ghép máy): mở app, xem xong quay ra
        // Quay lại app sau ≥ 30 phút không dùng: hỏi lại người xem (TV dùng chung cả nhà)
        if (ProfileUi.mustPick || (AppUse.idleReturn(this) && ProfileUi.mustAsk())) {
            ProfileUi.mustPick = false
            picker.show(gate = true)
        }
        if (picker.shown) {
            picker.refresh() // vừa thêm / sửa người xem trong Cài đặt
            return
        }
        if (!mainStarted) {
            startMain()
            return
        }
        showSiteLabel()
        val active = menuAdapter.active
        if (loadedProfile != Prefs.profileId ||
            (loadedBase != null && (loadedBase != Prefs.baseUrl || loadedProxy != Prefs.proxyConfig())) ||
            (loadedMix != null && loadedMix != Prefs.homeMix)
        ) {
            // Đổi người xem, link trang, proxy hoặc kiểu trang chủ trong Cài đặt → tải lại từ đầu
            reloadSite()
        } else if (homeToken == token && moreFrom == null && Prefs.homeMix && System.currentTimeMillis() - feedAt > FEED_REFRESH) {
            // Trang chủ tổng hợp mở lâu (xem phim xong quay ra sau 20 phút): làm mới
            showHome()
        } else if (active?.kind == Kind.HISTORY) {
            showHistory(focusFirst = false)
        } else if (active?.kind == Kind.FOLLOWS) {
            showFollows(focusFirst = false)
        } else if (homeToken == token) {
            refreshHead()
        }
        // Mở phim có tập mới rồi quay ra: số "mới" trên menu đổi theo; tên người xem / phim thu nhỏ cũng vậy
        refreshMenu()
        if (homeToken == token || active?.kind == Kind.FOLLOWS) Follows.check(this, done = ::onFollowsChecked)
    }

    /** Chọn xong người xem ở màn "Ai đang xem?" (đã nhập đúng mã PIN nếu có). */
    private fun onProfilePicked(p: Profile) {
        val changed = p.id != Prefs.profileId
        ProfileUi.switchTo(p)
        picker.hide()
        when {
            !mainStarted -> startMain()
            changed -> reloadSite()
            else -> focusMenu()
        }
        if (changed) toast("Người xem: ${p.name}")
    }

    /** Back ở màn "Ai đang xem?": xem tiếp bằng người đang dùng — trừ khi người đó có mã PIN và màn này đang chặn cửa. */
    private fun closePicker() {
        if (picker.gate && Prefs.profile.locked) {
            PlayerActivity.closeMini()
            finish()
            return
        }
        picker.hide()
        if (!mainStarted) startMain() else focusMenu()
    }

    private fun focusMenu() {
        if (Screen.narrow(this)) {
            if (!TouchUi.active) menuBtn.focusForRemote()
            return
        }
        val i = menuAdapter.items.indexOf(menuAdapter.active).coerceAtLeast(0)
        menu.scrollToPosition(i)
        menu.post { menu.findViewHolderForAdapterPosition(i)?.itemView?.focusForRemote() }
    }

    /** Tên trang đang dùng, kèm "proxy" khi đang bật proxy SOCKS5 (để nhớ là app đang đi qua proxy). */
    private fun showSiteLabel() {
        siteLabel.text = Site.hostOf(Prefs.baseUrl)?.let(Site::displayName) + (if (AppProxy.active) "  ·  proxy" else "") +
            (if (AppProxy.dohActive) "  ·  DoH" else "") +
            if (Prefs.privateMode) "  ·  xem riêng" else ""
        siteTag.text = siteLabel.text
    }

    private fun reloadSite() {
        loadedProfile = Prefs.profileId
        showSiteLabel()
        categories = emptyList()
        rebuildMenu()
        onMenu(homeEntry())
    }

    /**
     * Chọn trang phim trong danh sách (sửa danh sách trong Cài đặt). Mở ra là đo cùng lúc mọi trang: tải trang chủ
     * mất bao lâu, đọc được bao nhiêu phim, kèm điểm video của các lần xem trước.
     */
    private fun pickSite() {
        val sites = Prefs.sites
        // Cuối danh sách: thêm lại trang mẫu đã xoá (vd phimmois.fun), thêm trang khác bằng tên miền
        val missing = Prefs.missingPresets
        val names = sites.map { Site.hostOf(it) ?: it }
        val checks = arrayOfNulls<Result<SiteCheck>>(sites.size)
        val labels = (
            names.indices.map { siteLabel(names[it], null, false) } +
                missing.map { "＋ Thêm lại: ${Site.hostOf(it) ?: it}" } + "＋ Thêm trang khác…"
            ).toMutableList()
        val adapter = object : ArrayAdapter<String>(this, android.R.layout.simple_list_item_single_choice, labels) {
            override fun getView(position: Int, convertView: View?, parent: android.view.ViewGroup): View {
                val v = super.getView(position, convertView, parent) as android.widget.CheckedTextView
                // Dòng «＋ Thêm…» là lệnh chứ không phải một lựa chọn: bỏ nút tròn (giữ lại để dùng lại view)
                if (v.tag == null) v.tag = v.checkMarkDrawable ?: return v
                v.checkMarkDrawable = if (position < sites.size) v.tag as android.graphics.drawable.Drawable else null
                return v
            }
        }
        val dialog = AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Chọn trang phim")
            .setSingleChoiceItems(adapter, sites.indexOf(Prefs.baseUrl)) { dialog, which ->
                dialog.dismiss()
                when {
                    which < sites.size -> if (sites[which] != Prefs.baseUrl) {
                        Prefs.baseUrl = sites[which]
                        reloadSite()
                    }
                    which < sites.size + missing.size -> useNewSite(missing[which - sites.size])
                    else -> addSiteDialog()
                }
            }
            .setNeutralButton("Sửa tên miền…") { _, _ -> startActivity(Intent(this, SettingsActivity::class.java)) }
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

    /** Thêm trang vào danh sách rồi chuyển sang trang đó. */
    private fun useNewSite(url: String) {
        Prefs.addSite(url)
        Prefs.baseUrl = url
        reloadSite()
        toast("Đã thêm ${Site.hostOf(url)} vào danh sách trang phim.")
    }

    /** Gõ tên miền trang mới (vd phimmois.fun) — trang kiểu yanhh3d / HaLim / PhimMoi. */
    private fun addSiteDialog() {
        val input = EditText(this).apply {
            hint = "Tên miền, vd phimmois.fun"
            setSingleLine()
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_URI
            imeOptions = EditorInfo.IME_ACTION_DONE or EditorInfo.IME_FLAG_NO_EXTRACT_UI
            leaveWithUpDown()
        }
        val box = android.widget.FrameLayout(this).apply {
            setPadding(dp(24), dp(8), dp(24), 0)
            addView(input)
        }
        val d = AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Thêm trang phim")
            .setView(box)
            .setPositiveButton("Thêm", null)
            .setNegativeButton("Huỷ") { _, _ -> hideKeyboard(input) }
            .create()
        d.window?.setSoftInputMode(android.view.WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE)
        fun submit() {
            val url = Prefs.normalizeUrl(input.text.toString())
            if (url == null) {
                input.error = "Tên miền không hợp lệ (vd phimmois.fun)"
                return
            }
            hideKeyboard(input)
            d.dismiss()
            useNewSite(url)
        }
        d.setOnShowListener {
            d.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { submit() }
            input.requestFocus()
        }
        input.setOnEditorActionListener { _, _, _ ->
            submit()
            true
        }
        d.show()
    }

    /** Thể loại / quốc gia đã tải của trang đang dùng (đổi trang thì tải lại). */
    private var filters: Pair<String, Pair<List<Category>, List<Category>>>? = null

    private fun pickFilter(entry: Entry) {
        val base = Prefs.baseUrl
        val cached = filters?.takeIf { it.first == base }?.second
        if (cached == null) toast("Đang tải danh sách…")
        background({ cached ?: if (ApiSite.isKnown(base)) ApiSite.filters(base) else NextSite.filters(base) }) { r ->
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
        return Site.displayName(host) + "  —  " + parts.joinToString(" · ") + if (fastest) "  ★ nhanh nhất" else ""
    }

    /** Dựng lại menu nếu có mục đổi tên (số phim có tập mới, người xem, phim đang thu nhỏ). */
    private fun refreshMenu() {
        if (menuAdapter.items.map { it.title } != menuEntries().map { it.title }) rebuildMenu()
    }

    private fun menuEntries(): List<Entry> {
        val items = mutableListOf<Entry>()
        // Phim đang thu nhỏ ở góc màn hình (hình trong hình): phóng to / tắt bằng remote
        PlayerActivity.pipTitle?.let {
            items += Entry("▶ Phóng to: $it", Kind.PIP_EXPAND)
            items += Entry("✕ Tắt phim thu nhỏ", Kind.PIP_CLOSE)
        }
        // Người xem đang dùng — chọn để đổi / thêm người (hiện cả khi mới có một người, cho dễ thấy tính năng)
        items += Entry("Người xem: ${Prefs.profile.name}", Kind.PROFILE)
        items += Entry("Trang chủ", Kind.HOME)
        items += Entry("Tìm kiếm", Kind.SEARCH)
        items += Entry("Lịch sử xem", Kind.HISTORY)
        items += Entry(followsTitle(), Kind.FOLLOWS)
        categories.forEach { items += Entry(it.title, Kind.CATEGORY, it.url) }
        // Trang kiểu PhimMoi / trang API (mephim): đủ thể loại / quốc gia của bộ lọc (menu trang chỉ có vài mục)
        if (NextSite.isKnown(Prefs.baseUrl) || ApiSite.isKnown(Prefs.baseUrl)) {
            items += Entry("Thể loại…", Kind.FILTER, "genre")
            items += Entry("Quốc gia…", Kind.FILTER, "country")
        }
        items += Entry("Đổi trang phim", Kind.SITE)
        items += Entry("Duyệt web", Kind.WEB)
        items += Entry("Xoá dữ liệu web", Kind.CLEAR)
        items += Entry("Cài đặt", Kind.SETTINGS)
        return items
    }

    private fun rebuildMenu() {
        val items = menuEntries()
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
        closeDrawer()
        when (entry.kind) {
            Kind.PIP_EXPAND -> {
                PlayerActivity.expandPip(this)
                return
            }
            Kind.PIP_CLOSE -> {
                PlayerActivity.closePip()
                return
            }
            Kind.PROFILE -> {
                picker.show(gate = false)
                return
            }
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
        moreFrom = null
        homeNote.visibility = View.GONE
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
        loadedMix = Prefs.homeMix
        if (Prefs.homeMix) {
            showMixHome()
            return
        }
        homeNote.visibility = View.GONE
        autoFresh = emptyList()
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
                    Prefs.replaceSite(base, it)
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

    /** Một lượt tải trang chủ tổng hợp: t = token; shown = đã hiện gì đó (trang lần trước / bản lưu); fresh = đã có bản mới. */
    private class FeedRun(val t: Int, val refreshing: Boolean) {
        val startedAt = System.currentTimeMillis()
        var shown = refreshing
        var fresh = false
    }

    private fun debugFeed(run: FeedRun, step: String) {
        if (BuildConfig.DEBUG) android.util.Log.d("ChimToFeed", "$step sau ${System.currentTimeMillis() - run.startedAt} ms")
    }

    /**
     * Trang chủ tổng hợp: 1) hiện ngay trang chủ lần trước của người xem (một tệp nhỏ), 2) lập kế hoạch ở luồng nền (đọc lịch
     * sử, tính sở thích — TV yếu đọc JSON chậm), 3) chưa có trang lần trước thì dựng từ bản lưu từng nguồn, 4) tải mới mọi
     * nguồn (mỗi nguồn tối đa 6 giây) rồi thay vào — ô đang chọn giữ nguyên chỗ. Nguồn lỗi / chậm ghi ở dòng trạng thái bên
     * phải tiêu đề, không che màn hình.
     */
    private fun showMixHome() {
        // Trang chủ đang hiện (làm mới / vừa chọn chủ đề): giữ nguyên lưới tới khi có bản mới, không vẽ lại bản lưu
        val refreshing = homeToken == token && feedSnap != null && !gridAdapter.isEmpty
        val run = FeedRun(++token, refreshing)
        nextUrl = null
        loadedBase = Prefs.baseUrl
        loadedProxy = Prefs.proxyConfig()
        homeNote.visibility = if (Screen.narrow(this)) View.GONE else View.VISIBLE
        homeNote.text = "Đang làm mới…"
        if (run.refreshing) {
            homeToken = run.t
        } else {
            gridAdapter.clear()
            status.loading("Đang tải trang chủ…")
        }
        val pid = Prefs.profileId
        background({ if (run.refreshing) null else HomeFeed.last(pid) }) { lr ->
            if (run.t != token) return@background
            lr.getOrNull()?.let {
                run.shown = showSnapshot(run.t, it, first = true)
                debugFeed(run, "trang chủ lần trước")
            }
            planFeed(run)
        }
    }

    private fun planFeed(run: FeedRun) {
        background({ HomeFeed.plan() }) { pr ->
            if (run.t != token) return@background
            val plan = pr.getOrElse {
                if (!run.shown) status.message("Không tải được trang chủ.\n${it.friendly()}", ::showHome)
                return@background
            }
            homeNote.text = "Đang làm mới ${plan.src.count} nguồn…"
            if (!run.shown) {
                status.loading("Đang tải trang chủ từ ${plan.src.count} nguồn…")
                background({ HomeFeed.cached(plan) }) { r ->
                    debugFeed(run, "bản lưu từng nguồn")
                    if (run.t != token || run.fresh) return@background
                    r.getOrNull()?.let { run.shown = showSnapshot(run.t, it, first = true) }
                }
            }
            background({ HomeFeed.load(plan) }) { r ->
                if (run.t != token) return@background
                run.fresh = true
                debugFeed(run, "tải xong")
                r.onSuccess {
                    feedAt = System.currentTimeMillis()
                    showSnapshot(run.t, it, first = !run.shown)
                    followLate(run.t, it, 0)
                }.onFailure {
                    if (!run.shown) status.message("Không tải được trang chủ.\n${it.friendly()}", ::showHome)
                }
            }
        }
    }

    /**
     * Nguồn chậm (TV yếu đọc trang web mất 10–15 giây) về muộn: cứ 4 giây xem bản lưu của chúng đã mới chưa, có thì dựng lại
     * trang (ô đang chọn giữ nguyên). Thôi sau 5 lần — dòng trạng thái ghi «chậm, dùng bản lưu».
     */
    private fun followLate(t: Int, snap: HomeFeed.Snapshot, tries: Int) {
        if (!snap.pending) return
        if (tries >= LATE_TRIES) {
            if (t == token) homeNote.text = snap.noteGaveUp
            return
        }
        mainHandler.postDelayed({
            if (t != token || isFinishing || isDestroyed) return@postDelayed
            background({ HomeFeed.late(snap) }) { r ->
                if (t != token) return@background
                val next = r.getOrNull()
                if (next != null) showSnapshot(t, next, first = false)
                followLate(t, next ?: snap, tries + 1)
            }
        }, LATE_STEP)
    }

    /** Hiện một lần dựng trang chủ tổng hợp; first = thay cả lưới (chọn ô đầu / pendingFocus). false nếu không có gì. */
    private fun showSnapshot(t: Int, snap: HomeFeed.Snapshot, first: Boolean): Boolean {
        feedSnap = snap
        nextUrl = null // vừa từ «Xem thêm» (danh sách có trang sau) quay lại: trang chủ không tải nối
        autoFresh = snap.fresh
        homeNote.text = snap.note
        snap.movedTo?.let { (old, new) ->
            if (old == Prefs.baseUrl) {
                Prefs.replaceSite(old, new)
                loadedBase = new
                showSiteLabel()
                toast("Trang đã chuyển sang ${Site.hostOf(new)} — đã tự cập nhật link.")
            }
        }
        snap.categories?.let {
            if (it.isNotEmpty() && it != categories) {
                categories = it
                rebuildMenu()
            }
        }
        val body = feedRows(snap)
        val headNow = headRows()
        if (body.isEmpty() && headNow.isEmpty()) {
            if (!snap.fromCache) status.message("Không tải được phim từ nguồn nào.\n${snap.note}", ::showHome)
            return false
        }
        status.hide()
        head = headNow
        homeToken = t
        val rows = head + body
        val focusKey = pendingFocus ?: if (first) null else focusedKey()
        pendingFocus = null
        gridAdapter.submit(rows, focusFirst = first && focusKey == null)
        if (focusKey != null) focusKeyAfterSubmit(focusKey)
        if (!snap.fromCache) Follows.check(this, done = ::onFollowsChecked)
        return true
    }

    /** Khoá của ô đang chọn trong lưới (địa chỉ phim / id hàng «Xem thêm» / id chủ đề). */
    private fun focusedKey(): String? {
        val pos = grid.focusedChild?.let { grid.getChildAdapterPosition(it) } ?: return null
        return keyOf(gridAdapter.items.getOrNull(pos))
    }

    private fun keyOf(r: GridRow?): String? = when (r) {
        is FilmRow -> r.film.url
        is MoreRow -> "more:" + r.key
        is ChipRow -> "chip:" + r.id
        else -> null
    }

    private fun focusKeyAfterSubmit(key: String) {
        val i = gridAdapter.items.indexOfFirst { keyOf(it) == key }.takeIf { it >= 0 } ?: gridAdapter.firstFilmPosition()
        if (i < 0) return
        grid.post {
            grid.scrollToPosition(i)
            grid.post { grid.findViewHolderForAdapterPosition(i)?.itemView?.focusForRemote() }
        }
    }

    /** Các hàng phim của trang chủ tổng hợp (dưới «Có tập mới» / «Xem tiếp»): mỗi hàng một dòng, ô cuối «Xem thêm». */
    private fun feedRows(snap: HomeFeed.Snapshot): List<GridRow> {
        val out = ArrayList<GridRow>()
        val span = gridAdapter.spanCount
        if (Prefs.history.isEmpty()) {
            out += HeaderRow("Chọn vài chủ đề bạn thích — trang chủ sẽ gợi ý theo đó (OK để chọn / bỏ)")
            val on = Prefs.homeTopics
            HomeFeed.TOPICS.forEach { (id, label) -> out += ChipRow(id, label, id in on) }
        }
        for (row in snap.rows) {
            out += HeaderRow(row.title)
            val n = row.cards.size
            val show = if (n > span) row.cards.take(span - 1) else row.cards
            show.forEach { c -> out += FilmRow(c.film, chip = c.chip, links = c.links) }
            // Hàng có danh sách nhiều trang của nguồn: không ghi số phim (mới tải trang đầu)
            if (n > span) out += MoreRow(row.id, row.title, if (row.more != null) 0 else n)
        }
        return out
    }

    /** «Xem thêm»: cả hàng (danh sách có phân trang của nguồn nếu có, không thì mọi phim đã gộp). Back quay lại trang chủ. */
    private fun openMore(m: MoreRow) {
        val row = feedSnap?.rows?.firstOrNull { it.id == m.key } ?: return
        titleView.text = row.title
        homeNote.visibility = View.GONE
        if (row.more != null) {
            showList(row.more)
        } else {
            token++
            nextUrl = null
            status.hide()
            gridAdapter.submit(row.cards.map { FilmRow(it.film, chip = it.chip, links = it.links) }, focusFirst = true)
        }
        moreFrom = m.key
    }

    /** Back từ «Xem thêm»: dựng lại trang chủ từ lần tải gần nhất (không tải lại), chọn đúng ô «Xem thêm» vừa bấm. */
    private fun backFromMore(): Boolean {
        val key = moreFrom ?: return false
        moreFrom = null
        val snap = feedSnap ?: return false
        titleView.text = homeEntry().title
        homeNote.visibility = if (Screen.narrow(this)) View.GONE else View.VISIBLE
        pendingFocus = "more:$key"
        showSnapshot(++token, snap, first = true)
        return true
    }

    /** Chọn / bỏ một chủ đề (người xem chưa có lịch sử) rồi tải lại trang chủ, giữ ô chủ đề đang chọn. */
    private fun toggleTopic(c: ChipRow) {
        val set = Prefs.homeTopics.toMutableSet()
        if (!set.add(c.id)) set.remove(c.id)
        Prefs.homeTopics = set
        pendingFocus = "chip:" + c.id
        feedSnap?.let { showSnapshot(token, it, first = true) } // chủ đề đổi màu ngay, rồi tải các hàng theo chủ đề
        showHome()
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

    /**
     * Một lần tìm mọi trang. queue: trang chưa hỏi (theo thứ tự danh sách, trang đang dùng trước); running: trang đang hỏi
     * (tối đa SEARCH_AT_ONCE); rows = các hàng kết quả đã hiện (không kể dòng ghi chú).
     */
    private class SearchRun(val token: Int, val query: String, sites: List<String>) {
        val queue = ArrayDeque(sites)
        val running = ArrayList<String>()
        val empty = ArrayList<String>()
        val failed = ArrayList<String>()
        val rows = ArrayList<GridRow>()
    }

    private var searchRun: SearchRun? = null

    private companion object {
        /** Hỏi cùng lúc tối đa 3 trang (TV box yếu: mỗi trang một kết nối HTTPS + đọc HTML). */
        const val SEARCH_AT_ONCE = 3

        /** Mỗi trang chờ tối đa 12 giây kể từ lúc bắt đầu hỏi trang đó (không vào được thì thôi, nhường chỗ trang khác). */
        const val SEARCH_TIMEOUT = 12_000L

        /** Trang chủ tổng hợp: quay lại app sau 20 phút thì tải mới. */
        const val FEED_REFRESH = 20 * 60_000L

        /** Chờ nguồn chậm về muộn: 5 lần, mỗi lần 4 giây. */
        const val LATE_TRIES = 5
        const val LATE_STEP = 4_000L
    }

    /**
     * Tìm trên mọi trang trong danh sách, cùng lúc tối đa 3 trang: kết quả trang nào về trước hiện trước (không chờ trang
     * chậm / không vào được), dòng cuối ghi trang đang tìm / không có phim / không trả lời.
     */
    private fun doSearch() {
        val q = search.text.toString().trim()
        if (q.isEmpty()) return
        hideKeyboard(search)
        titleView.text = "Tìm: $q (mọi trang)"
        val t = ++token
        nextUrl = null
        gridAdapter.clear()
        val sites = (listOf(Prefs.baseUrl) + Prefs.sites).distinct()
        val run = SearchRun(t, q, sites)
        searchRun = run
        status.loading("Đang tìm «$q» trên ${sites.size} trang…")
        startSearches(run)
    }

    private fun searchLive(run: SearchRun) = searchRun === run && token == run.token && !isFinishing && !isDestroyed

    /** Hỏi tiếp các trang đang chờ cho tới khi đủ SEARCH_AT_ONCE trang đang hỏi. */
    private fun startSearches(run: SearchRun) {
        while (run.running.size < SEARCH_AT_ONCE && run.queue.isNotEmpty()) {
            val base = run.queue.removeFirst()
            val host = Site.hostOf(base) ?: base
            run.running += host
            // Luồng riêng mỗi trang: trang quá giờ (kết nối còn treo tới khi Http hết thời gian chờ) không giữ chỗ trang khác
            Thread {
                val r = runCatching { Site.search(run.query, base) }
                mainHandler.post { siteSearched(run, host, r) }
            }.apply { isDaemon = true }.start()
            mainHandler.postDelayed({ siteSearched(run, host, null) }, SEARCH_TIMEOUT)
        }
    }

    /** Một trang trả lời (r) hoặc quá giờ (r = null). Về muộn sau khi đã quá giờ / lần tìm khác thì bỏ. */
    private fun siteSearched(run: SearchRun, host: String, r: Result<FilmPage>?) {
        if (!searchLive(run) || !run.running.remove(host)) return
        val films = r?.getOrNull()?.let { p -> (p.films + p.sections.flatMap { it.films }).distinctBy { it.url } }
        when {
            films == null -> run.failed += host
            films.isEmpty() -> run.empty += host
        }
        startSearches(run)
        showSearch(run, if (films.isNullOrEmpty()) emptyList() else listOf(HeaderRow("$host — ${films.size} phim")) + films.map { FilmRow(it) })
    }

    private fun showSearch(run: SearchRun, added: List<GridRow>) {
        val notes = ArrayList<String>()
        val waiting = run.running + run.queue.map { Site.hostOf(it) ?: it }
        if (waiting.isNotEmpty()) notes += "Đang tìm: ${waiting.joinToString(", ")}…"
        if (run.empty.isNotEmpty()) notes += "Không có phim: ${run.empty.joinToString(", ")}"
        if (run.failed.isNotEmpty()) notes += "Không trả lời: ${run.failed.joinToString(", ")}"
        val note = notes.takeIf { it.isNotEmpty() }?.let { NoteRow(it.joinToString("   ·   ")) }
        val before = run.rows.size
        run.rows += added
        if (run.rows.isEmpty()) {
            val text = notes.joinToString("\n")
            when {
                waiting.isNotEmpty() -> status.loading("Đang tìm «${run.query}»…\n$text")
                run.empty.isEmpty() -> status.message("Không tìm được — các trang không trả lời.\n${notes.joinToString("\n")}", ::doSearch)
                else -> status.message("Không tìm thấy phim nào tên «${run.query}».\n$text")
            }
            return
        }
        status.hide()
        if (before == 0) gridAdapter.submit(run.rows + listOfNotNull(note), focusFirst = true)
        else gridAdapter.replaceTail(before, added + listOfNotNull(note))
    }

    /**
     * Đầu trang chủ: "Có tập mới" (phim theo dõi vừa ra tập; trang chủ tổng hợp: cả phim đang xem dở vừa có tập mới ở bất
     * kỳ nguồn nào) rồi "Xem tiếp" (phim xem gần nhất, mọi trang).
     */
    private fun headRows(): List<GridRow> {
        val rows = ArrayList<GridRow>()
        val fresh = Prefs.follows.filter { it.newCount > 0 }
        val followed = Prefs.follows.map { KkPhim.norm(it.title) }.toSet()
        // Đã mở phim / xem tập mới từ lúc dựng trang (seenEp tăng) thì thôi báo
        val auto = if (!Prefs.homeMix) emptyList() else autoFresh.filter { (Prefs.seenEp(it.key) ?: -1.0) < it.ep && it.key !in followed }
        if (fresh.isNotEmpty() || auto.isNotEmpty()) {
            rows += HeaderRow("Có tập mới")
            fresh.forEach { rows += FilmRow(followFilm(it), removable = true, follow = true) }
            auto.forEach { rows += FilmRow(it.card.film, chip = it.card.chip, links = it.card.links) }
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
        if (host == null || host == Site.hostOf(Prefs.baseUrl)) return Film(h.filmUrl, h.title, h.poster, h.epTitle)
        return Film(h.filmUrl, "${h.title} · ${siteTag(host, Prefs.sites)}", h.poster, h.epTitle)
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
                SyncNet.syncSoon()
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
                SyncNet.syncSoon()
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
        startActivity(if (item != null) PlayerActivity.resume(this, item) else DetailActivity.intent(this, row.film, row.links))
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        if (picker.shown) {
            closePicker()
            return
        }
        if (drawerOpen) {
            closeDrawer()
            return
        }
        // Đang xem cả một hàng («Xem thêm») của trang chủ tổng hợp → quay lại trang chủ
        if (backFromMore()) return
        // Đang ở lưới / ô tìm kiếm → quay về menu (điện thoại cầm dọc: mở ngăn kéo menu); đang ở menu → nhấn 2 lần để thoát
        if (Screen.narrow(this) && !TouchUi.active && (grid.hasFocus() || search.hasFocus())) {
            openDrawer()
            return
        }
        // (điện thoại cầm dọc + cảm ứng: menu ẩn trong ngăn kéo — bỏ bước này, Back hai lần là thoát)
        if (!Screen.narrow(this) && (grid.hasFocus() || search.hasFocus())) {
            val i = menuAdapter.items.indexOf(menuAdapter.active)
            menu.scrollToPosition(max(0, i))
            menu.post { menu.findViewHolderForAdapterPosition(i)?.itemView?.requestFocus() }
            return
        }
        val now = System.currentTimeMillis()
        if (now - lastBack < 2000) {
            PlayerActivity.closeMini() // phim ở cửa sổ nhỏ trong app: thoát app là tắt luôn
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
