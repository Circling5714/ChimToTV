import SwiftUI

/// Mã PIN phụ huynh (giới hạn giờ xem — WatchLimit): có mã thì hỏi mã; chưa có (hoặc đang đổi mã) thì đặt mã mới, nhập
/// hai lần. onDone chạy khi nhập đúng / đặt xong.
struct ParentPinView: View {
    var title = "Mã PIN phụ huynh"
    /// true: đặt mã mới dù đã có mã (Đổi mã PIN phụ huynh — đã nhập mã cũ trước đó).
    var changing = false
    let onCancel: () -> Void
    let onDone: () -> Void

    @State private var step = 1
    @State private var first = ""
    @State private var mismatch = false

    var body: some View {
        if Prefs.hasParentPin && !changing {
            PinPad(title: title, onCancel: onCancel) { pin in
                guard Prefs.checkParentPin(pin) else { return false }
                onDone()
                return true
            }
        } else {
            PinPad(title: step == 1 ? (changing ? "Mã PIN phụ huynh mới" : "Đặt mã PIN phụ huynh") : "Nhập lại mã PIN phụ huynh",
                   subtitle: step == 1
                       ? (mismatch ? "Hai lần nhập không khớp — nhập lại"
                           : "Dùng để đặt / đổi giới hạn giờ xem và cho xem thêm khi hết giờ; khi đã có giới hạn, thêm / xoá người xem cũng phải nhập mã này. Nên khác mã PIN của từng người xem.")
                       : nil,
                   onCancel: onCancel) { pin in
                if step == 1 {
                    first = pin
                    step = 2
                    return true
                }
                guard pin == first else {
                    mismatch = true
                    step = 1 // không khớp: nhập lại từ đầu
                    return false
                }
                Prefs.setParentPin(pin)
                onDone()
                return true
            }
            .id(step) // bước mới: bàn phím mới (xoá số cũ)
        }
    }
}

/// Cài đặt → Giới hạn giờ xem…: mỗi người xem — số phút được xem mỗi ngày, giờ đi ngủ (tới 6 giờ sáng). Mở phải nhập
/// mã PIN phụ huynh (chưa có thì đặt trước). Bản Android: ProfileUi.limits.
struct LimitsView: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var unlocked = false
    @State private var changingPin = false
    @State private var version = 0

    var body: some View {
        NavigationView {
            Group {
                if unlocked {
                    form
                } else {
                    Theme.bg
                }
            }
            .navigationTitle("Giới hạn giờ xem")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Xong") { dismiss() } } }
        }
        .navigationViewStyle(.stack)
        .overlay {
            if !unlocked {
                ParentPinView(onCancel: { dismiss() }, onDone: { unlocked = true })
            } else if changingPin {
                ParentPinView(changing: true, onCancel: { changingPin = false }) {
                    changingPin = false
                    app.showToast("Đã đổi mã PIN phụ huynh.")
                }
            }
        }
    }

    private var form: some View {
        Form {
            ForEach(app.profiles) { p in
                Section {
                    Picker("Mỗi ngày được xem", selection: daily(p)) {
                        ForEach(WatchLimit.daily, id: \.self) { Text(WatchLimit.dailyLabel($0)).tag($0) }
                    }
                    Picker("Giờ đi ngủ (không xem được tới 6 giờ sáng)", selection: bedtime(p)) {
                        ForEach(WatchLimit.bedtimes, id: \.self) { Text(WatchLimit.bedtimeLabel($0)).tag($0) }
                    }
                    if p.id == Prefs.profileId, let r = WatchLimit.remaining() {
                        Text("Hôm nay còn \(r > 0 ? WatchLimit.minutesText(r) : "0 phút")").foregroundColor(Theme.dim)
                    }
                } header: {
                    HStack(spacing: 8) {
                        ProfileAvatar(profile: p, size: 22)
                        Text(p.name)
                    }
                }
            }
            Section {
                Button("Đổi mã PIN phụ huynh") { changingPin = true }
                if !Prefs.anyLimits {
                    Button("Xoá mã PIN phụ huynh", role: .destructive) {
                        Prefs.setParentPin(nil)
                        dismiss()
                    }
                }
            } footer: {
                Text("Trình phát chỉ đếm lúc phim thật sự chạy. Còn 5 phút thì nhắc; hết giờ thì phim dừng, người lớn nhập mã PIN phụ huynh để cho xem thêm. Người lớn nên đặt mã PIN cho hồ sơ của mình (Cài đặt → Người xem) để trẻ không chuyển sang được.")
            }
        }
        .id(version)
    }

    private func daily(_ p: Profile) -> Binding<Int> {
        Binding(get: { Prefs.dailyLimit(p.id) }, set: {
            Prefs.setDailyLimit(p.id, $0)
            version += 1
        })
    }

    private func bedtime(_ p: Profile) -> Binding<Int> {
        Binding(get: { Prefs.bedtime(p.id) }, set: {
            Prefs.setBedtime(p.id, $0)
            version += 1
        })
    }
}
