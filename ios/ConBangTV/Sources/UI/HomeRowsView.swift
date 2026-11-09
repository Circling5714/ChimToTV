import SwiftUI

/// Một dòng trong danh sách các hàng (id trong HomeFeed.rows hoặc "custom-<id>").
private struct RowItem: Identifiable, Hashable {
    let id: String
    let on: Bool
}

/// Cài đặt → Trang chủ → «Các hàng trên trang chủ» (riêng từng người xem; bản Android: HomeRowsUi.kt): bật / tắt từng hàng,
/// ▲ ▼ đổi thứ tự, «Thêm hàng…» (chủ đề có sẵn hoặc loại phim + nước + thể loại + ngôn ngữ, tối đa HomeFeed.customMax), xoá
/// hàng tự thêm, «Khôi phục mặc định» (giữ hàng tự thêm). Đổi là lưu ngay; đóng Cài đặt thì trang chủ xếp lại, tải hàng mới.
/// Không dùng nút trên thanh tiêu đề (iPadOS 15 văng app lúc đóng bảng có nút toolbar).
struct HomeRowsView: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    /// Hồ sơ trẻ em có mã PIN phụ huynh: hỏi mã trước khi cho sửa (huỷ thì quay lại Cài đặt).
    var needPin = false
    @State private var unlocked = false
    @State private var prefs: [RowItem] = HomeRowsView.load()
    @State private var custom: [CustomRow] = Prefs.homeCustom
    @State private var confirmDelete: CustomRow?

    private static func load() -> [RowItem] { HomeFeed.rowPrefs().map { RowItem(id: $0.id, on: $0.on) } }

    private var kids: Bool { Prefs.kids }

    private func visible(_ id: String) -> Bool {
        if id.hasPrefix("custom-") { return true }
        return kids ? !HomeFeed.adultRows.contains(id) : !HomeFeed.kidsRows.contains(id)
    }

    private func movable(_ id: String) -> Bool { visible(id) && !HomeFeed.headRows.contains(id) }

    private func label(_ id: String) -> String {
        if id.hasPrefix("custom-") { return custom.first { $0.key == id }.map { HomeFeed.customTitle($0) } ?? id }
        let l = HomeFeed.rows.first { $0.id == id }?.label ?? id
        return l.replacingOccurrences(of: "Vì …", with: "Vì \(Prefs.profile.name)")
    }

    private func reload() {
        custom = Prefs.homeCustom
        prefs = Self.load()
    }

    private func save(_ p: [RowItem]) {
        Prefs.homeRows = p.map { $0.on ? $0.id : "-\($0.id)" }
        reload()
    }

    private func toggle(_ id: String) {
        save(prefs.map { $0.id == id ? RowItem(id: id, on: !$0.on) : $0 })
    }

    private func move(_ id: String, up: Bool) {
        var p = prefs
        guard let i = p.firstIndex(where: { $0.id == id }) else { return }
        let range: [Int] = up ? Array((0..<i).reversed()) : Array((i + 1)..<p.count)
        guard let j = range.first(where: { movable(p[$0].id) }) else { return }
        p.swapAt(i, j)
        save(p)
    }

    private func add(_ c: CustomRow) {
        if custom.contains(where: { $0.same(c) }) {
            app.showToast("Đã có hàng «\(HomeFeed.customTitle(c))».")
            return
        }
        Prefs.homeCustom = custom + [c]
        reload()
    }

    private func remove(_ c: CustomRow) {
        Prefs.homeCustom = custom.filter { $0.key != c.key }
        Prefs.homeRows = HomeFeed.rowPrefs().filter { $0.id != c.key }.map { $0.on ? $0.id : "-\($0.id)" }
        reload()
    }

    var body: some View {
        let shown = prefs.filter { visible($0.id) }
        let head = shown.filter { HomeFeed.headRows.contains($0.id) }
        let others = shown.filter { !HomeFeed.headRows.contains($0.id) }
        List {
            Section {
                ForEach(head) { r in row(r, others) }
            } header: {
                Text("Luôn ở đầu trang")
            }
            Section {
                ForEach(others) { r in row(r, others) }
            } header: {
                Text("Các hàng")
            } footer: {
                Text("Hàng tắt thì không tải; hàng nào không đủ phim thì tự ẩn." + (kids ? " Hồ sơ trẻ em: chỉ các hàng hoạt hình hợp trẻ em." : ""))
            }
            Section {
                if custom.count < HomeFeed.customMax {
                    NavigationLink {
                        AddHomeRowView(existing: custom) { add($0) }
                    } label: {
                        Label("Thêm hàng…", systemImage: "plus.circle")
                    }
                } else {
                    Text("Đã đủ \(HomeFeed.customMax) hàng tự thêm").foregroundColor(Theme.dim)
                }
                Button("Khôi phục mặc định") {
                    Prefs.homeRows = []
                    reload()
                }
            } footer: {
                Text("Hàng tự thêm: chủ đề có sẵn, hoặc tự chọn loại phim, nước, thể loại, ngôn ngữ. «Khôi phục mặc định» trả thứ tự và bật lại mọi hàng, giữ hàng tự thêm.")
            }
        }
        .navigationTitle("Các hàng trên trang chủ")
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarHidden(false)
        .onAppear { reload() }
        .overlay {
            if needPin && !unlocked {
                ParentPinView(title: "Mã PIN phụ huynh để sửa các hàng trang chủ", onCancel: { dismiss() }) { unlocked = true }
            }
        }
        .alert("Xoá hàng tự thêm?", isPresented: Binding(get: { confirmDelete != nil }, set: { if !$0 { confirmDelete = nil } }),
               presenting: confirmDelete) { c in
            Button("Xoá", role: .destructive) { remove(c) }
            Button("Huỷ", role: .cancel) {}
        } message: { c in
            Text("«\(HomeFeed.customTitle(c))»")
        }
    }

    private func row(_ r: RowItem, _ others: [RowItem]) -> some View {
        let at = others.firstIndex { $0.id == r.id } ?? 0
        let head = HomeFeed.headRows.contains(r.id)
        return HStack(spacing: 16) {
            Button {
                toggle(r.id)
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: r.on ? "checkmark.circle.fill" : "circle")
                        .foregroundColor(r.on ? Theme.brand : Theme.dim)
                    Text((r.id.hasPrefix("custom-") ? "＋ " : "") + label(r.id))
                        .foregroundColor(r.on ? Theme.text : Theme.dim)
                        .lineLimit(2)
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .accessibilityLabel((r.on ? "Đang bật: " : "Đang tắt: ") + label(r.id))
            if !head {
                Button { move(r.id, up: true) } label: { Image(systemName: "chevron.up") }
                    .disabled(at == 0)
                    .accessibilityLabel("Lên: " + label(r.id))
                Button { move(r.id, up: false) } label: { Image(systemName: "chevron.down") }
                    .disabled(at == others.count - 1)
                    .accessibilityLabel("Xuống: " + label(r.id))
                if r.id.hasPrefix("custom-") {
                    Button { confirmDelete = custom.first { $0.key == r.id } } label: {
                        Image(systemName: "trash").foregroundColor(.red)
                    }
                    .accessibilityLabel("Xoá: " + label(r.id))
                }
            }
        }
        .buttonStyle(.borderless)
    }
}

