import SwiftUI

/// Trang phim (bản Android: DetailActivity): thông tin, Xem tiếp / Xem tập mới nhất, theo dõi, bản phim, danh sách tập.
@MainActor
final class DetailModel: ObservableObject {
    let film: Film
    @Published private(set) var detail: FilmDetail?
    @Published private(set) var error: String?
    @Published var group: EpisodeGroup?
    @Published var ascending = true
    @Published private(set) var watched: Set<String> = []
    @Published private(set) var resumeEp: Episode?
    @Published private(set) var following = false
    @Published var jumpText = ""
    @Published var scrollTarget: String?
    @Published var flash: String?
    @Published var toast: String?
    /// Tập đang chọn bằng phím mũi tên.
    @Published var keyEp: String?
    /// Các hàng «Phim liên quan» (Related) — tải sau khi trang phim đã hiện.
    @Published private(set) var related: [Related.Row] = []
    private var relatedLoaded = false
    var epCols = 6

    init(film: Film) {
        self.film = film
        if let d = Site.cachedDetail(film.url) { apply(d) }
    }

    /// Hiện dữ liệu phim (giữ bản đang chọn nếu còn; chưa chọn thì bản đang xem dở).
    private func apply(_ d: FilmDetail) {
        detail = d
        let keep = group.flatMap { g in d.groups.first { $0.name == g.name } }
        group = keep ?? Prefs.historyFor(d.url).flatMap { d.groupOf($0.epUrl) } ?? d.groups.first
        refresh()
    }

    func load() async {
        error = nil
        do {
            let d = try await Site.detail(film.url)
            bind(d)
        } catch {
            if detail == nil { self.error = "Không tải được thông tin phim.\n\(friendly(error))" }
        }
    }

    private func bind(_ d: FilmDetail) {
        apply(d)
        // Đang theo dõi: mở phim là đã thấy các tập mới
        if let latest = d.latestEpisode { Prefs.updateFollow(d.url, count: d.episodeCount, latest: latest.title, seen: true) }
        HomeFeed.markSeen(d) // trang chủ tổng hợp: hết báo «tập mới» tới khi có tập sau
        refresh()
        AppModel.shared.rebuildMenu()
        if let r = resumeEp { scrollTarget = r.url }
        if !relatedLoaded {
            relatedLoaded = true
            let title = d.title, url = d.url
            Task { [weak self] in
                let rows = await Related.load(title: title, url: url)
                self?.related = rows
            }
        }
    }

    /// Phim lẻ (một bản, một tập): không cần ô tập — nút Xem là đủ, hàng phim liên quan lên ngay dưới.
    var isMovie: Bool { detail.map { $0.groups.count == 1 && $0.episodeCount == 1 } ?? false }

    /// Quay lại từ trình phát: cập nhật "Xem tiếp" và đánh dấu tập đã xem.
    func refresh() {
        watched = Prefs.watchedEpisodes()
        following = Prefs.followOf(film.url) != nil || (detail.map { Prefs.followOf($0.url) != nil } ?? false)
        guard let d = detail else { return }
        // Tập xem dở có thể thuộc bản khác với bản đang hiển thị
        resumeEp = Prefs.historyFor(d.url).flatMap { h in
            d.groups.lazy.compactMap { g in g.episodes.first { $0.url == h.epUrl } }.first
        }
    }

    var episodes: [Episode] {
        let eps = group?.episodes ?? []
        return ascending ? eps : eps.reversed()
    }

    func play(_ ep: Episode) {
        guard let d = detail else { return }
        Router.play(filmUrl: d.url, epUrl: ep.url)
    }

    func toggleFollow() {
        guard let d = detail else { return }
        if Prefs.followOf(d.url) != nil {
            Prefs.unfollow(d.url)
            showToast("Đã bỏ theo dõi.")
        } else {
            Prefs.follow(d)
            showToast("Đã theo dõi — trang chủ sẽ báo khi phim có tập mới.")
        }
        refresh()
        AppModel.shared.rebuildMenu()
        SyncService.shared.syncSoon()
    }

    // MARK: Bàn phím rời

    /// Phím có tác dụng: trang phim đang hiện, không có bảng / trình phát che.
    var keysActive: Bool { AppModel.shared.keysOnMain && AppModel.shared.detailFilm?.url == film.url }

    func keyMove(_ d: KeyNav.Dir) {
        guard keysActive else { return }
        let ids = episodes.map { $0.url }
        // Chưa chọn tập nào: bắt đầu từ tập đang xem dở (nếu có trong bản này)
        let from = keyEp ?? resumeEp.flatMap { r in ids.contains(r.url) ? r.url : nil }
        let next = (keyEp == nil && from != nil) ? from : KeyNav.move([ids], cols: epCols, from: from, d)
        keyEp = next
        if let n = next { scrollTarget = n }
    }

