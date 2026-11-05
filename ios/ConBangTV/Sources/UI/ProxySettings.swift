import SwiftUI

/// Cài đặt → Proxy SOCKS5 (bản Android: phần Proxy trong SettingsActivity). Dùng chung cả nhà, không theo người xem.
struct ProxySection: View {
    @EnvironmentObject var app: AppModel
    @State private var on = Prefs.proxyOn
    @State private var host = Prefs.proxyHost
    @State private var port = Prefs.proxyHost.isEmpty ? "" : String(Prefs.proxyPort)
    @State private var user = Prefs.proxyUser
    @State private var pass = Prefs.proxyPass
    @State private var remoteDns = Prefs.proxyRemoteDns
    @State private var sitesOnly = Prefs.proxySitesOnly
    @State private var status = ""
    @State private var failed = false
    @State private var testing = false
    @State private var showPass = false

    var body: some View {
        Section {
            Toggle("Dùng proxy SOCKS5", isOn: Binding(get: { on }, set: { setOn($0) }))
            TextField("Máy chủ, vd 103.1.2.3 (hoặc dán socks5://tên:mật-khẩu@máy:cổng)", text: $host)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .disableAutocorrection(true)
            TextField("Cổng, vd 1080", text: $port).keyboardType(.numberPad)
            TextField("Tên đăng nhập (bỏ trống nếu không có)", text: $user)
                .textInputAutocapitalization(.never)
                .disableAutocorrection(true)
            HStack {
                // Ô mật khẩu: không tự viết hoa chữ đầu / tự sửa chữ (bàn phím iPad bật sẵn Shift → sai mật khẩu mà
                // không thấy vì chữ bị che); nút con mắt để xem lại chữ đã gõ
                Group {
                    if showPass {
                        TextField("Mật khẩu (bỏ trống nếu không có)", text: $pass)
                    } else {
                        SecureField("Mật khẩu (bỏ trống nếu không có)", text: $pass)
                    }
                }
                .textInputAutocapitalization(.never)
                .disableAutocorrection(true)
                .textContentType(.password)
                Button { showPass.toggle() } label: {
                    Image(systemName: showPass ? "eye.slash" : "eye").foregroundColor(Theme.dim)
                }
                .buttonStyle(.borderless)
                .accessibilityLabel(showPass ? "Ẩn mật khẩu" : "Hiện mật khẩu")
            }
            HStack {
                Text("DNS")
                Spacer()
                Picker("DNS", selection: $remoteDns) {
                    Text("của proxy").tag(true)
                    Text("của máy").tag(false)
                }
                .pickerStyle(.segmented)
                .frame(maxWidth: 300)
            }
            .onChange(of: remoteDns) {
                Prefs.proxyRemoteDns = $0
                changed()
            }
            HStack {
                Text("Dùng cho")
                Spacer()
                Picker("Dùng cho", selection: $sitesOnly) {
                    Text("toàn bộ app").tag(false)
                    Text("chỉ trang phim").tag(true)
                }
                .pickerStyle(.segmented)
                .frame(maxWidth: 300)
            }
            .onChange(of: sitesOnly) {
                Prefs.proxySitesOnly = $0
                changed()
            }
            HStack(spacing: 12) {
                Button("Lưu proxy") { saveTapped() }.buttonStyle(.borderedProminent).tint(Theme.brand)
                Button(testing ? "Đang kiểm tra…" : "Kiểm tra proxy") { test() }.buttonStyle(.bordered).disabled(testing)
            }
            if !status.isEmpty { Text(status).foregroundColor(failed ? .red : Theme.dim) }
        } header: { Text("Proxy SOCKS5") } footer: {
            Text("DNS của proxy: proxy tự tìm địa chỉ trang (vượt chặn tên miền của nhà mạng); DNS của máy: máy tự tìm rồi chỉ gửi IP cho proxy. Chỉ trang phim: video, ảnh, cập nhật app đi thẳng mạng nhà (đỡ tốn proxy). Trên iOS 15–16, lúc bật proxy app không giữ cookie của trang web sau khi tắt app.")
        }
        .onAppear { show() }
    }

