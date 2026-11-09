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

/// DNS mã hoá (từ 1.7.1 bật sẵn; bản TV: Doh.kt, iPad: Doh.swift): hỏi địa chỉ tên miền qua DoH (RFC 8484) rồi DoT
/// (RFC 7858), mỗi giao thức lần lượt NextDNS (hồ sơ riêng của app — mã trong keys\nextdns.txt, đóng vào tệp chạy lúc dựng,
/// không có trên git) → Cloudflare → Google; lỗi hết thì bên gọi dùng DNS của máy — app không bao giờ hỏng vì DNS mã hoá bị
/// chặn. Máy chủ gọi bằng IP cố định, kiểm chứng chỉ theo tên máy chủ, đi thẳng (không qua proxy / cổng nội bộ). Máy chủ lỗi
/// thì nghỉ 2 phút. Trả lời «không có tên miền» (NXDOMAIN — vd hồ sơ NextDNS chặn) là trả lời thật: không hỏi máy chủ sau.
///
/// Tắt DNS mã hoá mà đang Vượt chặn: DNS của máy không trả lời / trả địa chỉ chặn (0.0.0.0, ::…) thì vẫn hỏi lại qua chuỗi
/// này (Unblocked) — vd ảnh bìa CLB Phim Xưa (clbphimxua.com) bị bộ lọc DNS trong nhà chặn.
public static class Doh
{
    public enum Kind { DoH, DoT }

