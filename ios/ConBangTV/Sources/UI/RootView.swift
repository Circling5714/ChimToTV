import SwiftUI
import UIKit

/// Mở trình phát / trình duyệt toàn màn hình (UIKit) từ bất cứ đâu.
@MainActor
enum Router {
    static var topController: UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        let windows = scenes.flatMap { $0.windows }
        var vc = (windows.first { $0.isKeyWindow } ?? windows.first)?.rootViewController
        while let p = vc?.presentedViewController, !p.isBeingDismissed { vc = p }
        return vc
    }

    static func play(filmUrl: String, epUrl: String, continueMode: Bool = false) {
        guard let top = topController, !(top is PlayerController) else { return }
        // Phim đang ở cửa sổ hình trong hình: mở phim khác thì đóng phim đó (lưu chỗ xem)
        PlayerModel.detachedModel?.closeDetached()
        present(PlayerModel(filmUrl: filmUrl, epUrl: epUrl, continueMode: continueMode), on: top)
    }

    /// Mở màn trình phát cho model (mới, hoặc phim đang hình trong hình vừa được phóng to — trình phát vẫn chạy).
    static func present(_ model: PlayerModel, on top: UIViewController? = nil) {
        guard let top = top ?? topController, !(top is PlayerController) else { return }
        let c = PlayerController(model: model)
        c.onDismissed = { AppModel.shared.playerClosed() }
        top.present(c, animated: true)
    }

    static func browse(_ url: String) {
        guard let top = topController else { return }
        top.present(BrowserController(url: url), animated: true)
    }

    /// Đóng lớp trên cùng (trình phát / trình duyệt / bảng) — cho cổng gỡ lỗi.
    static func back() {
        if let top = topController, top.presentingViewController != nil {
            if let p = top as? PlayerController { p.model.close() } else { top.dismiss(animated: true) }
            return
        }
        let app = AppModel.shared
        if app.sheet != nil { app.sheet = nil } else if app.detailFilm != nil { app.detailFilm = nil }
    }
}

struct RootView: View {
    @EnvironmentObject var app: AppModel
    @State private var showMenu = false

    var body: some View {
        GeometryReader { geo in
            let wide = geo.size.width >= 900
            ZStack(alignment: .leading) {
                HStack(spacing: 0) {
                    if wide {
                        Sidebar { }
                            .frame(width: 270)
                    }
                    MainContent(showMenuButton: !wide) { showMenu = true }
                }
                if !wide && showMenu {
                    Color.black.opacity(0.45)
                        .ignoresSafeArea()
                        .onTapGesture { showMenu = false }
                    Sidebar { showMenu = false }
                        .frame(width: min(300, geo.size.width - 56))
                        .transition(.move(edge: .leading))
                }
            }
            .animation(.easeOut(duration: 0.2), value: showMenu)
        }
        .background(Theme.bg.ignoresSafeArea())
        .background(MainShortcuts())
        .appDynamicType()
        .overlay(alignment: .bottom) {
            if let t = app.toast { ToastView(text: t).allowsHitTesting(false) }
        }
        .sheet(item: $app.sheet, onDismiss: { app.settingsClosed() }) { s in
            Group {
                switch s {
                case .sites: SitesView()
                case .settings: SettingsView().id(app.profileVersion) // đổi người xem: cài đặt của người mới
                case .filter(let title, let items): FilterView(title: title, items: items)
                }
            }
            .environmentObject(app)
            .preferredColorScheme(.dark)
            .appDynamicType()
        }
        .fullScreenCover(isPresented: $app.askingProfile) {
            ProfilePickerView().environmentObject(app).appDynamicType()
        }
        .background(
            Color.clear.alert("Xoá dữ liệu duyệt web?", isPresented: $app.confirmClear) {
                Button("Xoá", role: .destructive) { app.clearWebData() }
                Button("Huỷ", role: .cancel) {}
            } message: {
                Text("Xoá cookie, dữ liệu các trang đã lưu và bộ nhớ đệm của trình duyệt trong app.\nLịch sử xem, chỗ đang xem dở và cài đặt giữ nguyên.")
            }
        )
        .background(
            Color.clear.alert(app.removeTarget?.follow == true ? "Bỏ theo dõi?" : "Xoá khỏi lịch sử xem?",
                              isPresented: Binding(get: { app.removeTarget != nil }, set: { if !$0 { app.removeTarget = nil } }),
                              presenting: app.removeTarget) { item in
                Button(item.follow ? "Bỏ theo dõi" : "Xoá", role: .destructive) { app.remove(item) }
                Button("Huỷ", role: .cancel) {}
            } message: { item in
                Text(item.follow ? "«\(item.film.title)» — app sẽ không báo tập mới của phim này nữa."
                     : "«\(item.film.title)» và chỗ đang xem dở các tập của phim này sẽ bị xoá.")
            }
        )
        .background(
            Color.clear.alert("Có bản mới \(app.update?.version ?? "")",
                              isPresented: Binding(get: { app.update != nil }, set: { if !$0 { app.update = nil } }),
                              presenting: app.update) { rel in
                Button("Cài đặt ngay") { app.installUpdate(rel) }
                Button("Để sau", role: .cancel) { app.snoozeUpdate(rel) }
            } message: { rel in
                Text("Đang dùng \(UpdateCheck.currentVersion). Cài bằng TrollStore.\n\n" + UpdateCheck.summary(rel.notes))
            }
        )
        .onAppear { app.startIfNeeded() }
    }
}

