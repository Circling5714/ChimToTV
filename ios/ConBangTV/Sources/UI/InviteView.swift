import SwiftUI
import UIKit

/// Cài bản TV lên tivi bằng app Downloader (Cài đặt → Liên hệ): mã thật to, ba bước, nút gửi hướng dẫn cho người quen qua
/// bảng chia sẻ của hệ thống. Bản TV (InviteActivity) có thêm mã QR cho khách chụp; trên iPad / iPhone thì gửi thẳng tin nhắn.
struct InviteView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var sharing = false
    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Cài Côn Bằng TV lên tivi")
                .appFont(26, .bold)
                .foregroundColor(Theme.text)
            Text("Tivi / box Android nào cũng cài được bằng app Downloader — không cần USB hay máy tính.")
                .appFont(15)
                .foregroundColor(Theme.dim)
                .fixedSize(horizontal: false, vertical: true)
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    step("1.  Trên tivi, mở CH Play và cài app Downloader (của AFTVnews).")
                    step("2.  Mở Downloader, nhập mã này rồi bấm Go:")
                    Text(Invite.code.map { String($0) }.joined(separator: " "))
                        .appFont(50, .bold)
                        .foregroundColor(Theme.focus)
                        .lineLimit(1)
                        .minimumScaleFactor(0.5)
                        .textSelection(.enabled)
                    step("3.  Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.")
                    Text("Cài xong, các bản sau app tự cập nhật.")
                        .appFont(15)
                        .foregroundColor(Theme.dim)
                    Text("Trên trình duyệt: \(Invite.shortUrl)")
                        .appFont(14)
                        .foregroundColor(Theme.dim)
                        .textSelection(.enabled)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
            }
            .background(RoundedRectangle(cornerRadius: 12).fill(Theme.surface2))
            HStack(spacing: 12) {
                Button { sharing = true } label: { Text("Gửi hướng dẫn…").appFont(17, .semibold).frame(minWidth: 140) }
                    .buttonStyle(.borderedProminent)
                    .tint(Theme.brand)
                Button {
                    UIPasteboard.general.string = Invite.message
                    copied = true
                } label: { Text(copied ? "Đã chép" : "Chép").appFont(17).frame(minWidth: 70) }
                    .buttonStyle(.bordered)
                Button { dismiss() } label: { Text("Đóng").appFont(17).frame(minWidth: 70) }
                    .buttonStyle(.bordered)
            }
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 20)
        .frame(maxWidth: 900)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.bg.ignoresSafeArea())
        .sheet(isPresented: $sharing) { ActivityView(items: [Invite.message]) }
    }

    private func step(_ text: String) -> some View {
        Text(text)
            .appFont(18)
            .foregroundColor(Theme.text)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// Bảng chia sẻ của hệ thống (Zalo, Tin nhắn, AirDrop…) — ShareLink của SwiftUI chỉ có từ iOS 16, app chạy từ iOS 15.
struct ActivityView: UIViewControllerRepresentable {
    let items: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
