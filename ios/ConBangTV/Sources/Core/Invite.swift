import Foundation

/// Cài bản TV lên tivi bằng app Downloader (AFTVnews): người mới chỉ cần cài Downloader trên tivi rồi nhập `code`. `apkUrl`:
/// link cố định luôn tải bản TV mới nhất (tệp ConBangTV.apk); mã do aftv.news cấp trỏ vào link tên cũ …/ChimToTV.apk — mọi
/// release TV vẫn đính kèm cả tệp đó. Cùng giá trị với bản TV (Invite.kt), Windows (Invite.cs), release.ps1 và README (mục «Cài
/// trên Android TV bằng Downloader») — đổi mã / link thì sửa đủ các chỗ đó.
enum Invite {
    static let code = "9192365"
    static let shortUrl = "aftv.news/\(code)"
    static let apkUrl = "https://github.com/Circling5714/ChimToTV/releases/latest/download/ConBangTV.apk"

    /// Trang hướng dẫn tiếng Việt (mục Downloader của README).
    static let guideUrl = "https://github.com/Circling5714/ChimToTV/blob/main/README.md#downloader"

    /// Tin nhắn gửi cho người quen — cùng nội dung ở cả ba bản.
    static let message = [
        "Cài Côn Bằng TV trên tivi Android:",
        "1. Trên tivi mở CH Play, cài app Downloader (của AFTVnews).",
        "2. Mở Downloader, nhập \(code) rồi bấm Go.",
        "3. Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.",
        "Cài xong app tự cập nhật các bản sau.",
        "Hướng dẫn: \(guideUrl)",
    ].joined(separator: "\n")
}
