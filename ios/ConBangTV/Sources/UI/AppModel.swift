import SwiftUI
import UIKit
import WebKit

/// Một ô phim trên lưới (kèm cách mở: resume = phát tiếp luôn; removable = giữ để xoá khỏi lịch sử / bỏ theo dõi).
struct GridFilm: Identifiable, Hashable {
    let film: Film
    var resume = false
    var removable = false
    var follow = false
    var id: String { film.url + (resume ? "#r" : "") + (follow ? "#f" : "") }
}

struct GridSection: Identifiable, Hashable {
    let id: String
    let title: String?
    var items: [GridFilm]
    /// Dòng ghi chú chữ mờ dưới nhóm (vd tìm mọi trang: trang đang tìm / không có phim / không trả lời).
    var note: String? = nil
}

/// Trạng thái màn chính (bản Android: MainActivity): menu bên trái, lưới phim, tìm kiếm, lịch sử, theo dõi.
@MainActor
final class AppModel: ObservableObject {
    static let shared = AppModel()

    enum Kind: String { case home, search, history, follows, category, filter, site, web, clear, settings }

    struct Entry: Identifiable, Hashable {
        let title: String
        let kind: Kind
        var url: String?
        var id: String { "\(kind.rawValue)|\(url ?? "")" }

        var icon: String {
            switch kind {
            case .home: return "house"
            case .search: return "magnifyingglass"
            case .history: return "clock.arrow.circlepath"
            case .follows: return "bell"
            case .category: return "film"
            case .filter: return "line.3.horizontal.decrease.circle"
            case .site: return "arrow.left.arrow.right"
            case .web: return "safari"
            case .clear: return "trash"
            case .settings: return "gearshape"
            }
        }
    }

    enum Status: Equatable {
        case none
        case loading(String)
        case message(String, retry: Bool)
    }

    enum Sheet: Identifiable {
        case sites, settings
        case filter(title: String, items: [Category])
        var id: String {
            switch self {
            case .sites: return "sites"
            case .settings: return "settings"
            case .filter(let t, _): return "filter|\(t)"
            }
        }
    }

    @Published private(set) var menu: [Entry] = []
    @Published private(set) var active: Entry?
    @Published private(set) var title = "Trang chủ"
    @Published private(set) var sections: [GridSection] = []
    @Published private(set) var status: Status = .loading("Đang tải…")
    @Published private(set) var siteLabel = ""
    @Published private(set) var toast: String?
    @Published var detailFilm: Film?
    @Published var sheet: Sheet?
    @Published var confirmClear = false
    @Published var removeTarget: GridFilm?
    @Published var update: UpdateCheck.Release?
    @Published var searchText = ""
    @Published var searchFocus = false
    /// Đổi mỗi lần đóng trình phát (trang phim cập nhật "Xem tiếp" / tập đã xem).
    @Published private(set) var historyVersion = 0

    private var categories: [Category] = []
    private var token = 0
    private(set) var nextUrl: String?
    private var loadingMore = false
    private var loadedBase: String?
    private var homeToken = -1
    private var filters: (base: String, genres: [Category], countries: [Category])?
    private var retry: (() -> Void)?
    private var toastTask: Task<Void, Never>?
    private var updateBusy = false
    private var started = false

    static let headCount = 6

    // MARK: - Người xem (hồ sơ)

    @Published private(set) var profiles = Prefs.profiles
    @Published private(set) var profileId = Prefs.profileId
    /// Màn "Ai đang xem?" đang hiện.
    @Published var askingProfile = false
    /// Đổi mỗi lần đổi người xem (Cài đặt dựng lại theo người mới).
    @Published private(set) var profileVersion = 0
    /// Màn "Ai đang xem?" đang chặn cửa (mở app / quay lại sau lâu): người đang dùng có mã PIN thì không được đóng.
    private var pickerGate = false
    /// Vừa xoá người đang xem mà ai còn lại cũng có mã PIN: hỏi "Ai đang xem?" khi đóng Cài đặt.
    private var pendingGate = false
    /// Cài đặt proxy vừa đổi: đóng Cài đặt thì tải lại trang phim qua đường mới.
    var proxyChanged = false
    private var leftAt: Int64 = 0

