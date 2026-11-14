using System.IO;
using System.Net;
using System.Text;

namespace ConBangTV.Core;

// Mạng của app chạy trong lõi Rust (linux/core: net.rs — cổng SOCKS5 nội bộ, doh.rs — DNS mã hoá, unblock.rs — vượt chặn,
// shadowsocks.rs, http.rs, log.rs), gọi qua Native. WebView2 (--proxy-server) và HttpClient của app (ảnh bìa) đi qua cổng SOCKS5
// nội bộ 127.0.0.1 của lõi — DNS mã hoá, vượt chặn, proxy, chặn tên miền quảng cáo áp cho mọi kết nối như trước.

public sealed record SocksConfig(string Host, int Port, string User = "", string Pass = "", bool RemoteDns = true, bool SitesOnly = false);

/// Máy chủ Shadowsocks (Cài đặt → Proxy → Kiểu: Shadowsocks). Password: SS 2022 = các khoá base64 cách nhau bằng ":"; kiểu
/// cũ = mật khẩu.
public sealed record SsConfig(string Host, int Port, string Method, string Password, bool RemoteDns = true, bool SitesOnly = false);

/// Kiểu mã hoá hỗ trợ (TCP). KeySize = số byte khoá = số byte salt.
public sealed record SsMethod(string Id, int KeySize, bool Chacha, bool Is2022)
{
    public static readonly SsMethod B3Aes256 = new("2022-blake3-aes-256-gcm", 32, false, true);
    public static readonly SsMethod B3Aes128 = new("2022-blake3-aes-128-gcm", 16, false, true);
    public static readonly SsMethod B3Chacha = new("2022-blake3-chacha20-poly1305", 32, true, true);
    public static readonly SsMethod Aes256 = new("aes-256-gcm", 32, false, false);
    public static readonly SsMethod Aes128 = new("aes-128-gcm", 16, false, false);
    public static readonly SsMethod ChachaIetf = new("chacha20-ietf-poly1305", 32, true, false);

    public static readonly SsMethod[] All = { B3Aes256, B3Aes128, B3Chacha, Aes256, Aes128, ChachaIetf };
    public static SsMethod Default => B3Aes256;

    /// Theo tên (không phân biệt hoa / thường; trống = mặc định); "chacha20-poly1305" của Xray / 3X-UI = chacha20-ietf-poly1305.
    public static SsMethod? Of(string? name)
    {
        var n = (name ?? "").Trim().ToLowerInvariant();
        if (n.Length == 0) return Default;
        if (n == "chacha20-poly1305") return ChachaIetf;
        return All.FirstOrDefault(m => m.Id == n);
    }

    public static string Names => string.Join(", ", All.Select(m => m.Id));

    /// Máy có hỗ trợ không — lõi Rust tự mã hoá mọi kiểu (không phụ thuộc Windows như bản C# cũ).
    public bool Supported => true;
}

/// Lỗi Shadowsocks (máy chủ không trả lời, khoá sai…) — giao diện báo «Lỗi Shadowsocks: …».
public sealed class SsError : IOException
{
    public SsError(string message) : base(message) { }
}

/// Lỗi phía proxy SOCKS5 — giao diện báo «Lỗi proxy: …».
public sealed class SocksError : IOException
{
    public readonly int Code;

    public SocksError(int code, string message) : base(message)
    {
        Code = code;
    }
}

internal static class ProxyErrors
{
    /// Lỗi của lõi có dấu «ss|» / «socks|» → SsError / SocksError.
    public static Exception Typed(AppError e) =>
        e.Message.StartsWith("ss|") ? new SsError(e.Message[3..])
        : e.Message.StartsWith("socks|") ? new SocksError(1, e.Message[6..])
        : e;
}

public static class Net
{
    /// Mở cổng SOCKS5 nội bộ của lõi (gọi nhiều lần cũng chỉ mở một lần). Trả số cổng.
    public static int StartBridge() => Native.Sync<int>("net.startBridge");

    public static int BridgePort => Native.Sync<int>("net.bridgePort");

    /// Đổi cài đặt mạng (proxy / vượt chặn / DNS mã hoá): lõi đóng các kết nối đang giữ để mở lại theo cài đặt mới.
    public static void SettingsChanged() => Native.SyncVoid("net.settingsChanged");

    /// Ghi chú thêm vào lời báo lỗi mạng khi đang bật proxy.
    public static string Note() => Native.Sync<string>("net.note");

    /// Proxy cho HttpClient của app (ảnh bìa): cổng SOCKS5 nội bộ của lõi.
    public static IWebProxy Proxy => new WebProxy($"socks5://127.0.0.1:{StartBridge()}");
}

public static class Http
{
    /// User-Agent chung của app (trình phát đặt theo WebView2 để trang thấy cùng một trình duyệt).
    public static string UserAgent
    {
        get => Native.Sync<string>("app.userAgent");
        set => Native.SyncVoid("app.setUserAgent", new { ua = value });
    }

    public sealed class HttpError : Exception
    {
        public readonly int Code;
        public HttpError(int code) : base($"Trang trả về lỗi HTTP {code}") { Code = code; }
    }

    public sealed class Response
    {
        /// Địa chỉ cuối cùng (sau chuyển hướng)
        public string Url = "";
        public int Code;
        public string? ContentType;
        public string Text = "";
        public byte[] Data => Encoding.UTF8.GetBytes(Text);
    }

    /// Tải một trang / tệp nhỏ (chữ) qua mạng của app.
    public static Task<Response> Get(string url, Dictionary<string, string>? headers = null) =>
        Native.Call<Response>("http.get", new { url, headers });

