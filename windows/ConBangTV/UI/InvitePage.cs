using System.Windows;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Cài bản TV lên tivi bằng app Downloader (Cài đặt → Giới thiệu): mã thật to, ba bước, nút chép hướng dẫn để gửi người quen.
/// Bản TV (InviteActivity) có thêm mã QR cho khách chụp; trên máy tính thì chép chữ dán vào Zalo / Messenger tiện hơn.
public sealed class InvitePage : Page
{
    public InvitePage()
    {
        MenuKey = "settings";
    }

    protected override Task Load()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Cài Côn Bằng TV lên tivi"));
        Body.Children.Add(Ui.Dim("Tivi / box Android nào cũng cài được bằng app Downloader — không cần USB hay máy tính.",
            14, new Thickness(0, 0, 0, 16)));
        Body.Children.Add(Ui.Text("1.  Trên tivi, mở CH Play và cài app Downloader (của AFTVnews).", 17));
        Body.Children.Add(Ui.Text("2.  Mở Downloader, nhập mã này rồi bấm Go:", 17, margin: new Thickness(0, 12, 0, 0)));
        Body.Children.Add(Ui.Text(string.Join(" ", Invite.Code.ToCharArray()), 56, Ui.Res("Focus"), FontWeights.Bold, wrap: false,
            margin: new Thickness(26, 0, 0, 4)));
        Body.Children.Add(Ui.Text("3.  Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.", 17));
        Body.Children.Add(Ui.Dim("Cài xong, các bản sau app tự cập nhật.", 14, new Thickness(0, 12, 0, 18)));
        Body.Children.Add(Ui.Row(
            Ui.Pill("Chép hướng dẫn để gửi bạn bè", () => Copy(Invite.Message, "Đã chép hướng dẫn — dán vào Zalo, tin nhắn… để gửi.")),
            Ui.Pill("Chép mã", () => Copy(Invite.Code, $"Đã chép mã {Invite.Code}.")),
            Ui.Pill("Mở trang hướng dẫn", () => Ui.OpenBrowser(Invite.GuideUrl))));
        Body.Children.Add(Ui.Dim($"Trên trình duyệt: {Invite.ShortUrl}  •  Link tải trực tiếp: {Invite.ApkUrl}", 13,
            new Thickness(0, 10, 0, 0)));
        return Task.CompletedTask;
    }

    /// Bộ nhớ tạm của Windows có lúc đang bị app khác giữ — báo để bấm lại chứ không để app văng.
    private static void Copy(string text, string done)
    {
        try
        {
            Clipboard.SetDataObject(text, true);
            Main.Toast(done);
        }
        catch (Exception)
        {
            Main.Toast("Chưa chép được vào bộ nhớ tạm — bấm lại lần nữa.");
        }
    }
}
