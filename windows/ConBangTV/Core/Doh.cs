using System.Collections.Concurrent;
using System.IO;
using System.Net;
using System.Net.Security;
using System.Net.Sockets;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace ConBangTV.Core;

/// DNS mã hoá (từ 1.8 bật sẵn; bản TV: Doh.kt, iPad: Doh.swift): hỏi địa chỉ tên miền qua DoH (RFC 8484) rồi DoT
/// (RFC 7858), mỗi giao thức lần lượt NextDNS (hồ sơ riêng của app — mã trong keys\nextdns.txt, đóng vào tệp chạy lúc dựng,
/// không có trên git) → Cloudflare → Google; lỗi hết thì bên gọi dùng DNS của máy — app không bao giờ hỏng vì DNS mã hoá bị
/// chặn. Máy chủ gọi bằng IP, kiểm chứng chỉ theo tên máy chủ, đi thẳng (không qua proxy / cổng nội bộ). Từ 1.8.1:
/// - NextDNS dùng máy chủ gần nhất: IP anycast từ mạng Viettel bị đưa sang Hồng Kông → lần đầu hỏi chính NextDNS tên
///   «dns.nextdns.io» (NextDNS trả máy chủ gần IP của mạng này nhất), không được thì DNS của máy; nhớ 6 giờ; anycast đứng sau.
/// - Giữ kết nối TLS tới máy chủ DNS để dùng lại (DoT; DoH bằng HTTP/1.1 keep-alive): mở kết nối mới ~140–220 ms, dùng lại
///   ~20–40 ms. Kết nối nghỉ quá 90 giây thì bỏ; kết nối cũ bị máy chủ đóng thì mở kết nối mới ngay.
/// Máy chủ lỗi thì nghỉ 2 phút. Trả lời «không có tên miền» (NXDOMAIN — vd hồ sơ NextDNS chặn) là trả lời thật: không hỏi máy
/// chủ sau.
///
/// Tắt DNS mã hoá mà đang Vượt chặn: DNS của máy không trả lời / trả địa chỉ chặn (0.0.0.0, ::…) thì vẫn hỏi lại qua chuỗi
/// này (Unblocked) — vd ảnh bìa CLB Phim Xưa (clbphimxua.com) bị bộ lọc DNS trong nhà chặn.
public static class Doh
{
    public enum Kind { DoH, DoT }

    /// Steer = NextDNS: IP lấy theo máy chủ gần nhất (NextPops) rồi mới tới Ips cố định (anycast).
    public sealed record Server(string Label, Kind Kind, string Host, string Path, string[] Ips, bool Steer = false)
    {
        public int Port => Kind == Kind.DoT ? 853 : 443;
    }

    private static readonly string[] NextIps = { "45.90.28.0", "45.90.30.0" };
    private static readonly string[] GoogleIps = { "8.8.8.8", "8.8.4.4" };

    /// Thứ tự hỏi: DoH NextDNS → Cloudflare → Google, rồi DoT cũng vậy (không có mã NextDNS thì bỏ NextDNS).
    public static List<Server> Chain(string nextId)
    {
        var id = nextId.Trim();
        var l = new List<Server>();
        if (id.Length > 0) l.Add(new Server("DoH NextDNS", Kind.DoH, "dns.nextdns.io", "/" + id, NextIps, true));
        l.Add(new Server("DoH Cloudflare", Kind.DoH, "dns.cloudflare.com", "/dns-query", new[] { "104.16.248.249", "104.16.249.249" }));
        l.Add(new Server("DoH Google", Kind.DoH, "dns.google", "/dns-query", GoogleIps));
        if (id.Length > 0) l.Add(new Server("DoT NextDNS", Kind.DoT, id + ".dns.nextdns.io", "", NextIps, true));
        l.Add(new Server("DoT Cloudflare", Kind.DoT, "1dot1dot1dot1.cloudflare-dns.com", "", new[] { "1.1.1.1", "1.0.0.1" }));
        l.Add(new Server("DoT Google", Kind.DoT, "dns.google", "", GoogleIps));
        return l;
    }

    public static readonly List<Server> Servers = Chain(NextDnsId());

    /// Tên các nhà cung cấp theo thứ tự, để hiện trong Cài đặt: "NextDNS → Cloudflare → Google".
    public static string Providers => string.Join(" → ", Servers.Select(s => s.Label[(s.Label.IndexOf(' ') + 1)..]).Distinct());