    /// Mở app phải chọn người xem: nhà có từ 2 người (và bật "hỏi khi mở app"), hoặc người đang dùng có mã PIN.
    var mustAskProfile: Bool { (Prefs.profiles.count >= 2 && Prefs.askProfile) || Prefs.profile.locked }
    var pickerClosable: Bool { !(pickerGate && Prefs.profile.locked) }

    func startIfNeeded() {
        guard !started, !askingProfile else { return }
        if mustAskProfile {
            showPicker(gate: true) // chọn người xem trước khi tải (mỗi người một trang phim, lịch sử riêng)
            return
        }
        begin()
    }

    private func begin() {
        started = true
        showSiteLabel()
        rebuildMenu()
        select(menu[0])
        WidgetFeed.update()
        runPendingPlay()
    }

    // MARK: - Mở từ widget «Xem tiếp»

    /// Phim chạm trên widget lúc app chưa sẵn sàng (đang hỏi "Ai đang xem?" / chưa tải): phát sau khi chọn người xem.
    private var pendingPlay: (profile: String, film: String, ep: String)?

    func openURL(_ url: URL) {
        guard let p = WidgetFeed.parse(url) else { return }
        pendingPlay = p
        if started && !askingProfile { runPendingPlay() }
    }

    /// Chỉ phát khi người xem đang dùng đúng là người của widget (đổi sang người khác thì thôi — lịch sử riêng).
    private func runPendingPlay() {
        guard let p = pendingPlay else { return }
        pendingPlay = nil
        guard p.profile == Prefs.profileId else {
            showToast("Phim trên widget là của người xem khác.")
            return
        }
        detailFilm = nil
        sheet = nil
        Task {
            try? await Task.sleep(nanoseconds: 350_000_000) // chờ bảng đang mở (nếu có) đóng hẳn
            Router.play(filmUrl: p.film, epUrl: p.ep, continueMode: true)
        }
    }

    func refreshProfiles() {
        profiles = Prefs.profiles
        profileId = Prefs.profileId
    }

    func showPicker(gate: Bool) {
        refreshProfiles()
        pickerGate = gate
        askingProfile = true
    }

    /// Đóng màn "Ai đang xem?": xem tiếp bằng người đang dùng.
    func closePicker() {
        guard pickerClosable else { return }
        askingProfile = false
        if !started { begin() }
    }

    /// Chọn xong người xem (đã nhập đúng mã PIN nếu có).
    func pickProfile(_ p: Profile) {
        let changed = p.id != Prefs.profileId
        if changed {
            PlayerModel.closeForProfileSwitch()
            WatchLimit.flush() // giờ đã xem là của người cũ
            WatchLimit.reset()
            Prefs.switchProfile(p.id)
            profileVersion += 1
        }
        refreshProfiles()
        askingProfile = false
        if !started {
            begin()
        } else {
            if changed {
                reloadSite()
                showToast("Người xem: \(p.name)")
                WidgetFeed.update()
            }
            runPendingPlay()
        }
    }

    func deleteProfile(_ p: Profile) {
        let wasCurrent = p.id == Prefs.profileId
        if wasCurrent { PlayerModel.closeForProfileSwitch() }
        Prefs.deleteProfile(p.id)
        if wasCurrent {
            // Sang người không khoá; ai cũng có mã PIN thì hỏi "Ai đang xem?" (không tự vào hồ sơ có khoá)
            if let open = Prefs.profiles.first(where: { !$0.locked }) { Prefs.switchProfile(open.id) } else { pendingGate = true }
            profileVersion += 1
        }
        refreshProfiles()
        WidgetFeed.update()
        showToast("Đã xoá \(p.name).")
        if wasCurrent && started { reloadSite() }
    }

    /// App sang nền: nhớ lúc rời app (quay lại sau ≥ 30 phút thì hỏi lại người xem).
    func appLeft() {
        leftAt = nowMs()
        WidgetFeed.update()
    }

    func appReturned() {
        defer { leftAt = 0 }
        guard leftAt > 0, nowMs() - leftAt >= 30 * 60_000, started, !askingProfile, sheet == nil, mustAskProfile else { return }
        // Phim đang mở / đang ở cửa sổ hình trong hình: vẫn đang xem — thôi
        if Router.topController is PlayerController || PlayerModel.detachedModel != nil { return }
        showPicker(gate: true)
    }