    public sealed record Server(string Label, Kind Kind, string Host, string Path, string[] Ips)
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
        if (id.Length > 0) l.Add(new Server("DoH NextDNS", Kind.DoH, "dns.nextdns.io", "/" + id, NextIps));
        l.Add(new Server("DoH Cloudflare", Kind.DoH, "dns.cloudflare.com", "/dns-query", new[] { "104.16.248.249", "104.16.249.249" }));
        l.Add(new Server("DoH Google", Kind.DoH, "dns.google", "/dns-query", GoogleIps));
        if (id.Length > 0) l.Add(new Server("DoT NextDNS", Kind.DoT, id + ".dns.nextdns.io", "", NextIps));
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
        public sealed record Found(IPAddress Addr, int Ttl) : Answer;
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
            Answer answer;
            try
            {
                answer = await Ask(s, name, ct);
            }
            catch (Exception e)
            {
                // Máy chủ bị chặn / không trả lời: nghỉ 2 phút, lần sau thử IP còn lại của nó
                DownUntil[s.Label] = Util.NowMs() + DownMs;
                NextIp.AddOrUpdate(s.Label, 1, (_, v) => v + 1);
                Log.D("dns", $"{s.Label} lỗi: {e.GetType().Name} {e.Message}");
                continue;
            }
            switch (answer)
            {
                case Answer.Found f:
                    Cache[name] = (f.Addr, Util.NowMs() + Math.Clamp(f.Ttl, 30, 600) * 1000L);
#if DEBUG
                    Log.D("dns", $"{name} → {f.Addr} ({s.Label})");
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

    /// Hỏi một máy chủ (một trong các IP của nó).
    public static async Task<Answer> Ask(Server s, string name, CancellationToken ct)
    {
#if DEBUG
        if (DebugFail.Contains(s.Label)) throw new IOException("giả bị chặn");
#endif
        var ip = s.Ips[NextIp.GetValueOrDefault(s.Label) % s.Ips.Length];
        var id = s.Kind == Kind.DoT ? RandomNumberGenerator.GetInt32(0x10000) : 0; // DoH khuyên id 0 (dễ lưu đệm)
        var reply = await Exchange(s, ip, DnsWire.Query(name, id), ct);
        return reply == null ? new Answer.Failed() : DnsWire.Answer(reply, id);
    }

    private static async Task<byte[]?> Exchange(Server s, string ip, byte[] query, CancellationToken ct)
    {
        using var sock = new Socket(AddressFamily.InterNetwork, SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        using (var t = CancellationTokenSource.CreateLinkedTokenSource(ct))
        {
            t.CancelAfter(ConnectMs);
            await sock.ConnectAsync(new IPEndPoint(IPAddress.Parse(ip), s.Port), t.Token);
        }
        using var rt = CancellationTokenSource.CreateLinkedTokenSource(ct);
        rt.CancelAfter(ReadMs);
        await using var ssl = new SslStream(new NetworkStream(sock, false));
        // Kiểm chứng chỉ theo tên máy chủ (mặc định của SslStream)
        await ssl.AuthenticateAsClientAsync(new SslClientAuthenticationOptions { TargetHost = s.Host }, rt.Token);
        if (s.Kind == Kind.DoT)
        {
            await ssl.WriteAsync(new[] { (byte)(query.Length >> 8), (byte)query.Length }.Concat(query).ToArray(), rt.Token);
            var len = new byte[2];
            await ssl.ReadExactlyAsync(len, rt.Token);
            var reply = new byte[(len[0] << 8) | len[1]];
            await ssl.ReadExactlyAsync(reply, rt.Token);
            return reply;
        }
        var head = $"POST {s.Path} HTTP/1.1\r\nHost: {s.Host}\r\nContent-Type: application/dns-message\r\n" +
                   $"Accept: application/dns-message\r\nContent-Length: {query.Length}\r\nConnection: close\r\n\r\n";
        await ssl.WriteAsync(Encoding.ASCII.GetBytes(head).Concat(query).ToArray(), rt.Token);
        using var all = new MemoryStream();
        var buf = new byte[4096];
        int n;
        while ((n = await ssl.ReadAsync(buf, rt.Token)) > 0)
        {
            all.Write(buf, 0, n);
            if (all.Length > 70_000) throw new IOException("trả lời DNS quá dài");
        }
        return DohHttp.Body(all.ToArray());
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
        IPAddress? found = null;
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
                found ??= new IPAddress(b[i..(i + 4)]);
                ttl = (int)Math.Min(ttl, Math.Min(t, int.MaxValue));
            }
            i += len;
        }
        return found != null ? new Doh.Answer.Found(found, ttl == int.MaxValue ? 60 : ttl) : new Doh.Answer.NoIpv4();
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

/// Đọc trả lời HTTP/1.1 đã nhận trọn (Connection: close): thân khi mã 200, null nếu mã khác.
public static class DohHttp
{
    public static byte[]? Body(byte[] resp)
    {
        var sep = -1;
        for (var i = 0; i + 3 < resp.Length; i++)
        {
            if (resp[i] == '\r' && resp[i + 1] == '\n' && resp[i + 2] == '\r' && resp[i + 3] == '\n')
            {
                sep = i;
                break;
            }
        }
        if (sep < 0) return null;
        var head = Encoding.Latin1.GetString(resp, 0, sep);
        if (head.Split("\r\n")[0].Split(' ').ElementAtOrDefault(1) != "200") return null;
        var body = resp[(sep + 4)..];
        if (Regex.IsMatch(head, @"(?im)^transfer-encoding:\s*chunked")) return Dechunk(body);
        var m = Regex.Match(head, @"(?im)^content-length:\s*(\d+)");
        return m.Success && int.TryParse(m.Groups[1].Value, out var len) && len <= body.Length ? body[..len] : body;
    }

    private static byte[] Dechunk(byte[] b)
    {
        var o = new MemoryStream();
        var i = 0;
        while (i < b.Length)
        {
            var eol = -1;
            for (var j = i; j + 1 < b.Length; j++)
            {
                if (b[j] == '\r' && b[j + 1] == '\n')
                {
                    eol = j;
                    break;
                }
            }
            if (eol < 0) break;
            var size = Encoding.Latin1.GetString(b, i, eol - i).Split(';')[0].Trim();
            if (!int.TryParse(size, System.Globalization.NumberStyles.HexNumber, null, out var n) || n == 0) break;
            var start = eol + 2;
            if (start + n > b.Length) break;
            o.Write(b, start, n);
            i = start + n + 2;
        }
        return o.ToArray();
    }
}