    /// Mã NextDNS đóng vào tệp chạy lúc dựng (EmbeddedResource từ keys\nextdns.txt); không có thì "".
    private static string NextDnsId()
    {
        try
        {
            using var st = Assembly.GetExecutingAssembly().GetManifestResourceStream("nextdns.txt");
            if (st == null) return "";
            var id = new StreamReader(st).ReadToEnd().Trim();
            return Regex.IsMatch(id, "^[a-z0-9]+$") ? id : "";
        }
        catch (Exception)
        {
            return "";
        }
    }

    private const int ConnectMs = 2_500;
    private const int ReadMs = 3_000;
    private const long DownMs = 120_000;
    private const long IdleMs = 90_000;
    private const int MaxIdle = 4;
    private const long PopsMs = 6 * 3_600_000L;

    /// Địa chỉ đã hỏi; Addr null = tên miền không có (NXDOMAIN).
    private static readonly ConcurrentDictionary<string, (IPAddress? Addr, long Until)> Cache = new();
    private static readonly ConcurrentDictionary<string, long> DownUntil = new();
    private static readonly ConcurrentDictionary<string, int> NextIp = new();

#if DEBUG
    /// Chỉ bản debug (cổng gỡ lỗi /dohfail): coi các máy chủ này như bị chặn, để thử chuỗi dự phòng.
    public static volatile HashSet<string> DebugFail = new();
#endif

    /// Kết quả hỏi một máy chủ (máy chủ lỗi thì ném lỗi).
    public abstract record Answer
    {
        /// All = mọi bản ghi A (theo thứ tự máy chủ trả); Addr = cái đầu.
        public sealed record Found(IPAddress Addr, int Ttl, IReadOnlyList<IPAddress> All) : Answer;
        /// NXDOMAIN: tên miền không có (hoặc hồ sơ DNS chặn) — không hỏi máy chủ khác.
        public sealed record NoDomain : Answer;
        /// Có tên miền nhưng không có bản ghi A (vd chỉ có IPv6) — để DNS của máy thử.
        public sealed record NoIpv4 : Answer;
        /// SERVFAIL / REFUSED / gói hỏng — hỏi máy chủ sau.
        public sealed record Failed : Answer;
    }

    /// Địa chỉ IPv4 qua chuỗi DNS mã hoá (nhớ theo TTL, 30 giây – 10 phút). null = IP / localhost, hoặc mọi máy chủ lỗi /
    /// không có bản ghi A → bên gọi dùng DNS của máy. Tên miền không có → SocksError 4.
    public static async Task<IPAddress?> Resolve(string host, CancellationToken ct, IReadOnlyList<Server>? servers = null)
    {
        var name = host.Trim('[', ']').TrimEnd('.').ToLowerInvariant();
        if (IPAddress.TryParse(name, out _) || name == "localhost") return null;
        if (Cache.TryGetValue(name, out var hit) && hit.Until > Util.NowMs())
            return hit.Addr ?? throw new SocksError(4, $"không tìm thấy tên miền {name}");
        // Nhiều kết nối cùng lúc tới một tên miền (ảnh bìa, API…): chỉ hỏi một lần, các kết nối kia chờ kết quả đó
        var job = Inflight.GetOrAdd(name, n => new Lazy<Task<IPAddress?>>(() => Lookup(n, servers ?? Servers)));
        try
        {
            return await job.Value.WaitAsync(ct);
        }
        finally
        {
            Inflight.TryRemove(KeyValuePair.Create(name, job));
        }
    }

    private static readonly ConcurrentDictionary<string, Lazy<Task<IPAddress?>>> Inflight = new();