    /// Enter / Space: phát tập đang chọn; chưa chọn thì nút chính (Xem tiếp / tập mới nhất).
    func keyPlay() {
        guard keysActive else { return }
        if let id = keyEp, let ep = episodes.first(where: { $0.url == id }) {
            play(ep)
        } else if let ep = resumeEp ?? group?.episodes.last {
            play(ep)
        }
    }

    func keyBack() {
        guard keysActive else { return }
        AppModel.shared.detailFilm = nil
    }

    func keyFollow() {
        guard keysActive else { return }
        toggleFollow()
    }

    /// Bản phim (Vietsub / Thuyết Minh…) trước / sau.
    func keyGroup(_ delta: Int) {
        guard keysActive, let d = detail, d.groups.count > 1 else { return }
        let i = d.groups.firstIndex { $0 == group } ?? 0
        group = d.groups[(i + delta + d.groups.count) % d.groups.count]
        keyEp = nil
    }

    private static let number = Rx(#"\d+"#)

    /// Đến tập số n (số đầu tiên trong tên tập: "Tập 120", "Tập 01", "120 - Vietsub").
    func jump() {
        guard let n = Int(jumpText.trimmed) else { return }
        jumpText = ""
        guard let ep = episodes.first(where: { Self.number.group(0, in: $0.title).flatMap { Int($0) } == n }) else {
            showToast("Không có tập \(n) trong danh sách này.")
            return
        }
        flash = ep.url
        scrollTarget = ep.url
    }

    private func showToast(_ t: String) {
        toast = t
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            if self?.toast == t { self?.toast = nil }
        }
    }
}

struct DetailView: View {
    @EnvironmentObject var app: AppModel
    @StateObject private var m: DetailModel
    @FocusState private var jumpFocused: Bool
    @State private var expanded = false
    /// Màn hẹp (iPhone dọc, iPad chia đôi): ảnh bìa nhỏ, lề hẹp.
    @Environment(\.horizontalSizeClass) private var hClass
    private var narrow: Bool { hClass == .compact }
    /// Các nguồn có phim này (mở từ trang chủ tổng hợp) — có từ 2 nguồn thì có nút đổi nguồn.
    private let links: [FeedItem]
    @State private var pickSource = false

    init(film: Film, links: [FeedItem] = []) {
        _m = StateObject(wrappedValue: DetailModel(film: film))
        self.links = links
    }

    private func sourceName(_ url: String) -> String { Site.hostOf(url).map { Site.tag($0, Prefs.sites) } ?? url }

