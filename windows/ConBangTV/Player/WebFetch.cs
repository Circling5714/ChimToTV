using System.Drawing;
using System.IO;
using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Threading;
using ConBangTV.Core;
using Microsoft.Web.WebView2.Core;

namespace ConBangTV.Player;

/// Đọc hộ trang bật lớp chống bot của Cloudflare (lõi: web_fetch — hoathinh3d.pw từ 14/11/2026): mỗi trang bị chặn một WebView2
/// ẩn (cùng môi trường / cookie với trình phát), mở trang chủ cho trình duyệt tự qua lớp chặn, rồi mỗi yêu cầu là fetch() ngay
/// trong trang đó (web-fetch.js). Lõi hỏi bằng sự kiện `web.fetch`, app trả `web.reply`. Ảnh bìa của trang đó cũng lấy qua đây
/// (ImageLoader). Trang đòi bấm xác nhận thì báo lỗi — không giả / không giải hộ. Không dùng 5 phút thì đóng WebView cho nhẹ máy.
public static class WebFetch
{
    private static Window? _owner;
    private static readonly Dictionary<string, Session> Sessions = new();
    private static readonly string FetchJs = Res("web-fetch.js");
    private const string ReplyJs = "window.__cbReply=function(m){try{window.chrome.webview.postMessage(m)}catch(e){}};";
    /// Chờ trình duyệt qua lớp chặn tối đa
    private static readonly TimeSpan PassTimeout = TimeSpan.FromSeconds(30);
    private static readonly TimeSpan CallTimeout = TimeSpan.FromSeconds(45);
    private static readonly TimeSpan IdleClose = TimeSpan.FromMinutes(5);
    /** Cloudflare của trang chặn hẳn app thì thôi mở WebView lại trong 30 phút (báo lỗi ngay). */
    private static readonly TimeSpan BlockedFor = TimeSpan.FromMinutes(30);

    public sealed record Result(int Code, string Url, string? Type, string? Cf, byte[] Data);

    private static string Res(string name)
    {
        using var s = Assembly.GetExecutingAssembly().GetManifestResourceStream(name)!;
        using var r = new StreamReader(s);
        return r.ReadToEnd();
    }

    /// Mở app (sau khi cửa sổ chính có HWND): nhận việc của lõi.
    public static void Start(Window owner)
    {
        _owner = owner;
        Native.Event += (name, data) =>
        {
            if (name == "web.fetch" && data != null) owner.Dispatcher.BeginInvoke(() => _ = Handle(data));
        };
        Native.SyncVoid("web.enable", new { on = true });
    }

    private static async Task Handle(JsonNode d)
    {
        var id = d["id"]?.GetValue<long>() ?? 0;
        try
        {
            var headers = new Dictionary<string, string>();
            if (d["headers"] is JsonObject h)
                foreach (var (k, v) in h)
                    if (v != null) headers[k] = v.GetValue<string>();
            var r = await Fetch(d["url"]!.GetValue<string>(), d["method"]?.GetValue<string>() ?? "GET", d["body"]?.GetValue<string>(), headers);
            Native.SyncVoid("web.reply", new { id, code = r.Code, url = r.Url, type = r.Type, b64 = Convert.ToBase64String(r.Data) });
        }
        catch (Exception e)
        {
            Native.SyncVoid("web.reply", new { id, err = e is AppError ? e.Message : Util.Friendly(e) });
        }
    }

