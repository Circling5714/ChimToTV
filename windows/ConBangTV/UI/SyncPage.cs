using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Đồng bộ trong nhà với TV / iPad / iPhone (bản TV: Cài đặt → Đồng bộ…): mã ghép của máy này, máy trong mạng, máy đã ghép,
/// ghép bằng IP. Mở trang thì cho máy khác ghép (mã 4 số); rời trang thì thôi.
public sealed class SyncPage : Page
{
    private readonly TextBox _ip = new() { MinWidth = 260 };
    private readonly TextBox _code = new() { MinWidth = 120, MaxLength = 4 };
    private bool _browsing;

    public SyncPage()
    {
        MenuKey = "sync";
        Loaded += (_, _) =>
        {
            SyncService.StateChanged += OnState;
            SyncService.OpenPairing();
            _ = Browse();
        };
        Unloaded += (_, _) =>
        {
            SyncService.StateChanged -= OnState;
            SyncService.ClosePairing();
        };
    }

    private void OnState() => Dispatcher.BeginInvoke(() =>
    {
        if (IsLoaded) Rebuild();
    });

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    private async Task Browse()
    {
        _browsing = true;
        Rebuild();
        try
        {
            await SyncService.Browse();
        }
        catch (Exception e)
        {
            Log.D("sync", "tìm máy lỗi: " + e.Message);
        }
        _browsing = false;
        Rebuild();
    }

    /// Dựng lại, giữ mục đang chọn (theo chữ trên nút).
    private void Rebuild()
    {
        var focused = System.Windows.Input.Keyboard.FocusedElement;
        var text = (focused as Button)?.Content as string;
        Build();
        if (text != null)
            Dispatcher.BeginInvoke(() => KeyNav.Targets(Body).OfType<Button>().FirstOrDefault(b => b.Content as string == text)?.Focus(),
                DispatcherPriority.Loaded);
    }

    private static IEnumerable<string> LocalIps() =>
        NetworkInterface.GetAllNetworkInterfaces()
            .Where(n => n.OperationalStatus == OperationalStatus.Up && n.NetworkInterfaceType is not NetworkInterfaceType.Loopback)
            .SelectMany(n => n.GetIPProperties().UnicastAddresses)
            .Where(a => a.Address.AddressFamily == AddressFamily.InterNetwork && !a.Address.ToString().StartsWith("169.254."))
            .Select(a => a.Address.ToString()).Distinct();

    private void Build()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Đồng bộ trong nhà"));
        Body.Children.Add(Ui.Dim("Máy tính này, tivi (Côn Bằng TV 2.2+) và iPad / iPhone (1.3+) cùng mạng Wi‑Fi đồng bộ với nhau: Xem tiếp / " +
                                 "lịch sử, chỗ đang xem từng tập, phim theo dõi và danh sách trang phim. Xoá ở máy này thì máy kia cũng xoá.",
            14, new Thickness(0, 0, 0, 14)));

        // Máy này
        Body.Children.Add(Ui.Text("Máy này", 17, weight: FontWeights.SemiBold, margin: new Thickness(0, 4, 0, 6)));
        var ips = string.Join(", ", LocalIps().Select(ip => SyncService.Port == 8791 ? ip : $"{ip}:{SyncService.Port}"));
        Body.Children.Add(Ui.Dim($"{SyncService.DeviceName} · địa chỉ: {(ips.Length > 0 ? ips : "chưa có mạng")}", 15));
        var code = SyncService.PairCode;
        if (code != null)
        {
            var row = new WrapPanel { Margin = new Thickness(0, 8, 0, 4) };
            row.Children.Add(Ui.Text("Mã ghép:  ", 18));
            row.Children.Add(Ui.Text(code, 30, Ui.Res("Focus"), FontWeights.Bold));
            Body.Children.Add(row);
            Body.Children.Add(Ui.Dim("Trên tivi: Cài đặt → Đồng bộ với iPad / iPhone… → chọn máy này (hoặc nhập địa chỉ ở trên) → nhập mã. " +
                                     "iPad / iPhone: Cài đặt → Đồng bộ trong nhà. Mã đổi sau mỗi lần ghép.", 13.5, new Thickness(0, 0, 0, 6)));
            Body.Children.Add(Ui.Dim("Lần đầu Windows có thể hỏi cho phép Côn Bằng TV dùng mạng: chọn Cho phép (mạng riêng / Private).",
                13.5, new Thickness(0, 0, 0, 10)));
        }

