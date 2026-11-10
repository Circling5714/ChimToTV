import SwiftUI
import UIKit

/// Cài đặt → Proxy: SOCKS5 hoặc Shadowsocks (bản Android: phần Proxy trong SettingsActivity). Dùng chung cả nhà, không theo
/// người xem.
struct ProxySection: View {
    @EnvironmentObject var app: AppModel
    @State private var on = Prefs.proxyOn
    @State private var kind = Prefs.proxyKind
    @State private var ssHost = Prefs.ssHost
    @State private var ssPort = Prefs.ssPort > 0 ? String(Prefs.ssPort) : ""
    @State private var ssMethod = SsMethod.of(Prefs.ssMethod) ?? .defaultMethod
    @State private var ssKey = Prefs.ssKey
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
            Toggle("Dùng proxy", isOn: Binding(get: { on }, set: { setOn($0) }))
            Picker("Kiểu", selection: Binding(get: { kind }, set: { setKind($0) })) {
                Text("SOCKS5").tag("socks5")
                Text("Shadowsocks (mã hoá)").tag("ss")
            }
            .pickerStyle(.segmented)
            if kind == "ss" { ssFields } else { socksFields }
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
        } header: { Text("Proxy (SOCKS5 / Shadowsocks)") } footer: {
            Text("DNS của proxy: proxy tự tìm địa chỉ trang (vượt chặn tên miền của nhà mạng); DNS của máy: máy tự tìm rồi chỉ gửi IP cho proxy. Chỉ trang phim: video, ảnh, cập nhật app đi thẳng mạng nhà (đỡ tốn proxy). SOCKS5 không mã hoá: nhà mạng thấy tên trang, tên đăng nhập / mật khẩu proxy. Shadowsocks mã hoá cả đường tới máy chủ (vd lối vào Shadowsocks của 3X-UI, TCP, kiểu 2022-blake3-aes-256-gcm): nhà mạng chỉ thấy dữ liệu ngẫu nhiên. Trên iOS 15–16, lúc bật proxy app không giữ cookie của trang web sau khi tắt app.")
        }
        .onAppear { show() }
    }

    /// Ô nhập Shadowsocks: dán link ss:// (3X-UI: nút Copy / QR của người dùng) là tự điền.
    @ViewBuilder private var ssFields: some View {
        Button {
            if let t = UIPasteboard.general.string { pasteLink(t) } else { status = "Bộ nhớ tạm trống — chép link ss:// trước."; failed = true }
        } label: { Label("Dán link ss:// từ bộ nhớ tạm", systemImage: "doc.on.clipboard") }
        TextField("Máy chủ, vd 103.1.2.3 (hoặc dán link ss://…)", text: $ssHost)
            .keyboardType(.URL)
            .textInputAutocapitalization(.never)
            .disableAutocorrection(true)
            .onChange(of: ssHost) { if $0.lowercased().hasPrefix("ss://") { pasteLink($0) } }
        TextField("Cổng", text: $ssPort).keyboardType(.numberPad)
        Picker("Kiểu mã hoá", selection: $ssMethod) {
            ForEach(SsMethod.allCases, id: \.self) { Text($0.id).tag($0) }
        }
        HStack {
            Group {
                if showPass {
                    TextField("Khoá (3X-UI: khoá-máy-chủ:khoá-người-dùng) / mật khẩu", text: $ssKey)
                } else {
                    SecureField("Khoá (3X-UI: khoá-máy-chủ:khoá-người-dùng) / mật khẩu", text: $ssKey)
                }
            }
            .textInputAutocapitalization(.never)
            .disableAutocorrection(true)
            Button { showPass.toggle() } label: {
                Image(systemName: showPass ? "eye.slash" : "eye").foregroundColor(Theme.dim)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(showPass ? "Ẩn khoá" : "Hiện khoá")
        }
    }

    @ViewBuilder private var socksFields: some View {
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
    }

    /// Đổi kiểu: mỗi kiểu giữ máy chủ / khoá riêng (đang bật thì đổi luôn đường đi).
    private func setKind(_ k: String) {
        kind = k
        Prefs.proxyKind = k
        changed()
    }

    /// Link ss:// (dán vào ô máy chủ hoặc nút «Dán link»): đọc rồi điền các ô, chuyển sang kiểu Shadowsocks.
    private func pasteLink(_ text: String) {
        do {
            let c = try SsLink.parse(text, remoteDns: Prefs.proxyRemoteDns, sitesOnly: Prefs.proxySitesOnly)
            fillSs(c)
            if kind != "ss" { setKind("ss") }
            status = "Đã đọc link: \(c.host):\(c.port) · \(c.method) — nhấn «Kiểm tra proxy» rồi «Lưu proxy»."
            failed = false
        } catch {
            status = error.localizedDescription
            failed = true
        }
    }

    private func fillSs(_ c: SsConfig) {
        ssHost = c.host
        ssPort = String(c.port)
        ssMethod = SsMethod.of(c.method) ?? .defaultMethod
        ssKey = c.password
    }

    /// Đọc ô Shadowsocks; sai thì câu báo.
    private func readSs() -> Result<SsConfig, AppError> {
        var h = ssHost.trimmed
        var p = ssPort.trimmed
        if h.lowercased().hasPrefix("ss://") {
            do {
                return .success(try SsLink.parse(h, remoteDns: Prefs.proxyRemoteDns, sitesOnly: Prefs.proxySitesOnly))
            } catch {
                return .failure(AppError(error.localizedDescription))
            }
        }
        if h.filter({ $0 == ":" }).count == 1 || h.hasPrefix("["), let hp = SsLink.splitHostPort(h) {
            h = hp.0
            p = String(hp.1)
        }
        guard !h.isEmpty, !h.contains(" ") else { return .failure(AppError("Nhập máy chủ Shadowsocks (vd 103.1.2.3), hoặc dán link ss://…")) }
        guard let port = Int(p), (1...65535).contains(port) else { return .failure(AppError("Cổng không hợp lệ (1–65535).")) }
        let key = ssKey.trimmed
        do {
            _ = try Shadowsocks.keys(ssMethod, key)
        } catch {
            return .failure(AppError(error.localizedDescription))
        }
        return .success(SsConfig(host: h, port: port, method: ssMethod.id, password: key, remoteDns: Prefs.proxyRemoteDns,
                                 sitesOnly: Prefs.proxySitesOnly))
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
        status += on ? "\nĐã lưu proxy." : "\nĐã lưu proxy — bật «Dùng proxy» để dùng."
    }

    /// Đọc ô nhập (nhận cả dạng dán sẵn) và lưu; sai thì báo, trả false.
    private func saveFields() -> Bool {
        if kind == "ss" {
            switch readSs() {
            case .failure(let e):
                status = e.message
                failed = true
                return false
            case .success(let c):
                fillSs(c)
                Prefs.ssHost = c.host
                Prefs.ssPort = c.port
                Prefs.ssMethod = c.method
                Prefs.ssKey = c.password
                return true
            }
        }
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
        if let q = Prefs.ssConfig() {
            status = "Đang bật Shadowsocks: \(q.host):\(q.port) · \(q.method)" +
                (WebSetup.canProxy ? "" : "\n⚠ iOS bản này không cho trình phát đi qua proxy: video và Duyệt web vẫn đi thẳng, chỉ phần đọc trang đi qua proxy.")
            return
        }
        guard let c = Prefs.proxyConfig() else {
            status = "Đang tắt — app dùng mạng mặc định của máy."
            return
        }
        status = "Đang bật SOCKS5: \(c.host):\(c.port)" + (c.user.isEmpty ? "" : " · có đăng nhập") +
            (WebSetup.canProxy ? "" : "\n⚠ iOS bản này không cho trình phát đi qua proxy: video và Duyệt web vẫn đi thẳng, chỉ phần đọc trang đi qua proxy.")
    }

    /// Thử proxy đang nhập (chưa cần lưu / bật): mở HTTPS tới Cloudflare qua proxy, báo IP ra ngoài + thời gian.
    private func test() {
        if kind == "ss" {
            switch readSs() {
            case .failure(let e):
                status = e.message
                failed = true
            case .success(let c):
                fillSs(c)
                run("Đang kiểm tra Shadowsocks \(c.host):\(c.port)…") { try await AppProxy.check(c) }
            }
            return
        }
        guard case .success(let c) = AppProxy.parse(host: host, port: port, user: user, pass: pass) else {
            _ = saveFields() // hiện lỗi nhập
            return
        }
        run("Đang kiểm tra proxy \(c.host):\(c.port)…") { try await AppProxy.check(c) }
    }

    private func run(_ label: String, _ check: @escaping () async throws -> String) {
        testing = true
        failed = false
        status = label
        let start = Date()
        Task {
            do {
                let ip = try await check()
                let sec = String(format: "%.1f s", Date().timeIntervalSince(start)).replacingOccurrences(of: ".", with: ",")
                status = "✓ Proxy chạy — IP ra ngoài \(ip), \(sec)." + (on ? "" : " Bật «Dùng proxy» để dùng.")
            } catch {
                failed = true
                status = "✗ Proxy không chạy: \(friendly(error, proxyNote: false))"
            }
            testing = false
        }
    }
}

