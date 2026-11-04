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

    func startIfNeeded() {
        guard !started else { return }
        started = true
        showSiteLabel()
        rebuildMenu()
        select(menu[0])
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
        // Trang kiểu PhimMoi: đủ thể loại / quốc gia của bộ lọc (menu trang chỉ có vài mục)
        if NextSite.isKnown(Prefs.baseUrl) {
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
        var s = Site.hostOf(Prefs.baseUrl) ?? Prefs.baseUrl
        if Prefs.privateMode { s += "  ·  xem riêng" }
        siteLabel = s
    }

    func reloadSite() {
        showSiteLabel()
        categories = []
        rebuildMenu()
        select(menu[0])
    }

    /// Cài đặt / danh sách trang vừa đóng: link trang đổi thì tải lại từ đầu.
    func settingsClosed() {
        showSiteLabel()
        if loadedBase != nil && loadedBase != Prefs.baseUrl { reloadSite() } else { rebuildMenu() }
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
                    Prefs.baseUrl = moved
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

    func doSearch() {
        let q = searchText.trimmed
        guard !q.isEmpty else { return }
        title = "Tìm: \(q) (mọi trang)"
        showList(q) { try await Site.searchAll(q) }
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
        let short = host.before(".")
        let clash = Prefs.sites.contains { s in
            guard let o = Site.hostOf(s) else { return false }
            return o != host && o.before(".") == short
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
    func playerClosed() {
        historyVersion += 1
        rebuildMenu()
        if active?.kind == .history { showHistory() } else if active?.kind == .follows { showFollows() } else { refreshHead() }
    }

    func remove(_ item: GridFilm) {
        if item.follow {
            Prefs.unfollow(item.film.url)
            rebuildMenu()
        } else {
            Prefs.removeHistory(item.film.url)
        }
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
                let f = try await NextSite.filters(base)
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
