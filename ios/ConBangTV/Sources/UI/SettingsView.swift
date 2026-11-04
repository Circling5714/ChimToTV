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

    private var version: String {
        let v = UpdateCheck.currentVersion
        let b = (Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String) ?? ""
        return "\(v) (\(b))"
    }

    var body: some View {
        NavigationView {
            Form {
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
                } header: { Text("Danh sách trang phim (chọn để chuyển trang)") }

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
                } header: { Text("Phát video") }

                Section {
                    Button("Xoá lịch sử xem", role: .destructive) { confirmHistory = true }
                    Button("Xoá ảnh & dữ liệu duyệt web") {
                        app.clearWebData(images: true) { dataStatus = "Đã xoá ảnh, cookie và dữ liệu duyệt web." }
                    }
                    Toggle("Xem riêng (không lưu lịch sử)", isOn: $privateMode)
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
                    Text("App tự tìm bản iPad mới trên GitHub khi mở (tối đa 2 giờ một lần) rồi mở TrollStore để cài.")
                }

                Section {
                    Button("Telegram: t.me/tivikit") {
                        if let u = URL(string: "https://t.me/tivikit") { UIApplication.shared.open(u) }
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
                    Text("Côn Bằng TV \(version) — xem phim từ trang web trên iPad bằng chạm / bàn phím.")
                        .foregroundColor(Theme.dim)
                }
            }
            .navigationTitle("Cài đặt")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Xong") { dismiss() } } }
            .alert("Xoá lịch sử xem?", isPresented: $confirmHistory) {
                Button("Xoá", role: .destructive) {
                    Prefs.clearHistory()
                    dataStatus = "Đã xoá lịch sử xem."
                    app.playerClosed()
                }
                Button("Huỷ", role: .cancel) {}
            } message: { Text("Xoá mọi phim đã xem và chỗ đang xem dở.") }
        }
        .navigationViewStyle(.stack)
    }

    private var siteSection: some View {
        Section {
            TextField("Tên miền, vd yanhh3d.ee", text: $url)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .disableAutocorrection(true)
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
            if !siteStatus.isEmpty { Text(siteStatus).foregroundColor(Theme.dim) }
        } header: { Text("Địa chỉ trang phim") } footer: {
            Text("Nhập tên miền rồi nhấn Lưu để thêm trang mới hoặc cập nhật khi trang đổi tên miền (vd .ee → .xyz). Hỗ trợ các trang kiểu yanhh3d, theme HaLim (hoathinh3d, hhkungfu…) và kiểu PhimMoi (phimmoir). Nếu tên miền cũ tự chuyển hướng, app tự cập nhật.")
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
