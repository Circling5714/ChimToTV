import Network
import SwiftUI

/// Cài đặt → Đồng bộ trong nhà: mã ghép của máy này, máy đã ghép, máy tìm thấy trong mạng (chạm để nhập mã ghép).
struct SyncView: View {
    @ObservedObject private var sync = SyncService.shared
    @State private var target: Target?
    @State private var code = ""
    @State private var manualHost = ""
    @State private var pairing = false
    @State private var message = ""
    @FocusState private var codeFocused: Bool

    /// Máy đang ghép: tìm thấy qua Bonjour, hoặc gõ địa chỉ IP.
    private struct Target: Equatable {
        let name: String
        let endpoint: NWEndpoint
    }

    var body: some View {
        Form {
            Section {
                HStack {
                    Text("Mã ghép của máy này")
                    Spacer()
                    Text(sync.pairCode ?? "----").appFont(30, .bold, mono: true).foregroundColor(Theme.focus)
                }
            } footer: {
                Text("Muốn ghép từ máy kia (TV / iPad / iPhone): trên máy kia mở Cài đặt → Đồng bộ trong nhà, chọn «\(SyncService.deviceName)» rồi nhập mã này. Mã chỉ dùng được lúc màn này đang mở.")
            }

            Section {
                Toggle("Tự đồng bộ (mở app, xem xong một tập)", isOn: Binding(get: { sync.enabled }, set: { sync.enabled = $0 }))
                if !sync.peers.isEmpty {
                    Button {
                        Task { await sync.syncAll(manual: true) }
                    } label: {
                        HStack {
                            Label("Đồng bộ ngay", systemImage: "arrow.triangle.2.circlepath")
                            if sync.busy { Spacer(); ProgressView() }
                        }
                    }
                    .disabled(sync.busy)
                }
                ForEach(sync.peers) { p in
                    VStack(alignment: .leading, spacing: 3) {
                        Label(p.name, systemImage: icon(p.platform)).appFont(17, .semibold)
                        Text(peerLine(p)).appFont(13).foregroundColor(Theme.dim)
                    }
                    .padding(.vertical, 2)
                    .swipeActions {
                        Button("Bỏ ghép", role: .destructive) { sync.unpair(p) }
                    }
                    .contextMenu {
                        Button(role: .destructive) { sync.unpair(p) } label: { Label("Bỏ ghép", systemImage: "link.badge.plus") }
                    }
                }
            } header: { Text("Máy đã ghép") } footer: {
                Text(sync.peers.isEmpty ? "Chưa ghép máy nào." :
                    "Đồng bộ lịch sử xem, chỗ đang xem, tập đã xem, phim theo dõi (người xem cùng tên trên hai máy, và người xem đầu tiên của mỗi máy) và danh sách trang phim. Vuốt trái để bỏ ghép.")
            }

            Section {
                let others = sync.found.filter { f in !sync.peers.contains { $0.id == f.id } }
                if others.isEmpty {
                    HStack(spacing: 10) {
                        ProgressView()
                        Text("Đang tìm TV / iPad / iPhone có Côn Bằng TV trong mạng…").foregroundColor(Theme.dim)
                    }
                }
                ForEach(others) { f in
                    Button {
                        pick(Target(name: f.name, endpoint: f.endpoint))
                    } label: {
                        Label(f.name, systemImage: icon(f.platform)).foregroundColor(Theme.text)
                    }
                }
                HStack {
                    TextField("Hoặc gõ địa chỉ IP của máy kia", text: $manualHost)
                        .keyboardType(.numbersAndPunctuation)
                        .disableAutocorrection(true)
                        .textInputAutocapitalization(.never)
                    Button("Chọn") {
                        // "192.168.1.20" hoặc "192.168.1.20:8792" (cổng khác — bản thử)
                        let s = manualHost.trimmingCharacters(in: .whitespaces)
                        let parts = s.split(separator: ":")
                        let host = parts.first.map(String.init) ?? ""
                        let num = parts.count == 2 ? UInt16(parts[1]) ?? 8791 : 8791
                        guard !host.isEmpty, let port = NWEndpoint.Port(rawValue: num) else { return }
                        pick(Target(name: s, endpoint: .hostPort(host: NWEndpoint.Host(host), port: port)))
                    }
                    .disabled(manualHost.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                if let t = target {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Nhập mã ghép đang hiện trên «\(t.name)» (Cài đặt → Đồng bộ trong nhà):").appFont(15)
                        HStack {
                            TextField("4 số", text: $code)
                                .keyboardType(.numberPad)
                                .focused($codeFocused)
                                .appFont(22, .semibold, mono: true)
                                .frame(width: 110)
                                .onChange(of: code) { v in
                                    code = String(v.filter(\.isNumber).prefix(4))
                                    if code.count == 4 { doPair(t) }
                                }
                            Button("Ghép") { doPair(t) }
                                .buttonStyle(.borderedProminent)
                                .tint(Theme.brand)
                                .disabled(code.count != 4 || pairing)
                            if pairing { ProgressView() }
                            Spacer()
                            Button("Huỷ") { target = nil }
                        }
                    }
                    .padding(.vertical, 4)
                }
                if !message.isEmpty { Text(message).foregroundColor(Theme.dim) }
            } header: { Text("Ghép với máy khác") } footer: {
                Text("Hai máy phải cùng mạng Wi‑Fi trong nhà. TV cần Côn Bằng TV 2.2 trở lên. Lần đầu iPad / iPhone hỏi cho phép app tìm thiết bị trong mạng cục bộ: chọn Cho phép.")
            }
        }
        .navigationTitle("Đồng bộ trong nhà")
        .onAppear { sync.openPairing() }
        .onDisappear { sync.closePairing() }
    }

    private func pick(_ t: Target) {
        target = t
        code = ""
        message = ""
        codeFocused = true
    }

    private func doPair(_ t: Target) {
        guard code.count == 4, !pairing else { return }
        pairing = true
        let c = code
        Task {
            let err = await sync.pair(t.endpoint, code: c)
            pairing = false
            if let e = err {
                message = e
                code = ""
            } else {
                message = "Đã ghép với «\(t.name)»."
                target = nil
                codeFocused = false
            }
        }
    }

    private func icon(_ platform: String) -> String {
        switch platform {
        case "android": return "tv"
        case "ios": return "ipad.and.iphone"
        default: return "desktopcomputer"
        }
    }

    private func peerLine(_ p: SyncPeer) -> String {
        var s = p.status ?? ""
        if let last = p.last {
            let f = DateFormatter()
            f.locale = Locale(identifier: "vi_VN")
            f.dateFormat = "HH:mm dd/MM"
            s += (s.isEmpty ? "" : " · ") + "lần cuối \(f.string(from: Date(timeIntervalSince1970: Double(last) / 1000)))"
        }
        return s.isEmpty ? "Chưa đồng bộ" : s
    }
}
