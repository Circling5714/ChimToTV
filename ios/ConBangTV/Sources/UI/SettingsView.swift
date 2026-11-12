import SwiftUI

/// Cài đặt (bản Android: SettingsActivity): trang phim, phát video, dữ liệu, cập nhật, liên hệ.
struct SettingsView: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss

    @State private var url = Site.hostOf(Prefs.baseUrl) ?? Prefs.baseUrl
    @State private var siteStatus = ""
    @State private var sites = Prefs.sites
    @State private var base = Prefs.baseUrl
    @State private var seekStep = Prefs.seekStep
    @State private var autoNext = Prefs.autoNext
    @State private var privateMode = Prefs.privateMode
    @State private var autoClear = Prefs.autoClearWeb
    @State private var autoUpdate = Prefs.autoUpdate
    @State private var updateStatus = ""
    @State private var serverLabel = Prefs.serverLabel(Site.hostOf(Prefs.baseUrl))
    @State private var confirmHistory = false
    @State private var dataStatus = ""
    @State private var askProfile = Prefs.askProfile
    @State private var pipOnHome = Prefs.pipOnHome
    @State private var kkBackup = Prefs.kkBackup
    @State private var ncBackup = Prefs.ncBackup
    @State private var profileSheet: ProfileSheet?
    @State private var unlockFor: Profile?
    @State private var limitsShown = false
    /// Đã có giới hạn giờ xem: thêm người xem cần mã PIN phụ huynh (khỏi lách giới hạn bằng hồ sơ mới).
    @State private var parentForAdd = false
    @State private var homeMix = Prefs.homeMix
    @State private var kids = Prefs.kids
    /// Tắt hồ sơ trẻ em: hỏi mã PIN phụ huynh (nếu đã đặt).
    @State private var parentForKids = false

    /// Đang mở màn «Cài lên tivi» (bản thử: cổng gỡ lỗi /go?to=invite mở thẳng màn này để chụp).
    @State private var inviteShown = false

    @State private var resting = Feed.restingHosts()
    @ObservedObject private var textScale = TextScale.shared

    /// Thêm / sửa người xem.
    private enum ProfileSheet: Identifiable {
        case add
        case edit(Profile)
        var id: String {
            switch self {
            case .add: return "add"
            case .edit(let p): return "edit|\(p.id)"
            }
        }
    }

    /// Bản thử: /go?to=invite của cổng gỡ lỗi mở Cài đặt rồi vào thẳng màn «Cài lên tivi» (chụp màn hình trên máy ảo). Bản
    /// chính không có cổng gỡ lỗi — hàm này không làm gì.
    private func debugOpenInvite() {
        #if DEVTOOLS
        guard DebugServer.openInvite else { return }
        DebugServer.openInvite = false
        // Chờ bảng Cài đặt hiện xong rồi mới đẩy màn con (đẩy ngay lúc onAppear thì NavigationView bỏ qua)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.7) { inviteShown = true }
        #endif
    }

    /// Nhà nhiều người xem: các mục của riêng người đang dùng ghi rõ tên.
    private var who: String { app.profiles.count >= 2 ? " của \(Prefs.profile.name)" : "" }

    private var version: String {
        let v = UpdateCheck.currentVersion
        let b = (Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String) ?? ""
        return "\(v) (\(b))"
    }

    private var displaySection: some View {
        Section {
            Picker("Cỡ chữ", selection: Binding(get: { textScale.value }, set: { textScale.set($0) })) {
                ForEach(TextScale.choices, id: \.value) { c in Text(c.name).tag(c.value) }
            }
            .pickerStyle(.segmented)
            Text("Xem trước: Đấu Phá Thương Khung — Tập 120")
                .appFont(16)
                .foregroundColor(Theme.text)
        } header: { Text("Cỡ chữ") } footer: {
            Text("Áp dụng cho cả app (dùng chung mọi người xem): lưới phim, trang phim, trình phát, Cài đặt.")
        }
    }

    /// Trang chủ: tổng hợp mọi nguồn / theo trang đang dùng; hồ sơ trẻ em; thử lại nguồn đang tạm nghỉ.
    private var homeSection: some View {
        Section {
            Toggle("Trang chủ\(who) tổng hợp mọi nguồn", isOn: Binding(get: { homeMix || kids }, set: { v in
                guard !kids else {
                    app.showToast("Hồ sơ trẻ em luôn dùng trang chủ tổng hợp (chỉ phim hoạt hình hợp trẻ em).")
                    return
                }
                homeMix = v
                Prefs.homeMix = v
            }))
            Toggle("Hồ sơ trẻ em\(who)", isOn: Binding(get: { kids }, set: { v in
                if v {
                    kids = true
                    Prefs.kids = true
                } else if Prefs.hasParentPin {
                    parentForKids = true // tắt: phải nhập mã PIN phụ huynh
                } else {
                    kids = false
                    Prefs.kids = false
                }
            }))
            // Trang con thường (không dùng NavigationLink isActive — iOS 15 hay lỗi); hồ sơ trẻ em hỏi mã PIN ngay trong trang đó
            NavigationLink {
                HomeRowsView(needPin: kids && Prefs.hasParentPin)
            } label: {
                Text("Các hàng trên trang chủ\(who)").foregroundColor(Theme.text)
            }
            NavigationLink {
                HomeSourcesView(needPin: kids && Prefs.hasParentPin)
            } label: {
                Text("Nguồn cho trang chủ\(who)").foregroundColor(Theme.text)
            }
            if !resting.isEmpty {
                let until = Date(timeIntervalSince1970: Double(resting.values.max() ?? 0) / 1000)
                Button("Thử lại ngay: \(resting.keys.sorted().map { Site.tag($0, Prefs.sites) }.joined(separator: ", ")) (đang tạm nghỉ tới \(until.formatted(date: .omitted, time: .shortened)))") {
                    Feed.clearRest()
                    resting = [:]
                    app.showToast("Trang chủ sẽ hỏi lại các trang này ở lần tải tới.")
                }
            }
        } header: { Text("Trang chủ") } footer: {
            Text("Tổng hợp: trang chủ lấy phim mới từ mọi trang trong danh sách (KKPhim, NguonC, VSMOV, CLB Phim Xưa và trang đang dùng), gộp phim trùng giữa các trang («3 nguồn» — trang phim có nút đổi nguồn), xếp hàng theo lịch sử xem của từng người: phim đang xem dở có tập mới ở bất kỳ trang nào, phim giống phim vừa xem, thể loại hay xem. Trang nào lỗi / quá 6 giây thì dùng bản lưu; lỗi 3 lần liên tiếp thì tạm nghỉ 30 phút. Tắt: trang chủ của trang đang dùng như trước.\n\nHồ sơ trẻ em: trang chủ chỉ có phim hoạt hình hợp trẻ em — thiếu nhi, gia đình, lồng tiếng; bỏ phim kinh dị / hình sự / chiến tranh / 18+. Chỉ lọc trang chủ (tìm kiếm, mục của trang vẫn như cũ). Tắt cần mã PIN phụ huynh nếu đã đặt (Giới hạn giờ xem).\n\nCác hàng: bật / tắt, đổi thứ tự, thêm hàng của mình — trên trang chủ giữ một phim của hàng (hoặc tên hàng) để ẩn / chuyển nhanh hàng đó. Nguồn: chọn trang chủ lấy phim từ những nguồn nào.")
        }
    }

    private var header: some View {
        ZStack {
            Text("Cài đặt").font(.headline).foregroundColor(Theme.text)
            HStack {
                Spacer()
                Button("Xong") { dismiss() }
                    .font(.body.weight(.semibold))
                    .keyboardShortcut(.escape, modifiers: [])
            }
        }
        .padding(.horizontal, 20)
        .frame(maxWidth: .infinity)
        .frame(height: 52)
        .background(.bar)
    }

    var body: some View {
        NavigationView {
            Form {
                profileSection
                homeSection
                Section {
                    NavigationLink {
                        SyncView()
                    } label: {
                        Label("Đồng bộ trong nhà (TV ↔ iPad / iPhone)", systemImage: "arrow.triangle.2.circlepath")
                    }
                } footer: {
                    Text("Xem dở trên TV thì mở iPad xem tiếp đúng chỗ, và ngược lại — ghép máy bằng mã 4 số.")
                }
                displaySection
                siteSection
                Section {
                    ForEach(sites, id: \.self) { s in
                        Button {
                            base = s
                            Prefs.baseUrl = s
                            url = Site.hostOf(s) ?? s
                            serverLabel = Prefs.serverLabel(Site.hostOf(s))
                        } label: {
                            HStack {
                                Text(Site.hostOf(s) ?? s).foregroundColor(Theme.text)
                                Spacer()
                                if s == base { Image(systemName: "checkmark").foregroundColor(Theme.brand) }
                            }
                        }
                    }
                    // Trang mẫu đã xoá khỏi danh sách (vd phimmois.fun): thêm lại một chạm
                    ForEach(Prefs.missingPresets.filter { !sites.contains($0) }, id: \.self) { s in
                        Button {
                            Prefs.addSite(s)
                            sites = Prefs.sites
                            siteStatus = "Đã thêm lại \(Site.hostOf(s) ?? s) vào danh sách."
                        } label: {
                            Label("Thêm lại \(Site.hostOf(s) ?? s)", systemImage: "plus.circle").foregroundColor(Theme.brand)
                        }
                    }
                } header: { Text("Danh sách trang phim (chọn để chuyển trang)") }

                SecureDnsSection()
                UnblockSection()
                ProxySection()

                Section {
                    Button("Bước tua ◀ ▶ (bàn phím): \(seekStep) giây") {
                        let steps = [5, 10, 15, 30]
                        seekStep = steps[((steps.firstIndex(of: seekStep) ?? 1) + 1) % steps.count]
                        Prefs.seekStep = seekStep
                    }
                    Toggle("Tự chuyển tập khi hết", isOn: $autoNext)
                        .onChange(of: autoNext) { Prefs.autoNext = $0 }
                    HStack {
                        Text("Server ưu tiên (\(Site.hostOf(base) ?? "")): \(serverLabel ?? "chưa chọn")")
                        Spacer()
                        Button("Đặt lại") {
                            Prefs.setServer(Site.hostOf(base), type: nil, label: nil)
                            serverLabel = nil
                            app.showToast("Đã đặt lại. App sẽ nhớ server được chọn lần tới trong lúc xem.")
                        }
                        .disabled(serverLabel == nil)
                    }
                    Toggle("Server dự phòng KKPhim", isOn: $kkBackup)
                        .onChange(of: kkBackup) { Prefs.kkBackup = $0 }
                    Toggle("Server dự phòng NguonC", isOn: $ncBackup)
                        .onChange(of: ncBackup) { Prefs.ncBackup = $0 }
                    if PlayerModel.pipSupported {
                        Toggle("Vuốt lên từ mép dưới lúc đang xem: thu nhỏ phim (hình trong hình)", isOn: $pipOnHome)
                            .onChange(of: pipOnHome) { Prefs.pipOnHome = $0 }
                    }
                } header: { Text("Phát video") } footer: {
                    Text("Server dự phòng KKPhim / NguonC: với yanhh3d, hoathinh3d, hhkungfu… (trình phát của trang hay hỏng cùng lúc) app tìm cùng phim, cùng tập trên KKPhim và NguonC rồi thêm server «KKPhim Vietsub / Thuyết Minh», «NguonC Vietsub…» vào Menu lúc xem (NguonC hay có tập mới sớm; trình phát của nó chạy quảng cáo trước phim, app tự bỏ qua). Trang không vào được mà nguồn dự phòng có tập đó thì vẫn xem được.\n\nHình trong hình: lúc xem chạm nút thu nhỏ (cạnh nút Menu), hoặc vuốt lên từ mép dưới — phim chạy tiếp ở cửa sổ nổi, vẫn chọn phim khác / dùng app khác được; vuốt lên lần nữa để về màn hình chính. Cửa sổ nhỏ có nút phóng to và nút đóng. Tắt mục này thì vuốt lên về thẳng màn hình chính (phim dừng).")
                }

                Section {
                    Button("Xoá lịch sử xem\(who)", role: .destructive) { confirmHistory = true }
                    Button("Xoá ảnh & dữ liệu duyệt web") {
                        app.clearWebData(images: true) { dataStatus = "Đã xoá ảnh, cookie và dữ liệu duyệt web." }
                    }
                    Toggle("Xem riêng\(who) (không lưu lịch sử)", isOn: $privateMode)
                        .onChange(of: privateMode) {
                            Prefs.privateMode = $0
                            app.showSiteLabel()
                        }
                    Toggle("Tự xoá dữ liệu web khi rời app", isOn: $autoClear)
                        .onChange(of: autoClear) { Prefs.autoClearWeb = $0 }
                    if !dataStatus.isEmpty { Text(dataStatus).foregroundColor(Theme.dim) }
                } header: { Text("Dữ liệu") } footer: {
                    Text("Xem riêng: không ghi phim đã xem và chỗ đang xem (tắt app là quên). Tự xoá: xoá cookie, dữ liệu trang và bộ nhớ đệm mỗi khi rời app. Trong Lịch sử xem / hàng Xem tiếp: giữ ngón tay trên một phim để xoá riêng phim đó.")
                }

                Section {
                    Button("Kiểm tra cập nhật (đang dùng \(UpdateCheck.currentVersion))") {
                        app.checkUpdate(manual: true) { updateStatus = $0 }
                    }
                    if !updateStatus.isEmpty { Text(updateStatus).foregroundColor(Theme.dim) }
                    Toggle("Tự động kiểm tra cập nhật", isOn: $autoUpdate)
                        .onChange(of: autoUpdate) { Prefs.autoUpdate = $0 }
                } header: { Text("Cập nhật app") } footer: {
                    Text("App tự tìm bản mới (iPad / iPhone) trên GitHub khi mở (tối đa 2 giờ một lần) rồi mở TrollStore để cài.")
                }

                Section {
                    // Mã Downloader để cài bản TV lên tivi của mình / của người quen (InviteView)
                    NavigationLink("Cài lên tivi / mời bạn bè — mã Downloader \(Invite.code)", isActive: $inviteShown) {
                        InviteView().appDynamicType()
                    }
                    Button("Telegram: t.me/tivikit") {
                        if let u = URL(string: "https://t.me/tivikit") { UIApplication.shared.open(u) }
                    }
                    NavigationLink("Miễn trừ trách nhiệm & quyền riêng tư") {
                        DisclaimerView(gate: false).appDynamicType()
                    }
                } header: { Text("Liên hệ") } footer: {
                    Text("Góp ý, báo lỗi, hỏi đáp: nhắn nhóm Telegram @tivikit.")
                }

                #if DEVTOOLS
                Section {
                    Text(DebugServer.shared.addressText).textSelection(.enabled)
                } header: { Text("Gỡ lỗi (bản thử)") } footer: {
                    Text("Máy tính trong cùng mạng điều khiển, chụp màn hình và đọc nhật ký của app qua cổng này.")
                }
                #endif

                Section {
                    Text("Côn Bằng TV \(version) — xem phim từ trang web trên iPad / iPhone bằng chạm / bàn phím.")
                        .foregroundColor(Theme.dim)
                }
            }
            .navigationTitle("Cài đặt")
            .navigationBarTitleDisplayMode(.inline)
            // Thanh tiêu đề tự vẽ: nút «Xong» trên thanh tiêu đề của SwiftUI (toolbar) làm iPadOS 15 văng app lúc đóng
            // bảng (lỗi bố cục của SwiftUI, nhất là sau hộp thoại «Có bản mới»)
            .navigationBarHidden(true)
            .safeAreaInset(edge: .top, spacing: 0) { header }
            .onAppear { debugOpenInvite() }

            .alert("Xoá lịch sử xem\(who)?", isPresented: $confirmHistory) {
                Button("Xoá", role: .destructive) {
                    Prefs.clearHistory()
                    dataStatus = "Đã xoá lịch sử xem\(who)."
                    app.playerClosed()
                }
                Button("Huỷ", role: .cancel) {}
            } message: { Text("Xoá mọi phim đã xem và chỗ đang xem dở.") }
            .sheet(item: $profileSheet) { s in
                Group {
                    switch s {
                    case .add: ProfileEditView(editing: nil)
                    case .edit(let p): ProfileEditView(editing: p)
                    }
                }
                .environmentObject(app)
                .preferredColorScheme(.dark)
            }
        }
        .navigationViewStyle(.stack)
        .overlay {
            if parentForKids {
                ParentPinView(title: "Mã PIN phụ huynh để tắt hồ sơ trẻ em", onCancel: { parentForKids = false }) {
                    parentForKids = false
                    kids = false
                    Prefs.kids = false
                }
            }
            if parentForAdd {
                ParentPinView(title: "Mã PIN phụ huynh để thêm người xem", onCancel: { parentForAdd = false }) {
                    parentForAdd = false
                    profileSheet = .add
                }
            }
            if let p = unlockFor {
                PinPad(title: "Nhập mã PIN của \(p.name) để sửa", onCancel: { unlockFor = nil }) { pin in
                    guard Prefs.checkPin(p, pin) else { return false }
                    unlockFor = nil
                    profileSheet = .edit(p)
                    return true
                }
            }
        }
    }

    private var profileSection: some View {
        Section {
            ForEach(app.profiles) { p in
                Button {
                    if p.locked { unlockFor = p } else { profileSheet = .edit(p) }
                } label: {
                    HStack(spacing: 12) {
                        ProfileAvatar(profile: p, size: 32)
                        Text(p.name).foregroundColor(Theme.text)
                        if p.locked { Image(systemName: "lock.fill").appFont(13).foregroundColor(Theme.dim) }
                        Spacer()
                        if p.id == app.profileId { Text("đang xem").foregroundColor(Theme.dim) }
                    }
                }
            }
            if app.profiles.count < Prefs.maxProfiles {
                Button("Thêm người xem") {
                    if Prefs.anyLimits && Prefs.hasParentPin { parentForAdd = true } else { profileSheet = .add }
                }
            }
            Button("Giới hạn giờ xem…") { limitsShown = true }
                .sheet(isPresented: $limitsShown, onDismiss: { app.refreshProfiles() }) {
                    LimitsView().environmentObject(app).preferredColorScheme(.dark)
                }
            Toggle("Hỏi «Ai đang xem?» khi mở app", isOn: $askProfile)
                .onChange(of: askProfile) { Prefs.askProfile = $0 }
        } header: { Text("Người xem") } footer: {
            Text("Mỗi người có riêng: lịch sử xem, hàng Xem tiếp, chỗ đang xem dở, phim theo dõi, trang phim đang dùng, server ưu tiên, Xem riêng, bước tua, tự chuyển tập, khung hình. Dùng chung cả nhà: danh sách trang phim, proxy, dữ liệu duyệt web, điểm server, mốc nhạc mở đầu. Chạm một người để đổi tên / màu, đặt hoặc tắt mã PIN, chuyển sang, xoá. Giới hạn giờ xem: số phút mỗi ngày và giờ đi ngủ cho từng người (vd trẻ em), cần mã PIN phụ huynh.")
        }
    }

    private var siteSection: some View {
        Section {
            TextField("Tên miền, vd yanhh3d.ee", text: $url)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .disableAutocorrection(true)
            // Màn hẹp (iPhone): cuộn ngang thay vì gãy chữ trong nút
            ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 12) {
                Button("Lưu") { save() }.buttonStyle(.borderedProminent).tint(Theme.brand)
                Button("Kiểm tra kết nối") { check() }.buttonStyle(.bordered)
                Button("Về mặc định") {
                    url = Site.hostOf(Prefs.defaultBase) ?? Prefs.defaultBase
                    save()
                }
                .buttonStyle(.bordered)
                Button("Xoá khỏi danh sách", role: .destructive) { removeCurrent() }.buttonStyle(.bordered)
            }
            .fixedSize()
            }
            if !siteStatus.isEmpty { Text(siteStatus).foregroundColor(Theme.dim) }
        } header: { Text("Địa chỉ trang phim") } footer: {
            Text("Nhập tên miền rồi nhấn Lưu để thêm trang mới hoặc cập nhật khi trang đổi tên miền (vd .ee → .xyz). Hỗ trợ các trang kiểu yanhh3d, theme HaLim (hoathinh3d, hhkungfu…), kiểu PhimMoi (phimmois), mephim.ddns.net, KKPhim (kkphim.com), NguonC (phim.nguonc.com), VSMOV (vsmov.com) và CLB Phim Xưa (sc.k-20.xyz). Nếu tên miền cũ tự chuyển hướng, app tự cập nhật.")
        }
    }

    private func save() {
        guard let n = Prefs.normalizeUrl(url) else {
            siteStatus = "Link không hợp lệ. Ví dụ đúng: yanhh3d.ee"
            return
        }
        Prefs.baseUrl = n
        base = n
        sites = Prefs.sites
        serverLabel = Prefs.serverLabel(Site.hostOf(n))
        siteStatus = "Đã lưu: \(Site.hostOf(n) ?? n)"
    }

    private func check() {
        guard let b = Prefs.normalizeUrl(url) else {
            siteStatus = "Link không hợp lệ."
            return
        }
        siteStatus = "Đang kiểm tra \(Site.hostOf(b) ?? b)…"
        Task {
            do {
                let home = try await Site.home(b)
                let count = home.sections.reduce(0) { $0 + $1.films.count }
                let moved = home.movedTo.map { "\nTrang chuyển hướng sang \(Site.hostOf($0) ?? $0)." } ?? ""
                siteStatus = count > 0
                    ? "✓ Kết nối được — đọc được \(count) phim, \(home.categories.count) thể loại.\(moved)"
                    : "Kết nối được nhưng không đọc được danh sách phim (trang khác giao diện?). Vẫn có thể dùng mục «Duyệt web».\(moved)"
            } catch {
                siteStatus = "✗ Không kết nối được: \(friendly(error))"
            }
        }
    }

    private func removeCurrent() {
        let rest = Prefs.sites.filter { $0 != base }
        guard let first = rest.first else {
            siteStatus = "Danh sách cần ít nhất một trang."
            return
        }
        Prefs.sites = rest
        Prefs.baseUrl = first
        siteStatus = "Đã xoá \(Site.hostOf(base) ?? base) — đang dùng \(Site.hostOf(first) ?? first)"
        base = first
        url = Site.hostOf(first) ?? first
        sites = Prefs.sites
    }
}
