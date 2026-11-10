using System.Windows;
using System.Windows.Controls;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Proxy (bản TV: Cài đặt → Proxy): SOCKS5 hoặc Shadowsocks; bật / tắt, địa chỉ, cổng, đăng nhập / khoá, DNS của proxy hay
/// của máy, toàn app hay chỉ trang phim. Proxy lỗi thì báo lỗi, không lặng lẽ đi thẳng. Đổi xong có hiệu lực ngay (cả trình phát).
public sealed class ProxyPage : Page
{
    private readonly TextBox _host = new() { MinWidth = 300 };
    private readonly TextBox _port = new() { MinWidth = 110 };
    private readonly TextBox _user = new() { MinWidth = 300 };
    private readonly PasswordBox _pass = new() { MinWidth = 300 };
    // Shadowsocks
    private readonly TextBox _ssLink = new() { MinWidth = 520 };
    private readonly TextBox _ssHost = new() { MinWidth = 300 };
    private readonly TextBox _ssPort = new() { MinWidth = 110 };
    private readonly PasswordBox _ssKey = new() { MinWidth = 520 };
    private SsMethod _ssMethod = SsMethod.Of(Prefs.SsMethodId) ?? SsMethod.Default;
    private string _kind = Prefs.ProxyKind;
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
        _ssHost.Text = Prefs.SsHost;
        _ssPort.Text = Prefs.SsPort > 0 ? Prefs.SsPort.ToString() : "";
        _ssKey.Password = Prefs.SsKey;
        _ssKey.Style = (Style)Application.Current.Resources["PassBox"];
        // Dán link ss:// (3X-UI: nút Copy / QR của người dùng) là tự điền các ô
        _ssLink.TextChanged += (_, _) =>
        {
            if (_ssLink.Text.Trim().StartsWith("ss://", StringComparison.OrdinalIgnoreCase)) ReadLink();
        };
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
        Body.Children.Add(Ui.Header("Proxy (SOCKS5 / Shadowsocks)"));
        Body.Children.Add(Ui.Dim("Cho app đi qua proxy của anh. Proxy lỗi thì app báo lỗi chứ không lặng lẽ đi thẳng. Mật khẩu / khoá được " +
                                 "mã hoá bằng tài khoản Windows này.", 14, new Thickness(0, 0, 0, 8)));
        var on = Prefs.ProxyOn;
        Body.Children.Add(Ui.Row(
            Ui.Pill("Bật", () => Toggle(true), on ? "active" : null, 90),
            Ui.Pill("Tắt", () => Toggle(false), !on ? "active" : null, 90)));
        Body.Children.Add(Label("Kiểu proxy"));
        Body.Children.Add(Ui.Row(
            Ui.Pill("SOCKS5", () => SetKind("socks5"), _kind == "socks5" ? "active" : null),
            Ui.Pill("Shadowsocks (mã hoá)", () => SetKind("ss"), _kind == "ss" ? "active" : null)));
        Detach(_host, _port, _user, _pass, _ssLink, _ssHost, _ssPort, _ssKey);
        if (_kind == "ss") BuildSs();
        else BuildSocks();
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

    private void BuildSocks()
    {
        Body.Children.Add(Label("Địa chỉ proxy và cổng"));
        _host.Margin = new Thickness(0, 0, 10, 10);
        _port.Margin = new Thickness(0, 0, 10, 10);
        Body.Children.Add(Ui.Row(_host, _port));
        Body.Children.Add(Label("Tên đăng nhập / mật khẩu (để trống nếu proxy không cần)"));
        _user.Margin = new Thickness(0, 0, 10, 10);
        _pass.Margin = new Thickness(0, 0, 10, 10);
        Body.Children.Add(Ui.Row(_user, _pass));
        Body.Children.Add(Ui.Dim("SOCKS5 không mã hoá: nhà mạng thấy tên trang, tên đăng nhập / mật khẩu proxy. Cần giấu thì dùng Shadowsocks.",
            13.5, new Thickness(0, 0, 0, 4)));
    }