// MARK: - Menu bên trái

struct Sidebar: View {
    @EnvironmentObject var app: AppModel
    var onPick: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Image("Logo")
                    .resizable()
                    .frame(width: 46, height: 46)
                    .clipShape(RoundedRectangle(cornerRadius: 11))
                VStack(alignment: .leading, spacing: 2) {
                    Text("Côn Bằng TV").appFont(21, .bold).foregroundColor(Theme.text)
                    Text(app.siteLabel).appFont(13).foregroundColor(Theme.dim).lineLimit(1)
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, 18)
            .padding(.bottom, 12)
            // Người xem đang dùng — chạm để đổi / thêm người (hiện cả khi mới có một người, cho dễ thấy tính năng)
            if let p = app.profiles.first(where: { $0.id == app.profileId }) {
                Button {
                    app.showPicker(gate: false)
                    onPick()
                } label: {
                    HStack(spacing: 10) {
                        ProfileAvatar(profile: p, size: 28)
                        Text(p.name).appFont(15, .semibold).foregroundColor(Theme.text).lineLimit(1)
                        Image(systemName: "chevron.down").appFont(12, .semibold).foregroundColor(Theme.dim)
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(RoundedRectangle(cornerRadius: 10).fill(Theme.surface2))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Người xem: \(p.name)")
                .padding(.horizontal, 14)
                .padding(.bottom, 8)
            }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(app.menu) { e in
                        let on = app.active?.id == e.id
                        Button {
                            app.select(e)
                            onPick()
                        } label: {
                            HStack(spacing: 12) {
                                Image(systemName: e.icon).appFont(16, .medium).frame(width: 22)
                                Text(e.title).appFont(16, on ? .semibold : .regular).lineLimit(1)
                                Spacer(minLength: 0)
                            }
                            .foregroundColor(on ? Theme.onBrand : Theme.text)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 11)
                            .background(RoundedRectangle(cornerRadius: 10).fill(on ? Theme.brand : Color.clear))
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .hoverEffect(.highlight)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 20)
            }
            #if DEVTOOLS
            Text("Bản thử · gỡ lỗi: \(DebugServer.shared.addressText)")
                .appFont(11)
                .foregroundColor(Theme.dim)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            #endif
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .background(Theme.surface.ignoresSafeArea())
    }
}

// MARK: - Nội dung chính (lưới phim) + trang phim

struct MainContent: View {
    @EnvironmentObject var app: AppModel
    let showMenuButton: Bool
    let openMenu: () -> Void

    var body: some View {
        NavigationView {
            HomeContent(showMenuButton: showMenuButton, openMenu: openMenu)
                .navigationTitle(app.title)
                .navigationBarHidden(true)
                .background(
                    NavigationLink(isActive: Binding(get: { app.detailFilm != nil }, set: { if !$0 { app.detailFilm = nil } })) {
                        if let f = app.detailFilm {
                            DetailView(film: f).id(f.url)
                        }
                    } label: { EmptyView() }
                    .hidden()
                )
        }
        .navigationViewStyle(.stack)
    }
}

