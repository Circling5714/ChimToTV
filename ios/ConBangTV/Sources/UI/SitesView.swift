import SwiftUI

/// Chọn trang phim. Mở ra là đo cùng lúc mọi trang: tải trang chủ mất bao lâu, đọc được bao nhiêu phim, kèm điểm video
/// các lần xem trước; trang nhanh nhất có ★ (bản Android: MainActivity.pickSite).
struct SitesView: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var checks: [String: Result<SiteCheck, Error>] = [:]
    @State private var sites = Prefs.sites
    @State private var newSite = ""
    @State private var addError = ""

    private var fastest: String? {
        let ok = checks.compactMap { k, r -> (String, Int)? in
            if case .success(let c) = r, c.films > 0 { return (k, c.ms) }
            return nil
        }
        return ok.count >= 2 ? ok.min { $0.1 < $1.1 }?.0 : nil
    }

    var body: some View {
        NavigationView {
            List {
                Section {
                    ForEach(sites, id: \.self) { s in
                        Button { app.selectSite(s) } label: {
                            HStack(spacing: 12) {
                                Image(systemName: s == Prefs.baseUrl ? "checkmark.circle.fill" : "circle")
                                    .foregroundColor(s == Prefs.baseUrl ? Theme.brand : Theme.dim)
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(Site.hostOf(s) ?? s).font(.system(size: 17, weight: .semibold)).foregroundColor(Theme.text)
                                    Text(label(s)).font(.system(size: 14)).foregroundColor(Theme.dim)
                                }
                                Spacer()
                                if s == fastest { Text("★ nhanh nhất").font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.focus) }
                            }
                            .padding(.vertical, 4)
                        }
                    }
                    .onDelete { idx in
                        // Vuốt sang trái để xoá trang khỏi danh sách (còn ít nhất một trang)
                        for s in idx.map({ sites[$0] }) where Prefs.removeSite(s) { checks[s] = nil }
                        sites = Prefs.sites
                        app.showSiteLabel()
                    }
                } footer: {
                    Text("Vuốt một trang sang trái để xoá khỏi danh sách. Sửa tên miền khi trang đổi tên: Cài đặt.")
                }
                Section {
                    // Trang mẫu đã xoá (vd phimmois.fun): thêm lại một chạm
                    ForEach(Prefs.missingPresets.filter { !sites.contains($0) }, id: \.self) { s in
                        Button { add(s) } label: {
                            Label("Thêm lại \(Site.hostOf(s) ?? s)", systemImage: "plus.circle").foregroundColor(Theme.brand)
                        }
                    }
                    HStack {
                        TextField("Tên miền trang khác, vd phimmois.fun", text: $newSite)
                            .keyboardType(.URL)
                            .textInputAutocapitalization(.never)
                            .disableAutocorrection(true)
                            .onSubmit { addTyped() }
                        Button("Thêm") { addTyped() }.disabled(newSite.trimmed.isEmpty)
                    }
                    if !addError.isEmpty { Text(addError).foregroundColor(.red) }
                } header: { Text("Thêm trang") } footer: {
                    Text("Hỗ trợ các trang kiểu yanhh3d, theme HaLim (hoathinh3d, hhkungfu…) và kiểu PhimMoi (phimmoir). Thêm xong là chuyển sang trang đó.")
                }
            }
            .navigationTitle("Chọn trang phim")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Đóng") { dismiss() } }
                ToolbarItem(placement: .primaryAction) {
                    Button("Sửa tên miền…") {
                        app.sheet = nil
                        Task { @MainActor in
                            try? await Task.sleep(nanoseconds: 500_000_000)
                            app.sheet = .settings
                        }
                    }
                }
            }
        }
        .navigationViewStyle(.stack)
        .task {
            await withTaskGroup(of: (String, Result<SiteCheck, Error>).self) { group in
                for s in sites {
                    group.addTask {
                        do { return (s, .success(try await Site.check(s))) } catch { return (s, .failure(error)) }
                    }
                }
                for await (s, r) in group { checks[s] = r }
            }
        }
    }

    private func addTyped() {
        guard let u = Prefs.normalizeUrl(newSite) else {
            addError = "Tên miền không hợp lệ. Ví dụ đúng: phimmois.fun"
            return
        }
        add(u)
    }

    /// Thêm trang vào danh sách rồi chuyển sang trang đó.
    private func add(_ url: String) {
        addError = ""
        newSite = ""
        Prefs.addSite(url)
        sites = Prefs.sites
        app.showToast("Đã thêm \(Site.hostOf(url) ?? url) vào danh sách trang phim.")
        app.selectSite(url)
    }

    /// vd "0,4 s · 36 phim · video mượt"
    private func label(_ s: String) -> String {
        var parts: [String] = []
        switch checks[s] {
        case .none: parts.append("đang đo…")
        case .failure: parts.append("✗ không vào được")
        case .success(let c):
            parts.append(String(format: "%.1f s", Double(c.ms) / 1000).replacingOccurrences(of: ".", with: ","))
            parts.append(c.films > 0 ? "\(c.films) phim" : "không đọc được phim")
        }
        if let v = PlayStats.siteSummary(Site.hostOf(s)) { parts.append(v) }
        return parts.joined(separator: " · ")
    }
}

/// Thể loại / quốc gia đủ danh sách (trang kiểu PhimMoi).
struct FilterView: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let title: String
    let items: [Category]

    var body: some View {
        NavigationView {
            List(items) { c in
                Button { app.pickCategory(c) } label: {
                    Text(c.title).foregroundColor(Theme.text)
                }
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Đóng") { dismiss() } } }
        }
        .navigationViewStyle(.stack)
    }
}