/// «Thêm hàng…»: chủ đề có sẵn, hoặc tự chọn loại phim / nước / thể loại / ngôn ngữ. Hồ sơ trẻ em chỉ hoạt hình.
struct AddHomeRowView: View {
    let existing: [CustomRow]
    let onAdd: (CustomRow) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var type = "hoathinh"
    @State private var country = ""
    @State private var genre = ""
    @State private var lang = ""

    private var kids: Bool { Prefs.kids }

    private var row: CustomRow { CustomRow(id: "c\(nowMs())", type: type, country: country, genre: genre, lang: lang) }

    private func add(_ c: CustomRow) {
        onAdd(c)
        dismiss()
    }

    var body: some View {
        let topics = HomeFeed.topics.filter { t in
            (!kids || HomeFeed.kidsTopics.contains(t.id)) && !existing.contains { $0.topic == t.id }
        }
        let types = kids ? Array(HomeFeed.types.prefix(1)) : HomeFeed.types
        let genres = ApiSite.genreList.filter { !kids || !HomeFeed.adultGenres.contains($0.1) }
        Form {
            if !topics.isEmpty {
                Section {
                    ForEach(Array(topics.enumerated()), id: \.offset) { _, t in
                        Button(t.label) { add(CustomRow(id: "t-\(t.id)", topic: t.id)) }
                    }
                } header: {
                    Text("Chủ đề có sẵn")
                }
            }
            Section {
                Picker("Loại phim", selection: $type) {
                    ForEach(Array(types.enumerated()), id: \.offset) { _, t in Text(t.label).tag(t.id) }
                }
                Picker("Nước", selection: $country) {
                    Text("Mọi nước").tag("")
                    ForEach(Array(ApiSite.countryList.enumerated()), id: \.offset) { _, c in Text(c.0).tag(c.1) }
                }
                Picker("Thể loại", selection: $genre) {
                    Text("Mọi thể loại").tag("")
                    ForEach(Array(genres.enumerated()), id: \.offset) { _, g in Text(g.0).tag(g.1) }
                }
                Picker("Ngôn ngữ", selection: $lang) {
                    ForEach(Array(HomeFeed.langs.enumerated()), id: \.offset) { _, l in Text(l.label).tag(l.id) }
                }
                Button {
                    add(row)
                } label: {
                    Label("Thêm hàng «\(HomeFeed.customTitle(row))»", systemImage: "plus.circle.fill")
                }
            } header: {
                Text("Tự chọn")
            } footer: {
                Text("Phim lấy từ KKPhim (lọc được cả bốn mục), không có KKPhim thì VSMOV; phim cùng tên ở các trang khác tự gộp vào.")
            }
        }
        .navigationTitle("Thêm hàng")
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarHidden(false)
    }
}