    private void BuildSs()
    {
        Body.Children.Add(Label("Dán link ss:// (3X-UI: nút Copy / QR của người dùng) — tự điền các ô dưới"));
        _ssLink.Margin = new Thickness(0, 0, 10, 10);
        Body.Children.Add(Ui.Row(_ssLink));
        Body.Children.Add(Label("Máy chủ và cổng"));
        _ssHost.Margin = new Thickness(0, 0, 10, 10);
        _ssPort.Margin = new Thickness(0, 0, 10, 10);
        Body.Children.Add(Ui.Row(_ssHost, _ssPort));
        Body.Children.Add(Label("Kiểu mã hoá"));
        Body.Children.Add(Ui.Row(SsMethod.All.Select(m =>
            (UIElement)Ui.Pill(m.Id + (m.Supported ? "" : " (máy này không có)"), () => Set(() => _ssMethod = m), m == _ssMethod ? "active" : null)).ToArray()));
        Body.Children.Add(Label("Khoá (3X-UI: khoá-máy-chủ:khoá-người-dùng) / mật khẩu"));
        _ssKey.Margin = new Thickness(0, 0, 10, 10);
        Body.Children.Add(Ui.Row(_ssKey));
        Body.Children.Add(Ui.Dim("Shadowsocks mã hoá cả đường tới máy chủ (vd lối vào Shadowsocks của 3X-UI, TCP, kiểu 2022-blake3-aes-256-gcm): " +
                                 "nhà mạng chỉ thấy dữ liệu ngẫu nhiên. SS 2022 cần giờ trên máy đúng (lệch tối đa 30 giây).", 13.5,
            new Thickness(0, 0, 0, 4)));
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

    /// Đổi kiểu: mỗi kiểu giữ máy chủ / khoá riêng (đang bật thì đổi luôn đường đi).
    private void SetKind(string k)
    {
        _kind = k;
        Prefs.ProxyKind = k;
        Net.SettingsChanged();
        Build();
    }

    private void ReadLink()
    {
        try
        {
            var c = SsLink.Parse(_ssLink.Text, _remoteDns, _sitesOnly);
            _ssHost.Text = c.Host;
            _ssPort.Text = c.Port.ToString();
            _ssMethod = SsMethod.Of(c.Method) ?? SsMethod.Default;
            _ssKey.Password = c.Password;
            _ssLink.Text = "";
            _result = $"Đã đọc link: {c.Host}:{c.Port} · {c.Method} — bấm «Thử kết nối» rồi «Lưu».";
        }
        catch (SsError e)
        {
            _result = "✗ " + e.Message;
        }
        if (_kind != "ss") SetKind("ss");
        else Build();
    }

    private void Toggle(bool on)
    {
        if (on && !Save(false)) return;
        Prefs.ProxyOn = on;
        Net.SettingsChanged();
        Main.Toast(on ? $"Đã bật proxy {(_kind == "ss" ? "Shadowsocks" : "SOCKS5")}." : "Đã tắt proxy — đi thẳng mạng nhà.");
        Build();
    }

    /// Đọc ô Shadowsocks; sai thì báo, trả null.
    private SsConfig? ReadSs()
    {
        var host = _ssHost.Text.Trim();
        var portText = _ssPort.Text.Trim();
        if ((host.Count(ch => ch == ':') == 1 || host.StartsWith('[')) && SsLink.SplitHostPort(host) is { } hp)
        {
            host = hp.Host;
            portText = hp.Port.ToString();
        }
        if (host.Length == 0 || host.Contains(' ') || host.Contains('/'))
        {
            Main.Toast("Nhập máy chủ Shadowsocks (IP hoặc tên miền), hoặc dán link ss://…");
            return null;
        }
        if (!int.TryParse(portText, out var port) || port is <= 0 or > 65535)
        {
            Main.Toast("Cổng không hợp lệ (1–65535).");
            return null;
        }
        if (!_ssMethod.Supported)
        {
            Main.Toast($"Máy này không có {_ssMethod.Id} — đổi sang {SsMethod.Default.Id}.");
            return null;
        }
        var key = _ssKey.Password.Trim();
        try
        {
            Shadowsocks.Keys(_ssMethod, key);
        }
        catch (SsError e)
        {
            Main.Toast(e.Message);
            return null;
        }
        return new SsConfig(host, port, _ssMethod.Id, key, _remoteDns, _sitesOnly);
    }

    /// Lưu ô nhập; sai thì báo và trả false.
    private bool Save(bool notify)
    {
        if (_kind == "ss")
        {
            if (ReadSs() is not { } c) return false;
            Prefs.SsHost = c.Host;
            Prefs.SsPort = c.Port;
            Prefs.SsMethodId = c.Method;
            Prefs.SsKey = c.Password;
        }
        else
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
        }
        Prefs.ProxyRemoteDns = _remoteDns;
        Prefs.ProxySitesOnly = _sitesOnly;
        Net.SettingsChanged();
        if (notify) Main.Toast(Prefs.ProxyOn ? "Đã lưu — app đi qua proxy mới." : "Đã lưu (proxy đang tắt).");
        return true;
    }

    /// Thử theo ô đang nhập (chưa cần lưu): SOCKS5 — mở kết nối tới trang phim đang dùng; Shadowsocks — hỏi Cloudflare qua máy
    /// chủ, báo IP ra ngoài.
    private async Task Test()
    {
        var t0 = DateTime.UtcNow;
        if (_kind == "ss")
        {
            if (ReadSs() is not { } c) return;
            _result = $"Đang thử Shadowsocks {c.Host}:{c.Port}…";
            Build();
            try
            {
                var ip = await Shadowsocks.Check(c);
                _result = $"✓ Shadowsocks chạy — IP ra ngoài {ip} ({(int)(DateTime.UtcNow - t0).TotalMilliseconds} ms).";
            }
            catch (Exception e)
            {
                _result = "✗ " + (e is SsError or SocksError ? "Lỗi Shadowsocks: " + e.Message : Util.Friendly(e));
            }
            Build();
            return;
        }
        if (!int.TryParse(_port.Text.Trim(), out var port) || _host.Text.Trim().Length == 0)
        {
            Main.Toast("Nhập địa chỉ và cổng proxy trước.");
            return;
        }
        var sc = new SocksConfig(_host.Text.Trim(), port, _user.Text.Trim(), _pass.Password, _remoteDns);
        var target = Site.HostOf(Prefs.BaseUrl) ?? "github.com";
        _result = $"Đang thử qua proxy tới {target}…";
        Build();
        try
        {
            using var s = await Socks5.Connect(sc, target, 443, CancellationToken.None);
            _result = $"✓ Kết nối được qua proxy tới {target} ({(int)(DateTime.UtcNow - t0).TotalMilliseconds} ms).";
        }
        catch (Exception e)
        {
            _result = "✗ " + (e is SocksError ? "Lỗi proxy: " + e.Message : Util.Friendly(e));
        }
        Build();
    }
}