struct HomeContent: View {
    @EnvironmentObject var app: AppModel
    let showMenuButton: Bool
    let openMenu: () -> Void
    @FocusState private var searchFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 14) {
                if showMenuButton {
                    Button(action: openMenu) {
                        Image(systemName: "line.3.horizontal").appFont(22, .semibold).foregroundColor(Theme.text)
                            .frame(width: 44, height: 44)
                    }
                    .accessibilityLabel("Menu")
                }
                Text(app.title).appFont(26, .bold).foregroundColor(Theme.text).lineLimit(1).minimumScaleFactor(0.7)
                Spacer()
                if showMenuButton {
                    Text(app.siteLabel).appFont(14).foregroundColor(Theme.dim).lineLimit(1)
                }
            }
            .padding(.leading, showMenuButton ? 8 : 20)
            .padding(.trailing, 20)
            .padding(.top, 14)
            .padding(.bottom, 8)

            if app.active?.kind == .search {
                HStack(spacing: 10) {
                    HStack {
                        Image(systemName: "magnifyingglass").foregroundColor(Theme.dim)
                        TextField("Tên phim…", text: $app.searchText)
                            .focused($searchFocused)
                            .submitLabel(.search)
                            .onSubmit { app.doSearch() }
                            .disableAutocorrection(true)
                            .foregroundColor(Theme.text)
                    }
                    .padding(.horizontal, 14)
                    .padding(.vertical, 12)
                    .background(RoundedRectangle(cornerRadius: 12).fill(Theme.surface2))
                    Button("Tìm") { app.doSearch() }
                        .buttonStyle(.borderedProminent)
                        .tint(Theme.brand)
                        .controlSize(.large)
                }
                .padding(.horizontal, 20)
                .padding(.bottom, 8)
                .onAppear { if app.searchFocus { searchFocused = true; app.searchFocus = false } }
                .onChange(of: app.searchFocus) { f in if f { searchFocused = true; app.searchFocus = false } }
            }

            ZStack {
                if !app.sections.isEmpty { FilmGridView() }
                StatusView()
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(Theme.bg.ignoresSafeArea())
        .background { if !searchFocused { GridShortcuts() } }
    }
}

struct StatusView: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        switch app.status {
        case .none:
            EmptyView()
        case .loading(let text):
            VStack(spacing: 16) {
                ProgressView().scaleEffect(1.3).tint(Theme.text)
                Text(text).foregroundColor(Theme.dim)
            }
        case .message(let text, let retry):
            VStack(spacing: 16) {
                Text(text).appFont(17).foregroundColor(Theme.text).multilineTextAlignment(.center)
                if retry {
                    Button("Thử lại") { app.retryNow() }
                        .buttonStyle(.borderedProminent)
                        .tint(Theme.brand)
                }
            }
            .padding(40)
        }
    }
}

struct FilmGridView: View {
    @EnvironmentObject var app: AppModel
    @ObservedObject private var scale = TextScale.shared

    var body: some View {
        GeometryReader { geo in
            let minW = scale.layout(geo.size.width < 500 ? 105 : 150)
            let spacing: CGFloat = geo.size.width < 500 ? 12 : 16
            let pad: CGFloat = geo.size.width < 500 ? 14 : 20
            let cols = [GridItem(.adaptive(minimum: minW, maximum: scale.layout(230)), spacing: spacing, alignment: .top)]
            ScrollViewReader { proxy in
                grid(cols, pad: pad)
                    .onAppear { app.gridCols = KeyNav.columns(width: Double(geo.size.width - pad * 2), minimum: Double(minW), spacing: Double(spacing)) }
                    .onChange(of: geo.size.width) { w in
                        app.gridCols = KeyNav.columns(width: Double(w - pad * 2), minimum: Double(minW), spacing: Double(spacing))
                    }
                    .onChange(of: scale.value) { _ in
                        app.gridCols = KeyNav.columns(width: Double(geo.size.width - pad * 2), minimum: Double(minW), spacing: Double(spacing))
                    }
                    .onChange(of: app.keySel) { id in
                        guard let id = id else { return }
                        withAnimation(.easeOut(duration: 0.15)) { proxy.scrollTo(id) }
                    }
            }
        }
    }

    private func grid(_ cols: [GridItem], pad: CGFloat) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 14) {
                ForEach(app.sections) { s in
                    if let t = s.title {
                        Text(t).appFont(21, .semibold).foregroundColor(Theme.text)
                            .padding(.horizontal, pad)
                            .padding(.top, 8)
                    }
                    LazyVGrid(columns: cols, alignment: .leading, spacing: 20) {
                        ForEach(s.items) { item in FilmCard(item: item).id(item.id) }
                    }
                    .padding(.horizontal, pad)
                    if let n = s.note {
                        Text(n).appFont(15).foregroundColor(Theme.dim)
                            .padding(.horizontal, pad)
                            .padding(.top, 6)
                    }
                }
                if app.nextUrl != nil {
                    ProgressView()
                        .tint(Theme.text)
                        .frame(maxWidth: .infinity)
                        .padding(24)
                        .onAppear { app.loadMore() }
                }
            }
            .padding(.vertical, 10)
        }
    }
}