    // MARK: - Menu

    private func followsTitle() -> String {
        let n = Prefs.follows.filter { $0.newCount > 0 }.count
        return n > 0 ? "Theo dõi (\(n) mới)" : "Phim theo dõi"
    }

    func rebuildMenu() {
        var items = [
            Entry(title: "Trang chủ", kind: .home),
            Entry(title: "Tìm kiếm", kind: .search),
            Entry(title: "Lịch sử xem", kind: .history),
            Entry(title: followsTitle(), kind: .follows),
        ]
        items += categories.map { Entry(title: $0.title, kind: .category, url: $0.url) }
        // Trang kiểu PhimMoi / trang API (mephim): đủ thể loại / quốc gia của bộ lọc (menu trang chỉ có vài mục)
        if NextSite.isKnown(Prefs.baseUrl) || ApiSite.isKnown(Prefs.baseUrl) {
            items.append(Entry(title: "Thể loại…", kind: .filter, url: "genre"))
            items.append(Entry(title: "Quốc gia…", kind: .filter, url: "country"))
        }
        items += [
            Entry(title: "Đổi trang phim", kind: .site),
            Entry(title: "Duyệt web", kind: .web),
            Entry(title: "Xoá dữ liệu web", kind: .clear),
            Entry(title: "Cài đặt", kind: .settings),
        ]
        let a = active
        menu = items
        // Giữ mục đang chọn (tên mục "Phim theo dõi" có thể đổi theo số tập mới)
        active = items.first { $0.kind == a?.kind && $0.url == a?.url } ?? a
    }

    func select(_ e: Entry) {
        switch e.kind {
        case .web:
            Router.browse(Prefs.baseUrl)
            return
        case .settings:
            sheet = .settings
            return
        case .site:
            sheet = .sites
            return
        case .clear:
            confirmClear = true
            return
        case .filter:
            pickFilter(e)
            return
        default:
            break
        }
        detailFilm = nil
        active = e
        title = e.title
        switch e.kind {
        case .home: showHome()
        case .search:
            token += 1
            sections = []
            nextUrl = nil
            status = .message("Gõ tên phim cần tìm rồi nhấn Tìm — app tìm cùng lúc trên mọi trang trong danh sách.", retry: false)
            searchFocus = true
        case .history: showHistory()
        case .follows:
            showFollows()
            checkFollows()
        case .category: if let u = e.url { showList(u) }
        default: break
        }
    }

    func showSiteLabel() {
        var s = Site.hostOf(Prefs.baseUrl).map(Site.displayName) ?? Prefs.baseUrl
        if Prefs.privateMode { s += "  ·  xem riêng" }
        siteLabel = s
    }

    func reloadSite() {
        AppProxy.apply() // chế độ "chỉ trang phim": cập nhật danh sách tên miền đi qua proxy
        showSiteLabel()
        categories = []
        rebuildMenu()
        select(menu[0])
    }

    /// Cài đặt / danh sách trang vừa đóng: link trang đổi thì tải lại từ đầu.
    func settingsClosed() {
        refreshProfiles()
        if pendingGate {
            pendingGate = false
            showPicker(gate: true)
        }
        showSiteLabel()
        WidgetFeed.update() // xoá lịch sử / đổi tên người xem trong Cài đặt
        SyncService.shared.syncSoon()
        let proxy = proxyChanged
        proxyChanged = false
        if loadedBase != nil && (loadedBase != Prefs.baseUrl || proxy) { reloadSite() } else { rebuildMenu() }
    }

    func selectSite(_ url: String) {
        sheet = nil
        if url != Prefs.baseUrl {
            Prefs.baseUrl = url
            reloadSite()
        }
    }

    // MARK: - Nội dung