    private var title: String { m.detail?.title ?? m.film.title }
    private var poster: String? { m.detail?.poster ?? m.film.poster }

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 22) {
                    header
                    if let d = m.detail {
                        actions(d)
                        if d.groups.isEmpty {
                            Text("Phim này chưa có tập nào.").foregroundColor(Theme.dim)
                        } else if !m.isMovie {
                            episodesSection(d)
                        }
                        relatedSections
                    } else if let e = m.error {
                        VStack(alignment: .leading, spacing: 14) {
                            Text(e).foregroundColor(Theme.text)
                            Button("Thử lại") { Task { await m.load() } }
                                .buttonStyle(.borderedProminent)
                                .tint(Theme.brand)
                        }
                    } else {
                        ProgressView().tint(Theme.text).frame(maxWidth: .infinity).padding(40)
                    }
                }
                .padding(narrow ? 16 : 24)
            }
            .onChange(of: m.scrollTarget) { t in
                guard let t = t else { return }
                withAnimation { proxy.scrollTo(t, anchor: t == "related" ? .top : .center) }
                m.scrollTarget = nil
            }
        }
        .background(
            ZStack(alignment: .top) {
                Theme.bg
                PosterImage(url: poster, maxPixel: 200)
                    .frame(height: 420)
                    .frame(maxWidth: .infinity)
                    .blur(radius: 28)
                    .opacity(0.35)
                    .clipped()
                LinearGradient(colors: [.clear, Theme.bg], startPoint: .top, endPoint: .bottom).frame(height: 420)
            }
            .ignoresSafeArea()
        )
        .overlay(alignment: .bottom) {
            if let t = m.toast { ToastView(text: t).allowsHitTesting(false) }
        }
        .background { if !jumpFocused { DetailShortcuts(m: m) } }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarHidden(false)
        .task { await m.load() }
        .onChange(of: app.historyVersion) { _ in m.refresh() }
    }

    private var header: some View {
        HStack(alignment: .top, spacing: narrow ? 14 : 22) {
            Color.clear
                .frame(width: narrow ? 116 : 190, height: narrow ? 174 : 285)
                .overlay(PosterImage(url: poster))
                .clipShape(RoundedRectangle(cornerRadius: 12))
            VStack(alignment: .leading, spacing: 10) {
                Text(title).appFont(narrow ? 22 : 30, .bold).foregroundColor(Theme.text)
                if let o = m.detail?.otherTitle { Text(o).appFont(17).foregroundColor(Theme.dim) }
                if let i = m.detail?.info { Text(i).appFont(15).foregroundColor(Theme.dim) }
                if let desc = m.detail?.description {
                    Text(desc)
                        .appFont(16)
                        .foregroundColor(Theme.text.opacity(0.9))
                        .lineLimit(expanded ? nil : 5)
                        .onTapGesture { expanded.toggle() }
                }
            }
        }
    }

    @ViewBuilder
    private func actions(_ d: FilmDetail) -> some View {
        let eps = m.group?.episodes ?? []
        if let latest = eps.last {
            let resume = m.resumeEp
            let cols = [GridItem(.adaptive(minimum: narrow ? 150 : 200), spacing: 12, alignment: .leading)]
            LazyVGrid(columns: cols, alignment: .leading, spacing: 12) {
                ChipButton(text: resume.map { "Xem tiếp \($0.title)" } ?? (eps.count == 1 ? "Xem phim" : "Xem \(latest.title) (mới nhất)"),
                           icon: "play.fill", active: true) { m.play(resume ?? latest) }
                if let r = resume, r != latest {
                    ChipButton(text: "Mới nhất: \(latest.title)", icon: "forward.end") { m.play(latest) }
                }
                if eps.count > 1, let first = eps.first {
                    ChipButton(text: "Xem từ \(first.title)", icon: "backward.end") { m.play(first) }
                }
                // Phim có ở nhiều nguồn (trang chủ tổng hợp): đổi nguồn
                if links.count >= 2 {
                    ChipButton(text: "Nguồn: \(sourceName(m.film.url)) ▾", icon: "arrow.left.arrow.right") { pickSource = true }
                        .confirmationDialog("Mở phim ở nguồn khác", isPresented: $pickSource, titleVisibility: .visible) {
                            ForEach(links, id: \.url) { l in
                                Button(sourceName(l.url) + (l.badge.map { " — \($0)" } ?? "") + (l.url == m.film.url ? "  ✓" : "")) {
                                    guard l.url != m.film.url else { return }
                                    app.openDetail(Film(url: l.url, title: l.title, poster: l.poster ?? m.film.poster, badge: nil), links: links)
                                }
                            }
                        }
                }
                // Phim bộ: nhảy xuống các hàng phim liên quan ở cuối danh sách tập
                if !m.isMovie && !m.related.isEmpty {
                    ChipButton(text: "Phim liên quan ▼", icon: "square.stack") { m.scrollTarget = "related" }
                }
                ChipButton(text: m.following ? "✓ Đang theo dõi" : "＋ Theo dõi", icon: "bell") { m.toggleFollow() }
                ChipButton(text: "Mở trang web", icon: "safari") { Router.browse(d.url) }
            }
        }
    }

    /// Các hàng «Phim liên quan»: dải vuốt ngang như trang chủ (phim ở trang khác ghi tên trang ở góc).
    @ViewBuilder
    private var relatedSections: some View {
        if !m.related.isEmpty {
            let host = Site.hostOf(m.detail?.url ?? m.film.url)
            let sites = Prefs.sites
            let cardW: CGFloat = narrow ? 118 : 150
            VStack(alignment: .leading, spacing: 18) {
                ForEach(m.related) { row in
                    VStack(alignment: .leading, spacing: 10) {
                        Text(row.title).appFont(20, .semibold).foregroundColor(Theme.text).lineLimit(1)
                        ScrollView(.horizontal, showsIndicators: false) {
                            LazyHStack(alignment: .top, spacing: 12) {
                                ForEach(row.films) { f in
                                    let other = Site.hostOf(f.url).flatMap { $0 == host ? nil : Site.tag($0, sites) }
                                    FilmCard(item: GridFilm(film: f, chip: other))
                                        .frame(width: cardW)
                                }
                            }
                            .padding(.vertical, 4)
                        }
                    }
                }
            }
            .id("related")
        }
    }

    private var jumpBox: some View {
        HStack(spacing: 6) {
            TextField("Đến tập…", text: $m.jumpText)
                .keyboardType(.numberPad)
                .focused($jumpFocused)
                .frame(width: 96)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(RoundedRectangle(cornerRadius: 8).fill(Theme.surface2))
                .onSubmit { m.jump() }
            Button("Đến") {
                jumpFocused = false
                m.jump()
            }
            .buttonStyle(.bordered)
            .tint(Theme.brand)
        }
    }

    @ViewBuilder
    private func episodesSection(_ d: FilmDetail) -> some View {
        if d.groups.count > 1 {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 10) {
                    ForEach(d.groups) { g in
                        ChipButton(text: "\(g.name) (\(g.episodes.count))", active: g == m.group) { m.group = g }
                            .fixedSize()
                    }
                }
            }
        }
        let count = m.group?.episodes.count ?? 0
        HStack(spacing: 12) {
            Text("Danh sách tập (\(count))").appFont(20, .semibold).foregroundColor(Theme.text)
            Spacer()
            if count > 1 {
                Button { m.ascending.toggle() } label: {
                    Label(m.ascending ? "Cũ → mới" : "Mới → cũ", systemImage: "arrow.up.arrow.down")
                }
                .tint(Theme.dim)
            }
            if count > 20 && !narrow { jumpBox }
        }
        // Màn hẹp (iPhone dọc): ô "Đến tập" xuống dòng riêng
        if count > 20 && narrow { jumpBox }
        let epMin = TextScale.shared.layout(96)
        LazyVGrid(columns: [GridItem(.adaptive(minimum: epMin), spacing: 10)], spacing: 10) {
            ForEach(m.episodes) { ep in
                let seen = m.watched.contains(ep.url)
                let isResume = m.resumeEp?.url == ep.url
                Button { m.play(ep) } label: {
                    Text(ep.title)
                        .appFont(15, isResume ? .bold : .medium)
                        .lineLimit(1)
                        .minimumScaleFactor(0.7)
                        .foregroundColor(Theme.text)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 12)
                        .padding(.horizontal, 6)
                        .background(RoundedRectangle(cornerRadius: 8).fill(seen ? Theme.watched : Theme.surface2))
                        .overlay(RoundedRectangle(cornerRadius: 8)
                            .stroke(m.flash == ep.url || m.keyEp == ep.url ? Theme.focus : (isResume ? Theme.brand : Color.clear),
                                    lineWidth: m.keyEp == ep.url ? 3 : 2))
                }
                .buttonStyle(PressScale())
                .contentShape(.hoverEffect, RoundedRectangle(cornerRadius: 8))
                .hoverEffect(.highlight)
                .id(ep.url)
            }
        }
        .background(GeometryReader { g in
            Color.clear
                .onAppear { m.epCols = KeyNav.columns(width: Double(g.size.width), minimum: Double(epMin), spacing: 10) }
                .onChange(of: g.size.width) { w in m.epCols = KeyNav.columns(width: Double(w), minimum: Double(epMin), spacing: 10) }
        })
    }
}