    /// Trang của địa chỉ này đang chặn (lõi đã nhớ).
    public static bool Needs(string url)
    {
        try
        {
            return Native.Sync<bool>("web.needs", new { url });
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// Ảnh bìa của trang bị chặn (null = không lấy được).
    public static async Task<byte[]?> GetBytes(string url, string? referer)
    {
        try
        {
            var headers = new Dictionary<string, string> { ["Accept"] = "image/avif,image/webp,image/apng,image/*,*/*;q=0.8" };
            if (referer != null) headers["Referer"] = referer;
            var r = await Application.Current.Dispatcher.InvokeAsync(() => Fetch(url, "GET", null, headers)).Task.Unwrap();
            return r.Code is >= 200 and < 300 ? r.Data : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// Đọc một địa chỉ qua WebView (luồng giao diện). Lớp chặn quay lại giữa chừng (cookie hết hạn) → qua lại lớp chặn, thử
    /// thêm một lần.
    public static async Task<Result> Fetch(string url, string method, string? body, IDictionary<string, string> headers)
    {
        var host = new Uri(url).Host.ToLowerInvariant();
        if (!Sessions.TryGetValue(host, out var s)) Sessions[host] = s = new Session(host);
        for (var attempt = 0; ; attempt++)
        {
            await s.Ready();
            var r = await s.Run(url, method, body, headers);
            if (!string.Equals(r.Cf, "challenge", StringComparison.OrdinalIgnoreCase)) return r;
            if (attempt > 0) throw new AppError($"Trang {host} vẫn đòi qua lớp chống bot — chưa đọc được.");
            s.Invalidate();
        }
    }

    /// Gỡ lỗi: trạng thái các phiên.
    public static string Debug() =>
        Sessions.Count == 0 ? "chưa mở phiên nào" : string.Join("; ", Sessions.Values.Select(s => s.Describe()));

    private sealed class Session
    {
        private readonly string _host;
        private CoreWebView2Controller? _ctl;
        private CoreWebView2? _web;
        private Task? _ready;
        private long _next;
        private readonly Dictionary<long, TaskCompletionSource<JsonNode?>> _calls = new();
        private readonly DispatcherTimer _idle;
        private string _state = "mới";
        private TaskCompletionSource<bool>? _nav;
        private DateTime _blockedUntil;

        public Session(string host)
        {
            _host = host;
            _idle = new DispatcherTimer { Interval = IdleClose };
            _idle.Tick += (_, _) => Close("không dùng " + IdleClose.TotalMinutes + " phút");
        }

        public string Describe() => $"{_host}: {_state}{(_ctl == null ? " (đã đóng)" : "")}, đang chờ {_calls.Count}";

        public Task Ready()
        {
            _idle.Stop();
            _idle.Start();
            if (DateTime.UtcNow < _blockedUntil) return Task.FromException(Blocked());
            return _ready ??= Open();
        }

        private AppError Blocked() => new($"Trang {_host} chặn app này (Cloudflare «you have been blocked») — xem phim này ở nguồn khác, vd KKPhim.");

        public void Invalidate()
        {
            _ready = null;
            _state = "lớp chặn quay lại";
        }

        private async Task Open()
        {
            try
            {
                await Create();
                var t0 = DateTime.UtcNow;
                _state = "đang qua lớp chặn";
                await Navigate($"https://{_host}/");
                // Trang chặn («Just a moment…», window._cf_chl_opt) tự giải rồi nạp lại trang thật
                while (true)
                {
                    var st = await Probe();
                    if (st.Blocked)
                    {
                        // Quy tắc chặn của chủ trang (không phải thử thách) — không lách
                        _blockedUntil = DateTime.UtcNow + BlockedFor;
                        throw Blocked();
                    }
                    if (st.Passed) break;
                    if (DateTime.UtcNow - t0 > PassTimeout)
                        throw new AppError($"Trang {_host} đòi bấm xác nhận chống bot (Cloudflare) — app chưa đọc được trang này.");
                    await Task.Delay(500);
                }
                var passMs = (int)(DateTime.UtcNow - t0).TotalMilliseconds;
                // Đứng ở trang nhẹ cùng gốc (khỏi chạy quảng cáo / mã của trang chủ) — fetch() vẫn mang cookie của trang
                await Navigate($"https://{_host}/robots.txt");
                await Define();
                _state = $"sẵn sàng (qua lớp chặn {passMs} ms)";
                Log.D("web", $"{_host}: WebView đã qua lớp chống bot ({passMs} ms)");
            }
            catch (Exception e)
            {
                _ready = null;
                _state = "lỗi: " + e.Message;
                Log.D("web", $"{_host}: {e.Message}");
                Close(null);
                throw;
            }
        }

        private async Task Create()
        {
            if (_ctl != null) return;
            var env = await App.WebEnv();
            var hwnd = new WindowInteropHelper(_owner!).EnsureHandle();
            _ctl = await env.CreateCoreWebView2ControllerAsync(hwnd);
            // Khung ẩn: không vẽ lên cửa sổ app, trang vẫn chạy (đủ để trình duyệt qua lớp chặn)
            _ctl.Bounds = new Rectangle(0, 0, 0, 0);
            _ctl.IsVisible = false;
            _web = _ctl.CoreWebView2;
            var st = _web.Settings;
            st.AreDefaultContextMenusEnabled = false;
            st.AreDevToolsEnabled = false;
            st.IsStatusBarEnabled = false;
            st.IsGeneralAutofillEnabled = false;
            st.IsPasswordAutosaveEnabled = false;
            _web.IsMuted = true;
            _web.NewWindowRequested += (_, e) => e.Handled = true;
            _web.NavigationCompleted += (_, e) => _nav?.TrySetResult(e.IsSuccess);
            _web.WebMessageReceived += OnMessage;
            // Không cần ảnh / video / phông / quảng cáo của trang — chỉ fetch() của app (ngữ cảnh Fetch, không chặn)
            foreach (var ctx in new[] { CoreWebView2WebResourceContext.Image, CoreWebView2WebResourceContext.Media, CoreWebView2WebResourceContext.Font })
                _web.AddWebResourceRequestedFilter("*", ctx);
            foreach (var f in AdBlock.Filters()) _web.AddWebResourceRequestedFilter(f, CoreWebView2WebResourceContext.All);
            _web.WebResourceRequested += (_, e) =>
            {
                if (e.ResourceContext == CoreWebView2WebResourceContext.Fetch || e.ResourceContext == CoreWebView2WebResourceContext.XmlHttpRequest)
                {
                    if (!Uri.TryCreate(e.Request.Uri, UriKind.Absolute, out var u) || !AdBlock.IsAd(u.Host)) return;
                }
                e.Response = _web.Environment.CreateWebResourceResponse(null, 403, "Blocked", "Content-Type: text/plain");
            };
            await _web.AddScriptToExecuteOnDocumentCreatedAsync(ReplyJs + FetchJs);
        }

        private async Task Navigate(string url)
        {
            _nav = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
            _web!.Navigate(url);
            await Task.WhenAny(_nav.Task, Task.Delay(TimeSpan.FromSeconds(25)));
        }

        /// Tài liệu đang mở đã là trang thật (không còn là trang chặn) chưa.
        private async Task<(bool Passed, bool Blocked, string Title)> Probe()
        {
            string raw;
            try
            {
                raw = await _web!.ExecuteScriptAsync(
                    "JSON.stringify([location.host, document.readyState, document.title || '', typeof window._cf_chl_opt, " +
                    "!!document.querySelector('#challenge-form,#challenge-stage,#cf-challenge-running,.cf-turnstile'), " +
                    "!!document.querySelector('#cf-error-details,.cf-error-details') || /you have been blocked/i.test(document.body ? document.body.innerText.slice(0, 400) : '')])");
            }
            catch (Exception)
            {
                return (false, false, "");
            }
            if (JsonSerializer.Deserialize<string>(raw) is not { } s || JsonNode.Parse(s) is not JsonArray a) return (false, false, "");
            var host = a[0]?.GetValue<string>() ?? "";
            var ready = a[1]?.GetValue<string>() ?? "";
            var title = a[2]?.GetValue<string>() ?? "";
            var chl = a[3]?.GetValue<string>() ?? "";
            var form = a[4]?.GetValue<bool>() ?? false;
            var sameHost = string.Equals(host, _host, StringComparison.OrdinalIgnoreCase) && ready != "loading";
            var blocked = sameHost && ((a[5]?.GetValue<bool>() ?? false) || title.StartsWith("Attention Required", StringComparison.OrdinalIgnoreCase));
            var passed = string.Equals(host, _host, StringComparison.OrdinalIgnoreCase) && ready != "loading" && chl == "undefined" && !form &&
                         !title.StartsWith("Just a moment", StringComparison.OrdinalIgnoreCase);
            return (passed && !blocked, blocked, title);
        }

        /// web-fetch.js đã có trong tài liệu hiện tại (tài liệu robots.txt dạng chữ có thể không chạy script lúc tạo)
        private Task Define() => _web!.ExecuteScriptAsync(ReplyJs + FetchJs);

        public async Task<Result> Run(string url, string method, string? body, IDictionary<string, string> headers)
        {
            if (_web == null) throw new AppError($"WebView của {_host} đã đóng.");
            var id = ++_next;
            var tcs = new TaskCompletionSource<JsonNode?>(TaskCreationOptions.RunContinuationsAsynchronously);
            _calls[id] = tcs;
            try
            {
                await Define();
                var js = $"window.__cbFetch({id},{JsonSerializer.Serialize(url)},{JsonSerializer.Serialize(method)}," +
                         $"{JsonSerializer.Serialize(body)},{JsonSerializer.Serialize(headers)});0";
                await _web.ExecuteScriptAsync(js);
                var done = await Task.WhenAny(tcs.Task, Task.Delay(CallTimeout));
                if (done != tcs.Task) throw new AppError($"WebView không trả lời ({url}).");
                var m = tcs.Task.Result ?? throw new AppError("WebView trả lời rỗng.");
                if (m["err"]?.GetValue<string>() is { } err) throw new AppError($"WebView đọc {url} lỗi: {err}");
                var data = Convert.FromBase64String(m["b64"]?.GetValue<string>() ?? "");
                return new Result(m["code"]?.GetValue<int>() ?? 0, m["url"]?.GetValue<string>() ?? url, m["type"]?.GetValue<string>(),
                    m["cf"]?.GetValue<string>(), data);
            }
            finally
            {
                _calls.Remove(id);
            }
        }

        private void OnMessage(object? sender, CoreWebView2WebMessageReceivedEventArgs e)
        {
            JsonNode? m;
            try
            {
                m = JsonNode.Parse(e.WebMessageAsJson);
            }
            catch (Exception)
            {
                return;
            }
            if (m?["id"]?.GetValue<long>() is { } id && _calls.TryGetValue(id, out var tcs)) tcs.TrySetResult(m);
        }

        private void Close(string? why)
        {
            _idle.Stop();
            if (_calls.Count > 0 && why != null) return; // còn việc đang chờ: để lần sau
            try
            {
                _ctl?.Close();
            }
            catch (Exception)
            {
            }
            _ctl = null;
            _web = null;
            _ready = null;
            if (why != null)
            {
                _state = "đã đóng (" + why + ")";
                Log.D("web", $"{_host}: đóng WebView ẩn ({why})");
            }
        }
    }
}