    func showHome() {
        token += 1
        let t = token
        let base = Prefs.baseUrl
        nextUrl = nil
        sections = []
        status = .loading("Đang tải trang chủ \(Site.hostOf(base) ?? base)…")
        retry = { [weak self] in self?.showHome() }
        Task {
            do {
                let home = try await Site.home(base)
                guard t == token else { return }
                if let moved = home.movedTo {
                    Prefs.replaceSite(base, with: moved)
                    showSiteLabel()
                    showToast("Trang đã chuyển sang \(Site.hostOf(moved) ?? moved) — đã tự cập nhật link.")
                }
                loadedBase = Prefs.baseUrl
                if !home.categories.isEmpty && home.categories != categories {
                    categories = home.categories
                }
                rebuildMenu()
                let body = home.sections.enumerated().map { i, s in
                    GridSection(id: "home\(i)", title: s.title, items: s.films.map { GridFilm(film: $0) })
                }
                if body.isEmpty {
                    status = .message("Không đọc được danh sách phim. Có thể trang đã đổi giao diện — thử mục «Duyệt web».", retry: true)
                } else {
                    status = .none
                    homeToken = t
                    sections = headSections() + body
                    checkFollows()
                }
            } catch {
                guard t == token else { return }
                loadedBase = base
                status = .message("Không tải được trang chủ.\n\(friendly(error))", retry: true)
            }
        }
    }

    func showList(_ url: String, loader: (() async throws -> FilmPage)? = nil) {
        token += 1
        let t = token
        nextUrl = nil
        sections = []
        status = .loading("Đang tải…")
        retry = { [weak self] in self?.showList(url, loader: loader) }
        Task {
            do {
                let page = try await (loader ?? { try await Site.list(url) })()
                guard t == token else { return }
                nextUrl = page.nextUrl
                if !page.sections.isEmpty {
                    sections = page.sections.enumerated().map { i, s in
                        GridSection(id: "s\(i)", title: s.title, items: s.films.map { GridFilm(film: $0) })
                    }
                } else {
                    sections = [GridSection(id: "list", title: nil, items: page.films.map { GridFilm(film: $0) })]
                }
                status = sections.allSatisfy { $0.items.isEmpty } ? .message("Không có phim nào.", retry: false) : .none
            } catch {
                guard t == token else { return }
                status = .message("Không tải được danh sách.\n\(friendly(error))", retry: true)
            }
        }
    }

    /// Cuộn tới cuối lưới: tải trang sau.
    func loadMore() {
        guard let url = nextUrl, !loadingMore else { return }
        loadingMore = true
        let t = token
        Task {
            defer { loadingMore = false }
            do {
                let page = try await Site.list(url)
                guard t == token else { return }
                nextUrl = page.nextUrl
                if var last = sections.popLast() {
                    let have = Set(last.items.map { $0.film.url })
                    last.items += page.films.filter { !have.contains($0.url) }.map { GridFilm(film: $0) }
                    sections.append(last)
                }
            } catch {
                guard t == token else { return }
                nextUrl = nil
                showToast("Không tải thêm được: \(friendly(error))")
            }
        }
    }

    // MARK: - Bàn phím rời

    /// Ô phim đang chọn bằng phím mũi tên (id của GridFilm); nil = chưa dùng bàn phím.
    @Published var keySel: String?
    /// Số cột lưới phim hiện tại (lưới tự báo theo bề rộng).
    var gridCols = 4

    /// Màn chính đang ở trên cùng (không có bảng / trình phát / trang phim che): phím tắt của lưới mới có tác dụng.
    var keysOnGrid: Bool { sheet == nil && !askingProfile && detailFilm == nil && Router.topController?.presentingViewController == nil }
    /// Không có bảng / trình phát che (phím tắt ⌘ dùng được cả trong trang phim).
    var keysOnMain: Bool { sheet == nil && !askingProfile && Router.topController?.presentingViewController == nil }

    func keyMove(_ d: KeyNav.Dir) {
        guard keysOnGrid else { return }
        keySel = KeyNav.move(sections.map { $0.items.map { $0.id } }, cols: gridCols, from: keySel, d)
    }

    private var keyItem: GridFilm? {
        guard let id = keySel else { return nil }
        return sections.lazy.flatMap { $0.items }.first { $0.id == id }
    }

    /// Enter / Space: mở phim đang chọn (hàng Xem tiếp: phát tiếp luôn).
    func keyOpen() {
        guard keysOnGrid else { return }
        guard let item = keyItem else {
            keyMove(.right) // chưa chọn ô nào: chọn ô đầu
            return
        }
        open(item)
    }

