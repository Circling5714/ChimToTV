using System.IO;
using System.Windows;
using System.Windows.Threading;
using ConBangTV.Core;
using Microsoft.Web.WebView2.Core;

namespace ConBangTV;

public partial class App : Application
{
    /// Môi trường WebView2 dùng chung (dữ liệu duyệt web ở %LOCALAPPDATA%\ConBangTV\WebView2).
    private static Task<CoreWebView2Environment>? _env;

    public static Task<CoreWebView2Environment> WebEnv() => _env ??= CreateEnv();

    /// Trình duyệt đã chạy với cài đặt «Vượt chặn nhà mạng» lúc đó (đổi cài đặt thì phải mở lại app).
    public static int WebEnvUnblock { get; private set; } = -1;

    private static Task<CoreWebView2Environment> CreateEnv()
    {
        // Trình phát tự phát không cần bấm (app điều khiển bằng phím / remote)
        var args = "--autoplay-policy=no-user-gesture-required --disable-features=msSmartScreenProtection";
        WebEnvUnblock = Unblock.Level;
        if (WebEnvUnblock > 0) args += $" --proxy-server=http://127.0.0.1:{Unblock.StartProxy()}";
        return CoreWebView2Environment.CreateAsync(null, Path.Combine(Prefs.DataDir, "WebView2"),
            new CoreWebView2EnvironmentOptions { AdditionalBrowserArguments = args, Language = "vi-VN" });
    }

    /// Mở lại app (vd sau khi đổi «Vượt chặn nhà mạng»).
    public static void Restart()
    {
        Prefs.Flush();
        if (Environment.ProcessPath is { } exe)
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(exe) { UseShellExecute = false });
        Current.Shutdown();
    }

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        // Thử trên máy: thư mục dữ liệu riêng (CONBANG_DATA_DIR) để không đụng dữ liệu đang dùng
        Prefs.Load(Environment.GetEnvironmentVariable("CONBANG_DATA_DIR"));
        Log.Start();
        Log.D("app", $"Mở bản {UpdateCheck.CurrentVersion} ({string.Join(' ', e.Args)})");
        UpdateCheck.Cleanup();
        DispatcherUnhandledException += OnUnhandled;
        TaskScheduler.UnobservedTaskException += (_, ev) =>
        {
            Log.D("lỗi", ev.Exception.ToString());
            ev.SetObserved();
        };

        string? webView;
        try
        {
            webView = CoreWebView2Environment.GetAvailableBrowserVersionString();
        }
        catch (Exception)
        {
            webView = null;
        }
        if (string.IsNullOrEmpty(webView))
        {
            var r = MessageBox.Show(
                "Máy chưa có Microsoft Edge WebView2 Runtime (Windows 10 / 11 cập nhật thường có sẵn) — app cần nó để phát phim.\n\n" +
                "Nhấn OK để mở trang tải WebView2 (chọn «Evergreen Bootstrapper»), cài xong thì mở lại app.",
                "Côn Bằng TV", MessageBoxButton.OKCancel, MessageBoxImage.Warning);
            if (r == MessageBoxResult.OK) UI.Ui.OpenBrowser("https://developer.microsoft.com/microsoft-edge/webview2/");
            Shutdown();
            return;
        }
        Log.D("app", $"WebView2 {webView}");
        var w = new MainWindow();
        MainWindow = w;
        w.Show();
    }

    private void OnUnhandled(object sender, DispatcherUnhandledExceptionEventArgs e)
    {
        Log.D("lỗi", e.Exception.ToString());
        e.Handled = true;
        (MainWindow as MainWindow)?.Toast("Có lỗi: " + Util.Friendly(e.Exception));
    }

    protected override void OnExit(ExitEventArgs e)
    {
        Prefs.Flush();
        base.OnExit(e);
    }
}