    private static async Task<IPAddress?> Lookup(string name, IReadOnlyList<Server> servers)
    {
        var ct = CancellationToken.None; // dùng chung cho mọi kết nối đang chờ — chỉ dừng theo hạn giờ của từng máy chủ
        foreach (var s in servers)
        {
            if (DownUntil.TryGetValue(s.Label, out var down) && down > Util.NowMs()) continue;
            var ips = await IpsFor(s);
            var ip = ips[NextIp.GetValueOrDefault(s.Label) % ips.Count];
            var t0 = Util.NowMs();
            Answer answer;
            try
            {
                answer = await Ask(s, ip, name, ct);
            }
            catch (Exception e)
            {
                // Máy chủ bị chặn / không trả lời: nghỉ 2 phút, lần sau thử IP kế của nó
                DownUntil[s.Label] = Util.NowMs() + DownMs;
                NextIp.AddOrUpdate(s.Label, 1, (_, v) => v + 1);
                Log.D("dns", $"{s.Label} · {ip} lỗi: {e.GetType().Name} {e.Message}");
                continue;
            }
            switch (answer)
            {
                case Answer.Found f:
                    Cache[name] = (f.Addr, Util.NowMs() + Math.Clamp(f.Ttl, 30, 600) * 1000L);
#if DEBUG
                    Log.D("dns", $"{name} → {f.Addr} ({s.Label} · {ip}, {Util.NowMs() - t0} ms)");
#endif
                    return f.Addr;
                case Answer.NoDomain:
                    Cache[name] = (null, Util.NowMs() + 30_000); // nhớ 30 giây: kết nối khác khỏi hỏi lại
                    throw new SocksError(4, $"không tìm thấy tên miền {name}");
                case Answer.NoIpv4:
                    return null;
            }
        }
        return null;
    }

    // ---- NextDNS: máy chủ gần nhất ----

    private static volatile string[] _pops = Array.Empty<string>();
    private static long _popsUntil;
    private static readonly SemaphoreSlim PopsLock = new(1, 1);

    /// IP để gọi máy chủ s: NextDNS = máy chủ gần nhất rồi anycast; còn lại = IP cố định.
    public static async Task<IReadOnlyList<string>> IpsFor(Server s)
    {
        if (!s.Steer) return s.Ips;
        return (await NextPops()).Concat(s.Ips).Distinct().ToList();
    }

    private static async Task<string[]> NextPops()
    {
        if (Util.NowMs() < Interlocked.Read(ref _popsUntil)) return _pops;
        await PopsLock.WaitAsync();
        try
        {
            if (Util.NowMs() < Interlocked.Read(ref _popsUntil)) return _pops;
            var found = await DiscoverPops();
            _pops = found;
            Interlocked.Exchange(ref _popsUntil, Util.NowMs() + (found.Length > 0 ? PopsMs : 600_000));
            Log.D("dns", "NextDNS gần nhất: " + (found.Length > 0 ? string.Join(", ", found) : "(không tìm được — dùng anycast)"));
            return found;
        }
        finally
        {
            PopsLock.Release();
        }
    }

    /// Hỏi chính NextDNS (anycast, đã mã hoá) tên «dns.nextdns.io» — NextDNS trả máy chủ gần IP của mạng này; không được thì DNS máy.
    private static async Task<string[]> DiscoverPops()
    {
        var any = Servers.FirstOrDefault(x => x.Steer && x.Kind == Kind.DoH) ?? Servers.FirstOrDefault(x => x.Steer);
        if (any == null) return Array.Empty<string>();
        foreach (var ip in any.Ips)
        {
            try
            {
                if (await Ask(any, ip, "dns.nextdns.io", CancellationToken.None) is Answer.Found f)
                    return f.All.Where(a => !Blocked(a)).Select(a => a.ToString()).Distinct().ToArray();
            }
            catch (Exception)
            {
                // thử IP anycast kế
            }
        }
        try
        {
            return (await Dns.GetHostAddressesAsync("dns.nextdns.io"))
                .Where(a => a.AddressFamily == AddressFamily.InterNetwork && !Blocked(a)).Select(a => a.ToString()).Distinct().ToArray();
        }
        catch (Exception)
        {
            return Array.Empty<string>();
        }
    }

    /// Hỏi máy chủ s ở địa chỉ ip.
    public static async Task<Answer> Ask(Server s, string ip, string name, CancellationToken ct)
    {
#if DEBUG
        if (DebugFail.Contains(s.Label)) throw new IOException("giả bị chặn");
#endif
        var id = s.Kind == Kind.DoT ? RandomNumberGenerator.GetInt32(0x10000) : 0; // DoH khuyên id 0 (dễ lưu đệm)
        var reply = await Exchange(s, ip, DnsWire.Query(name, id), ct);
        return reply == null ? new Answer.Failed() : DnsWire.Answer(reply, id);
    }

