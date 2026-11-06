using System.IO;
using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace ConBangTV.Core;

/// Máy đã ghép để đồng bộ trong nhà.
public sealed class SyncPeer
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Platform { get; set; } = "";
    public string Token { get; set; } = "";
    /// Địa chỉ lần cuối kết nối được (khi không tìm thấy máy trong mạng).
    public string? Host { get; set; }
    public int? Port { get; set; }
    /// Lần đồng bộ được gần nhất (ms) và lời báo lần thử gần nhất.
    public long? Last { get; set; }
    public string? Status { get; set; }
}

/// Máy tìm thấy trong mạng (mDNS), chưa chắc đã ghép.
public sealed record SyncFound(string Id, string Name, string Platform, string Host, int Port);

/// Đồng bộ trong nhà (bản Android: SyncNet.kt, iPad: SyncService.swift — cùng giao thức). Mỗi máy mở cổng 8791 trong mạng nhà
/// và quảng bá `_conbangtv._tcp` (mDNS):
///   GET  /hello  — mã máy, tên, có đang cho ghép không
///   POST /pair   — {device, name, platform, code}: đúng mã ghép đang hiện trên máy này → trả khoá chung (token)
///   POST /sync   — thân = SyncPayload; ký X-Device / X-Time / X-Sig = HMAC-SHA256(token, "time\nthân"). Máy nhận gộp
///                  rồi trả lại SyncPayload đã gộp (cũng ký) — máy gửi gộp nốt. Mọi yêu cầu kèm X-Port (cổng của máy gửi)
///                  để máy kia nhớ địa chỉ gọi lại (khác lớp mạng thì mDNS không thấy).
/// Cổng chỉ mở khi đã ghép ít nhất một máy hoặc đang mở trang Đồng bộ. Đồng bộ đi thẳng mạng nhà (không qua proxy).
public static class SyncService
{
#if DEBUG
    public const int Port = 8792; // bản thử chạy song song bản chính trên cùng máy
#else
    public const int Port = 8791;
#endif
    public const string ServiceType = "_conbangtv._tcp";
    public const string Platform = "windows";

    public static string DeviceName => Environment.MachineName;

    /// Dữ liệu máy này vừa đổi do đồng bộ (giao diện vẽ lại). Gọi từ luồng nền.
    public static event Action? DataChanged;
    /// Danh sách máy / mã ghép / trạng thái đổi. Gọi từ luồng nền.
    public static event Action? StateChanged;
    /// Lời báo ngắn cho người xem. Gọi từ luồng nền.
    public static event Action<string>? Message;

    private static readonly object Lock = new();
    private static TcpListener? _listener;
    private static string? _pairCode;
    private static int _pairTries;
    private static List<SyncFound> _found = new();
    private static CancellationTokenSource? _soon;
    private static int _busy;

    public static string? PairCode
    {
        get { lock (Lock) return _pairCode; }
    }

    public static List<SyncFound> Found
    {
        get { lock (Lock) return _found.ToList(); }
    }

    public static bool Busy => _busy != 0;

    public static bool Listening => _listener != null;

    public static List<SyncPeer> Peers => Prefs.SyncPeers;

    private static void UpdatePeer(string id, Action<SyncPeer> change)
    {
        lock (Lock)
        {
            var list = Prefs.SyncPeers;
            var p = list.FirstOrDefault(x => x.Id == id);
            if (p == null) return;
            change(p);
            Prefs.SyncPeers = list;
        }
        StateChanged?.Invoke();
    }

    private static void AddPeer(SyncPeer p)
    {
        lock (Lock) Prefs.SyncPeers = Prefs.SyncPeers.Where(x => x.Id != p.Id).Append(p).ToList();
        StateChanged?.Invoke();
    }

    public static void Unpair(string id)
    {
        lock (Lock) Prefs.SyncPeers = Prefs.SyncPeers.Where(x => x.Id != id).ToList();
        if (Prefs.SyncPeers.Count == 0 && PairCode == null) Stop();
        StateChanged?.Invoke();
    }

    // ---- Mở cổng / quảng bá ----