/// Phím của trang phim: mũi tên chọn tập, Enter / Space phát, Esc / ⌘[ quay lại, ⌘D theo dõi, [ ] đổi bản phim.
private struct DetailShortcuts: View {
    @ObservedObject var m: DetailModel

    var body: some View {
        ZStack {
            DetailKey(title: "Tập trước", key: .leftArrow) { m.keyMove(.left) }
            DetailKey(title: "Tập sau", key: .rightArrow) { m.keyMove(.right) }
            DetailKey(title: "Lên", key: .upArrow) { m.keyMove(.up) }
            DetailKey(title: "Xuống", key: .downArrow) { m.keyMove(.down) }
            DetailKey(title: "Xem", key: .return) { m.keyPlay() }
            DetailKey(title: "Xem", key: .space) { m.keyPlay() }
            DetailKey(title: "Quay lại", key: .escape) { m.keyBack() }
            DetailKey(title: "Quay lại", key: "[", mods: .command) { m.keyBack() }
            DetailKey(title: "Theo dõi / bỏ theo dõi", key: "d", mods: .command) { m.keyFollow() }
            DetailKey(title: "Bản phim trước", key: "[") { m.keyGroup(-1) }
            DetailKey(title: "Bản phim sau", key: "]") { m.keyGroup(1) }
        }
        .allowsHitTesting(false)
    }
}

private struct DetailKey: View {
    let title: String
    let key: KeyEquivalent
    var mods: EventModifiers = []
    let action: () -> Void

    var body: some View {
        Button(title, action: action)
            .keyboardShortcut(key, modifiers: mods)
            .frame(width: 0, height: 0)
            .opacity(0)
            .accessibilityHidden(true)
    }
}
