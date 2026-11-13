//! Cài bản TV lên tivi bằng app Downloader (AFTVnews): người mới chỉ cần cài Downloader trên tivi rồi nhập Code. APK_URL: link
//! cố định luôn tải bản TV mới nhất (tệp ConBangTV.apk); mã do aftv.news cấp trỏ vào link tên cũ …/ChimToTV.apk — mọi release TV
//! vẫn đính kèm cả tệp đó. Cùng giá trị với bản TV (Invite.kt), iPad (Invite.swift), Windows (Invite.cs), release.ps1 và README
//! (mục «Cài trên Android TV bằng Downloader») — đổi mã / link thì sửa đủ các chỗ đó.

pub const CODE: &str = "9192365";
pub const SHORT_URL: &str = "aftv.news/9192365";
pub const APK_URL: &str = "https://github.com/Circling5714/ChimToTV/releases/latest/download/ConBangTV.apk";

/// Trang hướng dẫn tiếng Việt (mục Downloader của README).
pub const GUIDE_URL: &str = "https://github.com/Circling5714/ChimToTV/blob/main/README.md#downloader";

/// Tin nhắn gửi cho người quen — cùng nội dung ở mọi bản. Ghép bằng "\n".
pub fn message() -> String {
    [
        "Cài Côn Bằng TV trên tivi Android:".to_string(),
        "1. Trên tivi mở CH Play, cài app Downloader (của AFTVnews).".to_string(),
        format!("2. Mở Downloader, nhập {CODE} rồi bấm Go."),
        "3. Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.".to_string(),
        "Cài xong app tự cập nhật các bản sau.".to_string(),
        format!("Hướng dẫn: {GUIDE_URL}"),
    ]
    .join("\n")
}

/// Mã chia nhóm cho dễ đọc trên màn hình ("9 1 9 2 3 6 5").
pub fn spaced_code() -> String {
    CODE.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")
}

/// Dòng nhỏ dưới mã: link rút gọn + link tải trực tiếp.
pub fn links_line() -> String {
    format!("Trên trình duyệt: {SHORT_URL}  •  Link tải trực tiếp: {APK_URL}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Cùng ca với InviteTests.cs (Windows), InviteTest.kt (TV), InviteTests.swift (iPad).

    #[test]
    fn code_and_links() {
        assert!((5..=9).contains(&CODE.len()) && CODE.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(SHORT_URL, format!("aftv.news/{CODE}"));
        // Link cố định: release đang là Latest, tệp tên cố định (release.ps1 đính kèm ở mọi bản TV)
        assert!(APK_URL.ends_with("/releases/latest/download/ConBangTV.apk"));
        assert!(GUIDE_URL.starts_with("https://github.com/Circling5714/ChimToTV/"));
        assert_eq!(spaced_code(), "9 1 9 2 3 6 5");
    }

    /// Mọi bản gửi cùng một tin nhắn — sửa câu chữ thì sửa cả các bản và cả các test.
    #[test]
    fn message_is_the_same_on_every_platform() {
        assert_eq!(
            message(),
            "Cài Côn Bằng TV trên tivi Android:\n\
             1. Trên tivi mở CH Play, cài app Downloader (của AFTVnews).\n\
             2. Mở Downloader, nhập 9192365 rồi bấm Go.\n\
             3. Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.\n\
             Cài xong app tự cập nhật các bản sau.\n\
             Hướng dẫn: https://github.com/Circling5714/ChimToTV/blob/main/README.md#downloader"
        );
    }
}