    /// Mở app: đã ghép thì mở cổng, đồng bộ.
    public static void AppStarted()
    {
        if (Peers.Count == 0) return;
        Start();
        SyncSoon(2);
    }

    public static void Start()
    {
        StartListener();
        Mdns.Advertise(DeviceName, Port, Prefs.DeviceId, Platform);
    }

    public static void Stop()
    {
        try { _listener?.Stop(); } catch (Exception) { }
        _listener = null;
        Mdns.StopAdvertise();
        StateChanged?.Invoke();
    }

    private static void StartListener()
    {
        if (_listener != null) return;
        try
        {
            var l = new TcpListener(IPAddress.IPv6Any, Port);
            l.Server.DualMode = true;
            l.Start(16);
            _listener = l;
            Log.D("sync", $"mở cổng {Port}");
            _ = Task.Run(async () =>
            {
                while (true)
                {
                    TcpClient c;
                    try
                    {
                        c = await l.AcceptTcpClientAsync();
                    }
                    catch (Exception)
                    {
                        return;
                    }
                    _ = Task.Run(() => Serve(c));
                }
            });
        }
        catch (Exception e)
        {
            Log.D("sync", $"không mở được cổng {Port}: {e.Message}");
            Message?.Invoke($"Không mở được cổng {Port} để máy khác gọi tới ({e.Message}).");
        }
        StateChanged?.Invoke();
    }

    /// Trang Đồng bộ mở: hiện mã ghép 4 số (đổi mỗi lần mở), mở cổng.
    public static void OpenPairing()
    {
        lock (Lock)
        {
            _pairCode = RandomNumberGenerator.GetInt32(0, 10000).ToString("D4");
            _pairTries = 0;
        }
        Start();
        StateChanged?.Invoke();
    }

    public static void ClosePairing()
    {
        lock (Lock) _pairCode = null;
        if (Peers.Count == 0) Stop();
        StateChanged?.Invoke();
    }

    // ---- Máy chủ (máy kia gọi tới) ----

    private static async Task Serve(TcpClient c)
    {
        using var _ = c;
        try
        {
            var st = c.GetStream();
            using var t = new CancellationTokenSource(15_000);
            var req = await Http1.Read(st, true, t.Token);
            if (req == null) return;
            var ep = (IPEndPoint)c.Client.RemoteEndPoint!;
            req.RemoteHost = HostString(ep.Address);
            var resp = Handle(req);
            await st.WriteAsync(resp, t.Token);
        }
        catch (Exception e)
        {
            Log.D("sync", "phục vụ lỗi: " + e.Message);
        }
    }

    private static string HostString(IPAddress a) => (a.IsIPv4MappedToIPv6 ? a.MapToIPv4() : a).ToString();

    private static byte[] Handle(Http1.Message r)
    {
        switch (r.Method, r.Path)
        {
            case ("GET", "/hello"):
                return Http1.Response(200, Json(new Dictionary<string, object?>
                {
                    ["app"] = "conbangtv", ["device"] = Prefs.DeviceId, ["name"] = DeviceName, ["platform"] = Platform,
                    ["version"] = UpdateCheck.CurrentVersion, ["pairing"] = PairCode != null,
                }));
            case ("POST", "/pair"):
                return PairRequest(r);
            case ("POST", "/sync"):
                var id = r.Headers.GetValueOrDefault("x-device");
                var peer = Peers.FirstOrDefault(p => p.Id == id);
                if (id == null || peer == null) return Http1.Response(403, Json(new() { ["error"] = "unpaired" }));
                if (!Http1.Verify(r, peer.Token)) return Http1.Response(401, Json(new() { ["error"] = "sig" }));
                var remote = SyncPayload.Parse(r.Body);
                if (remote == null) return Http1.Response(400, Json(new() { ["error"] = "json" }));
                var changed = Absorb(remote);
                var back = Callback(r);
                UpdatePeer(id, p =>
                {
                    p.Name = remote.Name;
                    p.Last = Util.NowMs();
                    p.Status = changed ? "Đã nhận dữ liệu mới" : "Đã khớp";
                    if (back != null)
                    {
                        p.Host = back.Value.Host;
                        p.Port = back.Value.Port;
                    }
                });
                if (changed) DataChanged?.Invoke();
                Log.D("sync", $"{remote.Name} gọi đồng bộ: {(changed ? "có thay đổi" : "đã khớp")}");
                return Http1.Response(200, Payload().ToJson(), peer.Token);
            default:
                return Http1.Response(404, Json(new() { ["error"] = "not found" }));
        }
    }

