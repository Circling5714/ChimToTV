using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Đổi trang phim: danh sách trang, đo nhanh, thêm / xoá, thêm lại trang mẫu.
public sealed class SitesPage : Page
{
    private readonly Dictionary<string, string> _checks = new();
    private readonly TextBox _add = new() { MinWidth = 340 };

    public SitesPage()
    {
        MenuKey = "sites";
        _add.KeyDown += (_, e) =>
        {
            if (e.Key != System.Windows.Input.Key.Enter) return;
            e.Handled = true;
            Add();
        };
    }

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    private void Build()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Đổi trang phim"));
        Body.Children.Add(Ui.Dim("Trang đang dùng có màu xanh. Trang chậm / không vào được (nhà mạng chặn, đổi tên miền): bấm «Đo» rồi chọn trang khác.",
            14, new Thickness(0, 0, 0, 16)));
        var cur = Prefs.BaseUrl;
        foreach (var s in Prefs.Sites)
        {
            var row = new WrapPanel { Margin = new Thickness(0, 0, 0, 2) };
            var host = Site.HostOf(s) ?? s;
            row.Children.Add(Ui.Pill((s == cur ? "●  " : "") + Site.DisplayName(host), () => Use(s), s == cur ? "active" : null, 260));
            row.Children.Add(Ui.Pill("Đo", () => _ = Check(s)));
            row.Children.Add(Ui.Pill("Xoá", async () =>
            {
                if (!await Main.Confirm($"Xoá {host} khỏi danh sách?", "", "Xoá")) return;
                if (!Prefs.RemoveSite(s)) Main.Toast("Phải còn ít nhất một trang.");
                Main.RebuildMenu();
                Build();
            }));
            if (_checks.TryGetValue(s, out var r))
            {
                var t = Ui.Dim(r, 14);
                t.VerticalAlignment = VerticalAlignment.Center;
                t.Margin = new Thickness(6, 0, 0, 10);
                row.Children.Add(t);
            }
            Body.Children.Add(row);
        }
        Body.Children.Add(Ui.Row(Ui.Pill("Đo tất cả", () => _ = CheckAll())));

        Body.Children.Add(Ui.Header("Thêm trang", new Thickness(0, 22, 0, 12)));
        var add = new WrapPanel();
        _add.Margin = new Thickness(0, 0, 10, 10);
        // Ô nhập dùng lại qua các lần dựng: gỡ khỏi hàng cũ trước (không thì WPF báo «already the logical child»).
        (_add.Parent as Panel)?.Children.Remove(_add);
        add.Children.Add(_add);
        add.Children.Add(Ui.Pill("Thêm", Add));
        Body.Children.Add(add);
        Body.Children.Add(Ui.Dim("Gõ tên miền, vd yanhh3d.ee. App đọc được trang kiểu yanhh3d, theme HaLim (hoathinh3d…), kiểu PhimMoi " +
                                 "(phimmois.fun), mephim.ddns.net, KKPhim (kkphim.com), NguonC (phim.nguonc.com), VSMOV (vsmov.com) và " +
                                 "CLB Phim Xưa (sc.k-20.xyz).", 14, new Thickness(0, 0, 0, 10)));
        var missing = Prefs.MissingPresets;
        if (missing.Count > 0)
        {
            Body.Children.Add(Ui.Dim("Thêm lại trang mẫu:", 14, new Thickness(0, 8, 0, 8)));
            var row = new WrapPanel();
            foreach (var m in missing)
                row.Children.Add(Ui.Pill("+ " + Site.HostOf(m), () =>
                {
                    Prefs.AddSite(m);
                    Build();
                }));
            Body.Children.Add(row);
        }
    }

    private void Use(string s)
    {
        if (s == Prefs.BaseUrl)
        {
            Main.ShowRoot(new HomePage());
            return;
        }
        Prefs.BaseUrl = s;
        Main.SetCategories(new());
        Main.ShowRoot(new HomePage());
        Main.Toast($"Đã đổi sang {Site.HostOf(s)}.");
    }

    private void Add()
    {
        var u = Prefs.NormalizeUrl(_add.Text);
        if (u == null)
        {
            Main.Toast("Tên trang không hợp lệ.");
            return;
        }
        Prefs.AddSite(u);
        _add.Text = "";
        Build();
        _ = Check(u);
    }

    private async Task Check(string s)
    {
        _checks[s] = "đang đo…";
        Build();
        try
        {
            var r = await Util.WithTimeout(25, () => Site.Check(s));
            _checks[s] = r.Films > 0 ? $"{r.Ms} ms · {r.Films} phim" : $"{r.Ms} ms · không thấy phim (bị chặn / đổi giao diện?)";
        }
        catch (Exception e)
        {
            _checks[s] = "lỗi: " + Util.Friendly(e);
        }
        var focusedText = (System.Windows.Input.Keyboard.FocusedElement as Button)?.Content as string;
        Build();
        if (focusedText != null) Refocus(focusedText);
    }

    private void Refocus(string text)
    {
        Dispatcher.BeginInvoke(() =>
        {
            var b = KeyNav.Targets(Body).OfType<Button>().FirstOrDefault(x => x.Content as string == text);
            b?.Focus();
        }, DispatcherPriority.Loaded);
    }

    private async Task CheckAll() => await Task.WhenAll(Prefs.Sites.Select(Check));
}

