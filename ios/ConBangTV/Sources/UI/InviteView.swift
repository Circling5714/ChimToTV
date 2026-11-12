import SwiftUI
import UIKit

/// Cài bản TV lên tivi bằng app Downloader (Cài đặt → Liên hệ): mã thật to, ba bước, nút gửi hướng dẫn cho người quen qua
/// bảng chia sẻ của hệ thống. Bản TV (InviteActivity) có thêm mã QR cho khách chụp; trên iPad / iPhone thì gửi thẳng tin nhắn.
struct InviteView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.horizontalSizeClass) private var hSize
    @Environment(\.verticalSizeClass) private var vSize
    @State private var sharing = false
    @State private var copied = false

    /// iPhone cầm dọc (và iPad chia đôi màn hình): ba nút không đủ chỗ trên một hàng — màn rộng 375 pt thì nội dung tràn sát
    /// hai mép, 320 pt hoặc cỡ chữ lớn thì bị cắt (thấy trên máy ảo iPhone SE, 13/11/2026) → nút gửi một hàng riêng, hai nút
    /// kia chia đôi hàng dưới. Màn ngang / iPad: một hàng như cũ.
    private var narrow: Bool { hSize == .compact && vSize != .compact }

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
            if narrow {
                VStack(spacing: 10) {
                    shareButton
                    HStack(spacing: 10) {
                        copyButton
                        closeButton
                    }
                }
            } else {
                HStack(spacing: 12) {
                    shareButton
                    copyButton
                    closeButton
                }
            }
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 20)
        .frame(maxWidth: 900)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.bg.ignoresSafeArea())
        .sheet(isPresented: $sharing) { ActivityView(items: [Invite.message]) }
    }

    /// Chữ trên nút: màn hẹp thì nút giãn hết bề ngang được chia, màn rộng thì giữ bề ngang tối thiểu; chữ không xuống dòng,
    /// thiếu chỗ (cỡ chữ lớn) thì thu nhỏ bớt.
    private func label(_ text: String, minWidth: CGFloat, bold: Bool = false) -> some View {
        Text(text)
            .appFont(17, bold ? .semibold : .regular)
            .lineLimit(1)
            .minimumScaleFactor(0.7)
            .frame(minWidth: narrow ? nil : minWidth, maxWidth: narrow ? .infinity : nil)
    }

    private var shareButton: some View {
        Button { sharing = true } label: { label("Gửi hướng dẫn…", minWidth: 140, bold: true) }
            .buttonStyle(.borderedProminent)
            .tint(Theme.brand)
    }

    private var copyButton: some View {
        Button {
            UIPasteboard.general.string = Invite.message
            copied = true
        } label: { label(copied ? "Đã chép" : "Chép", minWidth: 70) }
            .buttonStyle(.bordered)
    }

    private var closeButton: some View {
        Button { dismiss() } label: { label("Đóng", minWidth: 70) }
            .buttonStyle(.bordered)
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