    // ---- Kết nối dùng lại ----

    private sealed class Conn : IDisposable
    {
        public readonly string Key;
        public readonly Socket Sock;
        public readonly SslStream Ssl;
        public readonly BufferedStream In;
        public long LastUsed = Util.NowMs();

        public Conn(string key, Socket sock, SslStream ssl)
        {
            Key = key;
            Sock = sock;
            Ssl = ssl;
            In = new BufferedStream(ssl, 4096);
        }

        public void Dispose()
        {
            try { Ssl.Dispose(); } catch (Exception) { }
            try { Sock.Dispose(); } catch (Exception) { }
        }
    }

    private static readonly ConcurrentDictionary<string, ConcurrentStack<Conn>> Idle = new();

    private static Conn? Take(string key)
    {
        if (!Idle.TryGetValue(key, out var q)) return null;
        while (q.TryPop(out var c))
        {
            if (Util.NowMs() - c.LastUsed < IdleMs && c.Sock.Connected) return c;
            c.Dispose();
        }
        return null;
    }

    private static void Give(Conn c)
    {
        c.LastUsed = Util.NowMs();
        var q = Idle.GetOrAdd(c.Key, _ => new ConcurrentStack<Conn>());
        if (q.Count >= MaxIdle) c.Dispose();
        else q.Push(c);
    }

    /// Một lần hỏi: dùng lại kết nối đang nghỉ (hỏng thì mở mới ngay), không có thì mở kết nối mới.
    private static async Task<byte[]?> Exchange(Server s, string ip, byte[] query, CancellationToken ct)
    {
        var key = s.Label + "|" + ip;
        if (Take(key) is { } old)
        {
            try
            {
                return await RoundTrip(s, old, query, ct);
            }
            catch (Exception) when (!ct.IsCancellationRequested)
            {
                // máy chủ đã đóng kết nối cũ — không phải máy chủ lỗi, mở kết nối mới bên dưới
            }
        }
        return await RoundTrip(s, await Open(s, ip, key, ct), query, ct);
    }

    private static async Task<Conn> Open(Server s, string ip, string key, CancellationToken ct)
    {
        var sock = new Socket(AddressFamily.InterNetwork, SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            using (var t = CancellationTokenSource.CreateLinkedTokenSource(ct))
            {
                t.CancelAfter(ConnectMs);
                await sock.ConnectAsync(new IPEndPoint(IPAddress.Parse(ip), s.Port), t.Token);
            }
            var ssl = new SslStream(new NetworkStream(sock, false));
            using (var t = CancellationTokenSource.CreateLinkedTokenSource(ct))
            {
                t.CancelAfter(ReadMs);
                // Kiểm chứng chỉ theo tên máy chủ (mặc định của SslStream)
                await ssl.AuthenticateAsClientAsync(new SslClientAuthenticationOptions { TargetHost = s.Host }, t.Token);
            }
            return new Conn(key, sock, ssl);
        }
        catch (Exception)
        {
            sock.Dispose();
            throw;
        }
    }

    /// Gửi một câu hỏi trên kết nối c, đọc đúng một trả lời rồi trả kết nối về chỗ nghỉ (lỗi / máy chủ báo đóng thì đóng).
    private static async Task<byte[]?> RoundTrip(Server s, Conn c, byte[] query, CancellationToken ct)
    {
        var keep = false;
        try
        {
            using var t = CancellationTokenSource.CreateLinkedTokenSource(ct);
            t.CancelAfter(ReadMs);
            if (s.Kind == Kind.DoT)
            {
                await c.Ssl.WriteAsync(new[] { (byte)(query.Length >> 8), (byte)query.Length }.Concat(query).ToArray(), t.Token);
                var len = new byte[2];
                await c.In.ReadExactlyAsync(len, t.Token);
                var reply = new byte[(len[0] << 8) | len[1]];
                await c.In.ReadExactlyAsync(reply, t.Token);
                keep = true;
                return reply;
            }
            var head = $"POST {s.Path} HTTP/1.1\r\nHost: {s.Host}\r\nContent-Type: application/dns-message\r\n" +
                       $"Accept: application/dns-message\r\nContent-Length: {query.Length}\r\n\r\n";
            await c.Ssl.WriteAsync(Encoding.ASCII.GetBytes(head).Concat(query).ToArray(), t.Token);
            var r = await DohHttp.ReadAsync(c.In, t.Token);
            keep = r.KeepAlive;
            return r.Status == 200 ? r.Body : null;
        }
        finally
        {
            if (keep) Give(c);
            else c.Dispose();
        }
    }

