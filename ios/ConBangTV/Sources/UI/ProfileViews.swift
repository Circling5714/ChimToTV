import SwiftUI

/// Người xem trên iPad (bản Android: ProfileUi.kt): vòng tròn màu, màn "Ai đang xem?", bàn phím số nhập mã PIN, thêm /
/// sửa người xem.
extension Theme {
    static func profileColor(_ index: Int) -> Color {
        let n = Prefs.profileColors.count
        return Color(hex: Prefs.profileColors[((index % n) + n) % n])
    }
}

/// Vòng tròn màu + chữ cái đầu tên; nil = ô "＋" (thêm người).
struct ProfileAvatar: View {
    let profile: Profile?
    var size: CGFloat = 96

    var body: some View {
        ZStack {
            Circle().fill(profile.map { Theme.profileColor($0.color) } ?? Theme.surface2)
            if let p = profile {
                Text(p.initial).font(.system(size: size * 0.42, weight: .bold)).foregroundColor(.white)
            } else {
                Image(systemName: "plus").font(.system(size: size * 0.36, weight: .semibold)).foregroundColor(Theme.text)
            }
        }
        .frame(width: size, height: size)
    }
}

/// Màn "Ai đang xem?" (phủ kín màn hình): mỗi người một ô, ô "＋ Thêm người", nút sửa người xem.
struct ProfilePickerView: View {
    @EnvironmentObject var app: AppModel
    @State private var pinFor: Profile?
    @State private var adding = false
    @State private var settings = false
    @State private var parentForAdd = false

    /// Cột cố định (tối đa 4 ô một hàng — vừa cả iPad dọc): lưới nằm giữa màn hình. Màn hẹp (iPhone) ô nhỏ hơn.
    private func cols(_ width: CGFloat) -> [GridItem] {
        let n = app.profiles.count + (app.profiles.count < Prefs.maxProfiles ? 1 : 0)
        let tile: CGFloat = width < 600 ? 118 : 150
        let spacing: CGFloat = width < 600 ? 12 : 24
        let fit = Int((width - 40 + spacing) / (tile + spacing))
        return Array(repeating: GridItem(.fixed(tile), spacing: spacing), count: max(1, min(4, n, fit)))
    }

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()
            GeometryReader { geo in
            let narrow = geo.size.width < 600
            ScrollView {
            VStack(spacing: 12) {
                Text("Ai đang xem?").appFont(narrow ? 30 : 38, .bold).foregroundColor(Theme.text)
                Text("Mỗi người có lịch sử xem, Xem tiếp, phim theo dõi và cài đặt riêng.")
                    .appFont(16).foregroundColor(Theme.dim).multilineTextAlignment(.center)
                LazyVGrid(columns: cols(geo.size.width), spacing: narrow ? 14 : 26) {
                    ForEach(app.profiles) { p in
                        Button {
                            if p.locked { pinFor = p } else { app.pickProfile(p) }
                        } label: { tile(p, avatar: narrow ? 84 : 108) }
                        .buttonStyle(PressScale())
                        .accessibilityLabel(p.name)
                    }
                    if app.profiles.count < Prefs.maxProfiles {
                        Button {
                            // Đã có giới hạn giờ xem: thêm người cần mã PIN phụ huynh
                            if Prefs.anyLimits && Prefs.hasParentPin { parentForAdd = true } else { adding = true }
                        } label: { tile(nil, avatar: narrow ? 84 : 108) }
                            .buttonStyle(PressScale())
                            .accessibilityLabel("Thêm người")
                    }
                }
                .padding(.vertical, narrow ? 14 : 26)
                Button { settings = true } label: {
                    Label("Sửa người xem…", systemImage: "pencil")
                        .appFont(16, .medium)
                        .foregroundColor(Theme.text)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 11)
                        .background(RoundedRectangle(cornerRadius: 10).fill(Theme.surface2))
                }
                .buttonStyle(PressScale())
            }
            .padding(narrow ? 20 : 30)
            .frame(maxWidth: .infinity, minHeight: geo.size.height) // giữa màn hình; màn thấp (iPhone ngang) thì cuộn
            }
            }
            // Mở từ menu (không phải lúc mở app), hoặc người đang dùng không khoá: được đóng, xem tiếp bằng người đang dùng
            if app.pickerClosable {
                VStack {
                    HStack {
                        Spacer()
                        Button { app.closePicker() } label: {
                            Image(systemName: "xmark.circle.fill").appFont(32).foregroundColor(Theme.dim)
                        }
                        .accessibilityLabel("Đóng")
                        .padding(24)
                    }
                    Spacer()
                }
            }
            if parentForAdd {
                ParentPinView(title: "Mã PIN phụ huynh để thêm người xem", onCancel: { parentForAdd = false }) {
                    parentForAdd = false
                    adding = true
                }
            }
            if let p = pinFor {
                PinPad(title: "Mã PIN của \(p.name)", onCancel: { pinFor = nil }) { pin in
                    guard Prefs.checkPin(p, pin) else { return false }
                    pinFor = nil
                    app.pickProfile(p)
                    return true
                }
            }
        }
        .preferredColorScheme(.dark)
        .sheet(isPresented: $adding) { ProfileEditView(editing: nil).environmentObject(app) }
        .sheet(isPresented: $settings, onDismiss: { app.refreshProfiles() }) {
            SettingsView().environmentObject(app).preferredColorScheme(.dark)
        }
    }

    private func tile(_ p: Profile?, avatar: CGFloat) -> some View {
        VStack(spacing: 12) {
            ProfileAvatar(profile: p, size: avatar)
            HStack(spacing: 6) {
                Text(p?.name ?? "Thêm người")
                    .appFont(19, .semibold)
                    .foregroundColor(p == nil ? Theme.dim : Theme.text)
                    .lineLimit(1)
                if p?.locked == true { Image(systemName: "lock.fill").appFont(14).foregroundColor(Theme.dim) }
            }
        }
        .padding(14)
        .contentShape(Rectangle())
    }
}