    private static byte[] PairRequest(Http1.Message r)
    {
        string? code;
        lock (Lock) code = _pairCode;
        if (code == null) return Http1.Response(403, Json(new() { ["error"] = "closed" }));
        var o = JsonScan.ParseObject(r.Body);
        var device = J.Str(o, "device");
        if (o == null || device.Length == 0 || !o.ContainsKey("code")) return Http1.Response(400, Json(new() { ["error"] = "json" }));
        if (J.Str(o, "code") != code)
        {
            lock (Lock)
            {
                // Đoán sai nhiều lần: đổi mã
                if (++_pairTries >= 5)
                {
                    _pairCode = RandomNumberGenerator.GetInt32(0, 10000).ToString("D4");
                    _pairTries = 0;
                }
            }
            StateChanged?.Invoke();
            return Http1.Response(403, Json(new() { ["error"] = "code" }));
        }
        var token = Convert.ToHexString(RandomNumberGenerator.GetBytes(32)).ToLowerInvariant();
        var back = Callback(r);
        var peer = new SyncPeer
        {
            Id = device, Name = J.Str(o, "name").NonEmpty() ?? "Máy khác", Platform = J.Str(o, "platform"), Token = token,
            Host = back?.Host ?? r.RemoteHost, Port = back?.Port, Status = "Vừa ghép",
        };
        AddPeer(peer);
        lock (Lock)
        {
            _pairCode = RandomNumberGenerator.GetInt32(0, 10000).ToString("D4"); // mã dùng một lần
            _pairTries = 0;
        }
        StateChanged?.Invoke();
        Message?.Invoke($"Đã ghép với {peer.Name}.");
        return Http1.Response(200, Json(new()
        {
            ["device"] = Prefs.DeviceId, ["name"] = DeviceName, ["platform"] = Platform, ["token"] = token,
        }));
    }

    /// Máy gọi tới báo cổng của nó (X-Port) → địa chỉ để gọi lại.
    private static (string Host, int Port)? Callback(Http1.Message m)
    {
        if (m.RemoteHost == null || !int.TryParse(m.Headers.GetValueOrDefault("x-port"), out var p) || p <= 0) return null;
        return (m.RemoteHost, p);
    }

    private static byte[] Json(Dictionary<string, object?> o) => JsonSerializer.SerializeToUtf8Bytes(o);

    // ---- Dữ liệu ----

    public static SyncPayload Payload() => new()
    {
        Device = Prefs.DeviceId, Name = DeviceName, Platform = Platform,
        Profiles = Prefs.Profiles.Select(Prefs.SyncProfileData).ToList(), Sites = Prefs.SyncSitesData,
    };

    /// Gộp dữ liệu máy kia vào máy này. true = máy này có thay đổi.
    public static bool Absorb(SyncPayload remote)
    {
        var local = Prefs.Profiles.Select(Prefs.SyncProfileData).ToList();
        var changed = false;
        foreach (var (l, r) in SyncMerge.Match(local, remote.Profiles))
            if (Prefs.ApplySync(SyncMerge.Profile(local[l], remote.Profiles[r]))) changed = true;
        if (Prefs.ApplySites(SyncMerge.Sites(Prefs.SyncSitesData, remote.Sites))) changed = true;
        return changed;
    }

    // ---- Máy khách (máy này gọi tới máy kia) ----

    /// Tìm máy trong mạng (mDNS, ~2 giây).
    public static async Task<List<SyncFound>> Browse()
    {
        var list = await Mdns.Browse(ServiceType, TimeSpan.FromSeconds(2));
        list = list.Where(f => f.Id != Prefs.DeviceId).ToList();
        lock (Lock) _found = list;
        StateChanged?.Invoke();
        return list;
    }