    public static void ClearCache()
    {
        Cache.Clear();
        DownUntil.Clear();
    }

    // ---- Vượt chặn khi tắt DNS mã hoá ----

    /// Địa chỉ DNS chặn trả: trỏ về chính máy chứ không phải trang thật.
    public static bool Blocked(IPAddress a) =>
        a.Equals(IPAddress.Any) || a.Equals(IPAddress.IPv6Any) || IPAddress.IsLoopback(a);

    /// DNS của máy không trả lời / chỉ trả địa chỉ chặn → địa chỉ qua DNS mã hoá; null = cứ dùng DNS của máy.
    public static async Task<IPAddress?> Unblocked(string host, CancellationToken ct)
    {
        var name = host.Trim('[', ']').TrimEnd('.').ToLowerInvariant();
        if (IPAddress.TryParse(name, out _) || name == "localhost") return null;
        try
        {
            var list = await Dns.GetHostAddressesAsync(name, ct);
            if (list.Any(a => !Blocked(a))) return null;
        }
        catch (SocketException)
        {
            // DNS của máy không trả lời → hỏi DNS mã hoá
        }
        return await Resolve(name, ct);
    }

    /// Địa chỉ cho một kết nối mà máy tự phân giải (đi thẳng, hoặc proxy dùng «DNS của máy»): DNS mã hoá nếu bật, không thì
    /// Vượt chặn DNS; null = dùng DNS của máy như thường.
    public static Task<IPAddress?> ForConnect(string host, CancellationToken ct) =>
        Prefs.SecureDns ? Resolve(host, ct) : Unblock.Level > 0 ? Unblocked(host, ct) : Task.FromResult<IPAddress?>(null);
}

/// Gói DNS (RFC 1035): hỏi bản ghi A, đọc trả lời.
public static class DnsWire
{
    public static byte[] Query(string name, int id = 0)
    {
        var q = new List<byte> { (byte)(id >> 8), (byte)id, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0 }; // RD, 1 câu hỏi
        foreach (var label in name.TrimEnd('.').Split('.'))
        {
            var b = Encoding.ASCII.GetBytes(label);
            if (b.Length is < 1 or > 63) throw new ArgumentException($"tên miền sai: {name}");
            q.Add((byte)b.Length);
            q.AddRange(b);
        }
        q.AddRange(new byte[] { 0, 0, 1, 0, 1 }); // hết tên, kiểu A, lớp IN
        return q.ToArray();
    }

    /// Trả lời → Answer; id khác (DoT) hay gói hỏng → Failed.
    public static Doh.Answer Answer(byte[] b, int id)
    {
        if (b.Length < 12) return new Doh.Answer.Failed();
        int U16(int i) => (b[i] << 8) | b[i + 1];
        if (U16(0) != id) return new Doh.Answer.Failed();
        var rcode = b[3] & 0x0F;
        if (rcode == 3) return new Doh.Answer.NoDomain();
        if (rcode != 0) return new Doh.Answer.Failed();
        var i = 12;
        for (var k = U16(4); k > 0; k--) i = SkipName(b, i) + 4;
        var all = new List<IPAddress>();
        var ttl = int.MaxValue;
        for (var k = U16(6); k > 0; k--)
        {
            i = SkipName(b, i);
            if (i + 10 > b.Length) return new Doh.Answer.Failed();
            var type = U16(i);
            var t = (long)b[i + 4] << 24 | (long)b[i + 5] << 16 | (long)b[i + 6] << 8 | b[i + 7];
            var len = U16(i + 8);
            i += 10;
            if (i + len > b.Length) return new Doh.Answer.Failed();
            if (type == 1 && len == 4)
            {
                all.Add(new IPAddress(b[i..(i + 4)]));
                ttl = (int)Math.Min(ttl, Math.Min(t, int.MaxValue));
            }
            i += len;
        }
        return all.Count > 0 ? new Doh.Answer.Found(all[0], ttl == int.MaxValue ? 60 : ttl, all) : new Doh.Answer.NoIpv4();
    }