/// Cài đặt: tua, tự chuyển tập, cỡ giao diện, toàn màn hình, cập nhật, xoá lịch sử, phím tắt.
public sealed class SettingsPage : Page
{
    public SettingsPage()
    {
        MenuKey = "settings";
    }

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    private void Option(string label, IEnumerable<(string Text, bool Active, Action Act)> choices, string? note = null)
    {
        Body.Children.Add(Ui.Text(label, 16, weight: FontWeights.SemiBold, margin: new Thickness(0, 14, 0, 8)));
        var row = new WrapPanel();
        foreach (var c in choices)
        {
            var act = c.Act;
            row.Children.Add(Ui.Pill(c.Text, () =>
            {
                var text = c.Text;
                act();
                Build();
                Dispatcher.BeginInvoke(() =>
                    KeyNav.Targets(Body).OfType<Button>().FirstOrDefault(x => x.Content as string == text)?.Focus(), DispatcherPriority.Loaded);
            }, c.Active ? "active" : null, 90));
        }
        Body.Children.Add(row);
        if (note != null) Body.Children.Add(Ui.Dim(note, 13.5, new Thickness(0, 0, 0, 4)));
    }

    private static void SetUnblock(int level)
    {
        Prefs.UnblockLevel = level;
        Net.SettingsChanged();
    }

    private void Build()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Cài đặt"));
        Body.Children.Add(Ui.Row(
            Ui.Pill($"🌐  Trang phim: {Site.DisplayName(Site.HostOf(Prefs.BaseUrl) ?? Prefs.BaseUrl)}", () => Main.ShowRoot(new SitesPage())),
            Ui.Pill($"👤  Người xem: {Prefs.CurrentProfile.Name} — quản lý / giới hạn giờ xem…", () => Main.Push(new ProfilesPage()))));