    /// Tải tệp lớn (bản cập nhật) vào path, báo tiến độ 0..1.
    public static Task DownloadTo(string url, string path, Action<double>? progress = null) =>
        Native.CallNode("http.download", new { url, path }, p =>
        {
            if (p != null) progress?.Invoke(p.GetValue<double>());
        });
}

public static class Doh
{
    /// "NextDNS → Cloudflare → Google" (Cài đặt).
    public static string Providers => Native.Sync<string>("doh.providers");

    /// Nhãn các máy chủ theo thứ tự hỏi.
    public static List<string> Servers => Native.Sync<List<string>>("doh.servers");

    /// Cổng gỡ lỗi: giả máy chủ DNS mã hoá bị chặn (theo nhãn).
    public static HashSet<string> DebugFail
    {
        get => Native.Sync<List<string>>("doh.debugFail").ToHashSet();
        set => Native.SyncVoid("doh.setDebugFail", new { labels = value.ToList() });
    }

    private static HashSet<string> _block = new();
    private static IPAddress _blockIp = IPAddress.Any;

    /// Cổng gỡ lỗi: giả DNS của máy chặn các trang này (trả DebugDnsBlockIp).
    public static HashSet<string> DebugDnsBlock
    {
        get => _block;
        set
        {
            _block = value;
            Native.SyncVoid("doh.setDebugDnsBlock", new { hosts = _block.ToList(), ip = _blockIp.ToString() });
        }
    }

    public static IPAddress DebugDnsBlockIp
    {
        get => _blockIp;
        set
        {
            _blockIp = value;
            Native.SyncVoid("doh.setDebugDnsBlock", new { hosts = _block.ToList(), ip = _blockIp.ToString() });
        }
    }

    public static void ForgetAnswers() => Native.SyncVoid("doh.forgetAnswers");
}

public static class Unblock
{
    public static int Level => Prefs.UnblockLevel;

    /// Cổng gỡ lỗi: giả nhà mạng ghép được các mẩu ở Mức 1 cho các trang này.
    public static HashSet<string> DebugWholeHello
    {
        get => Native.Sync<List<string>>("unblock.debugWholeHello").ToHashSet();
        set => Native.SyncVoid("unblock.setDebugWholeHello", new { hosts = value.ToList() });
    }

    /// Trang đã tự lên Mức 2 (nhớ 7 ngày).
    public static class Level2Memo
    {
        private sealed class State
        {
            public Dictionary<string, long> Hosts = new();
            public long AutoLevel2;
        }

        public static Dictionary<string, long> All() => Native.Sync<State>("unblock.level2").Hosts;

        public static long AutoCount => Native.Sync<State>("unblock.level2").AutoLevel2;

        public static void Clear() => Native.SyncVoid("unblock.level2Clear");
    }
}

public static class Shadowsocks
{
    /// Khoá / mật khẩu đúng dạng chưa (sai → SsError).
    public static void Keys(SsMethod m, string password)
    {
        try
        {
            Native.SyncVoid("ss.keys", new { method = m.Id, password });
        }
        catch (AppError e)
        {
            throw ProxyErrors.Typed(e);
        }
    }

    /// «Thử kết nối»: IP ra ngoài qua máy chủ, vd "103.1.2.3 (SG)".
    public static async Task<string> Check(SsConfig c)
    {
        try
        {
            return await Native.Call<string>("ss.check", c);
        }
        catch (AppError e)
        {
            throw ProxyErrors.Typed(e);
        }
    }
}

/// Link ss:// (SIP002 / kiểu cũ) của 3X-UI, Outline…
public static class SsLink
{
    private sealed class HostPort
    {
        public string Host = "";
        public int Port;
    }

    public static SsConfig Parse(string link, bool remoteDns, bool sitesOnly)
    {
        try
        {
            return Native.Sync<SsConfig>("ss.parseLink", new { link, remoteDns, sitesOnly });
        }
        catch (AppError e)
        {
            throw ProxyErrors.Typed(e);
        }
    }

    /// "host:cổng" / "[ipv6]:cổng" → (host, cổng); không đúng dạng → null.
    public static (string Host, int Port)? SplitHostPort(string s) =>
        Native.Sync<HostPort?>("ss.splitHostPort", new { s }) is { } hp ? (hp.Host, hp.Port) : null;
}

/// «Thử kết nối» proxy SOCKS5 theo ô đang nhập (chưa lưu).
public static class Socks5
{
    private sealed class Done : IDisposable
    {
        public void Dispose() { }
    }

    public static async Task<IDisposable> Connect(SocksConfig c, string target, int port, CancellationToken ct)
    {
        try
        {
            await Native.CallNode("net.testSocks", new { c.Host, c.Port, c.User, c.Pass, c.RemoteDns, target, targetPort = port });
            return new Done();
        }
        catch (AppError e)
        {
            throw ProxyErrors.Typed(e);
        }
    }
}

/// Nhật ký của app — chung một tệp log.txt với lõi (lõi ghi, giữ 2000 dòng gần nhất cho cổng gỡ lỗi).
public static class Log
{
    public static void Start()
    {
    }

    public static void D(string tag, string message)
    {
        try
        {
            Native.SyncVoid("log.d", new { tag, message });
        }
        catch (Exception)
        {
        }
        System.Diagnostics.Debug.WriteLine($"{tag}: {message}");
    }

    /// Các dòng có số thứ tự &gt; since; trả kèm số thứ tự cuối để lần sau hỏi tiếp.
    public static (int Last, List<string> Lines) Since(int since)
    {
        var n = Native.SyncNode("log.since", new { since });
        var seq = n?["seq"]?.GetValue<long>() ?? 0;
        var lines = n?["lines"]?.AsArray().Select(x => x?.GetValue<string>() ?? "").ToList() ?? new();
        return ((int)seq, lines);
    }
}
