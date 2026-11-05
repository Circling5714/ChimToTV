using System.Windows;
using System.Windows.Controls;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Proxy SOCKS5 (bản TV: Cài đặt → Proxy SOCKS5): bật / tắt, địa chỉ, cổng, đăng nhập, DNS của proxy hay của máy, toàn app
/// hay chỉ trang phim. Proxy lỗi thì báo lỗi, không lặng lẽ đi thẳng. Đổi xong có hiệu lực ngay (cả trình phát).
public sealed class ProxyPage : Page
{
    private readonly TextBox _host = new() { MinWidth = 300 };
    private readonly TextBox _port = new() { MinWidth = 110 };
    private readonly TextBox _user = new() { MinWidth = 300 };
    private readonly PasswordBox _pass = new() { MinWidth = 300 };
    private bool _remoteDns = Prefs.ProxyRemoteDns;
    private bool _sitesOnly = Prefs.ProxySitesOnly;
    private string _result = "";

    public ProxyPage()
    {
        MenuKey = "settings";
        _host.Text = Prefs.ProxyHost;
        _port.Text = Prefs.ProxyPort.ToString();
        _user.Text = Prefs.ProxyUser;
        _pass.Password = Prefs.ProxyPass;
        _pass.Style = (Style)Application.Current.Resources["PassBox"];
    }

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    private static TextBlock Label(string t) => Ui.Text(t, 15, weight: FontWeights.SemiBold, margin: new Thickness(0, 12, 0, 6));

    private void Build()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Proxy SOCKS5"));
        Body.Children.Add(Ui.Dim("Cho app đi qua proxy SOCKS5 của anh (có hoặc không có tên đăng nhập). Proxy lỗi thì app báo lỗi chứ không " +
                                 "lặng lẽ đi thẳng. Mật khẩu được mã hoá bằng tài khoản Windows này.", 14, new Thickness(0, 0, 0, 8)));
        var on = Prefs.ProxyOn;
        Body.Children.Add(Ui.Row(
            Ui.Pill("Bật", () => Toggle(true), on ? "active" : null, 90),
            Ui.Pill("Tắt", () => Toggle(false), !on ? "active" : null, 90)));
        Body.Children.Add(Label("Địa chỉ proxy và cổng"));
        Detach(_host, _port, _user, _pass);
        _host.Margin = new Thickness(0, 0, 10, 10);
        _port.Margin = new Thickness(0, 0, 10, 10);
        Body.Children.Add(Ui.Row(_host, _port));
        Body.Children.Add(Label("Tên đăng nhập / mật khẩu (để trống nếu proxy không cần)"));
        _user.Margin = new Thickness(0, 0, 10, 10);
        _pass.Margin = new Thickness(0, 0, 10, 10);
        Body.Children.Add(Ui.Row(_user, _pass));
        Body.Children.Add(Label("Tên miền phân giải ở"));
        Body.Children.Add(Ui.Row(
            Ui.Pill("DNS của proxy", () => Set(() => _remoteDns = true), _remoteDns ? "active" : null),
            Ui.Pill("DNS của máy này", () => Set(() => _remoteDns = false), !_remoteDns ? "active" : null)));
        Body.Children.Add(Label("Đi qua proxy"));
        Body.Children.Add(Ui.Row(
            Ui.Pill("Toàn bộ app", () => Set(() => _sitesOnly = false), !_sitesOnly ? "active" : null),
            Ui.Pill("Chỉ trang phim", () => Set(() => _sitesOnly = true), _sitesOnly ? "active" : null)));
        Body.Children.Add(Ui.Dim("«Chỉ trang phim»: chỉ các trang trong danh sách đi qua proxy; video, ảnh bìa, GitHub… đi thẳng mạng nhà " +
                                 "(dùng khi video chậm / bị chặn qua proxy).", 13.5, new Thickness(0, 0, 0, 8)));
        Body.Children.Add(Ui.Row(
            Ui.Pill("💾  Lưu", () => Save(true)),
            Ui.Pill("Thử kết nối", () => _ = Test())));
        if (_result.Length > 0) Body.Children.Add(Ui.Text(_result, 15, margin: new Thickness(0, 6, 0, 0)));
    }

    /// Ô nhập dùng lại giữa các lần dựng trang: gỡ khỏi hàng cũ trước khi thêm vào hàng mới.
    private static void Detach(params FrameworkElement[] items)
    {
        foreach (var i in items)
            if (i.Parent is Panel p) p.Children.Remove(i);
    }

    private void Set(Action change)
    {
        change();
        Build();
    }

    private void Toggle(bool on)
    {
        if (on && !Save(false)) return;
        Prefs.ProxyOn = on;
        Net.SettingsChanged();
        Main.Toast(on ? "Đã bật proxy SOCKS5." : "Đã tắt proxy — đi thẳng mạng nhà.");
        Build();
    }

    /// Lưu ô nhập; sai thì báo và trả false.
    private bool Save(bool notify)
    {
        var host = _host.Text.Trim();
        if (!int.TryParse(_port.Text.Trim(), out var port) || port is <= 0 or > 65535)
        {
            Main.Toast("Cổng không hợp lệ (1–65535).");
            return false;
        }
        if (host.Length == 0 || host.Contains(' ') || host.Contains('/'))
        {
            Main.Toast("Nhập địa chỉ proxy (IP hoặc tên miền, không có http://).");
            return false;
        }
        Prefs.ProxyHost = host;
        Prefs.ProxyPort = port;
        Prefs.ProxyUser = _user.Text.Trim();
        Prefs.ProxyPass = _pass.Password;
        Prefs.ProxyRemoteDns = _remoteDns;
        Prefs.ProxySitesOnly = _sitesOnly;
        Net.SettingsChanged();
        if (notify) Main.Toast(Prefs.ProxyOn ? "Đã lưu — app đi qua proxy mới." : "Đã lưu (proxy đang tắt).");
        return true;
    }

    /// Mở kết nối tới trang phim đang dùng qua proxy (theo ô đang nhập, chưa cần lưu).
    private async Task Test()
    {
        if (!int.TryParse(_port.Text.Trim(), out var port) || _host.Text.Trim().Length == 0)
        {
            Main.Toast("Nhập địa chỉ và cổng proxy trước.");
            return;
        }
        var c = new SocksConfig(_host.Text.Trim(), port, _user.Text.Trim(), _pass.Password, _remoteDns);
        var target = Site.HostOf(Prefs.BaseUrl) ?? "github.com";
        _result = $"Đang thử qua proxy tới {target}…";
        Build();
        var t0 = DateTime.UtcNow;
        try
        {
            using var s = await Socks5.Connect(c, target, 443, CancellationToken.None);
            _result = $"✓ Kết nối được qua proxy tới {target} ({(int)(DateTime.UtcNow - t0).TotalMilliseconds} ms).";
        }
        catch (Exception e)
        {
            _result = "✗ " + (e is SocksError ? "Lỗi proxy: " + e.Message : Util.Friendly(e));
        }
        Build();
    }
}