    /// "10.0.0.5", "10.0.0.5:8791", "[fe80::1%5]:8791" → (máy, cổng).
    public static (string Host, int Port)? ParseAddress(string input)
    {
        var s = input.Trim();
        if (s.Length == 0) return null;
        if (s.StartsWith('['))
        {
            var end = s.IndexOf(']');
            if (end < 0) return null;
            var h = s[1..end];
            var p = end + 2 < s.Length && s[end + 1] == ':' && int.TryParse(s[(end + 2)..], out var pp) ? pp : 8791;
            return (h, p);
        }
        var colon = s.LastIndexOf(':');
        if (colon > 0 && s.IndexOf(':') == colon && int.TryParse(s[(colon + 1)..], out var port)) return (s[..colon], port);
        return (s, 8791);
    }

    /// Ghép với máy kia bằng mã 4 số đang hiện trên máy đó. Trả về lời báo lỗi (null = được).
    public static async Task<string?> Pair(string host, int port, string code)
    {
        var body = JsonSerializer.SerializeToUtf8Bytes(new Dictionary<string, string>
        {
            ["device"] = Prefs.DeviceId, ["name"] = DeviceName, ["platform"] = Platform, ["code"] = code,
        });
        try
        {
            Start(); // mở cổng trước để máy kia gọi lại được
            var r = await Http1.Send(host, port, "POST", "/pair", body, null);
            switch (r.Status)
            {
                case 200: break;
                case 403:
                    var err = J.Str(JsonScan.ParseObject(r.Body), "error");
                    return err == "closed" ? "Máy kia chưa mở màn Đồng bộ (mở để hiện mã ghép)." : "Sai mã ghép.";
                default: return $"Máy kia trả lỗi {r.Status}.";
            }
            var o = JsonScan.ParseObject(r.Body);
            var id = J.Str(o, "device");
            var token = J.Str(o, "token");
            if (id.Length == 0 || token.Length == 0) return "Máy kia trả dữ liệu lạ.";
            AddPeer(new SyncPeer
            {
                Id = id, Name = J.Str(o, "name").NonEmpty() ?? "Máy khác", Platform = J.Str(o, "platform"), Token = token,
                Host = host, Port = port, Status = "Vừa ghép",
            });
            await SyncAll(true);
            return null;
        }
        catch (Exception e)
        {
            return "Không kết nối được: " + e.Message;
        }
    }

