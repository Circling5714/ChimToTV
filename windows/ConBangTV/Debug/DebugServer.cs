#if DEBUG
using System.IO;
using System.Net;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows;
using System.Windows.Automation.Peers;
using System.Windows.Automation.Provider;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using ConBangTV.Core;
using ConBangTV.UI;

namespace ConBangTV;

/// Cổng gỡ lỗi của bản Debug (giống DebugServer của bản Android, cổng 8790, chỉ máy này): xem trạng thái, chụp màn hình,
/// bấm phím / bấm nút theo chữ, chạy JS trong trình phát, đọc nhật ký. Bản Release không có tệp này.
public static class DebugServer
{
    private static MainWindow? _w;

    public static void Start(MainWindow w)
    {
        _w = w;
        var l = new HttpListener();
        l.Prefixes.Add("http://localhost:8790/");
        try
        {
            l.Start();
        }
        catch (Exception e)
        {
            Log.D("debug", "Không mở được cổng 8790: " + e.Message);
            return;
        }
        Log.D("debug", "Cổng gỡ lỗi: http://localhost:8790/");
        Task.Run(async () =>
        {
            while (l.IsListening)
            {
                var ctx = await l.GetContextAsync();
                _ = Task.Run(() => Handle(ctx));
            }
        });
    }