/// Bàn phím số nhập mã PIN (4 số): đủ số là kiểm luôn; check trả false = sai (rung, xoá để nhập lại).
struct PinPad: View {
    let title: String
    var subtitle: String?
    let onCancel: () -> Void
    let check: (String) -> Bool

    @State private var digits = ""
    @State private var wrong = 0
    /// iPhone nằm ngang: màn thấp — chữ + chấm bên trái, bàn phím số bên phải.
    @Environment(\.verticalSizeClass) private var vClass

    init(title: String, subtitle: String? = nil, onCancel: @escaping () -> Void, check: @escaping (String) -> Bool) {
        self.title = title
        self.subtitle = subtitle
        self.onCancel = onCancel
        self.check = check
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.6).ignoresSafeArea().onTapGesture { onCancel() }
            if vClass == .compact {
                HStack(spacing: 28) {
                    VStack(spacing: 14) { header }
                        .frame(maxWidth: 260)
                    keypad(size: 54, spacing: 8)
                }
                .padding(20)
                .background(RoundedRectangle(cornerRadius: 22).fill(Theme.surface))
            } else {
                VStack(spacing: 18) {
                    header
                    keypad(size: 76, spacing: 12)
                }
                .padding(28)
                .frame(maxWidth: 380)
                .background(RoundedRectangle(cornerRadius: 22).fill(Theme.surface))
            }
        }
        .background(PinKeys(press: press, delete: { if !digits.isEmpty { digits.removeLast() } }, cancel: onCancel))
    }

    @ViewBuilder
    private var header: some View {
                Text(title).appFont(22, .bold).foregroundColor(Theme.text).multilineTextAlignment(.center)
                if let s = subtitle {
                    Text(s).appFont(14).foregroundColor(Theme.dim).multilineTextAlignment(.center)
                }
                HStack(spacing: 18) {
                    ForEach(0..<Prefs.pinLength, id: \.self) { i in
                        Circle()
                            .fill(i < digits.count ? Theme.text : Color.clear)
                            .overlay(Circle().stroke(Theme.text, lineWidth: 2))
                            .frame(width: 18, height: 18)
                    }
                }
                .modifier(Shake(amount: CGFloat(wrong)))
                .animation(.linear(duration: 0.35), value: wrong)
                Text(wrong > 0 ? "Sai mã PIN" : " ").appFont(14, .medium).foregroundColor(.red)
    }

    private func keypad(size: CGFloat, spacing: CGFloat) -> some View {
        VStack(spacing: spacing) {
            ForEach([["1", "2", "3"], ["4", "5", "6"], ["7", "8", "9"], ["Huỷ", "0", "⌫"]], id: \.self) { row in
                HStack(spacing: spacing * 1.5) {
                    ForEach(row, id: \.self) { key in keyButton(key, size: size) }
                }
            }
        }
    }

    private func keyButton(_ key: String, size: CGFloat) -> some View {
        Button {
            switch key {
            case "Huỷ": onCancel()
            case "⌫": if !digits.isEmpty { digits.removeLast() }
            default: press(key)
            }
        } label: {
            Text(key)
                .font(.system(size: key.count > 1 ? 17 : size * 0.37, weight: .medium))
                .foregroundColor(Theme.text)
                .frame(width: size, height: size)
                .background(Circle().fill(key.count > 1 && key != "⌫" ? Color.clear : Theme.surface2))
        }
        .buttonStyle(PressScale())
        .accessibilityLabel(key == "⌫" ? "Xoá số" : key)
    }

    private func press(_ d: String) {
        guard digits.count < Prefs.pinLength else { return }
        digits += d
        guard digits.count == Prefs.pinLength else { return }
        let pin = digits
        if !check(pin) {
            wrong += 1
            digits = ""
        }
    }
}