    /// Đồng bộ sau một lúc (gộp các lần gọi sát nhau: thoát phim, theo dõi phim…).
    public static void SyncSoon(double seconds = 3)
    {
        if (!Prefs.SyncAuto || Peers.Count == 0) return;
        CancellationTokenSource cts;
        lock (Lock)
        {
            _soon?.Cancel();
            _soon = cts = new CancellationTokenSource();
        }
        _ = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(TimeSpan.FromSeconds(seconds), cts.Token);
            }
            catch (TaskCanceledException)
            {
                return;
            }
            await SyncAll(false);
        });
    }

    public static async Task SyncAll(bool manual)
    {
        if (Peers.Count == 0 || Interlocked.Exchange(ref _busy, 1) == 1) return;
        StateChanged?.Invoke();
        try
        {
            Start();
            // Tìm lại địa chỉ trong mạng (máy kia đổi IP)
            if (Found.Count == 0 || manual)
            {
                try { await Browse(); } catch (Exception) { }
            }
            var changed = false;
            foreach (var p in Peers)
                if (await SyncOne(p)) changed = true;
            if (changed) DataChanged?.Invoke();
            if (manual) Message?.Invoke(changed ? "Đã đồng bộ — có dữ liệu mới." : "Đã đồng bộ.");
        }
        finally
        {
            Interlocked.Exchange(ref _busy, 0);
            StateChanged?.Invoke();
        }
    }

    /// Đồng bộ với một máy. true = máy này có thay đổi.
    private static async Task<bool> SyncOne(SyncPeer p)
    {
        var f = Found.FirstOrDefault(x => x.Id == p.Id);
        var targets = new List<(string Host, int Port)>();
        if (f != null) targets.Add((f.Host, f.Port));
        if (p.Host != null) targets.Add((p.Host, p.Port ?? 8791));
        targets = targets.Distinct().ToList();
        if (targets.Count == 0)
        {
            UpdatePeer(p.Id, x => x.Status = "Không thấy máy trong mạng");
            return false;
        }
        var body = Payload().ToJson();
        Exception? last = null;
        foreach (var (host, port) in targets)
        {
            try
            {
                var r = await Http1.Send(host, port, "POST", "/sync", body, p.Token);
                if (r.Status == 403)
                {
                    UpdatePeer(p.Id, x => x.Status = "Máy kia đã bỏ ghép — ghép lại");
                    return false;
                }
                var remote = r.Status == 200 && Http1.Verify(r, p.Token) ? SyncPayload.Parse(r.Body) : null;
                if (remote == null)
                {
                    UpdatePeer(p.Id, x => x.Status = $"Lỗi {r.Status}");
                    return false;
                }
                var changed = Absorb(remote);
                UpdatePeer(p.Id, x =>
                {
                    x.Name = remote.Name;
                    x.Last = Util.NowMs();
                    x.Status = changed ? "Đã nhận dữ liệu mới" : "Đã khớp";
                    x.Host = host;
                    x.Port = port;
                });
                Log.D("sync", $"đồng bộ với {remote.Name} ({host}:{port}): {(changed ? "có thay đổi" : "đã khớp")}");
                return changed;
            }
            catch (Exception e)
            {
                last = e;
            }
        }
        UpdatePeer(p.Id, x => x.Status = "Không kết nối được");
        Log.D("sync", $"không đồng bộ được với {p.Name}: {last?.Message}");
        return false;
    }
}

/// HTTP/1.1 tối giản trên TCP cho đồng bộ (giống SyncHTTP của bản iPad).
public static class Http1
{
    public sealed class Message
    {
        public string Method = "";
        public string Path = "";
        public int Status;
        public Dictionary<string, string> Headers = new();
        public byte[] Body = Array.Empty<byte>();
        public string? RemoteHost;
    }

    /// Đọc đủ một yêu cầu / trả lời (đầu + thân theo Content-Length).
    public static async Task<Message?> Read(Stream s, bool request, CancellationToken ct)
    {
        var buf = new MemoryStream();
        var chunk = new byte[16 * 1024];
        while (buf.Length < 8 << 20)
        {
            var n = await s.ReadAsync(chunk, ct);
            if (n <= 0) break;
            buf.Write(chunk, 0, n);
            var m = Parse(buf.GetBuffer().AsSpan(0, (int)buf.Length), request);
            if (m != null) return m;
        }
        return null;
    }

    public static Message? Parse(ReadOnlySpan<byte> data, bool request)
    {
        var end = data.IndexOf("\r\n\r\n"u8);
        if (end < 0) return null;
        var lines = Encoding.UTF8.GetString(data[..end]).Split("\r\n");
        var first = lines[0].Split(' ');
        if (first.Length < 2) return null;
        var m = new Message();
        foreach (var l in lines.Skip(1))
        {
            var i = l.IndexOf(':');
            if (i > 0) m.Headers[l[..i].Trim().ToLowerInvariant()] = l[(i + 1)..].Trim();
        }
        var length = int.TryParse(m.Headers.GetValueOrDefault("content-length"), out var len) ? len : 0;
        var start = end + 4;
        if (data.Length - start < length) return null;
        m.Body = data.Slice(start, length).ToArray();
        if (request)
        {
            m.Method = first[0];
            m.Path = first[1].Split('?')[0];
        }
        else
        {
            m.Status = int.TryParse(first[1], out var st) ? st : 0;
        }
        return m;
    }

    public static string Sign(string token, string time, byte[] body)
    {
        using var h = new HMACSHA256(Encoding.UTF8.GetBytes(token));
        var data = Encoding.UTF8.GetBytes(time + "\n").Concat(body).ToArray();
        return Convert.ToHexString(h.ComputeHash(data)).ToLowerInvariant();
    }

