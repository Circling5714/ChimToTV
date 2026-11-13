using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// Mã Downloader + tin nhắn gửi người quen (Invite.cs) — cùng ca với InviteTest.kt của bản TV và InviteTests.swift của bản iPad.
public class InviteTests
{
    [Fact]
    public void CodeAndLinks()
    {
        Assert.Matches(@"^\d{5,9}$", Invite.Code);
        Assert.Equal("aftv.news/" + Invite.Code, Invite.ShortUrl);
        // Link cố định: release đang là Latest, tệp tên cố định (release.ps1 đính kèm ở mọi bản TV)
        Assert.EndsWith("/releases/latest/download/ConBangTV.apk", Invite.ApkUrl);
        Assert.StartsWith("https://github.com/Circling5714/ChimToTV/", Invite.GuideUrl);
    }

    /// Ba bản gửi cùng một tin nhắn — sửa câu chữ thì sửa cả ba bản và cả ba test.
    [Fact]
    public void MessageIsTheSameOnEveryPlatform()
    {
        Assert.Equal(
            "Cài Côn Bằng TV trên tivi Android:\n" +
            "1. Trên tivi mở CH Play, cài app Downloader (của AFTVnews).\n" +
            "2. Mở Downloader, nhập 9192365 rồi bấm Go.\n" +
            "3. Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.\n" +
            "Cài xong app tự cập nhật các bản sau.\n" +
            "Hướng dẫn: https://github.com/Circling5714/ChimToTV/blob/main/README.md#downloader",
            Invite.Message);
    }
}