    /// Bỏ qua một tên (có thể nén bằng con trỏ 0xC0).
    private static int SkipName(byte[] b, int i)
    {
        while (i < b.Length)
        {
            var l = b[i];
            if (l == 0) return i + 1;
            if ((l & 0xC0) == 0xC0) return i + 2;
            i += 1 + l;
        }
        return b.Length;
    }
}

/// Trả lời HTTP/1.1 của máy chủ DoH: đọc đúng một trả lời trên kết nối (để dùng lại kết nối).
public static class DohHttp
{
    public sealed record Reply(int Status, byte[] Body, bool KeepAlive);

    /// Đọc đầu trả lời rồi thân theo Content-Length / chunked; không có độ dài thì đọc tới khi máy chủ đóng.
    public static async Task<Reply> ReadAsync(Stream s, CancellationToken ct)
    {
        var head = new MemoryStream();
        var one = new byte[1];
        var last = 0;
        while (last != 0x0D0A0D0A)
        {
            if (await s.ReadAsync(one, ct) == 0) throw new IOException("máy chủ ngắt giữa chừng");
            head.WriteByte(one[0]);
            last = (last << 8) | one[0];
            if (head.Length > 16_384) throw new IOException("đầu trả lời quá dài");
        }
        var h = Encoding.Latin1.GetString(head.ToArray());
        if (!int.TryParse(h.Split("\r\n")[0].Split(' ').ElementAtOrDefault(1), out var status)) throw new IOException("trả lời HTTP sai");
        var close = Regex.IsMatch(h, @"(?im)^connection:\s*close");
        if (Regex.IsMatch(h, @"(?im)^transfer-encoding:\s*chunked")) return new Reply(status, await Dechunk(s, ct), !close);
        var m = Regex.Match(h, @"(?im)^content-length:\s*(\d+)");
        if (!m.Success) return new Reply(status, await ReadToEnd(s, ct), false);
        var len = int.Parse(m.Groups[1].Value);
        if (len > 70_000) throw new IOException("trả lời DNS quá dài");
        var body = new byte[len];
        await s.ReadExactlyAsync(body, ct);
        return new Reply(status, body, !close);
    }

    /// Trả lời đã nhận trọn: thân khi mã 200, null nếu mã khác / hỏng (để test).
    public static byte[]? Body(byte[] resp)
    {
        try
        {
            var r = ReadAsync(new MemoryStream(resp), CancellationToken.None).GetAwaiter().GetResult();
            return r.Status == 200 ? r.Body : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static async Task<string> Line(Stream s, CancellationToken ct)
    {
        var b = new StringBuilder();
        var one = new byte[1];
        while (true)
        {
            if (await s.ReadAsync(one, ct) == 0) throw new IOException("máy chủ ngắt giữa chừng");
            if (one[0] == '\n') return b.ToString().TrimEnd('\r');
            b.Append((char)one[0]);
            if (b.Length > 1024) throw new IOException("dòng quá dài");
        }
    }

    private static async Task<byte[]> Dechunk(Stream s, CancellationToken ct)
    {
        var o = new MemoryStream();
        while (true)
        {
            var size = (await Line(s, ct)).Split(';')[0].Trim();
            if (!int.TryParse(size, System.Globalization.NumberStyles.HexNumber, null, out var n)) throw new IOException("chunk sai");
            if (n == 0)
            {
                while ((await Line(s, ct)).Length > 0)
                {
                    // bỏ phần đuôi (trailer)
                }
                return o.ToArray();
            }
            if (o.Length + n > 70_000) throw new IOException("trả lời DNS quá dài");
            var chunk = new byte[n];
            await s.ReadExactlyAsync(chunk, ct);
            o.Write(chunk);
            await Line(s, ct); // CRLF sau mỗi khúc
        }
    }

    private static async Task<byte[]> ReadToEnd(Stream s, CancellationToken ct)
    {
        var o = new MemoryStream();
        var buf = new byte[4096];
        int n;
        while ((n = await s.ReadAsync(buf, ct)) > 0)
        {
            o.Write(buf, 0, n);
            if (o.Length > 70_000) throw new IOException("trả lời DNS quá dài");
        }
        return o.ToArray();
    }
}
