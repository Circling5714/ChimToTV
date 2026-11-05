using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Text;

namespace ConBangTV.Core;

/// Proxy SOCKS5 người xem nhập trong Cài đặt (bản TV: SocksConfig trong Socks5.kt).
/// RemoteDns: proxy phân giải tên miền (gửi tên miền); false: máy tự phân giải rồi gửi IP.
/// SitesOnly: chỉ trang phim (trong danh sách) đi qua proxy; video / ảnh CDN, GitHub… đi thẳng mạng nhà.
public sealed record SocksConfig(string Host, int Port, string User = "", string Pass = "", bool RemoteDns = true, bool SitesOnly = false);

/// Lỗi khi mở kết nối; ViaProxy = lỗi ở phía proxy (để báo người xem, không lặng lẽ đi thẳng).
public sealed class SocksError : IOException
{
    public readonly int Code;

    public SocksError(int code, string message) : base(message)
    {
        Code = code;
    }
}

/// Phía khách SOCKS5 (RFC 1928, đăng nhập theo RFC 1929): đi qua proxy thật tới một đích.
public static class Socks5
{
    private static readonly Dictionary<int, string> Replies = new()
    {
        [1] = "lỗi chung", [2] = "proxy không cho phép", [3] = "mạng không tới được", [4] = "máy đích không tới được",
        [5] = "máy đích từ chối", [6] = "hết thời gian", [7] = "lệnh không hỗ trợ", [8] = "kiểu địa chỉ không hỗ trợ",
    };

    public static async Task<Socket> Connect(SocksConfig c, string host, int port, CancellationToken ct)
    {
        var s = new Socket(SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            using var t = CancellationTokenSource.CreateLinkedTokenSource(ct);
            t.CancelAfter(10_000);
            await s.ConnectAsync(new DnsEndPoint(c.Host, c.Port), t.Token);
        }
        catch (Exception e) when (e is not OperationCanceledException || !ct.IsCancellationRequested)
        {
            s.Dispose();
            throw new SocksError(1, e is SocketException { SocketErrorCode: SocketError.HostNotFound }
                ? $"không tìm thấy proxy {c.Host}" : $"không kết nối được proxy {c.Host}:{c.Port}");
        }
        try
        {
            using var t = CancellationTokenSource.CreateLinkedTokenSource(ct);
            t.CancelAfter(15_000);
            await Handshake(new NetworkStream(s, false), c, host, port, t.Token);
            return s;
        }
        catch (Exception e)
        {
            s.Dispose();
            if (e is SocksError) throw;
            throw new SocksError(1, $"proxy {c.Host}:{c.Port} ngắt kết nối ({e.Message})");
        }
    }

    private static async Task Handshake(NetworkStream st, SocksConfig c, string host, int port, CancellationToken ct)
    {
        var login = c.User.Length > 0;
        await st.WriteAsync(login ? new byte[] { 5, 2, 0, 2 } : new byte[] { 5, 1, 0 }, ct);
        var g = await Read(st, 2, ct);
        if (g[0] != 5) throw new SocksError(1, $"{c.Host}:{c.Port} không phải proxy SOCKS5");
        switch (g[1])
        {
            case 0:
                break;
            case 2:
                if (!login) throw new SocksError(1, "proxy cần tên đăng nhập / mật khẩu");
                var u = Encoding.UTF8.GetBytes(c.User);
                var p = Encoding.UTF8.GetBytes(c.Pass);
                if (u.Length > 255 || p.Length > 255) throw new SocksError(1, "tên đăng nhập / mật khẩu quá dài (tối đa 255 byte)");
                await st.WriteAsync(new byte[] { 1, (byte)u.Length }.Concat(u).Append((byte)p.Length).Concat(p).ToArray(), ct);
                var a = await Read(st, 2, ct);
                if (a[1] != 0) throw new SocksError(1, "proxy báo sai tên đăng nhập / mật khẩu");
                break;
            default:
                throw new SocksError(1, login ? "proxy không nhận cách đăng nhập này" : "proxy cần tên đăng nhập / mật khẩu");
        }
        var req = new List<byte> { 5, 1, 0 };
        req.AddRange(await Address(c, host, ct));
        req.Add((byte)(port >> 8));
        req.Add((byte)port);
        await st.WriteAsync(req.ToArray(), ct);
        var r = await Read(st, 4, ct);
        if (r[0] != 5) throw new SocksError(1, "proxy trả lời sai giao thức");
        var rep = r[1];
        var skip = r[3] switch { 1 => 4, 4 => 16, 3 => (await Read(st, 1, ct))[0], _ => 0 };
        await Read(st, skip + 2, ct);
        // Mã 2 khi có đăng nhập: nhiều proxy nhận mọi mật khẩu ở bước đăng nhập rồi mới từ chối ở đây
        if (rep == 2 && login) throw new SocksError(2, "proxy từ chối (thường do sai tên đăng nhập / mật khẩu)");
        if (rep != 0) throw new SocksError(rep, $"proxy báo lỗi: {Replies.GetValueOrDefault(rep, $"mã {rep}")} ({host}:{port})");
    }