    /// Delete: xoá phim đang chọn khỏi lịch sử / bỏ theo dõi (có hỏi lại).
    func keyRemove() {
        guard keysOnGrid, let item = keyItem, item.removable else { return }
        removeTarget = item
    }

    /// ⌥▲ / ⌥▼: mục menu trước / sau (bỏ qua các mục mở bảng: Đổi trang phim, Duyệt web, Cài đặt…).
    func keyMenu(_ delta: Int) {
        guard keysOnMain else { return }
        let lists: Set<Kind> = [.home, .search, .history, .follows, .category]
        let items = menu.filter { lists.contains($0.kind) }
        guard !items.isEmpty else { return }
        let i = items.firstIndex { $0.id == active?.id } ?? 0
        select(items[(i + delta + items.count) % items.count])
    }

    /// Phím tắt ⌘: chọn mục menu theo loại (Trang chủ / Tìm kiếm / Lịch sử / Theo dõi / Đổi trang / Cài đặt…).
    func keySelect(_ kind: Kind) {
        guard keysOnMain, let e = menu.first(where: { $0.kind == kind }) else { return }
        select(e)
    }

    /// ⌘R: tải lại danh sách đang xem.
    func keyReload() {
        guard keysOnMain else { return }
        detailFilm = nil
        if let a = active { select(a) }
    }

    // MARK: - Tìm mọi trang

    /// Một lần tìm mọi trang (bản Android: MainActivity.SearchRun). queue: trang chưa hỏi (trang đang dùng trước);
    /// running: trang đang hỏi (tối đa searchAtOnce).
    private struct SearchRun {
        let token: Int
        let query: String
        var queue: [String]
        var running: [String] = []
        var empty: [String] = []
        var failed: [String] = []
        var found: [GridSection] = []
    }

    private var searchRun: SearchRun?
    /// Truy vấn đang chạy (tìm lượt mới thì huỷ hết — URLSession huỷ luôn kết nối).
    private var searchTasks: [Task<Void, Never>] = []

    /// Hỏi cùng lúc tối đa 3 trang; mỗi trang chờ tối đa 12 giây kể từ lúc bắt đầu hỏi trang đó.
    private static let searchAtOnce = 3
    private static let searchTimeout = 12.0

    /// Tìm trên mọi trang trong danh sách, cùng lúc tối đa 3 trang: kết quả trang nào về trước hiện trước (không chờ trang
    /// chậm / không vào được), dòng cuối ghi trang đang tìm / không có phim / không trả lời.
    func doSearch() {
        let q = searchText.trimmed
        guard !q.isEmpty else { return }
        title = "Tìm: \(q) (mọi trang)"
        token += 1
        nextUrl = nil
        sections = []
        searchTasks.forEach { $0.cancel() }
        searchTasks = []
        let sites = ([Prefs.baseUrl] + Prefs.sites).distinct { $0 }
        searchRun = SearchRun(token: token, query: q, queue: sites)
        status = .loading("Đang tìm «\(q)» trên \(sites.count) trang…")
        retry = { [weak self] in self?.doSearch() }
        startSearches()
    }

    /// Hỏi tiếp các trang đang chờ cho tới khi đủ searchAtOnce trang đang hỏi.
    private func startSearches() {
        guard var run = searchRun, run.token == token else { return }
        while run.running.count < Self.searchAtOnce && !run.queue.isEmpty {
            let base = run.queue.removeFirst()
            let host = Site.hostOf(base) ?? base
            let q = run.query
            let t = run.token
            let timeout = Self.searchTimeout
            run.running.append(host)
            searchTasks.append(Task { [weak self] in
                let r: Result<FilmPage, Error>
                do {
                    r = .success(try await withTimeout(timeout) { try await Site.search(q, base: base) })
                } catch {
                    r = .failure(error)
                }
                if Task.isCancelled { return }
                self?.siteSearched(t, host, r)
            })
        }
        searchRun = run
    }