    /// Chữ ký đúng và thời điểm lệch không quá 10 phút.
    public static bool Verify(Message m, string token)
    {
        if (!m.Headers.TryGetValue("x-time", out var t) || !m.Headers.TryGetValue("x-sig", out var sig) || !long.TryParse(t, out var ms) ||
            Math.Abs(Util.NowMs() - ms) >= 10 * 60_000) return false;
        return CryptographicOperations.FixedTimeEquals(Encoding.ASCII.GetBytes(Sign(token, t, m.Body)),
            Encoding.ASCII.GetBytes(sig.ToLowerInvariant()));
    }

    public static byte[] Response(int status, byte[] body, string? token = null)
    {
        var head = new StringBuilder();
        head.Append($"HTTP/1.1 {status} {(status == 200 ? "OK" : "Error")}\r\nContent-Type: application/json; charset=utf-8\r\n");
        head.Append($"Content-Length: {body.Length}\r\nConnection: close\r\n");
        if (token != null)
        {
            var time = Util.NowMs().ToString();
            head.Append($"X-Time: {time}\r\nX-Sig: {Sign(token, time, body)}\r\n");
        }
        head.Append("\r\n");
        return Encoding.UTF8.GetBytes(head.ToString()).Concat(body).ToArray();
    }

    /// Gửi một yêu cầu tới máy kia (thẳng trong mạng nhà, không qua proxy), chờ tối đa 8 giây.
    public static async Task<Message> Send(string host, int port, string method, string path, byte[] body, string? token)
    {
        var head = new StringBuilder();
        head.Append($"{method} {path} HTTP/1.1\r\nHost: conbangtv\r\nContent-Type: application/json\r\n");
        head.Append($"Content-Length: {body.Length}\r\nConnection: close\r\nX-Port: {SyncService.Port}\r\n");
        if (token != null)
        {
            var time = Util.NowMs().ToString();
            head.Append($"X-Device: {Prefs.DeviceId}\r\nX-Time: {time}\r\nX-Sig: {Sign(token, time, body)}\r\n");
        }
        head.Append("\r\n");
        using var t = new CancellationTokenSource(8000);
        using var c = new TcpClient(AddressFamily.InterNetworkV6) { NoDelay = true };
        c.Client.DualMode = true;
        try
        {
            if (IPAddress.TryParse(host, out var ip)) await c.ConnectAsync(ip, port, t.Token);
            else await c.ConnectAsync(host, port, t.Token);
            var s = c.GetStream();
            await s.WriteAsync(Encoding.UTF8.GetBytes(head.ToString()).Concat(body).ToArray(), t.Token);
            var m = await Read(s, false, t.Token) ?? throw new IOException("máy kia đóng kết nối");
            m.RemoteHost = host;
            return m;
        }
        catch (OperationCanceledException)
        {
            throw new IOException("quá giờ");
        }
    }
}

/// mDNS / DNS-SD tối giản: tìm `_conbangtv._tcp` (gửi câu hỏi kiểu "legacy unicast" — máy trả lời gửi thẳng lại cổng của
/// app) và quảng bá máy này bằng DnsServiceRegister của Windows (Windows 10 1809+).
public static class Mdns
{
    private static readonly IPEndPoint Group = new(IPAddress.Parse("224.0.0.251"), 5353);