    private static async Task Handle(HttpListenerContext ctx)
    {
        var q = ctx.Request.QueryString;
        byte[] body;
        var type = "text/plain; charset=utf-8";
        try
        {
            switch (ctx.Request.Url!.AbsolutePath)
            {
                case "/shot":
                    body = await _w!.Dispatcher.InvokeAsync(() => q["render"] == "1" ? ShotRender() : Shot());
                    type = "image/png";
                    break;
                case "/key":
                    var keys = (q["k"] ?? "").Split(',', StringSplitOptions.RemoveEmptyEntries);
                    foreach (var k in keys)
                    {
                        await _w!.Dispatcher.InvokeAsync(() => Key(Enum.Parse<Key>(k, true)));
                        await Task.Delay(int.TryParse(q["gap"], out var g) ? g : 120);
                    }
                    body = Text(await _w!.Dispatcher.InvokeAsync(Info));
                    break;
                case "/click":
                    body = Text(await _w!.Dispatcher.InvokeAsync(() => Click(q["text"] ?? "", int.TryParse(q["n"], out var n) ? n : 0)));
                    break;
                case "/js":
                    var code = q["code"] ?? (ctx.Request.HasEntityBody ? new StreamReader(ctx.Request.InputStream).ReadToEnd() : "");
                    var player = await _w!.Dispatcher.InvokeAsync(() => _w.OpenPlayer);
                    body = Text(player == null ? "trình phát chưa mở" : await await _w.Dispatcher.InvokeAsync(() => player.Js(code)));
                    break;
                case "/log":
                    var (last, lines) = Log.Since(int.TryParse(q["since"], out var s) ? s : 0);
                    body = Text($"#{last}\n" + string.Join("\n", lines.TakeLast(int.TryParse(q["n"], out var c) ? c : 300)));
                    break;
                case "/type":
                    body = Text(await _w!.Dispatcher.InvokeAsync(() =>
                    {
                        // field=N: chọn ô nhập thứ N trong cửa sổ trước khi gõ
                        if (int.TryParse(q["field"], out var fi))
                            KeyNav.Targets(_w).Where(x => x is TextBox or PasswordBox).ElementAtOrDefault(fi)?.Focus();
                        if (Keyboard.FocusedElement is PasswordBox pb)
                        {
                            pb.Password = q["text"] ?? "";
                            return "đã gõ mật khẩu";
                        }
                        if (Keyboard.FocusedElement is not TextBox tb) return "không có ô chữ đang chọn";
                        tb.Text = q["text"] ?? "";
                        tb.CaretIndex = tb.Text.Length;
                        return "đã gõ: " + tb.Text;
                    }));
                    break;
                case "/pref":
                    Prefs.DebugSet(q["key"]!, q["json"]!);
                    body = Text("ok");
                    break;
                case "/dohfail":
                    // Giả máy chủ DNS mã hoá bị chặn: /dohfail?labels=DoH NextDNS,DoT NextDNS (trống = bỏ); trả thứ tự máy chủ
                    Doh.DebugFail = (q["labels"] ?? "").Split(',').Select(x => x.Trim()).Where(x => x.Length > 0).ToHashSet();
                    Net.SettingsChanged(); // đóng kết nối đang giữ (HttpClient dùng lại) → kết nối mới phải hỏi DNS lại
                    body = Text(string.Join(" → ", Doh.Servers.Select(x => x + (Doh.DebugFail.Contains(x) ? " (giả chặn)" : ""))));
                    break;
                case "/dnsblock":
                    // Giả DNS của máy chặn: /dnsblock?hosts=a.com,b.com (trống = bỏ)&ip=203.0.113.9 (mặc định 0.0.0.0)
                    Doh.DebugDnsBlock = (q["hosts"] ?? "").Split(',').Select(x => x.Trim().ToLowerInvariant()).Where(x => x.Length > 0).ToHashSet();
                    Doh.DebugDnsBlockIp = IPAddress.Parse(string.IsNullOrWhiteSpace(q["ip"]) ? "0.0.0.0" : q["ip"]!);
                    Doh.ForgetAnswers();
                    Net.SettingsChanged();
                    body = Text(string.Join(",", Doh.DebugDnsBlock) + " → " + Doh.DebugDnsBlockIp);
                    break;
                case "/unblockhang":
                    // Giả nhà mạng ghép được các mẩu ở Mức 1 (gửi nguyên gói bắt tay) cho các trang này: /unblockhang?hosts=phimmoir.fun
                    Unblock.DebugWholeHello = (q["hosts"] ?? "").Split(',').Select(x => x.Trim().ToLowerInvariant()).Where(x => x.Length > 0).ToHashSet();
                    Net.SettingsChanged();
                    body = Text(string.Join(",", Unblock.DebugWholeHello));
                    break;
                case "/level2":
                    // Trang đã tự lên Mức 2: /level2 (xem), /level2?clear=1 (quên hết)
                    if (q["clear"] == "1") Unblock.Level2Memo.Clear();
                    body = Encoding.UTF8.GetBytes(System.Text.Json.JsonSerializer.Serialize(
                        new { hosts = Unblock.Level2Memo.All(), autoLevel2 = Unblock.Level2Memo.AutoCount, bridge = Net.BridgePort }));
                    type = "application/json; charset=utf-8";
                    break;
                case "/web":
                    // Đọc hộ trang bật chống bot: /web (phiên WebView ẩn + trang lõi đang nhớ là chặn), /web?url=… (đọc thử một địa chỉ)
                    if (q["url"] is { } wu)
                    {
                        var r = await _w!.Dispatcher.InvokeAsync(() => Player.WebFetch.Fetch(wu, "GET", null, new Dictionary<string, string>())).Task.Unwrap();
                        body = Text($"{r.Code} {r.Url} {r.Type} cf={r.Cf} {r.Data.Length} byte\n" +
                                    System.Text.Encoding.UTF8.GetString(r.Data, 0, Math.Min(400, r.Data.Length)));
                        break;
                    }
                    body = Text(await _w!.Dispatcher.InvokeAsync(() => Player.WebFetch.Debug()) + "\nlõi nhớ: " +
                                string.Join(", ", Native.Sync<List<string>>("web.hosts")));
                    break;
                case "/native":
                    // Thử đường tự chuyển của trình phát native: /native?fail=1 (mpv báo lỗi giữa chừng)
                    body = Text(await _w!.Dispatcher.InvokeAsync(() => q["fail"] == "1" ? _w.OpenPlayer?.DebugNativeFail() ?? "trình phát chưa mở" : "ok"));
                    break;
                case "/play":
                    await _w!.Dispatcher.InvokeAsync(() => _w.Play(q["film"]!, q["ep"]!));
                    body = Text("ok");
                    break;
                case "/open":
                    await _w!.Dispatcher.InvokeAsync(() => _w.Push(new DetailPage(q["url"]!)));
                    body = Text("ok");
                    break;
                default:
                    body = Text(await _w!.Dispatcher.InvokeAsync(Info));
                    break;
            }
        }
        catch (Exception e)
        {
            body = Text("lỗi: " + e);
            ctx.Response.StatusCode = 500;
        }
        ctx.Response.ContentType = type;
        ctx.Response.ContentLength64 = body.Length;
        await ctx.Response.OutputStream.WriteAsync(body);
        ctx.Response.Close();
    }