    private func siteSearched(_ t: Int, _ host: String, _ r: Result<FilmPage, Error>) {
        // Lần tìm khác: bỏ
        guard t == token, var run = searchRun, run.token == t, let i = run.running.firstIndex(of: host) else { return }
        run.running.remove(at: i)
        switch r {
        case .success(let page):
            let films = (page.films + page.sections.flatMap { $0.films }).distinct { $0.url }
            if films.isEmpty {
                run.empty.append(host)
            } else {
                run.found.append(GridSection(id: "site|\(host)", title: "\(host) — \(films.count) phim", items: films.map { GridFilm(film: $0) }))
            }
        case .failure(let e):
            Log.d("Search", "\(host): \(friendly(e))")
            run.failed.append(host)
        }
        searchRun = run
        startSearches()
        run = searchRun ?? run
        var notes: [String] = []
        let waiting = run.running + run.queue.map { Site.hostOf($0) ?? $0 }
        if !waiting.isEmpty { notes.append("Đang tìm: " + waiting.joined(separator: ", ") + "…") }
        if !run.empty.isEmpty { notes.append("Không có phim: " + run.empty.joined(separator: ", ")) }
        if !run.failed.isEmpty { notes.append("Không trả lời: " + run.failed.joined(separator: ", ")) }
        guard !run.found.isEmpty else {
            sections = []
            let text = notes.joined(separator: "\n")
            if !waiting.isEmpty {
                status = .loading("Đang tìm «\(run.query)»…\n\(text)")
            } else if run.empty.isEmpty {
                status = .message("Không tìm được — các trang không trả lời.\n\(text)", retry: true)
            } else {
                status = .message("Không tìm thấy phim nào tên «\(run.query)».\n\(text)", retry: false)
            }
            return
        }
        status = .none
        sections = run.found + (notes.isEmpty ? [] : [GridSection(id: "search-note", title: nil, items: [], note: notes.joined(separator: "   ·   "))])
    }

    func retryNow() { retry?() }

    // MARK: - Đầu trang chủ: "Có tập mới" rồi "Xem tiếp"

    private func headSections() -> [GridSection] {
        var out: [GridSection] = []
        let fresh = Prefs.follows.filter { $0.newCount > 0 }
        if !fresh.isEmpty {
            out.append(GridSection(id: "head-new", title: "Có tập mới",
                                   items: fresh.map { GridFilm(film: followFilm($0), removable: true, follow: true) }))
        }
        let items = Prefs.history.prefix(Self.headCount)
        if !items.isEmpty {
            out.append(GridSection(id: "head-continue", title: "Xem tiếp",
                                   items: items.map { GridFilm(film: historyFilm($0), resume: true, removable: true) }))
        }
        return out
    }

    /// Cập nhật hàng đầu trang chủ (xem xong quay ra / vừa xoá / có tập mới).
    func refreshHead() {
        guard homeToken == token, active?.kind == .home else { return }
        let body = sections.filter { !$0.id.hasPrefix("head-") }
        let head = headSections()
        if head + body != sections { sections = head + body }
    }

    /// vd "Tập 166 mới", "+3 · Tập 168", hoặc tập mới nhất khi đã xem hết.
    func followFilm(_ f: Follow) -> Film {
        let badge: String
        switch f.newCount {
        case 0: badge = f.latest
        case 1: badge = "\(f.latest) mới"
        default: badge = "+\(f.newCount) · \(f.latest)"
        }
        return Film(url: f.filmUrl, title: f.title, poster: f.poster, badge: badge.nonEmpty)
    }

    /// Phim trong lịch sử; phim của trang khác ghi thêm tên trang (vd hai bản "Tiên Nghịch" ở hai trang).
    func historyFilm(_ h: HistoryItem) -> Film {
        guard let host = Site.hostOf(h.filmUrl), host != Site.hostOf(Prefs.baseUrl) else {
            return Film(url: h.filmUrl, title: h.title, poster: h.poster, badge: h.epTitle)
        }
        // Hai trang cùng tên ngắn (hoathinh3d.ee / hoathinh3d.pw) → ghi đủ tên miền cho khỏi lẫn
        let short = Site.shortName(host)
        let clash = Prefs.sites.contains { s in
            guard let o = Site.hostOf(s) else { return false }
            return o != host && Site.shortName(o) == short
        }
        return Film(url: h.filmUrl, title: "\(h.title) · \(clash ? host : short)", poster: h.poster, badge: h.epTitle)
    }