struct FilmCard: View {
    @EnvironmentObject var app: AppModel
    let item: GridFilm

    var body: some View {
        Button { app.open(item) } label: {
            VStack(alignment: .leading, spacing: 7) {
                Color.clear
                    .aspectRatio(2.0 / 3.0, contentMode: .fit)
                    .overlay(PosterImage(url: item.film.poster))
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                    .overlay {
                        // Ô đang chọn bằng bàn phím
                        if app.keySel == item.id {
                            RoundedRectangle(cornerRadius: 10).stroke(Theme.focus, lineWidth: 4)
                        }
                    }
                    .overlay(alignment: .topLeading) {
                        if let b = item.film.badge {
                            Text(b)
                                .appFont(12, .bold)
                                .lineLimit(1)
                                .foregroundColor(Theme.onBrand)
                                .padding(.horizontal, 7)
                                .padding(.vertical, 3)
                                .background(Capsule().fill(item.follow ? Theme.focus : Theme.brand))
                                .padding(7)
                        }
                    }
                    .overlay(alignment: .bottomTrailing) {
                        if item.resume {
                            Image(systemName: "play.circle.fill")
                                .appFont(34)
                                .foregroundColor(.white)
                                .shadow(radius: 4)
                                .padding(8)
                        }
                    }
                Text(item.film.title)
                    .appFont(15, .medium)
                    .foregroundColor(Theme.text)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .buttonStyle(PressScale())
        .contentShape(.hoverEffect, RoundedRectangle(cornerRadius: 10))
        .hoverEffect(.lift)
        .accessibilityLabel(item.film.title)
        .contextMenu {
            if item.removable {
                Button(role: .destructive) { app.removeTarget = item } label: {
                    Label(item.follow ? "Bỏ theo dõi" : "Xoá khỏi lịch sử xem", systemImage: "trash")
                }
            }
            if item.resume {
                Button { app.detailFilm = item.film } label: { Label("Trang phim", systemImage: "info.circle") }
            }
        }
    }
}

// MARK: - Bàn phím rời / chuột

/// Nút ẩn mang phím tắt (SwiftUI chỉ nhận phím tắt gắn với nút đang có trên màn hình). Giữ phím ⌘ thì iPad liệt kê
/// các phím tắt có ⌘ / ⌥.
private struct KeyButton: View {
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

/// Phím tắt ⌘ dùng ở màn chính và trang phim.
struct MainShortcuts: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        ZStack {
            KeyButton(title: "Tìm kiếm", key: "f", mods: .command) { app.keySelect(.search) }
            KeyButton(title: "Trang chủ", key: "1", mods: .command) { app.keySelect(.home) }
            KeyButton(title: "Lịch sử xem", key: "2", mods: .command) { app.keySelect(.history) }
            KeyButton(title: "Phim theo dõi", key: "3", mods: .command) { app.keySelect(.follows) }
            KeyButton(title: "Tải lại", key: "r", mods: .command) { app.keyReload() }
            KeyButton(title: "Đổi trang phim", key: "o", mods: .command) { app.keySelect(.site) }
            KeyButton(title: "Đổi người xem", key: "e", mods: .command) {
                if app.keysOnMain { app.showPicker(gate: false) }
            }
            KeyButton(title: "Cài đặt", key: ",", mods: .command) { app.keySelect(.settings) }
            KeyButton(title: "Mục menu trước", key: .upArrow, mods: .option) { app.keyMenu(-1) }
            KeyButton(title: "Mục menu sau", key: .downArrow, mods: .option) { app.keyMenu(1) }
        }
        .allowsHitTesting(false)
    }
}

/// Phím không có ⌘ của lưới phim: mũi tên chọn phim, Enter / Space mở, Delete xoá khỏi lịch sử. Tắt lúc đang gõ ô tìm
/// kiếm (mũi tên / dấu cách để cho ô nhập).
struct GridShortcuts: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        ZStack {
            KeyButton(title: "Sang trái", key: .leftArrow) { app.keyMove(.left) }
            KeyButton(title: "Sang phải", key: .rightArrow) { app.keyMove(.right) }
            KeyButton(title: "Lên", key: .upArrow) { app.keyMove(.up) }
            KeyButton(title: "Xuống", key: .downArrow) { app.keyMove(.down) }
            KeyButton(title: "Mở phim", key: .return) { app.keyOpen() }
            KeyButton(title: "Mở phim", key: .space) { app.keyOpen() }
            KeyButton(title: "Xoá khỏi lịch sử", key: .delete) { app.keyRemove() }
        }
        .allowsHitTesting(false)
    }
}
