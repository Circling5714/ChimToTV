namespace ConBangTV.Core;

/// Chặn tên miền quảng cáo / theo dõi (bản Android: AdBlock.kt — cùng danh sách). Không chặn thứ trình phát cần:
/// JW Player tải từ ssl.p.jwpcdn.com và hỏi giấy phép ở entitlements.jwplayer.com, video nằm trên CDN.
/// Trên Windows: bộ lọc yêu cầu của WebView2 (chỉ các tên miền này đi qua app, mọi thứ khác trình duyệt tự tải).
public static class AdBlock
{
    public static readonly string[] Hosts =
    {
        "doubleclick.net", "googlesyndication.com", "googletagmanager.com", "google-analytics.com",
        "adservice.google.com", "popads.net", "popcash.net", "propellerads.com", "adsterra.com",
        "exoclick.com", "juicyads.com", "hilltopads.net", "onclickads.net", "clickadu.com",
        "a-ads.com", "histats.com", "adnxs.com", "taboola.com", "outbrain.com",
        // Đo trên 5 trang (11/2026): thống kê Cloudflare, JW Player, Ahrefs; quảng cáo Adcash, cửa sổ quảng cáo hoathinh3d
        "cloudflareinsights.com", "jwpltx.com", "analytics.ahrefs.com", "acscdn.com", "excavatenearbywand.com",
        // Gặp khi thử bản iPad (11/2026): iframe quảng cáo trong trang tập hoathinh3d.pw
        "atroblxac.in",
        // Thống kê / theo dõi phổ biến khác
        "connect.facebook.net", "clarity.ms", "mc.yandex.ru", "statcounter.com", "hotjar.com",
    };

    public static bool IsAd(string? host)
    {
        if (host == null) return false;
        var h = host.ToLowerInvariant();
        return Hosts.Any(x => h == x || h.EndsWith("." + x));
    }

    /// Mẫu địa chỉ cho AddWebResourceRequestedFilter (tên miền và mọi tên miền con).
    public static IEnumerable<string> Filters() =>
        Hosts.SelectMany(h => new[] { $"*://{h}/*", $"*://*.{h}/*" });
}