    func showHistory() {
        token += 1
        nextUrl = nil
        let items = Prefs.history
        if items.isEmpty {
            sections = []
            status = .message("Chưa có phim nào trong lịch sử xem.", retry: false)
            return
        }
        status = .none
        sections = [GridSection(id: "history", title: nil, items: items.map { GridFilm(film: historyFilm($0), removable: true) })]
    }

    func showFollows() {
        token += 1
        nextUrl = nil
        let items = Prefs.follows.sorted { ($0.newCount > 0 ? 0 : 1) < ($1.newCount > 0 ? 0 : 1) }
        if items.isEmpty {
            sections = []
            status = .message("Chưa theo dõi phim nào.\nMở một phim rồi chọn «＋ Theo dõi» — trang chủ sẽ báo khi phim có tập mới.", retry: false)
            return
        }
        status = .none
        sections = [GridSection(id: "follows", title: nil, items: items.map { GridFilm(film: followFilm($0), removable: true, follow: true) })]
    }

    // MARK: - Mở phim

    func open(_ item: GridFilm) {
        if item.resume, let h = Prefs.historyFor(item.film.url) {
            Router.play(filmUrl: h.filmUrl, epUrl: h.epUrl, continueMode: true)
        } else {
            detailFilm = item.film
        }
    }

    /// Đóng trình phát: cập nhật hàng Xem tiếp, trang phim, số "mới" của phim theo dõi.
    /// Đồng bộ trong nhà vừa nhận dữ liệu mới: vẽ lại hàng Xem tiếp / lịch sử / theo dõi, widget.
    func syncApplied() {
        historyVersion += 1
        rebuildMenu()
        if active?.kind == .history { showHistory() } else if active?.kind == .follows { showFollows() } else { refreshHead() }
        WidgetFeed.update()
    }

    func playerClosed() {
        historyVersion += 1
        WidgetFeed.update()
        SyncService.shared.syncSoon()
        rebuildMenu()
        if active?.kind == .history { showHistory() } else if active?.kind == .follows { showFollows() } else { refreshHead() }
    }

    func remove(_ item: GridFilm) {
        if item.follow {
            Prefs.unfollow(item.film.url)
            rebuildMenu()
        } else {
            Prefs.removeHistory(item.film.url)
            WidgetFeed.update()
        }
        SyncService.shared.syncSoon()
        switch active?.kind {
        case .history: showHistory()
        case .follows: showFollows()
        default: refreshHead()
        }
    }

    // MARK: - Phim theo dõi

    func checkFollows(force: Bool = false) {
        Task {
            guard let fresh = await Follows.check(force: force) else { return }
            rebuildMenu()
            if active?.kind == .follows { showFollows() } else { refreshHead() }
            if !fresh.isEmpty {
                showToast("Có tập mới: " + fresh.prefix(3).map { "\($0.title) (\($0.latest))" }.joined(separator: ", ") +
                          (fresh.count > 3 ? "…" : ""))
            }
        }
    }

    // MARK: - Thể loại / quốc gia (trang kiểu PhimMoi)

    private func pickFilter(_ e: Entry) {
        let base = Prefs.baseUrl
        if let f = filters, f.base == base {
            presentFilter(e, f.genres, f.countries)
            return
        }
        showToast("Đang tải danh sách…")
        Task {
            do {
                let f = ApiSite.isKnown(base) ? ApiSite.filters(base) : try await NextSite.filters(base)
                filters = (base, f.genres, f.countries)
                presentFilter(e, f.genres, f.countries)
            } catch {
                showToast("Không tải được danh sách: \(friendly(error))")
            }
        }
    }

    private func presentFilter(_ e: Entry, _ genres: [Category], _ countries: [Category]) {
        let list = e.url == "genre" ? genres : countries
        if list.isEmpty {
            showToast("Trang không có danh sách này.")
            return
        }
        sheet = .filter(title: e.title.trimmingChars("…"), items: list)
    }

    func pickCategory(_ c: Category) {
        sheet = nil
        detailFilm = nil
        active = menu.first { $0.kind == .filter }
        title = c.title
        showList(c.url)
    }

