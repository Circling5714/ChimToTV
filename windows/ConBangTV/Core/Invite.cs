namespace ConBangTV.Core;

/// Cài bản TV lên tivi bằng app Downloader (AFTVnews): người mới chỉ cần cài Downloader trên tivi rồi nhập Code. Mã do aftv.news
/// cấp, trỏ vào ApkUrl — link cố định luôn tải bản TV mới nhất. Cùng giá trị với bản TV (Invite.kt), iPad (Invite.swift),
/// release.ps1 và README (mục «Cài trên Android TV bằng Downloader») — đổi mã / link thì sửa đủ các chỗ đó.
public static class Invite
{
    public const string Code = "9192365";
    public const string ShortUrl = "aftv.news/" + Code;
    public const string ApkUrl = "https://github.com/Circling5714/ChimToTV/releases/latest/download/ChimToTV.apk";

    /// Trang hướng dẫn tiếng Việt (mục Downloader của README).
    public const string GuideUrl = "https://github.com/Circling5714/ChimToTV/blob/main/README.md#downloader";

    /// Tin nhắn gửi cho người quen — cùng nội dung ở cả ba bản. Ghép bằng "\n" (không dùng chuỗi nhiều dòng: xuống dòng trong
    /// tệp nguồn trên Windows là CRLF).
    public static readonly string Message = string.Join("\n",
        "Cài Côn Bằng TV trên tivi Android:",
        "1. Trên tivi mở CH Play, cài app Downloader (của AFTVnews).",
        $"2. Mở Downloader, nhập {Code} rồi bấm Go.",
        "3. Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.",
        "Cài xong app tự cập nhật các bản sau.",
        $"Hướng dẫn: {GuideUrl}");
}