        Option("Mỗi lần bấm ◀ ▶ khi xem phim tua", new[] { 5, 10, 15, 30 }.Select(s =>
            ($"{s} giây", Prefs.SeekStep == s, (Action)(() => Prefs.SeekStep = s))), "Giữ phím thì tua nhanh dần.");
        Option("Hết tập tự chuyển sang tập sau", new[]
        {
            ("Bật", Prefs.AutoNext, (Action)(() => Prefs.AutoNext = true)),
            ("Tắt", !Prefs.AutoNext, () => Prefs.AutoNext = false),
        });
        Option("Server dự phòng KKPhim", new[]
        {
            ("Bật", Prefs.KkBackup, (Action)(() => Prefs.KkBackup = true)),
            ("Tắt", !Prefs.KkBackup, () => Prefs.KkBackup = false),
        }, "Với yanhh3d, hoathinh3d, hhkungfu… (trình phát của trang hay hỏng cùng lúc) app tìm cùng phim, cùng tập trên KKPhim " +
           "và thêm server «KKPhim Vietsub / Thuyết Minh» vào bảng ▼ lúc xem; trang không vào được mà KKPhim có tập đó thì vẫn xem được.");
        Option("Server dự phòng NguonC", new[]
        {
            ("Bật", Prefs.NcBackup, (Action)(() => Prefs.NcBackup = true)),
            ("Tắt", !Prefs.NcBackup, () => Prefs.NcBackup = false),
        }, "Như KKPhim nhưng tìm trên NguonC (hay có tập mới sớm) và thêm server «NguonC Vietsub…». Trình phát của NguonC chạy " +
           "quảng cáo trước phim — app tự bỏ qua.");
        Option("Cỡ giao diện", new[] { 1.0, 1.25, 1.5, 1.75 }.Select(s =>
            ($"{s * 100:0}%", Math.Abs(Prefs.UiScale - s) < 0.01, (Action)(() =>
            {
                Prefs.UiScale = s;
                Main.ApplyScale();
            }))), "Máy cắm tivi, ngồi xa: chọn 150% hoặc 175%.");
        Option("Mở app ở chế độ toàn màn hình", new[]
        {
            ("Bật", Prefs.StartFullscreen, (Action)(() =>
            {
                Prefs.StartFullscreen = true;
                Main.SetFullscreen(true);
            })),
            ("Tắt", !Prefs.StartFullscreen, () =>
            {
                Prefs.StartFullscreen = false;
                Main.SetFullscreen(false);
            }),
        }, "F11 (hoặc Alt+Enter) để bật / tắt toàn màn hình bất cứ lúc nào; đang toàn màn hình thì menu có mục «Thoát toàn màn hình», " +
           "Esc ở menu cũng thoát.");
        Option("Thu nhỏ cửa sổ app lúc đang xem phim", new[]
        {
            ("Phim chạy tiếp ở cửa sổ nhỏ", Prefs.MiniOnMinimize, (Action)(() => Prefs.MiniOnMinimize = true)),
            ("Dừng theo app", !Prefs.MiniOnMinimize, () => Prefs.MiniOnMinimize = false),
        }, "Lúc xem: nút «▣ Thu nhỏ» (phím W) đưa phim ra cửa sổ nhỏ góc màn hình; bấm đúp thanh dưới cửa sổ nhỏ để phóng to.");
        Option("Vượt chặn nhà mạng", new[]
        {
            ("Tắt", Prefs.UnblockLevel == 0, (Action)(() => SetUnblock(0))),
            ("Mức 1", Prefs.UnblockLevel == 1, () => SetUnblock(1)),
            ("Mức 2", Prefs.UnblockLevel == 2, () => SetUnblock(2)),
        }, "Trang không vào được / quá thời gian chờ (nhà mạng chặn theo tên trang, vd phimmois.fun): bật Mức 1. " +
           "Vẫn bị chặn thì thử Mức 2. Không đổi DNS, không cần VPN.");
        Body.Children.Add(Ui.Row(Ui.Pill(Prefs.ProxyConfig() is { } pc ? $"Proxy SOCKS5: {pc.Host}:{pc.Port}" : "Proxy SOCKS5: tắt",
            () => Main.Push(new ProxyPage()))));
        Option("Tự kiểm tra bản mới", new[]
        {
            ("Bật", Prefs.AutoUpdate, (Action)(() => Prefs.AutoUpdate = true)),
            ("Tắt", !Prefs.AutoUpdate, () => Prefs.AutoUpdate = false),
        });
        Body.Children.Add(Ui.Row(
            Ui.Pill("Kiểm tra cập nhật", () => _ = Main.CheckUpdate(true)),
            Ui.Pill("Xoá lịch sử xem", async () =>
            {
                if (!await Main.Confirm("Xoá cả lịch sử xem?", "Xoá danh sách Xem tiếp và chỗ đang xem của mọi tập.", "Xoá hết")) return;
                Prefs.ClearHistory();
                Main.Toast("Đã xoá lịch sử xem.");
            }),
            Ui.Pill("Xoá dữ liệu duyệt web", async () =>
            {
                if (!await Main.Confirm("Xoá dữ liệu duyệt web?", "Xoá cookie, bộ nhớ đệm của các trang phim và ảnh bìa đã lưu.", "Xoá")) return;
                await Player.PlayerView.ClearBrowsingData();
                Main.Toast("Đã xoá dữ liệu duyệt web.");
            })));

        Body.Children.Add(Ui.Header("Phím tắt", new Thickness(0, 26, 0, 10)));
        Body.Children.Add(Ui.Dim(
            "Mũi tên: di chuyển  •  Enter: chọn  •  Esc / Backspace / Back của remote: quay lại  •  F11: toàn màn hình  •  Ctrl+F: tìm kiếm\n" +
            "Khi xem phim: Space / Enter / K: phát / dừng  •  ◀ ▶ (J / L): tua  •  ▲: thời gian  •  ▼ / M: menu (server, tập, tốc độ…)  •  " +
            "N / P (PageDown / PageUp): tập sau / trước  •  0–9: nhảy tới 0–90%  •  F: toàn màn hình  •  W: thu nhỏ phim  •  Esc: thoát phim\n" +
            "Chuột: rê để hiện thanh điều khiển, bấm vào phim để phát / dừng, bấm đúp để toàn màn hình.", 14.5));

        Body.Children.Add(Ui.Header("Giới thiệu", new Thickness(0, 26, 0, 10)));
        Body.Children.Add(Ui.Dim($"Côn Bằng TV cho Windows — bản {UpdateCheck.CurrentVersion}", 15));
        Body.Children.Add(Ui.Row(
            Ui.Pill("Liên hệ: t.me/tivikit", () => Ui.OpenBrowser("https://t.me/tivikit")),
            Ui.Pill("Trang phát hành", () => Ui.OpenBrowser(UpdateCheck.ReleasesPage))));
        Body.Children.Add(Ui.Dim($"Dữ liệu của app: {Prefs.DataDir}", 13, new Thickness(0, 6, 0, 0)));
    }
}