    /// Địa chỉ đích: tên miền (DNS của proxy) hoặc IP (máy tự phân giải).
    private static async Task<byte[]> Address(SocksConfig c, string host, CancellationToken ct)
    {
        if (IPAddress.TryParse(host.Trim('[', ']'), out var ip)) return Ip(ip);
        if (c.RemoteDns)
        {
            var b = Encoding.ASCII.GetBytes(host);
            if (b.Length > 255) throw new SocksError(1, "tên miền quá dài");
            return new byte[] { 3, (byte)b.Length }.Concat(b).ToArray();
        }
        IPAddress[] list;
        try
        {
            list = await Dns.GetHostAddressesAsync(host, ct);
        }
        catch (SocketException)
        {
            throw new SocksError(4, $"không tìm thấy tên miền {host} (DNS của máy)");
        }
        var pick = list.FirstOrDefault(x => x.AddressFamily == AddressFamily.InterNetwork) ?? list.FirstOrDefault()
            ?? throw new SocksError(4, $"không tìm thấy tên miền {host} (DNS của máy)");
        return Ip(pick);
    }

    private static byte[] Ip(IPAddress ip) =>
        ip.AddressFamily == AddressFamily.InterNetwork ? new byte[] { 1 }.Concat(ip.GetAddressBytes()).ToArray()
            : new byte[] { 4 }.Concat(ip.GetAddressBytes()).ToArray();

    private static async Task<byte[]> Read(Stream s, int n, CancellationToken ct)
    {
        var b = new byte[n];
        await s.ReadExactlyAsync(b, ct);
        return b;
    }
}

/// Mọi kết nối ra mạng của app (trừ đồng bộ trong nhà) đi qua đây: thẳng, hoặc qua proxy SOCKS5; vượt chặn nhà mạng
/// tách gói bắt tay. Trình phát (WebView2) dùng cổng SOCKS5 nội bộ 127.0.0.1 (Bridge) cũng đi qua đây — đổi cài đặt là
/// có hiệu lực ngay (các kết nối cũ bị đóng để trình phát mở lại theo cài đặt mới).
public static class Net
{
    private static readonly object Lock = new();
    private static string? _lastProxyError;
    private static long _lastProxyErrorAt;

    /// Proxy đang bật (null = đi thẳng).
    public static SocksConfig? Proxy => Prefs.ProxyConfig();

    /// Có đi qua proxy tới host này không (chế độ "chỉ trang phim": chỉ tên miền trong danh sách trang).
    public static bool UsesProxy(SocksConfig? c, string host)
    {
        if (c == null) return false;
        if (!c.SitesOnly) return true;
        var h = host.ToLowerInvariant();
        return Prefs.Sites.Append(Prefs.BaseUrl).Select(Site.HostOf).OfType<string>()
            .Any(s => h == s || h.EndsWith("." + s));
    }

    /// Kết nối tới host:port theo cài đặt; trả kèm có qua proxy không.
    public static async Task<(Socket Socket, bool ViaProxy)> Connect(string host, int port, CancellationToken ct)
    {
        var c = Proxy;
        if (UsesProxy(c, host))
        {
            try
            {
                return (await Socks5.Connect(c!, host, port, ct), true);
            }
            catch (SocksError e)
            {
                lock (Lock)
                {
                    _lastProxyError = e.Message;
                    _lastProxyErrorAt = Util.NowMs();
                }
                throw;
            }
        }
        var s = new Socket(SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            await s.ConnectAsync(new DnsEndPoint(host, port), ct);
            return (s, false);
        }
        catch
        {
            s.Dispose();
            throw;
        }
    }

    /// ConnectCallback của HttpClient (trang, API, ảnh bìa). Kết nối được ghi lại để đổi cài đặt thì đóng (HttpClient giữ kết
    /// nối dùng lại — không đóng thì vẫn đi đường cũ).
    public static async ValueTask<Stream> HttpConnect(SocketsHttpConnectionContext ctx, CancellationToken ct)
    {
        var (s, via) = await Connect(ctx.DnsEndPoint.Host, ctx.DnsEndPoint.Port, ct);
        lock (Open)
        {
            Open.RemoveWhere(x => x.SafeHandle.IsClosed);
            Open.Add(s);
        }
        var ns = new NetworkStream(s, true);
        var level = Unblock.Level;
        return level > 0 && ctx.DnsEndPoint.Port == 443 ? new Unblock.HelloSplit(ns, s, ctx.DnsEndPoint.Host, level, via) : ns;
    }

    /// Lời nhắc thêm vào câu báo lỗi khi đang bật proxy (kèm lỗi phía proxy trong 2 phút gần đây).
    public static string Note()
    {
        if (Proxy == null) return "";
        lock (Lock)
        {
            var recent = _lastProxyError != null && Util.NowMs() - _lastProxyErrorAt < 120_000 ? $" Lỗi proxy: {_lastProxyError}." : "";
            return $"\nĐang bật proxy SOCKS5.{recent}";
        }
    }

