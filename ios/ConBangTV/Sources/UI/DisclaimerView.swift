import SwiftUI
import UIKit

/// Màn miễn trừ trách nhiệm (nội dung: Disclaimer). gate = true: mở app lần đầu / sau khi cập nhật lên bản có màn này —
/// «Đồng ý» mới vào app, «Thoát» đưa app về nền (lần mở sau vẫn hỏi). gate = false: Cài đặt → Giới thiệu, chỉ đọc.
struct DisclaimerView: View {
    var gate: Bool
    var onAgree: () -> Void = {}
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(gate ? "Trước khi bắt đầu" : Disclaimer.title)
                .appFont(26, .bold)
                .foregroundColor(Theme.text)
            Text(gate ? "Côn Bằng TV — \(Disclaimer.title). Đọc hết rồi chọn «Đồng ý» để vào app."
                 : "Côn Bằng TV · bản \(Disclaimer.version) – \(Disclaimer.date)")
                .appFont(15)
                .foregroundColor(Theme.dim)
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    ForEach(Disclaimer.sections, id: \.title) { s in
                        VStack(alignment: .leading, spacing: 4) {
                            Text(s.title).appFont(17, .semibold).foregroundColor(Theme.focus)
                            Text(s.body).appFont(16).foregroundColor(Theme.text).fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    Text("\(Disclaimer.consent)  (bản \(Disclaimer.version) – \(Disclaimer.date))")
                        .appFont(14)
                        .italic()
                        .foregroundColor(Theme.dim)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
            }
            .background(RoundedRectangle(cornerRadius: 12).fill(Theme.surface2))
            HStack(spacing: 12) {
                if gate {
                    Button { onAgree() } label: { Text("Đồng ý").appFont(17, .semibold).frame(minWidth: 120) }
                        .buttonStyle(.borderedProminent)
                        .tint(Theme.brand)
                        .keyboardShortcut(.defaultAction)
                    Button {
                        // iOS không cho app tự đóng: đưa về màn hình chính, lần mở sau vẫn hỏi
                        UIControl().sendAction(#selector(URLSessionTask.suspend), to: UIApplication.shared, for: nil)
                    } label: { Text("Thoát").appFont(17).frame(minWidth: 90) }
                        .buttonStyle(.bordered)
                } else {
                    Button { dismiss() } label: { Text("Đóng").appFont(17, .semibold).frame(minWidth: 120) }
                        .buttonStyle(.borderedProminent)
                        .tint(Theme.brand)
                }
            }
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 20)
        .frame(maxWidth: 900)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.bg.ignoresSafeArea())
        .interactiveDismissDisabled(gate)
    }
}

/// Mở app lần đầu (hoặc vừa cập nhật lên bản có màn miễn trừ): phủ toàn màn, «Đồng ý» xong mới bắt đầu (AppModel.startIfNeeded
/// chờ cờ này). Gắn bên ngoài RootView — RootView đã có fullScreenCover riêng cho «Ai đang xem?».
struct DisclaimerGate: ViewModifier {
    @ObservedObject var app = AppModel.shared

    func body(content: Content) -> some View {
        content.fullScreenCover(isPresented: $app.needsDisclaimer) {
            DisclaimerView(gate: true) { app.acceptDisclaimer() }
                .appDynamicType()
                .preferredColorScheme(.dark)
        }
    }
}