    // MARK: - Dữ liệu web

    func clearWebData(images: Bool = false, done: (() -> Void)? = nil) {
        BrowsingData.clear(images: images) { [weak self] in
            self?.showToast(images ? "Đã xoá ảnh, cookie và dữ liệu duyệt web." : "Đã xoá cookie và dữ liệu duyệt web.")
            done?()
        }
    }

    // MARK: - Cập nhật

    /// Tự hỏi GitHub khi mở app (tối đa 2 giờ một lần); manual: nút trong Cài đặt.
    func checkUpdate(manual: Bool, status: ((String) -> Void)? = nil) {
        if !manual && (!Prefs.autoUpdate || nowMs() - Prefs.lastUpdateCheck < UpdateCheck.checkEvery) { return }
        if updateBusy { return }
        updateBusy = true
        status?("Đang kiểm tra…")
        Task {
            defer { updateBusy = false }
            do {
                Prefs.lastUpdateCheck = nowMs()
                guard let rel = try await UpdateCheck.latest(), UpdateCheck.isNewer(rel.version) else {
                    status?("Đang dùng bản mới nhất (\(UpdateCheck.currentVersion)).")
                    return
                }
                if !manual && Prefs.isUpdateSnoozed(rel.version) { return }
                status?("Có bản mới \(rel.version).")
                update = rel
            } catch {
                status?("Không kiểm tra được: \(friendly(error))")
            }
        }
    }

    func installUpdate(_ rel: UpdateCheck.Release) {
        update = nil
        guard let url = UpdateCheck.trollStoreURL(rel.ipaUrl) else { return }
        UIApplication.shared.open(url, options: [:]) { [weak self] ok in
            if !ok {
                // Không có TrollStore: mở link tải trong Safari
                if let u = URL(string: rel.ipaUrl) { UIApplication.shared.open(u) }
                Task { @MainActor in self?.showToast("Không mở được TrollStore — đã mở link tải .ipa trong Safari.") }
            }
        }
    }

    func snoozeUpdate(_ rel: UpdateCheck.Release) {
        update = nil
        Prefs.snoozeUpdate(rel.version, until: nowMs() + UpdateCheck.snooze)
    }

    // MARK: - Thông báo

    func showToast(_ text: String) {
        toast = text
        toastTask?.cancel()
        toastTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 3_500_000_000)
            if !Task.isCancelled { self?.toast = nil }
        }
    }

    /// Cho cổng gỡ lỗi.
    func debugState() -> [String: Any] {
        [
            "profile": Prefs.profile.name, "profiles": profiles.map { $0.name }, "askingProfile": askingProfile,
            "site": Prefs.baseUrl, "title": title, "active": active?.title ?? NSNull(),
            "status": "\(status)", "detail": detailFilm?.url ?? NSNull(),
            "sections": sections.map { ["title": $0.title ?? "", "count": $0.items.count, "first": $0.items.prefix(3).map { $0.film.title }] },
            "menu": menu.map { $0.title }, "toast": toast ?? NSNull(), "next": nextUrl ?? NSNull(),
        ]
    }
}

/// Xoá dữ liệu duyệt web: cookie, dữ liệu trang (localStorage, IndexedDB…), bộ nhớ đệm (bản Android: BrowsingData.kt).
/// Lịch sử xem, chỗ đang xem dở, điểm server, cài đặt giữ nguyên.
enum BrowsingData {
    @MainActor
    static func clear(images: Bool, done: @escaping @MainActor () -> Void) {
        let store = WKWebsiteDataStore.default()
        // Đang bật proxy (iPadOS 15–16): WebView dùng kho riêng — xoá cả ở đó
        WebSetup.proxiedStore?.removeData(ofTypes: WKWebsiteDataStore.allWebsiteDataTypes(), modifiedSince: .distantPast) {}
        store.removeData(ofTypes: WKWebsiteDataStore.allWebsiteDataTypes(), modifiedSince: .distantPast) {
            Task { @MainActor in
                HTTPCookieStorage.shared.removeCookies(since: .distantPast)
                URLCache.shared.removeAllCachedResponses()
                if images { ImageCache.shared.clear() }
                Prefs.webDirty = false
                done()
            }
        }
    }
}