/// Gõ mã PIN bằng bàn phím rời: số 0–9, Delete xoá số, Esc huỷ.
private struct PinKeys: View {
    let press: (String) -> Void
    let delete: () -> Void
    let cancel: () -> Void

    var body: some View {
        ZStack {
            ForEach(0..<10, id: \.self) { d in
                Button("\(d)") { press("\(d)") }.keyboardShortcut(KeyEquivalent(Character("\(d)")), modifiers: [])
            }
            Button("Xoá số", action: delete).keyboardShortcut(.delete, modifiers: [])
            Button("Huỷ", action: cancel).keyboardShortcut(.escape, modifiers: [])
        }
        .frame(width: 0, height: 0)
        .opacity(0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Rung ngang (nhập sai mã PIN).
struct Shake: GeometryEffect {
    var amount: CGFloat
    var animatableData: CGFloat {
        get { amount }
        set { amount = newValue }
    }

    func effectValue(size: CGSize) -> ProjectionTransform {
        ProjectionTransform(CGAffineTransform(translationX: 10 * sin(amount * .pi * 6), y: 0))
    }
}

/// Thêm (editing = nil) / sửa người xem: tên, màu, mã PIN, chuyển sang, xoá.
struct ProfileEditView: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let editing: Profile?

    @State private var name: String
    @State private var color: Int
    @State private var error = ""
    /// Đặt mã PIN: 1 = nhập mã mới, 2 = nhập lại
    @State private var pinStep = 0
    @State private var firstPin = ""
    @State private var mismatch = false
    @State private var confirmDelete = false
    @State private var parentForDelete = false
    @State private var locked: Bool

    init(editing: Profile?) {
        self.editing = editing
        _name = State(initialValue: editing?.name ?? "")
        // Người mới: màu chưa ai dùng (như Prefs.addProfile)
        let used = Set(Prefs.profiles.map { $0.color })
        _color = State(initialValue: editing?.color ?? (Prefs.profileColors.indices.first { !used.contains($0) } ?? 0))
        _locked = State(initialValue: editing?.locked ?? false)
    }

    var body: some View {
        NavigationView {
            Form {
                Section {
                    TextField("Tên (vd Bố, Mẹ, Bé Na)", text: $name)
                        .textInputAutocapitalization(.words)
                        .disableAutocorrection(true)
                    if !error.isEmpty { Text(error).foregroundColor(.red) }
                } header: { Text("Tên") }
                Section {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 52), spacing: 14)], spacing: 14) {
                        ForEach(Prefs.profileColors.indices, id: \.self) { i in
                            Button { color = i } label: {
                                Circle().fill(Theme.profileColor(i))
                                    .frame(width: 46, height: 46)
                                    .overlay(Circle().stroke(Color.white, lineWidth: color == i ? 3 : 0).padding(-4))
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel(Prefs.profileColorNames[i])
                        }
                    }
                    .padding(.vertical, 6)
                } header: { Text("Màu") }
                if let p = editing {
                    Section {
                        if locked {
                            Button("Đổi mã PIN") { startPin() }
                            Button("Tắt mã PIN") {
                                Prefs.setProfilePin(p.id, nil)
                                locked = false
                                app.refreshProfiles()
                            }
                        } else {
                            Button("Đặt mã PIN") { startPin() }
                        }
                    } header: { Text("Mã PIN") } footer: {
                        Text("Người có mã PIN phải nhập mã mới mở được hồ sơ (lúc mở app, lúc đổi người xem) và mới sửa / xoá được. Quên mã thì chỉ xoá app rồi cài lại mới mở được — mất cả lịch sử, cài đặt.")
                    }
                    Section {
                        if p.id != app.profileId {
                            Button("Chuyển sang \(p.name)") {
                                app.pickProfile(p)
                                dismiss()
                            }
                        }
                        if app.profiles.count > 1 {
                            Button("Xoá người xem này", role: .destructive) {
                                if Prefs.anyLimits && Prefs.hasParentPin { parentForDelete = true } else { confirmDelete = true }
                            }
                        }
                    }
                }
            }
            .navigationTitle(editing == nil ? "Thêm người xem" : "Sửa người xem")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Huỷ") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) { Button(editing == nil ? "Thêm" : "Lưu") { save() } }
            }
            .alert("Xoá «\(editing?.name ?? "")»?", isPresented: $confirmDelete) {
                Button("Xoá", role: .destructive) {
                    if let p = editing { app.deleteProfile(p) }
                    dismiss()
                }
                Button("Huỷ", role: .cancel) {}
            } message: {
                Text("Lịch sử xem, hàng Xem tiếp, chỗ đang xem, phim theo dõi và cài đặt riêng của \(editing?.name ?? "") sẽ bị xoá.")
            }
        }
        .navigationViewStyle(.stack)
        .overlay {
            if parentForDelete {
                ParentPinView(title: "Mã PIN phụ huynh để xoá người xem", onCancel: { parentForDelete = false }) {
                    parentForDelete = false
                    confirmDelete = true
                }
            }
            if pinStep > 0, let p = editing {
                PinPad(title: pinStep == 1 ? "Mã PIN mới cho \(p.name)" : "Nhập lại mã PIN",
                       subtitle: pinStep == 1 ? (mismatch ? "Hai lần nhập không khớp — nhập lại" : "\(Prefs.pinLength) số") : nil,
                       onCancel: { pinStep = 0 }) { pin in
                    if pinStep == 1 {
                        firstPin = pin
                        pinStep = 2
                        return true
                    }
                    guard pin == firstPin else {
                        mismatch = true
                        pinStep = 1 // không khớp: nhập lại từ đầu
                        return false
                    }
                    mismatch = false
                    Prefs.setProfilePin(p.id, pin)
                    locked = true
                    pinStep = 0
                    app.refreshProfiles()
                    return true
                }
                .id(pinStep) // nhập lại: bàn phím mới (xoá số cũ)
            }
        }
    }

    private func startPin() {
        firstPin = ""
        mismatch = false
        pinStep = 1
    }

    private func save() {
        let n = name.trimmed
        guard !n.isEmpty else {
            error = "Nhập tên người xem"
            return
        }
        guard !app.profiles.contains(where: { $0.name.caseInsensitiveCompare(n) == .orderedSame && $0.id != editing?.id }) else {
            error = "Đã có người tên này"
            return
        }
        if let p = editing {
            Prefs.renameProfile(p.id, n)
            Prefs.setProfileColor(p.id, color)
        } else {
            guard app.profiles.count < Prefs.maxProfiles else {
                error = "Tối đa \(Prefs.maxProfiles) người xem."
                return
            }
            let p = Prefs.addProfile(n)
            Prefs.setProfileColor(p.id, color)
            app.showToast("Đã thêm \(n). Đổi người xem: chạm tên người xem ở đầu menu, hoặc chọn lúc mở app.")
        }
        app.refreshProfiles()
        dismiss()
    }
}
