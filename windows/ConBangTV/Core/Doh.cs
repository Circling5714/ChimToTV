using System.Collections.Concurrent;
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Text.Json;

namespace ConBangTV.Core;

/// Vượt chặn bằng DNS (bản TV: Doh.resolveAny + Socks5Bridge.deviceDns). DNS chặn trang — của nhà mạng, hay bộ lọc quảng
/// cáo trong mạng nhà — thường trả 0.0.0.0 / :: (có nơi 127.x / ::1), vd ảnh bìa CLB Phim Xưa (clbphimxua.com): tách gói
/// bắt tay thôi thì không đủ, không có địa chỉ thật để kết nối. Đang bật Vượt chặn mà DNS của máy không trả lời / trả địa
/// chỉ chặn thì hỏi lại qua DNS mã hoá (Cloudflare 1.1.1.1, lỗi thì Google 8.8.8.8 — gọi bằng IP, đi thẳng).
public static class Doh
{
    private static readonly (string Label, string Url)[] Servers =
    {
        ("Cloudflare 1.1.1.1", "https://1.1.1.1/dns-query"),
        ("Google 8.8.8.8", "https://8.8.8.8/resolve"),
    };

    // Đi thẳng, không qua Net.HttpConnect (proxy / vượt chặn) — máy chủ DoH gọi bằng IP, khỏi phải hỏi DNS
    private static readonly HttpClient Client = new(new SocketsHttpHandler { UseProxy = false }) { Timeout = TimeSpan.FromSeconds(6) };
    private static readonly ConcurrentDictionary<string, (IPAddress Addr, long Until)> Cache = new();

    /// Địa chỉ DNS chặn trả: trỏ về chính máy chứ không phải trang thật.
    public static bool Blocked(IPAddress a) =>
        a.Equals(IPAddress.Any) || a.Equals(IPAddress.IPv6Any) || IPAddress.IsLoopback(a);

    /// Vượt chặn: DNS của máy không trả lời / chỉ trả địa chỉ chặn → địa chỉ thật qua DNS mã hoá; null = cứ dùng DNS của máy.
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

    /// Hỏi bản ghi A qua DNS mã hoá (nhớ theo TTL, 10 giây – 5 phút); null nếu cả hai máy chủ đều lỗi.
    public static async Task<IPAddress?> Resolve(string name, CancellationToken ct)
    {
        var now = Util.NowMs();
        if (Cache.TryGetValue(name, out var hit) && hit.Until > now) return hit.Addr;
        foreach (var (label, url) in Servers)
        {
            try
            {
                using var req = new HttpRequestMessage(HttpMethod.Get, $"{url}?name={Uri.EscapeDataString(name)}&type=A");
                req.Headers.Accept.ParseAdd("application/dns-json");
                using var res = await Client.SendAsync(req, ct);
                if (!res.IsSuccessStatusCode) continue;
                var addr = Parse(await res.Content.ReadAsStringAsync(ct), out var ttl);
                if (addr == null) continue;
                Cache[name] = (addr, now + Math.Clamp(ttl, 10, 300) * 1000L);
                Log.D("dns", $"DNS mã hoá (vượt chặn): {name} → {addr} ({label})");
                return addr;
            }
            catch (Exception e) when (e is HttpRequestException or TaskCanceledException or JsonException)
            {
                if (ct.IsCancellationRequested) throw;
                // thử máy chủ sau
            }
        }
        return null;
    }

    /// {"Answer":[{"type":5,…CNAME},{"type":1,"TTL":75,"data":"104.21.35.220"}]} → bản ghi A đầu tiên.
    public static IPAddress? Parse(string json, out int ttl)
    {
        ttl = 60;
        using var doc = JsonDocument.Parse(json);
        if (!doc.RootElement.TryGetProperty("Answer", out var answers) || answers.ValueKind != JsonValueKind.Array) return null;
        foreach (var a in answers.EnumerateArray())
        {
            if (!a.TryGetProperty("type", out var t) || t.ValueKind != JsonValueKind.Number || t.GetInt32() != 1) continue;
            if (!a.TryGetProperty("data", out var d) || !IPAddress.TryParse(d.GetString(), out var ip)) continue;
            if (ip.AddressFamily != AddressFamily.InterNetwork || Blocked(ip)) continue;
            if (a.TryGetProperty("TTL", out var l) && l.ValueKind == JsonValueKind.Number) ttl = l.GetInt32();
            return ip;
        }
        return null;
    }
}
