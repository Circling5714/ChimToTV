using System.Globalization;
using System.IO;
using System.Reflection;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Threading;
using ConBangTV.Core;
using ConBangTV.UI;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;

namespace ConBangTV.Player;

/// Trình phát (bản Android: PlayerActivity.kt). Video phát trong WebView2 bằng đúng cách của bản TV:
/// - trang có server (player.php / Next.js / API): trang bọc (player.html) chứa iframe trình phát của trang, hoặc hls.html
///   tự phát luồng .m3u8 (bỏ quảng cáo chèn giữa phim). Trang bọc được trả lời tại chính địa chỉ trang tập (như
///   loadDataWithBaseURL của Android) để trình phát nhúng thấy Referer của trang phim.
/// - trang không chia server (pageMode): mở thẳng trang tập, page-focus.js phóng trình phát ra cả màn hình.
/// player-agent.js chạy trong mọi khung (AddScriptToExecuteOnDocumentCreated), điều khiển &lt;video&gt; và báo trạng thái.
public sealed class PlayerView : UserControl
{
    private static readonly double[] Rates = { 0.75, 1.0, 1.25, 1.5, 2.0 };

    /// CSS của agent (agent thêm lúc tài liệu vừa tạo có thể chưa có &lt;head&gt; → thêm lại khi trang dựng xong).
    private const string StyleShim =
        "(function(){var css='.jw-controlbar,.jw-title{display:none!important}.chimto-pin{position:fixed!important;left:0!important;" +
        "top:0!important;right:auto!important;bottom:auto!important;width:100vw!important;height:100vh!important;max-width:none!important;" +
        "max-height:none!important;margin:0!important;transform:none!important;z-index:2147483646!important;background:#000!important}';" +
        "function add(){try{if(document.getElementById('chimto-css'))return;var s=document.createElement('style');s.id='chimto-css';" +
        "s.textContent=css;(document.head||document.documentElement).appendChild(s);}catch(e){}}" +
        "if(document.documentElement)add();document.addEventListener('DOMContentLoaded',add);})();\n";

    private static PlayerView? _instance;

    private readonly MainWindow _main;
    private readonly WebView2 _web;
    private readonly PlayerOverlay _ov;
    private Task? _init;
    private readonly string _agentJs, _focusJs, _wrapperHtml, _hlsHtml;
    private readonly byte[] _hlsJs;
    private string? _scriptId;

    // Trang bọc trả lời tại địa chỉ trang tập (một lần)
    private string? _serveUrl;
    private string? _serveHtml;
    private string? _expectNav;

    private readonly DispatcherTimer _poll = new() { Interval = TimeSpan.FromSeconds(1) };
    private readonly DispatcherTimer _osdTimer = new();
    private readonly DispatcherTimer _noticeTimer = new();
    private readonly DispatcherTimer _hudTimer = new() { Interval = TimeSpan.FromMilliseconds(900) };
    private readonly DispatcherTimer _seekTimer = new() { Interval = TimeSpan.FromMilliseconds(500) };
    private readonly DispatcherTimer _clickTimer = new() { Interval = TimeSpan.FromMilliseconds(260) };

    public bool IsOpen { get; private set; }

    // ---- Trạng thái lần xem (giống PlayerActivity) ----
    private sealed record PlayState(double T, double D, bool Paused, bool Ended, bool Waiting, bool Ready);

    private string _filmUrl = "";
    private FilmDetail? _detail;
    private EpisodeGroup? _group;
    private List<Episode> Episodes => _group?.Episodes ?? new();
    private int _index;
    private bool _resumeOpen;
    private string _openEpUrl = "";
    private int _loadToken;

    private bool _pageMode;
    private EpisodeSource? _source;
    private Server? _server;
    private int _token;
    private int _pageLoad;
    private string? _embedUrl;
    private string? _hlsUrl;
    private string _episodeUrl = "";
    private bool _errShown;
    private long _lastAdAt;

    private PlayState? _state;
    private bool _started;
    private int _resumeAt;
    private bool _resumePending;
    private long _resumeIssuedAt;
    private bool _endedHandled;
    private bool _hintShown;
    private long _loadStartedAt;
    private long _lastSave;
    private double? _seekTarget;
    private double _rate = 1;
    private int _startHint;
    private bool _introDone;
    private int _repeat;
    private bool _dragging;
    private bool _clickThrough;
    private bool _polling;

    // Cửa sổ phim nhỏ (MiniWindow) và giới hạn giờ xem
    private MiniWindow? _miniWin;
    private bool _limitWarned;

    /// Đang ở cửa sổ phim nhỏ (phím / chuột của cửa sổ app dành cho app, không cho trình phát).
    public bool IsMini => _miniWin != null;

    public PlayerView(MainWindow main)
    {
        _instance = this;
        _main = main;
        Background = Brushes.Black;
        Focusable = false;
        _web = new WebView2 { DefaultBackgroundColor = System.Drawing.Color.Black, Focusable = false };
        Content = _web;

        _agentJs = Res("player-agent.js");
        _focusJs = Res("page-focus.js");
        _wrapperHtml = Res("player.html");
        _hlsHtml = Res("hls.html");
        using (var s = Assembly.GetExecutingAssembly().GetManifestResourceStream("hls.light.min.js")!)
        using (var ms = new MemoryStream())
        {
            s.CopyTo(ms);
            _hlsJs = ms.ToArray();
        }

        _ov = new PlayerOverlay { Owner = main };
        SetupOverlay();
        _poll.Tick += async (_, _) => await Poll();
        _osdTimer.Tick += (_, _) => HideOsd();
        _noticeTimer.Tick += (_, _) =>
        {
            _noticeTimer.Stop();
            _ov.NoticeBox.Visibility = Visibility.Collapsed;
        };
        _hudTimer.Tick += (_, _) =>
        {
            _hudTimer.Stop();
            _ov.HudBox.Visibility = Visibility.Collapsed;
        };
        _seekTimer.Tick += (_, _) => FlushSeek();
        _clickTimer.Tick += (_, _) =>
        {
            _clickTimer.Stop();
            Toggle();
        };
        SizeChanged += (_, _) => Follow();
    }

    private static string Res(string name)
    {
        using var s = Assembly.GetExecutingAssembly().GetManifestResourceStream(name)!;
        using var r = new StreamReader(s);
        return r.ReadToEnd();
    }

    // ---- WebView2 ----

    private Task EnsureWeb() => _init ??= InitWeb();