    public static async Task<List<SyncFound>> Browse(string type, TimeSpan wait)
    {
        var name = $"{type}.local";
        var query = BuildQuery(name);
        var sockets = new List<UdpClient>();
        foreach (var addr in LocalIPv4())
        {
            try
            {
                var u = new UdpClient(new IPEndPoint(addr, 0));
                u.Client.SetSocketOption(SocketOptionLevel.IP, SocketOptionName.MulticastInterface, addr.GetAddressBytes());
                u.Client.SetSocketOption(SocketOptionLevel.IP, SocketOptionName.MulticastTimeToLive, 255);
                sockets.Add(u);
            }
            catch (Exception)
            {
            }
        }
        var records = new List<(Rec Rec, IPAddress From)>();
        using var cts = new CancellationTokenSource(wait);
        var tasks = sockets.Select(async u =>
        {
            try
            {
                await u.SendAsync(query, Group);
                await Task.Delay(400);
                await u.SendAsync(query, Group);
                while (!cts.IsCancellationRequested)
                {
                    var r = await u.ReceiveAsync(cts.Token);
                    var recs = ParseRecords(r.Buffer);
                    lock (records) records.AddRange(recs.Select(x => (x, r.RemoteEndPoint.Address)));
                }
            }
            catch (Exception)
            {
            }
        }).ToList();
        await Task.WhenAll(tasks);
        foreach (var u in sockets) u.Dispose();

        var outList = new List<SyncFound>();
        var all = records.Select(x => x.Rec).ToList();
        foreach (var (ptr, from) in records.Where(x => x.Rec.Type == 12 && x.Rec.Name.Equals(name, StringComparison.OrdinalIgnoreCase)))
        {
            var instance = ptr.Target!;
            var srv = all.FirstOrDefault(r => r.Type == 33 && r.Name.Equals(instance, StringComparison.OrdinalIgnoreCase));
            var txt = all.FirstOrDefault(r => r.Type == 16 && r.Name.Equals(instance, StringComparison.OrdinalIgnoreCase));
            var id = txt?.Txt?.GetValueOrDefault("id");
            if (id == null) continue;
            var a = srv?.Target != null
                ? all.FirstOrDefault(r => r.Type == 1 && r.Name.Equals(srv.Target, StringComparison.OrdinalIgnoreCase))?.Address
                : null;
            var host = (a ?? from).ToString();
            var label = instance.EndsWith("." + name, StringComparison.OrdinalIgnoreCase) ? instance[..^(name.Length + 1)] : instance;
            outList.Add(new SyncFound(id, label.Replace("\\032", " "), txt!.Txt!.GetValueOrDefault("p") ?? "", host, srv?.Port ?? 8791));
        }
        return outList.DistinctBy(f => f.Id).OrderBy(f => f.Name).ToList();
    }

    private static IEnumerable<IPAddress> LocalIPv4() =>
        NetworkInterface.GetAllNetworkInterfaces()
            .Where(n => n.OperationalStatus == OperationalStatus.Up && n.NetworkInterfaceType != NetworkInterfaceType.Loopback &&
                        n.SupportsMulticast)
            .SelectMany(n => n.GetIPProperties().UnicastAddresses)
            .Select(a => a.Address).Where(a => a.AddressFamily == AddressFamily.InterNetwork && !IPAddress.IsLoopback(a))
            .Distinct();

    private static byte[] BuildQuery(string name)
    {
        var b = new List<byte> { 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0 };
        foreach (var label in name.Split('.'))
        {
            var l = Encoding.UTF8.GetBytes(label);
            b.Add((byte)l.Length);
            b.AddRange(l);
        }
        b.Add(0);
        b.AddRange(new byte[] { 0, 12, 0x80, 1 }); // PTR, IN + QU (xin trả lời thẳng)
        return b.ToArray();
    }

    public sealed class Rec
    {
        public string Name = "";
        public int Type;
        public string? Target;
        public int Port;
        public IPAddress? Address;
        public Dictionary<string, string>? Txt;
    }

    public static List<Rec> ParseRecords(byte[] p)
    {
        var list = new List<Rec>();
        if (p.Length < 12) return list;
        int qd = p[4] << 8 | p[5], an = p[6] << 8 | p[7], ns = p[8] << 8 | p[9], ar = p[10] << 8 | p[11];
        var i = 12;
        try
        {
            for (var q = 0; q < qd; q++)
            {
                ReadName(p, ref i);
                i += 4;
            }
            for (var k = 0; k < an + ns + ar; k++)
            {
                var r = new Rec { Name = ReadName(p, ref i) };
                r.Type = p[i] << 8 | p[i + 1];
                var len = p[i + 8] << 8 | p[i + 9];
                var d = i + 10;
                switch (r.Type)
                {
                    case 12:
                        var j = d;
                        r.Target = ReadName(p, ref j);
                        break;
                    case 33:
                        r.Port = p[d + 4] << 8 | p[d + 5];
                        var j2 = d + 6;
                        r.Target = ReadName(p, ref j2);
                        break;
                    case 16:
                        r.Txt = new();
                        for (var t = d; t < d + len;)
                        {
                            var l = p[t];
                            var kv = Encoding.UTF8.GetString(p, t + 1, l);
                            var eq = kv.IndexOf('=');
                            if (eq > 0) r.Txt[kv[..eq]] = kv[(eq + 1)..];
                            t += l + 1;
                        }
                        break;
                    case 1:
                        if (len == 4) r.Address = new IPAddress(p.AsSpan(d, 4));
                        break;
                }
                list.Add(r);
                i = d + len;
            }
        }
        catch (Exception)
        {
        }
        return list;
    }