/// Cài đặt → DNS mã hoá (dùng chung cả nhà, bật sẵn từ 1.10.1): DoH rồi DoT, NextDNS → Cloudflare → Google (Doh).
struct SecureDnsSection: View {
    @EnvironmentObject var app: AppModel
    @State private var on = Prefs.secureDns

    var body: some View {
        Section {
            Toggle("DNS mã hoá (DoH / DoT)", isOn: $on)
                .onChange(of: on) {
                    Prefs.secureDns = $0
                    AppProxy.apply()
                    app.proxyChanged = true
                }
        } header: { Text("DNS mã hoá") } footer: {
            Text("Hỏi địa chỉ trang qua DNS mã hoá thay cho DNS nhà mạng: DoH rồi DoT, lần lượt \(Doh.providers); lỗi hết thì dùng DNS của máy — app không hỏng khi DNS mã hoá bị chặn. Nhà mạng không đọc / sửa được truy vấn DNS (vẫn thấy tên trang lúc kết nối — Vượt chặn lo phần này). Đang bật proxy với «DNS của proxy» thì proxy tự hỏi DNS.")
        }
    }
}

/// Cài đặt → Vượt chặn nhà mạng (dùng chung cả nhà): chia nhỏ gói bắt tay HTTPS (Socks5Bridge.writeHello).
struct UnblockSection: View {
    @EnvironmentObject var app: AppModel
    @State private var on = Prefs.unblock
    @State private var level = Prefs.unblockLevel