    // ---- Cổng SOCKS5 nội bộ cho WebView2 ----

    private static TcpListener? _listener;
    private static readonly HashSet<Socket> Open = new();

    public static int BridgePort { get; private set; }

    /// Mở cổng nội bộ (127.0.0.1, không cần đăng nhập) — trình phát luôn đi qua đây.
    public static int StartBridge()
    {
        if (_listener != null) return BridgePort;
        _listener = new TcpListener(IPAddress.Loopback, 0);
        _listener.Start(256);
        BridgePort = ((IPEndPoint)_listener.LocalEndpoint).Port;
        _ = Task.Run(async () =>
        {
            while (true)
            {
                Socket c;
                try
                {
                    c = await _listener.AcceptSocketAsync();
                }
                catch (Exception)
                {
                    return;
                }
                _ = Task.Run(() => Serve(c));
            }
        });
        Log.D("net", $"Cổng nội bộ 127.0.0.1:{BridgePort}");
        return BridgePort;
    }

    /// Đổi cài đặt mạng (proxy / vượt chặn): đóng các kết nối đang mở của trình phát để nó mở lại theo cài đặt mới.
    public static void SettingsChanged()
    {
        List<Socket> list;
        lock (Open) list = Open.ToList();
        foreach (var s in list)
        {
            try { s.Dispose(); } catch (Exception) { }
        }
        lock (Lock) _lastProxyError = null;
    }

    private static async Task Serve(Socket client)
    {
        Socket? up = null;
        lock (Open) Open.Add(client);
        try
        {
            client.NoDelay = true;
            var cs = new NetworkStream(client, false);
            using var t = new CancellationTokenSource(30_000);
            var g = new byte[2];
            await cs.ReadExactlyAsync(g, t.Token);
            if (g[0] != 5) return;
            await cs.ReadExactlyAsync(new byte[g[1]], t.Token);
            await cs.WriteAsync(new byte[] { 5, 0 }, t.Token);
            var r = new byte[4];
            await cs.ReadExactlyAsync(r, t.Token);
            if (r[1] != 1)
            {
                await cs.WriteAsync(new byte[] { 5, 7, 0, 1, 0, 0, 0, 0, 0, 0 });
                return;
            }
            string host;
            switch (r[3])
            {
                case 1:
                    var v4 = new byte[4];
                    await cs.ReadExactlyAsync(v4, t.Token);
                    host = new IPAddress(v4).ToString();
                    break;
                case 4:
                    var v6 = new byte[16];
                    await cs.ReadExactlyAsync(v6, t.Token);
                    host = new IPAddress(v6).ToString();
                    break;
                case 3:
                    var len = new byte[1];
                    await cs.ReadExactlyAsync(len, t.Token);
                    var name = new byte[len[0]];
                    await cs.ReadExactlyAsync(name, t.Token);
                    host = Encoding.ASCII.GetString(name);
                    break;
                default:
                    return;
            }
            var pb = new byte[2];
            await cs.ReadExactlyAsync(pb, t.Token);
            var port = pb[0] << 8 | pb[1];
            bool via;
            try
            {
                (up, via) = await Connect(host, port, t.Token);
            }
            catch (Exception e)
            {
                var code = e is SocksError se ? (byte)Math.Clamp(se.Code, 1, 8) : (byte)4;
                await cs.WriteAsync(new byte[] { 5, code, 0, 1, 0, 0, 0, 0, 0, 0 });
                return;
            }
            lock (Open) Open.Add(up);
            await cs.WriteAsync(new byte[] { 5, 0, 0, 1, 0, 0, 0, 0, 0, 0 });
            var us = new NetworkStream(up, false);
            var level = Unblock.Level;
            var helloPending = port == 443 && level > 0;
            var upSocket = up;
            var toUp = Task.Run(async () =>
            {
                var buf = new byte[32 * 1024];
                int n;
                while ((n = await cs.ReadAsync(buf)) > 0)
                {
                    if (helloPending)
                    {
                        helloPending = false;
                        Unblock.WriteHello(upSocket, buf.AsSpan(0, n), host, level, via);
                    }
                    else await us.WriteAsync(buf.AsMemory(0, n));
                }
                upSocket.Shutdown(SocketShutdown.Send);
            });
            var toClient = us.CopyToAsync(cs, 64 * 1024);
            await Task.WhenAny(toUp, toClient);
            await Task.WhenAny(Task.WhenAll(toUp, toClient), Task.Delay(2000));
        }
        catch (Exception)
        {
        }
        finally
        {
            lock (Open)
            {
                Open.Remove(client);
                if (up != null) Open.Remove(up);
            }
            up?.Dispose();
            client.Dispose();
        }
    }
}