    /// Bật thì lưu luôn máy chủ / cổng đang nhập (sai thì báo, không bật).
    private func setOn(_ v: Bool) {
        if v && !saveFields() { return }
        on = v
        Prefs.proxyOn = v
        changed()
    }

    private func saveTapped() {
        guard saveFields() else { return }
        changed()
        status += on ? "\nĐã lưu proxy." : "\nĐã lưu proxy — bật «Dùng proxy SOCKS5» để dùng."
    }

    /// Đọc ô nhập (nhận cả dạng dán sẵn) và lưu; sai thì báo, trả false.
    private func saveFields() -> Bool {
        switch AppProxy.parse(host: host, port: port, user: user, pass: pass) {
        case .failure(let e):
            status = e.message
            failed = true
            return false
        case .success(let c):
            host = c.host
            port = String(c.port)
            user = c.user
            pass = c.pass
            Prefs.proxyHost = c.host
            Prefs.proxyPort = c.port
            Prefs.proxyUser = c.user
            Prefs.proxyPass = c.pass
            return true
        }
    }

    /// Đổi cài đặt: mở lại cổng nội bộ theo cài đặt mới; đóng Cài đặt thì app tải lại trang phim.
    private func changed() {
        AppProxy.apply()
        app.proxyChanged = true
        show()
    }

    private func show() {
        failed = false
        guard let c = Prefs.proxyConfig() else {
            status = "Đang tắt — app dùng mạng mặc định của máy."
            return
        }
        status = "Đang bật: \(c.host):\(c.port)" + (c.user.isEmpty ? "" : " · có đăng nhập") +
            (WebSetup.canProxy ? "" : "\n⚠ iOS bản này không cho trình phát đi qua proxy: video và Duyệt web vẫn đi thẳng, chỉ phần đọc trang đi qua proxy.")
    }

    /// Thử proxy đang nhập (chưa cần lưu / bật): mở HTTPS tới Cloudflare qua proxy, báo IP ra ngoài + thời gian.
    private func test() {
        guard case .success(let c) = AppProxy.parse(host: host, port: port, user: user, pass: pass) else {
            _ = saveFields() // hiện lỗi nhập
            return
        }
        testing = true
        failed = false
        status = "Đang kiểm tra proxy \(c.host):\(c.port)…"
        let start = Date()
        Task {
            do {
                let ip = try await AppProxy.check(c)
                let sec = String(format: "%.1f s", Date().timeIntervalSince(start)).replacingOccurrences(of: ".", with: ",")
                status = "✓ Proxy chạy — IP ra ngoài \(ip), \(sec)." + (on ? "" : " Bật «Dùng proxy SOCKS5» để dùng.")
            } catch {
                failed = true
                status = "✗ Proxy không chạy: \(friendly(error, proxyNote: false))"
            }
            testing = false
        }
    }
}

/// Cài đặt → Vượt chặn nhà mạng (dùng chung cả nhà): chia nhỏ gói bắt tay HTTPS (Socks5Bridge.writeHello).
struct UnblockSection: View {
    @EnvironmentObject var app: AppModel
    @State private var on = Prefs.unblock

    var body: some View {
        Section {
            Toggle("Vượt chặn nhà mạng", isOn: $on)
                .onChange(of: on) {
                    Prefs.unblock = $0
                    AppProxy.apply()
                    app.proxyChanged = true
                }
            if on && !WebSetup.canProxy {
                Text("⚠ iOS bản này không cho trình phát đi qua cổng vượt chặn: chỉ phần đọc trang được vượt chặn.")
                    .foregroundColor(Theme.dim)
            }
        } header: { Text("Vượt chặn nhà mạng") } footer: {
            Text("Một số nhà mạng chặn trang phim (vd phimmoir.fun) bằng cách đọc tên trang trong gói bắt tay HTTPS: trang mở mãi không được, dùng VPN thì vào được. Bật mục này: app chia nhỏ gói đó nên nhà mạng không đọc được tên trang — không cần VPN hay proxy. Trên iOS 15–16, lúc bật app không giữ cookie của trang web sau khi tắt app.")
        }
    }
}