    private static string ReadName(byte[] p, ref int i)
    {
        var labels = new List<string>();
        var pos = i;
        var jumped = false;
        for (var guard = 0; guard < 64; guard++)
        {
            int l = p[pos];
            if (l == 0)
            {
                pos++;
                break;
            }
            if ((l & 0xC0) == 0xC0)
            {
                var ptr = (l & 0x3F) << 8 | p[pos + 1];
                if (!jumped) i = pos + 2;
                jumped = true;
                pos = ptr;
                continue;
            }
            labels.Add(Encoding.UTF8.GetString(p, pos + 1, l));
            pos += l + 1;
        }
        if (!jumped) i = pos;
        return string.Join(".", labels);
    }

    // ---- Quảng bá (DnsServiceRegister, dnsapi.dll) ----

    [StructLayout(LayoutKind.Sequential)]
    private struct RegisterRequest
    {
        public uint Version;
        public uint InterfaceIndex;
        public IntPtr ServiceInstance;
        public IntPtr Callback;
        public IntPtr Context;
        public IntPtr Credentials;
        public int UnicastEnabled;
    }

    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    private delegate void RegisterComplete(uint status, IntPtr context, IntPtr instance);

    [DllImport("dnsapi.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr DnsServiceConstructInstance(string serviceName, string hostName, IntPtr ip4, IntPtr ip6, ushort port,
        ushort priority, ushort weight, uint count, string[] keys, string[] values);

    [DllImport("dnsapi.dll")]
    private static extern uint DnsServiceRegister(ref RegisterRequest req, IntPtr cancel);

    [DllImport("dnsapi.dll")]
    private static extern uint DnsServiceDeRegister(ref RegisterRequest req, IntPtr cancel);

    [DllImport("dnsapi.dll")]
    private static extern void DnsServiceFreeInstance(IntPtr instance);

    private static readonly RegisterComplete Done = (status, _, inst) =>
    {
        Log.D("sync", $"quảng bá mDNS: mã {status}");
    };

    private static RegisterRequest? _req;

    public static void Advertise(string deviceName, int port, string id, string platform)
    {
        if (_req != null) return;
        try
        {
            var inst = DnsServiceConstructInstance($"{deviceName}.{SyncService.ServiceType}.local", $"{Dns.GetHostName()}.local",
                IntPtr.Zero, IntPtr.Zero, (ushort)port, 0, 0, 2, new[] { "id", "p" }, new[] { id, platform });
            if (inst == IntPtr.Zero) return;
            var req = new RegisterRequest
            {
                Version = 1, ServiceInstance = inst, Callback = Marshal.GetFunctionPointerForDelegate(Done),
            };
            var r = DnsServiceRegister(ref req, IntPtr.Zero);
            if (r != 9506) // DNS_REQUEST_PENDING
            {
                Log.D("sync", $"không quảng bá được mDNS (mã {r})");
                DnsServiceFreeInstance(inst);
                return;
            }
            _req = req;
        }
        catch (Exception e)
        {
            // Windows cũ chưa có API này: máy kia nhập IP của máy này để ghép
            Log.D("sync", "không quảng bá được mDNS: " + e.Message);
        }
    }

    public static void StopAdvertise()
    {
        if (_req is not { } req) return;
        _req = null;
        try
        {
            DnsServiceDeRegister(ref req, IntPtr.Zero);
        }
        catch (Exception)
        {
        }
    }
}