    private static byte[] Text(string s) => Encoding.UTF8.GetBytes(s);

    private static string Info()
    {
        var w = _w!;
        var f = Keyboard.FocusedElement as FrameworkElement;
        var sb = new StringBuilder();
        sb.AppendLine($"page={w.Current?.GetType().Name} fullscreen={w.IsFullscreen} state={w.WindowState} size={w.ActualWidth}x{w.ActualHeight}");
        sb.AppendLine($"focus={f?.GetType().Name} '{Describe(f)}'");
        sb.AppendLine($"player={w.OpenPlayer?.Debug() ?? "đóng"}{(w.OpenPlayer?.IsMini == true ? " (cửa sổ nhỏ)" : "")}");
        sb.AppendLine($"site={Prefs.BaseUrl}");
        return sb.ToString();
    }

    private static string Describe(FrameworkElement? f) => f switch
    {
        null => "",
        Button { Content: string s } => s,
        Button { Content: TextBlock t } => t.Text,
        Button { Content: Panel pnl } => string.Join(" ", pnl.Children.OfType<TextBlock>().Select(x => x.Text)),
        Button { DataContext: Film film } => "thẻ: " + film.Title,
        TextBox t => "ô chữ: " + t.Text,
        _ => f.ToString(),
    };

    private static void Key(Key k)
    {
        var w = _w!;
        var player = w.Player;
        IInputElement target = Keyboard.FocusedElement ?? (player != null ? player.Overlay : w);
        var src = PresentationSource.FromVisual(target as Visual ?? (player != null ? player.Overlay : w)) ?? PresentationSource.FromVisual(w)!;
        var e = new KeyEventArgs(Keyboard.PrimaryDevice, src, 0, k) { RoutedEvent = Keyboard.PreviewKeyDownEvent };
        target.RaiseEvent(e);
        if (e.Handled) return;
        e = new KeyEventArgs(Keyboard.PrimaryDevice, src, 0, k) { RoutedEvent = Keyboard.KeyDownEvent };
        target.RaiseEvent(e);
        e = new KeyEventArgs(Keyboard.PrimaryDevice, src, 0, k) { RoutedEvent = Keyboard.KeyUpEvent };
        target.RaiseEvent(e);
    }

    private static string Click(string text, int n)
    {
        var w = _w!;
        var roots = new List<DependencyObject> { w };
        if (w.Player != null) roots.Insert(0, w.Player.Overlay);
        var found = roots.SelectMany(r => KeyNav.Targets(r)).OfType<Button>()
            .Where(b => Describe(b).Contains(text, StringComparison.OrdinalIgnoreCase)).ToList();
        if (found.Count <= n) return $"không thấy nút «{text}» ({found.Count})";
        var btn = found[n];
        (new ButtonAutomationPeer(btn).GetPattern(PatternInterface.Invoke) as IInvokeProvider)?.Invoke();
        return "đã bấm: " + Describe(btn);
    }