        // Máy đã ghép
        var peers = SyncService.Peers;
        Body.Children.Add(Ui.Text("Máy đã ghép", 17, weight: FontWeights.SemiBold, margin: new Thickness(0, 12, 0, 6)));
        if (peers.Count == 0)
        {
            Body.Children.Add(Ui.Dim("Chưa ghép máy nào.", 15, new Thickness(0, 0, 0, 6)));
        }
        else
        {
            foreach (var p in peers)
            {
                var row = new WrapPanel();
                var last = p.Last is { } ms ? DateTimeOffset.FromUnixTimeMilliseconds(ms).LocalDateTime.ToString("HH:mm dd/MM") : "chưa";
                var t = Ui.Text($"{Icon(p.Platform)}  {p.Name}  ·  {p.Status ?? ""}  ·  lần cuối: {last}", 15, wrap: false,
                    margin: new Thickness(0, 8, 14, 10));
                row.Children.Add(t);
                row.Children.Add(Ui.Pill("Bỏ ghép", async () =>
                {
                    if (!await Main.Confirm($"Bỏ ghép với {p.Name}?", "Hai máy thôi đồng bộ (dữ liệu đã có vẫn giữ).", "Bỏ ghép")) return;
                    SyncService.Unpair(p.Id);
                    Rebuild();
                }));
                Body.Children.Add(row);
            }
            Body.Children.Add(Ui.Row(Ui.Pill(SyncService.Busy ? "Đang đồng bộ…" : "⇄  Đồng bộ ngay", () => _ = SyncService.SyncAll(true))));
        }
        Body.Children.Add(Ui.Row(
            Ui.Pill(Prefs.SyncAuto ? "Tự đồng bộ: Bật" : "Tự đồng bộ: Tắt", () =>
            {
                Prefs.SyncAuto = !Prefs.SyncAuto;
                Rebuild();
            })));
        Body.Children.Add(Ui.Dim("Tự đồng bộ: lúc mở app, thoát phim, theo dõi / bỏ theo dõi, xoá lịch sử.", 13.5, new Thickness(0, 0, 0, 8)));

        // Máy trong mạng
        Body.Children.Add(Ui.Text("Máy trong mạng", 17, weight: FontWeights.SemiBold, margin: new Thickness(0, 12, 0, 6)));
        var found = SyncService.Found.Where(f => peers.All(p => p.Id != f.Id)).ToList();
        if (found.Count == 0)
            Body.Children.Add(Ui.Dim(_browsing ? "Đang tìm…" : "Không thấy máy nào chưa ghép (máy kia phải đang mở Côn Bằng TV; khác lớp mạng thì nhập địa chỉ bên dưới).",
                15, new Thickness(0, 0, 0, 6)));
        var frow = new WrapPanel();
        foreach (var f in found) frow.Children.Add(Ui.Pill($"{Icon(f.Platform)}  Ghép với {f.Name}", () => _ = AskCode(f.Host, f.Port, f.Name)));
        frow.Children.Add(Ui.Pill(_browsing ? "Đang tìm…" : "↻  Tìm lại", () => _ = Browse()));
        Body.Children.Add(frow);

        // Ghép bằng địa chỉ
        Body.Children.Add(Ui.Text("Ghép bằng địa chỉ máy kia", 17, weight: FontWeights.SemiBold, margin: new Thickness(0, 14, 0, 6)));
        Detach(_ip, _code);
        _ip.Margin = new Thickness(0, 0, 10, 10);
        _code.Margin = new Thickness(0, 0, 10, 10);
        Body.Children.Add(Ui.Row(_ip, _code, Ui.Pill("Ghép", () => _ = PairManual())));
        Body.Children.Add(Ui.Dim("Ô đầu: địa chỉ IP của tivi / iPad (vd hiện trên màn Đồng bộ của máy đó); ô sau: mã ghép 4 số đang hiện ở máy đó.",
            13.5));
    }

    private static string Icon(string platform) => platform switch
    {
        "android" => "📺",
        "ios" => "📱",
        "windows" => "💻",
        _ => "🖥",
    };

    private static void Detach(params FrameworkElement[] items)
    {
        foreach (var i in items)
            if (i.Parent is Panel p) p.Children.Remove(i);
    }

    private async Task AskCode(string host, int port, string name)
    {
        var box = new TextBox { MaxLength = 4, MinWidth = 140, HorizontalAlignment = HorizontalAlignment.Left };
        Dispatcher.BeginInvoke(() => box.Focus(), DispatcherPriority.Input);
        box.KeyDown += (_, e) =>
        {
            if (e.Key == System.Windows.Input.Key.Enter) e.Handled = true;
        };
        var r = await Main.Ask($"Ghép với {name}", "Nhập mã ghép 4 số đang hiện trên màn Đồng bộ của máy đó.", new[] { "Ghép", "Huỷ" }, box);
        if (r != 0) return;
        await DoPair(host, port, box.Text.Trim());
    }

    private async Task PairManual()
    {
        if (SyncService.ParseAddress(_ip.Text) is not { } a)
        {
            Main.Toast("Nhập địa chỉ IP của máy kia.");
            return;
        }
        await DoPair(a.Host, a.Port, _code.Text.Trim());
    }

    private async Task DoPair(string host, int port, string code)
    {
        if (code.Length != 4 || !code.All(char.IsDigit))
        {
            Main.Toast("Mã ghép gồm 4 chữ số.");
            return;
        }
        Main.Toast($"Đang ghép với {host}…");
        var err = await SyncService.Pair(host, port, code);
        Main.Toast(err ?? "Đã ghép và đồng bộ xong.");
        Rebuild();
    }
}
