import SwiftUI

/// Chọn trang phim. Mở ra là đo cùng lúc mọi trang: tải trang chủ mất bao lâu, đọc được bao nhiêu phim, kèm điểm video
/// các lần xem trước; trang nhanh nhất có ★ (bản Android: MainActivity.pickSite).
struct SitesView: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var checks: [String: Result<SiteCheck, Error>] = [:]

    private var sites: [String] { Prefs.sites }

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
                } footer: {
                    Text("Thêm trang mới hoặc sửa tên miền trong Cài đặt.")
                }
            }
            .navigationTitle("Chọn trang phim")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Đóng") { dismiss() } }
                ToolbarItem(placement: .primaryAction) {
                    Button("Thêm / sửa…") {
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