    [DllImport("user32.dll")] private static extern IntPtr GetDC(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern int ReleaseDC(IntPtr hwnd, IntPtr dc);
    [DllImport("gdi32.dll")] private static extern IntPtr CreateCompatibleDC(IntPtr dc);
    [DllImport("gdi32.dll")] private static extern IntPtr CreateCompatibleBitmap(IntPtr dc, int w, int h);
    [DllImport("gdi32.dll")] private static extern IntPtr SelectObject(IntPtr dc, IntPtr obj);
    [DllImport("gdi32.dll")] private static extern bool BitBlt(IntPtr dst, int x, int y, int w, int h, IntPtr src, int sx, int sy, int rop);
    [DllImport("gdi32.dll")] private static extern bool DeleteObject(IntPtr obj);
    [DllImport("gdi32.dll")] private static extern bool DeleteDC(IntPtr dc);
    [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll")] private static extern bool PrintWindow(IntPtr hwnd, IntPtr dc, uint flags);

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT
    {
        public int Left, Top, Right, Bottom;
    }

    /// Chụp vùng màn hình của cửa sổ chính (kể cả trình phát và lớp phủ).
    private static byte[] Shot()
    {
        var hwnd = new WindowInteropHelper(_w!).Handle;
        GetWindowRect(hwnd, out var r);
        int w = r.Right - r.Left, h = r.Bottom - r.Top;
        var screen = GetDC(IntPtr.Zero);
        var mem = CreateCompatibleDC(screen);
        var bmp = CreateCompatibleBitmap(screen, w, h);
        var old = SelectObject(mem, bmp);
        // PrintWindow (PW_RENDERFULLCONTENT) chụp đúng cửa sổ app kể cả khi cửa sổ khác đè lên; lỗi thì chụp màn hình
        if (!PrintWindow(hwnd, mem, 2)) BitBlt(mem, 0, 0, w, h, screen, r.Left, r.Top, 0x00CC0020 | 0x40000000);
        SelectObject(mem, old);
        var src = Imaging.CreateBitmapSourceFromHBitmap(bmp, IntPtr.Zero, Int32Rect.Empty, BitmapSizeOptions.FromEmptyOptions());
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(IntPtr.Zero, screen);
        BitmapSource outImg = src;
        if (w > 1600) outImg = new TransformedBitmap(src, new ScaleTransform(1600.0 / w, 1600.0 / w));
        var enc = new PngBitmapEncoder();
        enc.Frames.Add(BitmapFrame.Create(outImg));
        using var ms = new MemoryStream();
        enc.Save(ms);
        return ms.ToArray();
    }

    /// /shot?render=1 — vẽ lại cây giao diện WPF ra ảnh, không qua màn hình: dùng khi máy đang khoá màn hình (Windows không
    /// vẽ lại cửa sổ, PrintWindow trả hình cũ). Không có hình của WebView2 (trình phát) và các cửa sổ phủ.
    private static byte[] ShotRender()
    {
        var root = (FrameworkElement)_w!.Content;
        var dpi = VisualTreeHelper.GetDpi(root);
        var area = new Rect(0, 0, root.ActualWidth, root.ActualHeight);
        var dv = new DrawingVisual();
        using (var dc = dv.RenderOpen())
        {
            dc.DrawRectangle(_w.Background ?? Brushes.Black, null, area);
            dc.DrawRectangle(new VisualBrush(root), null, area);
        }
        var rtb = new RenderTargetBitmap((int)(area.Width * dpi.DpiScaleX), (int)(area.Height * dpi.DpiScaleY),
            dpi.PixelsPerInchX, dpi.PixelsPerInchY, PixelFormats.Pbgra32);
        rtb.Render(dv);
        BitmapSource outImg = rtb;
        if (rtb.PixelWidth > 1600) outImg = new TransformedBitmap(rtb, new ScaleTransform(1600.0 / rtb.PixelWidth, 1600.0 / rtb.PixelWidth));
        var enc = new PngBitmapEncoder();
        enc.Frames.Add(BitmapFrame.Create(outImg));
        using var ms = new MemoryStream();
        enc.Save(ms);
        return ms.ToArray();
    }
}
#endif