    var body: some View {
        Section {
            Toggle("Vượt chặn nhà mạng", isOn: $on)
                .onChange(of: on) {
                    Prefs.unblock = $0
                    AppProxy.apply()
                    app.proxyChanged = true
                }
            if on {
                Picker("Mức", selection: $level) {
                    Text("Mức 1: chia nhỏ").tag(1)
                    Text("Mức 2: đảo thứ tự").tag(2)
                }
                .pickerStyle(.segmented)
                .onChange(of: level) {
                    Prefs.unblockLevel = $0
                    AppProxy.apply()
                    app.proxyChanged = true
                }
            }
            if on && !WebSetup.canProxy {
                Text("⚠ iOS bản này không cho trình phát đi qua cổng vượt chặn: chỉ phần đọc trang được vượt chặn.")
                    .foregroundColor(Theme.dim)
            }
        } header: { Text("Vượt chặn nhà mạng") } footer: {
            Text("Một số nhà mạng chặn trang phim (vd phimmoir.fun) bằng cách đọc tên trang trong gói bắt tay HTTPS: trang mở mãi không được, dùng VPN thì vào được. Bật mục này: app chia nhỏ gói đó nên nhà mạng không đọc được tên trang — không cần VPN hay proxy. Trang / ảnh bìa bị chặn bằng DNS (DNS trả 0.0.0.0) thì app tự hỏi lại địa chỉ qua DNS mã hoá. Mức 2 (khi mức 1 không còn lọt): thêm gửi đảo thứ tự — mẩu đầu đặt TTL thấp để chết giữa đường trong nhà, hệ thống tự gửi lại sau, nên máy soi của nhà mạng thấy gói bắt tay thiếu đầu. Trên iOS 15–16, lúc bật app không giữ cookie của trang web sau khi tắt app.")
        }
    }
}