    private async Task InitWeb()
    {
        var env = await App.WebEnv();
        await _web.EnsureCoreWebView2Async(env);
        var c = _web.CoreWebView2;
        var st = c.Settings;
        st.AreDefaultContextMenusEnabled = false;
        st.IsStatusBarEnabled = false;
        st.IsZoomControlEnabled = false;
        st.AreBrowserAcceleratorKeysEnabled = false;
        st.IsGeneralAutofillEnabled = false;
        st.IsPasswordAutosaveEnabled = false;
        st.IsSwipeNavigationEnabled = false;
#if DEBUG
        st.AreDevToolsEnabled = true;
        // Thử trên máy làm việc: tắt tiếng (biến môi trường CONBANG_MUTE=1)
        if (Environment.GetEnvironmentVariable("CONBANG_MUTE") == "1") c.IsMuted = true;
#else
        st.AreDevToolsEnabled = false;
#endif
        Http.UserAgent = st.UserAgent;
        c.NewWindowRequested += (_, e) => e.Handled = true; // cửa sổ quảng cáo
        c.NavigationStarting += OnNavigationStarting;
        c.DOMContentLoaded += (_, _) => InjectPage();
        c.NavigationCompleted += (_, _) => InjectPage();
        c.ProcessFailed += (_, e) =>
        {
            Log.D("player", $"Trình duyệt lỗi: {e.ProcessFailedKind}");
            if (IsOpen) ShowError("Trình duyệt bị lỗi khi phát.\nNhấn ▼ → «Tải lại».");
        };
        foreach (var f in AdBlock.Filters()) c.AddWebResourceRequestedFilter(f, CoreWebView2WebResourceContext.All);
        c.AddWebResourceRequestedFilter("*/__chimto/hls.js", CoreWebView2WebResourceContext.All);
        c.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.Document);
        c.WebResourceRequested += OnResource;
        Log.D("player", $"WebView2 sẵn sàng, UA: {st.UserAgent}");
    }

    private void OnResource(object? sender, CoreWebView2WebResourceRequestedEventArgs e)
    {
        var env = _web.CoreWebView2.Environment;
        var uri = e.Request.Uri;
        if (!Uri.TryCreate(uri, UriKind.Absolute, out var u)) return;
        if (AdBlock.IsAd(u.Host))
        {
            e.Response = env.CreateWebResourceResponse(null, 403, "Blocked", "Content-Type: text/plain");
            return;
        }
        if (u.AbsolutePath.EndsWith("/__chimto/hls.js"))
        {
            e.Response = env.CreateWebResourceResponse(new MemoryStream(_hlsJs), 200, "OK",
                "Content-Type: application/javascript; charset=utf-8\nCache-Control: no-store");
            return;
        }
        if (e.ResourceContext == CoreWebView2WebResourceContext.Document && _serveUrl != null && _serveHtml != null &&
            e.Request.Method == "GET" && SameUrl(uri, _serveUrl))
        {
            var bytes = System.Text.Encoding.UTF8.GetBytes(_serveHtml);
            _serveUrl = null;
            e.Response = env.CreateWebResourceResponse(new MemoryStream(bytes), 200, "OK",
                "Content-Type: text/html; charset=utf-8\nCache-Control: no-store");
        }
    }

    private static bool SameUrl(string a, string b) =>
        Uri.TryCreate(a, UriKind.Absolute, out var x) && Uri.TryCreate(b, UriKind.Absolute, out var y) &&
        string.Equals(x.GetLeftPart(UriPartial.Query), y.GetLeftPart(UriPartial.Query), StringComparison.OrdinalIgnoreCase);

    /// Chặn mọi điều hướng cả trang do trang tự gây ra (quảng cáo); app tự nạp trang. Chuyển hướng phía máy chủ thì cho qua.
    private void OnNavigationStarting(object? sender, CoreWebView2NavigationStartingEventArgs e)
    {
        if (_expectNav != null && SameUrl(e.Uri, _expectNav))
        {
            _expectNav = null;
            return;
        }
        if (e.IsRedirected || e.Uri == "about:blank") return;
        Log.D("player", "Chặn điều hướng: " + e.Uri);
        e.Cancel = true;
    }

    private void InjectPage()
    {
        if (!_pageMode || _web.CoreWebView2 == null) return;
        _ = _web.CoreWebView2.ExecuteScriptAsync(_focusJs);
    }

    /// Nạp trang (html != null: trả lời bằng trang bọc của app tại địa chỉ url).
    private async Task Navigate(string url, string? html, int start)
    {
        await EnsureWeb();
        var c = _web.CoreWebView2;
        if (_scriptId != null) c.RemoveScriptToExecuteOnDocumentCreated(_scriptId);
        var js = $"window.__chimtoStart={start};if(window.top===window)window.__chimtoLoad={_pageLoad};\n" + StyleShim + _agentJs;
        _scriptId = await c.AddScriptToExecuteOnDocumentCreatedAsync(js);
        _serveUrl = html != null ? url : null;
        _serveHtml = html;
        _expectNav = url;
        c.Navigate(url);
    }

    private void Blank()
    {
        if (_web.CoreWebView2 == null) return;
        _expectNav = "about:blank";
        _serveUrl = null;
        _web.CoreWebView2.Navigate("about:blank");
    }

    private void Cmd(string name, double? arg = null)
    {
        if (_web.CoreWebView2 == null) return;
        var a = arg?.ToString(CultureInfo.InvariantCulture) ?? "null";
        _ = _web.CoreWebView2.ExecuteScriptAsync($"window.chimtoCmd&&chimtoCmd('{name}',{a})");
    }

    /// Xoá cookie / bộ nhớ đệm của trình duyệt trong app và ảnh bìa đã lưu.
    public static async Task ClearBrowsingData()
    {
        try
        {
            if (_instance?._web.CoreWebView2 is { } c)
            {
                await c.Profile.ClearBrowsingDataAsync();
            }
            else
            {
                var env = await App.WebEnv();
                var hwnd = new WindowInteropHelper(Application.Current.MainWindow).Handle;
                var ctl = await env.CreateCoreWebView2ControllerAsync(hwnd);
                await ctl.CoreWebView2.Profile.ClearBrowsingDataAsync();
                ctl.Close();
            }
        }
        catch (Exception e)
        {
            Log.D("player", "Xoá dữ liệu lỗi: " + e.Message);
        }
        try
        {
            var img = Path.Combine(Prefs.DataDir, "img");
            if (Directory.Exists(img)) Directory.Delete(img, true);
        }
        catch (Exception)
        {
        }
    }

    // ---- Mở / đóng ----

    public void Open(string filmUrl, string epUrl, bool resume)
    {
        if (IsMini) ExitMini(false);
        var wasOpen = IsOpen;
        IsOpen = true;
        if (wasOpen)
        {
            _token++;
            StopVideo();
        }
        HidePanel();
        _ov.Show();
        Follow();
        _ov.Activate();
        _filmUrl = filmUrl;
        _openEpUrl = epUrl;
        _resumeOpen = resume;
        _detail = null;
        // Phim mới chưa tải xong / tải lỗi: bảng ▼ không còn server, bản, tập của phim trước
        _source = null;
        _group = null;
        _index = 0;
        _limitWarned = false;
        _poll.Start();
        ApplyOverlayScale();
        var t = ++_loadToken;
        if (Site.CachedDetail(filmUrl) is { } cached)
        {
            Start(cached, epUrl);
            return;
        }
        ShowLoading("Đang tải thông tin phim…");
        _ov.TitleText.Text = "";
        _main.Title = "Côn Bằng TV";
        _ = LoadDetail(t, filmUrl, epUrl);
    }

    private async Task LoadDetail(int t, string filmUrl, string epUrl)
    {
        try
        {
            var d = await Site.Detail(filmUrl);
            if (t != _loadToken || !IsOpen) return;
            Start(d, epUrl);
        }
        catch (Exception e)
        {
            if (t != _loadToken || !IsOpen) return;
            ShowError("Không tải được phim.\n" + Util.Friendly(e));
        }
    }

    private void Start(FilmDetail d, string epUrl)
    {
        _detail = d;
        // Phim đang theo dõi: đã vào xem thì không còn là "tập mới"
        if (d.LatestEpisode is { } last) Prefs.UpdateFollow(d.Url, d.EpisodeCount, last.Title, true);
        var g = d.GroupOf(epUrl) ?? d.Groups.FirstOrDefault();
        if (g == null)
        {
            ShowError("Phim này chưa có tập nào.");
            return;
        }
        _group = g;
        var i = Math.Max(0, g.Episodes.FindIndex(e => e.Url == epUrl));
        var next = _resumeOpen && Prefs.Finished(epUrl) && i + 1 < g.Episodes.Count;
        OpenEpisode(next ? i + 1 : i);
    }

    /// Thoát trình phát (lưu chỗ xem, về trang trước).
    public void Close(bool notify = true)
    {
        if (!IsOpen) return;
        if (IsMini) ExitMini(false);
        IsOpen = false;
        _token++;
        _loadToken++;
        StopVideo();
        WatchLimit.Flush();
        _poll.Stop();
        _clickTimer.Stop();
        ExitClickThrough();
        HidePanel();
        _ov.Hide();
        _main.Title = "Côn Bằng TV";
        if (notify) _main.PlayerClosed();
    }

    /// Cửa sổ chính được chọn (vd bấm thanh tác vụ): đưa lớp phủ lên lại để giữ phím.
    public void Reactivate()
    {
        if (!IsOpen || _clickThrough || IsMini) return;
        Dispatcher.BeginInvoke(() =>
        {
            if (IsOpen && _ov.IsVisible) _ov.Activate();
        }, DispatcherPriority.Input);
    }

    /// Đặt lớp phủ đúng chỗ trình phát (theo cửa sổ chính).
    public void Follow()
    {
        if (!IsOpen || IsMini) return;
        if (_main.WindowState == WindowState.Minimized) return;
        var src = PresentationSource.FromVisual(this);
        if (src?.CompositionTarget == null || ActualWidth <= 0) return;
        var m = src.CompositionTarget.TransformFromDevice;
        var a = m.Transform(PointToScreen(new Point(0, 0)));
        var b = m.Transform(PointToScreen(new Point(ActualWidth, ActualHeight)));
        _ov.Left = a.X;
        _ov.Top = a.Y;
        _ov.Width = Math.Max(1, b.X - a.X);
        _ov.Height = Math.Max(1, b.Y - a.Y);
        ApplyOverlayScale();
    }

    private void ApplyOverlayScale()
    {
        // Màn nhỏ: thu nhỏ thanh điều khiển cho vừa; màn lớn theo cỡ giao diện
        var s = Prefs.UiScale * Math.Clamp(_ov.Width / 1280, 0.75, 1.0);
        _ov.Scaled.LayoutTransform = new ScaleTransform(s, s);
        _ov.Panel.Width = Math.Min(470, Math.Max(300, _ov.Width / s * 0.45));
    }

    // ---- Mở tập / server ----

    private void OpenEpisode(int i)
    {
        var d = _detail;
        if (d == null || i < 0 || i >= Episodes.Count) return;
        var ep = Episodes[i];
        // Người đang xem đã hết giờ xem: chưa mở tập, hỏi mã phụ huynh trước
        if (WatchLimit.Blocked() is { } blocked)
        {
            LimitBlock(blocked, d.Url, ep.Url);
            return;
        }
        _index = i;
        var t = ++_token;
        StopVideo();
        _episodeUrl = ep.Url;
        _source = null;
        var groupName = d.Groups.Count > 1 ? $" ({_group?.Name})" : "";
        _ov.TitleText.Text = $"{d.Title} — {ep.Title}{groupName}";
        _main.Title = $"{d.Title} — {ep.Title} · Côn Bằng TV";
        _resumeAt = Prefs.PositionOf(ep.Url);
        _introDone = false;
        Prefs.AddHistory(new HistoryItem
        {
            FilmUrl = d.Url, Title = d.Title, Poster = d.Poster, EpTitle = ep.Title, EpUrl = ep.Url, Time = Util.NowMs(),
        });
        ShowLoading($"Đang mở {ep.Title}…");
        ShowOsd();
        _ = LoadSource(t, d, _group, ep);
    }

    private async Task LoadSource(int t, FilmDetail d, EpisodeGroup? group, Episode ep)
    {
        try
        {
            // Trang tập + server dự phòng KKPhim (tải song song; trang lỗi mà KKPhim có tập này thì vẫn phát được)
            var src = await KkPhim.Source(d, group, ep);
            if (t != _token || !IsOpen) return;
            _source = src;
            if (src.PageOnly)
            {
                await PlayPage(src);
                return;
            }
            var host = Site.HostOf(src.EpUrl);
            var usual = src.Servers.FirstOrDefault(s => s.Type == Prefs.ServerType(host)) ?? src.Servers.FirstOrDefault();
            await PlayServer(usual);
        }
        catch (Exception e)
        {
            if (t != _token || !IsOpen) return;
            ShowError($"Không mở được {ep.Title}.\n{Util.Friendly(e)}");
        }
    }

    /// Mở thẳng trang tập: trang tự nạp trình phát, page-focus.js phóng nó ra toàn màn hình.
    private async Task PlayPage(EpisodeSource src, Server? asServer = null)
    {
        StopVideo();
        _server = asServer;
        _pageMode = true;
        _loadStartedAt = Util.NowMs();
        ShowLoading("Đang mở trang tập phim…");
        _startHint = StartFor();
        await Navigate(src.EpUrl, null, _startHint);
    }

    private async Task PlayServer(Server? s)
    {
        var src = _source;
        if (src == null) return;
        // "Trình phát của trang" (trang mở thẳng trang tập, có thêm server KKPhim)
        if (s?.Type == EpisodeSource.PageServer)
        {
            ++_token;
            await PlayPage(src, s);
            return;
        }
        _server = s;
        var t = ++_token;
        StopVideo();
        ShowLoading($"Đang tải {s?.Label ?? "trình phát"}…");
        try
        {
            var v = await Site.VideoOf(src, s);
            if (t != _token || !IsOpen) return;
            _loadStartedAt = Util.NowMs();
            _startHint = StartFor();
            var isHls = v.File || Uri.TryCreate(v.Url, UriKind.Absolute, out var vu) && vu.AbsolutePath.EndsWith(".m3u8");
            string page;
            if (isHls)
            {
                _hlsUrl = v.Url;
                page = _hlsHtml.Replace("{{SRC}}", JsonSerializer.Serialize(v.Url))
                    .Replace("{{START}}", _startHint.ToString(CultureInfo.InvariantCulture))
                    .Replace("{{SUB}}", v.Subtitle != null ? JsonSerializer.Serialize(v.Subtitle) : "null")
                    .Replace("{{AGENT}}", "");
            }
            else
            {
                _embedUrl = v.Url;
                page = _wrapperHtml.Replace("{{SRC}}", System.Net.WebUtility.HtmlEncode(v.Url));
            }
            Log.D("player", $"Phát {(isHls ? "HLS" : "iframe")} {v.Url} (server {s?.Label}, bắt đầu {_startHint}s)");
            await Navigate(src.EpUrl, page, _startHint);
        }
        catch (Exception e)
        {
            if (t != _token || !IsOpen) return;
            ShowError($"{Util.Friendly(e)}\nNhấn ▼ (hoặc ☰) để chọn server khác.");
            ShowPanel();
        }
    }

    private void StopVideo()
    {
        SavePosition();
        _pageMode = false;
        _embedUrl = null;
        _hlsUrl = null;
        _errShown = false;
        _lastAdAt = 0;
        _state = null;
        _started = false;
        _resumePending = false;
        _endedHandled = false;
        _hintShown = false;
        _seekTarget = null;
        _seekTimer.Stop();
        _pageLoad++;
        Blank();
    }

    /// Đổi server (nút trong bảng tuỳ chọn): nhớ lựa chọn, xem tiếp từ chỗ đang xem.
    private void SwitchServer(Server s)
    {
        var src = _source;
        if (src == null) return;
        Prefs.SetServer(Site.HostOf(src.EpUrl), s.Type, s.Label);
        if (CurrentTime() is { } t) _resumeAt = t;
        HidePanel();
        _ = PlayServer(s);
    }

    private void Reload()
    {
        HidePanel();
        if (CurrentTime() is { } t) _resumeAt = t;
        var src = _source;
        if (_detail == null) Open(_filmUrl, _openEpUrl, _resumeOpen); // chưa tải được thông tin phim: tải lại
        else if (src == null) OpenEpisode(_index);
        else if (src.PageOnly) _ = PlayPage(src);
        else _ = PlayServer(_server);
    }

    private void Next()
    {
        if (_index + 1 < Episodes.Count) OpenEpisode(_index + 1);
        else Notice("Đây là tập mới nhất.");
    }

    private void Prev()
    {
        if (_index > 0) OpenEpisode(_index - 1);
        else Notice("Đây là tập đầu tiên.");
    }

    /// Sang bản khác (Vietsub / Thuyết Minh…): mở tập cùng tên và xem tiếp đúng chỗ; bản đó chưa có tập này thì mở tập cùng vị trí.
    private void SwitchGroup(EpisodeGroup g)
    {
        HidePanel();
        if (g == _group || _detail == null) return;
        var cur = _index < Episodes.Count ? Episodes[_index] : null;
        var same = g.Episodes.FindIndex(e => e.Title == cur?.Title);
        var target = same >= 0 ? same : Math.Min(_index, g.Episodes.Count - 1);
        if (same >= 0)
        {
            if (CurrentTime() is { } t) Prefs.SavePosition(g.Episodes[target].Url, t, _detail.Url);
        }
        else
        {
            Notice($"{g.Name} chưa có {cur?.Title} — mở {g.Episodes[target].Title}.", 6000);
        }
        _group = g;
        OpenEpisode(target);
    }

    // ---- Trạng thái phát ----

    private async Task Poll()
    {
        if (!IsOpen || _polling || _web.CoreWebView2 == null) return;
        _polling = true;
        string? raw;
        try
        {
            raw = await _web.CoreWebView2.ExecuteScriptAsync("JSON.stringify([window.__chimtoLoad||0,window.__chimtoState||null])");
        }
        catch (Exception)
        {
            raw = null;
        }
        finally
        {
            _polling = false;
        }
        OnState(raw);
    }

    private void OnState(string? raw)
    {
        Dictionary<string, object?>? o = null;
        try
        {
            if (raw != null && JsonSerializer.Deserialize<string>(raw) is { } s && JsonScan.Parse(s) is List<object?> arr &&
                arr.Count == 2 && arr[0] is long load && load == _pageLoad)
                o = arr[1] as Dictionary<string, object?>;
        }
        catch (Exception e)
        {
            Log.D("player", "Đọc trạng thái lỗi: " + e.Message);
        }
        var now = Util.NowMs();
        var inAd = o != null && J.Bool(o, "ad");
        if (inAd) _lastAdAt = now;
        var waited = now - Math.Max(_loadStartedAt, _lastAdAt);
        if (!_started && !_errShown && !_hintShown && !inAd && (_embedUrl != null || _hlsUrl != null || _pageMode) && waited > 25_000)
        {
            _hintShown = true;
            HideLoading();
            var other = _source?.PageOnly != false ? "▼ → «Tải lại» / «Bấm thẳng vào trang»" : "▼ để đổi server";
            Notice($"Video chưa chạy? Nhấn Enter để thử phát, hoặc {other}.", 10_000);
        }
        if (o == null) return;
        if (inAd)
        {
            // Trình phát đang chạy quảng cáo (agent tự bỏ qua khi được phép) — chưa phải phim
            if (!_started) ShowLoading("Trình phát đang chạy quảng cáo — tự bỏ qua khi được phép…");
            return;
        }
        double Num(string k) => o.TryGetValue(k, out var v) ? v switch { long l => l, double d => d, _ => 0 } : 0;
        var st = new PlayState(Num("t"), Num("d"), J.Bool(o, "paused"), J.Bool(o, "ended"), J.Bool(o, "waiting"), J.Bool(o, "ready"));
        var wasPaused = _state?.Paused;
        _state = st;
        var err = J.Str(o, "err");
        if (err.Length > 0 && !_errShown)
        {
            _errShown = true;
            if (_started)
            {
                Notice($"Mất kết nối video ({err}).\nNhấn ▼ → «Tải lại», hoặc chọn server khác.", 10_000);
            }
            else
            {
                ShowError($"{_server?.Label ?? "Server"} không phát được video ({err}).\nNhấn ▼ để chọn server khác.");
                ShowPanel();
            }
        }
        WatchLimit.Tick(_started && !st.Paused && !st.Waiting, now);
        if (CheckLimit()) return;
        _miniWin?.Update(_ov.TitleText.Text, st.Paused);
        // Bắt đầu khi video thật sự chạy (không phải lúc mới nạp xong mà còn dừng)
        if (!_started && st.Ready && !st.Paused)
        {
            _started = true;
            HideLoading();
            Log.D("player", $"Video chạy t={(int)st.T}, báo trước {_startHint}");
            if (_resumeAt > 30 && (st.D <= 0 || _resumeAt < st.D - 30))
            {
                _introDone = true;
                // Trình phát đã bắt đầu đúng chỗ (app báo trước) thì khỏi tua lại
                if (Math.Abs(st.T - _resumeAt) > 10)
                {
                    _resumePending = true;
                    _resumeIssuedAt = now;
                    Cmd("seekTo", _resumeAt);
                }
                Notice($"Xem tiếp từ {Util.FormatTime(_resumeAt)}");
            }
            else if (_startHint > 0 && st.T >= _startHint - 10)
            {
                _introDone = true;
                Notice($"Đã bỏ qua nhạc mở đầu (tới {Util.FormatTime(_startHint)}) — ◀ để xem lại.");
            }
            if (Math.Abs(_rate - 1) > 0.01) Cmd("rate", _rate);
            if (Prefs.VideoFill) Cmd("fit", 1);
            ShowOsd();
        }
        if (_resumePending && (st.T >= _resumeAt - 5 || now - _resumeIssuedAt > 30_000)) _resumePending = false;
        if (_started && !_introDone && !st.Paused) SkipIntro(st);
        if (_ov.Osd.Visibility == Visibility.Visible) UpdateOsd();
        if (wasPaused != null && wasPaused != st.Paused) ShowOsd();
        if (_started && now - _lastSave > 5000)
        {
            _lastSave = now;
            SavePosition();
        }
        if (st.Ended && !_endedHandled)
        {
            _endedHandled = true;
            if (Prefs.AutoNext && _index + 1 < Episodes.Count)
            {
                Notice($"Hết tập — chuyển sang {Episodes[_index + 1].Title}");
                OpenEpisode(_index + 1);
            }
        }
    }

    // ---- Giới hạn giờ xem (WatchLimit) ----

    /// Mỗi lần đọc trạng thái: sắp hết giờ thì nhắc, hết giờ thì dừng phim, thoát trình phát và hỏi mã PIN phụ huynh.
    /// true = đã chặn.
    private bool CheckLimit()
    {
        var b = WatchLimit.Blocked();
        if (b == null)
        {
            if (_started && !_limitWarned && WatchLimit.Remaining() is { } r and > 0 and <= 300)
            {
                _limitWarned = true;
                Notice($"Còn {WatchLimit.MinutesText(r)} xem hôm nay ({Prefs.CurrentProfile.Name}).", 8000);
            }
            return false;
        }
        Cmd("pause");
        LimitBlock(b.Value, _detail?.Url ?? _filmUrl, _episodeUrl);
        return true;
    }

    private void LimitBlock(WatchLimit.Block b, string film, string ep)
    {
        Log.D("player", $"Hết giờ xem ({b}) — {Prefs.CurrentProfile.Name}");
        Close();
        _ = _main.LimitReached(b, film, ep);
    }

    // ---- Cửa sổ phim nhỏ ----

    /// Thu nhỏ: chuyển trình phát sang cửa sổ nhỏ góc màn hình, cửa sổ app trở lại các trang.
    public void EnterMini()
    {
        if (!IsOpen || IsMini) return;
        HidePanel();
        ExitClickThrough();
        _ov.Hide();
        Content = null;
        _miniWin = new MiniWindow(Toggle, () => ExitMini(true), () => Close());
        _miniWin.Host.Content = _web;
        _miniWin.Update(_ov.TitleText.Text, _state?.Paused);
        var mw = _miniWin;
        mw.Closed += (_, _) =>
        {
            // Đóng bằng Alt+F4: lấy trình phát về rồi tắt phim
            if (_miniWin != mw) return;
            _miniWin = null;
            mw.Host.Content = null;
            Content = _web;
            Dispatcher.BeginInvoke(() => Close());
        };
        _miniWin.Show();
        _main.PlayerMinimized();
    }

    /// Phóng to lại (show = hiện trình phát trong cửa sổ app); false khi sắp đóng / mở phim khác.
    public void ExitMini(bool show)
    {
        var w = _miniWin;
        if (w == null) return;
        _miniWin = null;
        w.Host.Content = null;
        Content = _web;
        w.Close();
        if (!show) return;
        _main.PlayerRestored();
        _ov.Show();
        Follow();
        _ov.Activate();
        ShowOsd();
    }

    private string IntroKey => _detail?.Url ?? _filmUrl;

    /// Phim đã đánh dấu nhạc mở đầu: video vừa chạy tới mốc đầu (trễ tối đa 8 giây) thì tua tới mốc hết, một lần mỗi tập.
    private void SkipIntro(PlayState s)
    {
        if (Prefs.Intro(IntroKey) is not { } mark || mark.End <= mark.Start || _seekTarget != null) return;
        if (s.T >= mark.Start + 8 || s.T >= mark.End - 3)
        {
            _introDone = true;
            return;
        }
        if (s.T < mark.Start - 1 || (s.D > 0 && mark.End > s.D - 30)) return;
        _introDone = true;
        Cmd("seekTo", mark.End);
        Notice($"Đã bỏ qua nhạc mở đầu (tới {Util.FormatTime(mark.End)}) — ◀ để xem lại.");
    }

    /// Chỗ bắt đầu báo trước cho lần nạp trang này: chỗ xem dở; hoặc hết nhạc mở đầu nếu nhạc nằm ngay đầu tập.
    private int StartFor()
    {
        if (_resumeAt > 30) return _resumeAt;
        if (Prefs.Intro(IntroKey) is not { } mark) return 0;
        return !_introDone && mark.Start <= 1 && mark.End > mark.Start ? mark.End : 0;
    }

    /// Chỗ đang xem để mang sang (đổi server / tải lại / đổi bản).
    private int? CurrentTime() => _resumePending ? _resumeAt : _state is { } s && _started && s.T > 5 ? (int)s.T : null;

    private void SavePosition()
    {
        var s = _state;
        if (s == null || !_started || _episodeUrl.Length == 0 || _resumePending) return;
        // Xem gần hết thì coi như xong (lần sau mở lại từ đầu)
        var done = s.Ended || (s.D > 0 && s.T > s.D - 90);
        Prefs.SavePosition(_episodeUrl, done ? 0 : (int)s.T, _detail?.Url ?? _filmUrl);
    }

    // ---- Điều khiển ----

    private void Toggle()
    {
        Cmd(_started ? "toggle" : "play");
        ShowOsd();
    }

    private double Step(int repeat) => Prefs.SeekStep * Math.Min(4.0, 1 + repeat / 8.0);

    /// Gộp các lần bấm tua liên tiếp thành một lệnh tua (đỡ giật khi giữ phím).
    private void SeekBy(double delta)
    {
        var s = _state;
        if (s == null || !_started) return;
        var max = s.D > 0 ? s.D - 1 : double.MaxValue;
        _seekTarget = Math.Clamp((_seekTarget ?? s.T) + delta, 0, max);
        ShowOsd();
        _seekTimer.Stop();
        _seekTimer.Start();
    }

    private void SeekTo(double target)
    {
        _seekTarget = target;
        ShowOsd();
        _seekTimer.Stop();
        FlushSeek();
    }

    private void FlushSeek()
    {
        _seekTimer.Stop();
        if (_seekTarget is not { } target) return;
        _seekTarget = null;
        _resumePending = false; // người xem tự tua: từ giờ lấy theo trình phát
        Cmd("seekTo", target);
        if (_state != null) _state = _state with { T = target };
        UpdateOsd();
    }

    public void HandleKey(KeyEventArgs e)
    {
        var key = e.Key == Key.System ? e.SystemKey : e.Key;
        _repeat = e.IsRepeat ? _repeat + 1 : 0;
        if (_clickThrough)
        {
            if (key == Key.Escape)
            {
                e.Handled = true;
                ExitClickThrough();
            }
            return;
        }
        if (key == Key.F11 || (key == Key.Enter && Keyboard.Modifiers == ModifierKeys.Alt) || key == Key.F)
        {
            e.Handled = true;
            if (!e.IsRepeat) _main.ToggleFullscreen();
            return;
        }
        if (_ov.Panel.Visibility == Visibility.Visible)
        {
            if (key is Key.Escape or Key.BrowserBack or Key.Back or Key.M or Key.Apps)
            {
                e.Handled = true;
                HidePanel();
                return;
            }
            if (KeyNav.DirOf(key) is { } dir)
            {
                e.Handled = true;
                var focused = Keyboard.FocusedElement as FrameworkElement;
                if (focused == null || !KeyNav.IsInside(focused, _ov.PanelStack))
                {
                    if (KeyNav.First(_ov.PanelStack) is { } f) KeyNav.FocusAndShow(f);
                    return;
                }
                if (KeyNav.Next(_ov.PanelStack, focused, dir) is { } n) KeyNav.FocusAndShow(n);
            }
            return; // Enter / Space: nút đang chọn tự xử lý
        }
        var first = !e.IsRepeat;
        e.Handled = true;
        switch (key)
        {
            case Key.Escape or Key.BrowserBack or Key.Back:
                if (first) Close();
                break;
            case Key.W:
                if (first) EnterMini();
                break;
            case Key.Space or Key.Enter or Key.K or Key.MediaPlayPause or Key.Play:
                if (first) Toggle();
                break;
            case Key.MediaStop or Key.Pause:
                if (first) Cmd("pause");
                break;
            case Key.Left or Key.J:
                SeekBy(-Step(_repeat));
                break;
            case Key.Right or Key.L:
                SeekBy(Step(_repeat));
                break;
            case Key.Up or Key.I:
                ShowOsd(6000);
                break;
            case Key.Down or Key.M or Key.Apps:
                if (first) ShowPanel();
                break;
            case Key.N or Key.PageDown or Key.MediaNextTrack:
                if (first) Next();
                break;
            case Key.P or Key.PageUp or Key.MediaPreviousTrack:
                if (first) Prev();
                break;
            case >= Key.D0 and <= Key.D9 or >= Key.NumPad0 and <= Key.NumPad9:
                var n = key >= Key.NumPad0 ? key - Key.NumPad0 : key - Key.D0;
                if (first && _state is { D: > 0 } s) SeekTo(s.D * n / 10.0);
                break;
            default:
                e.Handled = false;
                break;
        }
    }

    // ---- Giao diện phủ ----

    private void SetupOverlay()
    {
        var o = _ov;
        o.PreviewKeyDown += (_, e) => HandleKey(e);
        o.BtnBack.Click += (_, _) => Close();
        o.BtnFull.Click += (_, _) => _main.ToggleFullscreen();
        o.BtnMini.Click += (_, _) => EnterMini();
        o.BtnPlay.Click += (_, _) => Toggle();
        o.BtnBack10.Click += (_, _) => SeekBy(-10);
        o.BtnFwd10.Click += (_, _) => SeekBy(10);
        o.BtnPrev.Click += (_, _) => Prev();
        o.BtnNext.Click += (_, _) => Next();
        o.BtnMenu.Click += (_, _) => ShowPanel();
        o.BtnPanelClose.Click += (_, _) => HidePanel();
        o.BtnDone.Click += (_, _) => ExitClickThrough();
        o.Closing += (_, e) =>
        {
            // Alt+F4 trên lớp phủ = thoát phim, không đóng hẳn
            e.Cancel = true;
            Close();
        };
        o.HitArea.MouseMove += (_, _) =>
        {
            if (_clickThrough) return;
            o.HitArea.Cursor = Cursors.Arrow;
            ShowOsd(3000);
        };
        o.HitArea.MouseLeftButtonDown += (_, e) =>
        {
            if (_clickThrough || (!ReferenceEquals(e.OriginalSource, o.HitArea) && !ReferenceEquals(e.OriginalSource, o.Scaled))) return;
            if (o.Panel.Visibility == Visibility.Visible)
            {
                HidePanel();
                return;
            }
            if (e.ClickCount >= 2)
            {
                _clickTimer.Stop();
                _main.ToggleFullscreen();
                return;
            }
            _clickTimer.Stop();
            _clickTimer.Start();
        };
        // Thanh trượt tự đánh dấu đã xử lý sự kiện chuột (nhảy tới chỗ bấm) → nghe cả sự kiện đã xử lý
        o.Seek.AddHandler(UIElement.PreviewMouseLeftButtonDownEvent, new MouseButtonEventHandler((_, _) => _dragging = true), true);
        o.Seek.AddHandler(UIElement.PreviewMouseLeftButtonUpEvent, new MouseButtonEventHandler((_, _) =>
        {
            if (!_dragging) return;
            _dragging = false;
            if (_state is { D: > 0 }) SeekTo(o.Seek.Value);
        }), true);
        o.Seek.ValueChanged += (_, _) =>
        {
            if (_dragging && _state is { D: > 0 } s) o.TimeText.Text = $"{Util.FormatTime(o.Seek.Value)} / {Util.FormatTime(s.D)}";
        };
        o.Deactivated += (_, _) => _repeat = 0;
    }

    private void ShowOsd(int ms = 4000)
    {
        if (!IsOpen || _clickThrough || IsMini) return;
        _ov.Osd.Visibility = Visibility.Visible;
        _ov.TopBar.Visibility = Visibility.Visible;
        UpdateOsd();
        _osdTimer.Stop();
        _osdTimer.Interval = TimeSpan.FromMilliseconds(ms);
        _osdTimer.Start();
    }

    private void HideOsd()
    {
        _osdTimer.Stop();
        // Đang dừng / chuột đang ở trên thanh / đang tua: giữ thanh
        if (_state?.Paused == true || _seekTarget != null || _dragging || _ov.Osd.IsMouseOver || _ov.TopBar.IsMouseOver ||
            _ov.Panel.Visibility == Visibility.Visible || !_started)
        {
            _osdTimer.Interval = TimeSpan.FromSeconds(2);
            _osdTimer.Start();
            return;
        }
        _ov.Osd.Visibility = Visibility.Collapsed;
        _ov.TopBar.Visibility = Visibility.Collapsed;
        _ov.HitArea.Cursor = Cursors.None;
    }

    private void UpdateOsd()
    {
        var s = _state;
        var target = _seekTarget;
        var t = target ?? s?.T ?? 0;
        var d = s?.D ?? 0;
        if (!_dragging)
        {
            _ov.TimeText.Text = d > 0 ? $"{Util.FormatTime(t)} / {Util.FormatTime(d)}" : Util.FormatTime(t);
            _ov.Seek.Maximum = Math.Max(1, d);
            _ov.Seek.Value = Math.Min(t, _ov.Seek.Maximum);
        }
        _ov.StateText.Text = s == null ? "Đang tải"
            : target != null ? (target >= s.T ? "Tua tới" : "Tua lùi")
            : s.Waiting ? "Đang tải"
            : s.Paused ? "Tạm dừng"
            : Math.Abs(_rate - 1) > 0.01 ? $"Đang phát · {RateText(_rate)}" : "Đang phát";
        _ov.BtnPlay.Content = s is { Paused: false } ? "⏸  Dừng" : "▶  Phát";
        _ov.BtnPrev.IsEnabled = _index > 0;
        _ov.BtnNext.IsEnabled = _index + 1 < Episodes.Count;
    }

    public void Notice(string text, int ms = 4000)
    {
        if (!IsOpen || IsMini) return;
        _ov.NoticeText.Text = text;
        _ov.NoticeBox.Visibility = Visibility.Visible;
        _noticeTimer.Stop();
        _noticeTimer.Interval = TimeSpan.FromMilliseconds(ms);
        _noticeTimer.Start();
    }

    private void ShowLoading(string text)
    {
        _ov.LoadingBox.Visibility = Visibility.Visible;
        _ov.LoadingBar.Visibility = Visibility.Visible;
        _ov.LoadingText.Text = text;
    }

    private void ShowError(string text)
    {
        _ov.LoadingBox.Visibility = Visibility.Visible;
        _ov.LoadingBar.Visibility = Visibility.Collapsed;
        _ov.LoadingText.Text = text;
        ShowOsd(8000);
    }

    private void HideLoading() => _ov.LoadingBox.Visibility = Visibility.Collapsed;

    private static string RateText(double r) => (r % 1 == 0 ? ((int)r).ToString() : r.ToString(CultureInfo.InvariantCulture).Replace('.', ',')) + "×";

    // ---- Bảng tuỳ chọn ----

    private void Section(string title, string? note = null)
    {
        _ov.PanelStack.Children.Add(Ui.Text(title, 16, weight: FontWeights.SemiBold, margin: new Thickness(0, 14, 0, 8)));
        if (note != null) _ov.PanelStack.Children.Add(Ui.Dim(note, 13, new Thickness(0, -4, 0, 8)));
    }

    private WrapPanel Row()
    {
        var r = new WrapPanel();
        _ov.PanelStack.Children.Add(r);
        return r;
    }

    private void ShowPanel()
    {
        if (!IsOpen) return;
        var p = _ov.PanelStack;
        p.Children.Clear();
        _ov.PanelTitle.Text = _ov.TitleText.Text;
        Button? focus = null;

        var src = _source;
        if (src != null && !src.PageOnly && src.Servers.Count > 0)
        {
            Section("Server", "Video giật / không chạy: chọn server khác (app nhớ cho lần sau).");
            var row = Row();
            foreach (var s in src.Servers)
            {
                var active = s.Type == _server?.Type;
                var b = Ui.Pill(s.Label, () => SwitchServer(s), active ? "active" : null);
                row.Children.Add(b);
                if (active) focus = b;
            }
        }
        else if (src is { PageOnly: true })
        {
            Section("Server", "Trang này không chia server: app mở thẳng trang tập phim.");
        }

        if (_detail is { Groups.Count: > 1 } d)
        {
            Section("Bản");
            var row = Row();
            foreach (var g in d.Groups)
                row.Children.Add(Ui.Pill(g.Name, () => SwitchGroup(g), g == _group ? "active" : null));
        }

        Section("Xem");
        var view = Row();
        var prev = Ui.Pill("⏮  Tập trước", () =>
        {
            HidePanel();
            Prev();
        });
        prev.IsEnabled = _index > 0;
        var next = Ui.Pill("Tập sau  ⏭", () =>
        {
            HidePanel();
            Next();
        });
        next.IsEnabled = _index + 1 < Episodes.Count;
        view.Children.Add(prev);
        view.Children.Add(next);
        view.Children.Add(Ui.Pill("↻  Tải lại", Reload));
        focus ??= next.IsEnabled ? next : prev;

        var rates = Row();
        foreach (var r in Rates)
        {
            var rr = r;
            rates.Children.Add(Ui.Pill(RateText(r), () =>
            {
                _rate = rr;
                Cmd("rate", rr);
                Notice($"Tốc độ {RateText(rr)}");
                ShowPanel();
            }, Math.Abs(_rate - r) < 0.01 ? "active" : null, 64));
        }
        Row().Children.Add(Ui.Pill(Prefs.VideoFill ? "Khung hình: Lấp đầy" : "Khung hình: Vừa màn hình", () =>
        {
            Prefs.VideoFill = !Prefs.VideoFill;
            Cmd("fit", Prefs.VideoFill ? 1 : 0);
            ShowPanel();
        }));

        // Nhạc mở đầu
        var t = (int)(_state?.T ?? 0);
        var mark = Prefs.Intro(IntroKey);
        Section("Nhạc mở đầu (cả phim)", mark == null ? "Chưa đánh dấu — đang ở đầu / cuối nhạc thì bấm nút tương ứng."
            : mark.Value.End < 0 ? $"Bắt đầu {Util.FormatTime(mark.Value.Start)}, chưa đánh dấu chỗ hết."
            : $"{Util.FormatTime(mark.Value.Start)} → {Util.FormatTime(mark.Value.End)}, tự bỏ qua ở các tập.");
        var intro = Row();
        var bStart = Ui.Pill($"Bắt đầu ở {Util.FormatTime(t)}", () =>
        {
            HidePanel();
            var end = Prefs.Intro(IntroKey) is { } m && m.End > t ? m.End : -1;
            Prefs.SetIntro(IntroKey, t, end);
            Notice(end > t ? $"Nhạc mở đầu: {Util.FormatTime(t)} → {Util.FormatTime(end)}, các tập sẽ tự bỏ qua."
                : "Đã đánh dấu đầu nhạc mở đầu. Hết nhạc thì nhấn ▼ → «Hết nhạc ở …».", 6000);
        });
        var bEnd = Ui.Pill($"Hết nhạc ở {Util.FormatTime(t)}", () =>
        {
            HidePanel();
            var start = Prefs.Intro(IntroKey) is { } m && m.Start < t ? m.Start : 0;
            Prefs.SetIntro(IntroKey, start, t);
            _introDone = true;
            Notice($"Nhạc mở đầu: {Util.FormatTime(start)} → {Util.FormatTime(t)}, các tập sẽ tự bỏ qua.", 6000);
        });
        bStart.IsEnabled = bEnd.IsEnabled = _started;
        intro.Children.Add(bStart);
        intro.Children.Add(bEnd);
        if (mark != null)
            intro.Children.Add(Ui.Pill("Bỏ mốc", () =>
            {
                HidePanel();
                Prefs.ClearIntro(IntroKey);
                Notice("Đã bỏ mốc nhạc mở đầu của phim này.");
            }));

        // Danh sách tập
        if (Episodes.Count > 1)
        {
            Section($"Tập ({Episodes.Count})");
            var eps = Row();
            var watched = Prefs.WatchedEpisodes();
            for (var i = 0; i < Episodes.Count; i++)
            {
                var idx = i;
                var e = Episodes[i];
                var b = Ui.Pill(e.Title, () =>
                {
                    HidePanel();
                    OpenEpisode(idx);
                }, i == _index ? "active" : watched.Contains(e.Url) ? "watched" : null, 84);
                b.Padding = new Thickness(10, 7, 10, 7);
                eps.Children.Add(b);
            }
        }

        Section("Khác");
        var other = Row();
        other.Children.Add(Ui.Pill("▣  Thu nhỏ (cửa sổ nhỏ)", EnterMini));
        other.Children.Add(Ui.Pill("🖱  Bấm thẳng vào trang", EnterClickThrough));
        other.Children.Add(Ui.Pill("↗  Mở trang tập trên trình duyệt", () =>
        {
            HidePanel();
            Ui.OpenBrowser(_episodeUrl);
        }));
        other.Children.Add(Ui.Pill("✕  Thoát phim", () => Close()));

        _ov.Panel.Visibility = Visibility.Visible;
        _ov.HitArea.Cursor = Cursors.Arrow;
        ShowOsd();
        var f = focus;
        Dispatcher.BeginInvoke(() =>
        {
            _ov.Activate();
            if (f != null) KeyNav.FocusAndShow(f);
        }, DispatcherPriority.Loaded);
    }

    private void HidePanel()
    {
        _ov.Panel.Visibility = Visibility.Collapsed;
        Keyboard.Focus(_ov);
        _ov.Focus();
    }

    /// Chế độ bấm thẳng vào trang (trình phát cần bấm nút riêng, vd «Bấm để xem»): bỏ nền của lớp phủ để chuột đi xuyên.
    private void EnterClickThrough()
    {
        HidePanel();
        _clickThrough = true;
        _ov.Osd.Visibility = Visibility.Collapsed;
        _ov.TopBar.Visibility = Visibility.Collapsed;
        _ov.HitArea.Background = null;
        _ov.HitArea.Cursor = Cursors.Arrow;
        _ov.BtnDone.Visibility = Visibility.Visible;
        Notice("Dùng chuột bấm vào trang. Xong thì bấm nút «Xong» ở trên cùng.", 6000);
    }

    private void ExitClickThrough()
    {
        if (!_clickThrough) return;
        _clickThrough = false;
        _ov.HitArea.Background = new SolidColorBrush(Color.FromArgb(1, 0, 0, 0));
        _ov.BtnDone.Visibility = Visibility.Collapsed;
        _ov.Activate();
        ShowOsd();
    }

#if DEBUG
    /// Cổng gỡ lỗi: chạy JS trong trình phát.
    public async Task<string> Js(string code) => _web.CoreWebView2 == null ? "null" : await _web.CoreWebView2.ExecuteScriptAsync(code);

    public string Debug() =>
        $"open={IsOpen} pageMode={_pageMode} started={_started} server={_server?.Label} ep={_episodeUrl} t={_state?.T:0.0} d={_state?.D:0.0} " +
        $"paused={_state?.Paused} embed={_embedUrl} hls={_hlsUrl} load={_pageLoad}";

    public PlayerOverlay Overlay => _ov;
#endif
}
