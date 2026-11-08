using System.Globalization;
using System.Text;

namespace ConBangTV.Core;

using JObj = Dictionary<string, object?>;

/// Server dự phòng KKPhim (API công khai phimapi.com) cho các trang tự có trình phát — bản Android: KkPhim.kt, iPad:
/// KkPhim.swift (giữ cùng tên hàm để sửa song song). yanhh3d, hoathinh3d.ee, hhkungfu nhúng chung streamfree.vip — hỏng là
/// hỏng cả ba. Mở một tập thì tìm đúng phim đó trên KKPhim (slug của trang, không khớp thì tìm theo tên; tên phim hoặc tên
/// gốc phải trùng), lấy tập cùng số rồi thêm server "KKPhim Vietsub / Thuyết Minh" (luồng HLS — app tự phát, bỏ quảng cáo)
/// vào bảng tuỳ chọn. Trang chia bản (Việt Sub / Thuyết Minh) thì chỉ thêm đúng ngôn ngữ của bản đang xem. Trang lỗi hẳn mà
/// KKPhim có tập này thì vẫn xem được bằng KKPhim. Trang đã có sẵn KKPhim (mephim / kkphim — ApiSite, phimmoi — NextSite)
/// không thêm.
public static class KkPhim
{
    public const string Api = "https://phimapi.com";

    /// Đầu tên loại server KKPhim, vd "kk-vs", "kk-tm" (server quen lưu theo tên này).
    public const string Prefix = "kk-";

    private const long Ttl = 10 * 60_000; // chi tiết phim (danh sách tập) giữ 10 phút
    private const long MissTtl = 30 * 60_000; // phim không có trên KKPhim: 30 phút sau mới hỏi lại

    private static readonly object Lock = new();
    private static readonly Dictionary<string, (long At, JObj? Detail)> Cache = new();
    /// Phim của trang → slug trên KKPhim đã tìm thấy (khỏi tìm lại theo tên).
    private static readonly Dictionary<string, string> Slugs = new();

    /// Trang dùng nguồn dự phòng: bật trong Cài đặt và không phải trang đã có sẵn KKPhim.
    public static bool AppliesTo(string url)
    {
        if (!Prefs.KkBackup || ApiSite.IsKnown(url) || NextSite.IsKnown(url)) return false;
        var h = Site.HostOf(url);
        return h != null && !h.EndsWith("phimapi.com") && !h.EndsWith("kkphim.com");
    }

    /// Trang tập + server KKPhim, tải song song. KKPhim chậm / lỗi thì thôi; trang tập lỗi mà KKPhim có tập này → chỉ KKPhim.
    public static Task<EpisodeSource> Source(FilmDetail d, EpisodeGroup? group, Episode ep) =>
        AppliesTo(d.Url) ? Combine(ep.Url, () => Site.Source(ep.Url), () => Servers(d, group, ep)) : Site.Source(ep.Url);

    /// Chạy song song trang tập (site) và tìm KKPhim (kk). Trang xong thì chờ KKPhim thêm tới waitMs tính từ lúc bắt đầu;
    /// trang lỗi thì chờ tới failWaitMs (lúc đó KKPhim là đường duy nhất). KKPhim lỗi = không có server dự phòng.
    public static async Task<EpisodeSource> Combine(string epUrl, Func<Task<EpisodeSource>> site, Func<Task<List<(Server, Video)>>> kk,
        long waitMs = 6_000, long failWaitMs = 15_000)
    {
        var start = Util.NowMs();
        var task = Task.Run(async () =>
        {
            try { return await kk(); } catch (Exception) { return new List<(Server, Video)>(); }
        });
        EpisodeSource? src = null;
        Exception? siteError = null;
        try { src = await site(); } catch (Exception e) { siteError = e; }
        var budget = Math.Max(1, (src != null ? waitMs : failWaitMs) - (Util.NowMs() - start));
        var extra = await Task.WhenAny(task, Task.Delay(TimeSpan.FromMilliseconds(budget))) == task ? task.Result : new();
        if (src != null) return Attach(src, extra);
        if (extra.Count == 0) throw siteError ?? new AppError("Không mở được tập này");
        return new EpisodeSource
        {
            EpUrl = epUrl, Servers = extra.Select(x => x.Item1).ToList(), Links = extra.ToDictionary(x => x.Item1.Type, x => x.Item2),
        };
    }

    /// Thêm server KKPhim sau các server của trang. Trang mở thẳng trang tập (PageMode, vd hoathinh3d.pw) thì trình phát
    /// của trang thành một server ("Trình phát của trang") để chọn qua lại được.
    public static EpisodeSource Attach(EpisodeSource src, List<(Server, Video)> extra)
    {
        if (extra.Count == 0 || src.Next) return src;
        var own = src.PageMode ? new List<Server> { new("Trình phát của trang", EpisodeSource.PageServer) } : src.Servers;
        var links = new Dictionary<string, Video>(src.Links);
        foreach (var (s, v) in extra) links[s.Type] = v;
        return new EpisodeSource
        {
            EpUrl = src.EpUrl, Servers = own.Concat(extra.Select(x => x.Item1)).ToList(), PostId = src.PostId, Ep = src.Ep, Sv = src.Sv,
            PlayerUrl = src.PlayerUrl, DirectEmbed = src.DirectEmbed, PageMode = src.PageMode, Links = links, Next = src.Next,
        };
    }

    /// Server KKPhim cho một tập (rỗng nếu KKPhim không có phim / tập này). Lỗi mạng → ném lỗi.
    public static async Task<List<(Server, Video)>> Servers(FilmDetail d, EpisodeGroup? group, Episode ep)
    {
        var num = Site.EpNumber(ep.Title) ?? Site.EpNumber(ep.Url.TrimEnd('/').AfterLast('/'));
        if (num == null) return new();
        var detail = await Detail(d);
        return detail == null ? new() : Pick(detail, num.Value, group == null ? null : Lang(group.Name));
    }

    /// Server có tập số num trong chi tiết phim KKPhim ({movie, episodes:[{server_name, is_ai, server_data:[{name, slug,
    /// link_m3u8}]}]}). lang: ngôn ngữ của bản đang xem ("vs" / "tm" / "lt"; null = trang không chia bản → mọi ngôn ngữ).
    public static List<(Server, Video)> Pick(JObj root, double num, string? lang)
    {
        var output = new List<(Server, Video)>();
        var used = new HashSet<string>();
        foreach (var s in J.Objects(J.Arr(root, "episodes")))
        {
            var l = Lang(J.Str(s, "server_name")) ?? "vs";
            if (lang != null && l != lang) continue;
            var e = J.Objects(J.Arr(s, "server_data"))
                .FirstOrDefault(x => (Site.EpNumber(J.Str(x, "name")) ?? Site.EpNumber(J.Str(x, "slug"))) == num);
            if (e == null) continue;
            var m3u8 = J.Str(e, "link_m3u8");
            if (!m3u8.StartsWith("http")) continue;
            var ai = J.Bool(s, "is_ai");
            var b = l + (ai ? "-ai" : "");
            var type = Prefix + b;
            var n = 1;
            while (!used.Add(type)) type = Prefix + b + "-" + ++n;
            var name = l == "tm" ? "Thuyết Minh" : l == "lt" ? "Lồng Tiếng" : "Vietsub";
            output.Add((new Server($"KKPhim {name}" + (ai ? " (AI)" : "") + (n > 1 ? $" {n}" : ""), type), new Video(m3u8)));
        }
        return output;
    }

    private static readonly Rx TmWord = new(@"\btm\b");

    /// Ngôn ngữ ghi trong tên server / tên bản ("vs" / "tm" / "lt"); null = không rõ — như PlayStats.lang của bản TV.
    public static string? Lang(string label)
    {
        var s = Plain(label);
        if (s.Contains("thuyet minh") || TmWord.Matches(s)) return "tm";
        if (s.Contains("long tieng")) return "lt";
        if (s.Contains("sub")) return "vs";
        return null;
    }

    /// Bỏ dấu, chữ thường: "Thuyết minh 1080 V1" → "thuyet minh 1080 v1".
    private static string Plain(string s)
    {
        var d = s.Replace('đ', 'd').Replace('Đ', 'D').Normalize(NormalizationForm.FormD);
        var sb = new StringBuilder(d.Length);
        foreach (var c in d)
            if (CharUnicodeInfo.GetUnicodeCategory(c) != UnicodeCategory.NonSpacingMark) sb.Append(c);
        return sb.ToString().Normalize(NormalizationForm.FormC).ToLowerInvariant();
    }

    // ---- Tìm phim trên KKPhim ----

    private static async Task<JObj?> Detail(FilmDetail d)
    {
        var now = Util.NowMs();
        lock (Lock)
        {
            if (Cache.TryGetValue(d.Url, out var hit) && now - hit.At < (hit.Detail == null ? MissTtl : Ttl)) return hit.Detail;
        }
        var found = await Find(d);
        lock (Lock)
        {
            Cache[d.Url] = (now, found);
            if (Cache.Count > 40) Cache.Remove(Cache.MinBy(x => x.Value.At).Key);
        }
        return found;
    }

    /// Chi tiết phim KKPhim khớp với phim của trang; null = KKPhim không có.
    private static async Task<JObj?> Find(FilmDetail d)
    {
        var mine = new[] { d.Title, d.OtherTitle }.OfType<string>().Select(Norm).Where(s => s.Length > 0).ToHashSet();
        if (mine.Count == 0) return null;
        string? known;
        lock (Lock) Slugs.TryGetValue(d.Url, out known);
        if (known != null && await Movie(known) is { } k) return k;
        // Slug của trang thường trùng KKPhim (tien-nghich, dau-pha-thuong-khung-phan-5…); chỉ nhận phim hoạt hình ở bước này
        if (SlugOf(d.Url) is { } own && await Movie(own) is { } m && J.Obj(m, "movie") is { } mv &&
            J.Str(mv, "type") == "hoathinh" && Matches(mv, mine))
        {
            Remember(d.Url, own);
            return m;
        }
        var q = $"{Api}/v1/api/tim-kiem?keyword=" + (d.Title.Length > 80 ? d.Title[..80] : d.Title).FormEncoded() + "&limit=20";
        var items = J.Objects(J.Arr(J.Obj(await GetJson(q), "data"), "items"));
        var hit = Choose(items, mine);
        var slug = hit == null ? null : J.Str(hit, "slug").NonEmpty();
        if (slug == null) return null;
        var r = await Movie(slug);
        if (r != null) Remember(d.Url, slug);
        return r;
    }

    /// Kết quả tìm khớp tên (phim hoạt hình trước — tìm kiếm của KKPhim có thể ra phim người đóng trùng tên).
    public static JObj? Choose(List<JObj> items, HashSet<string> mine)
    {
        var hits = items.Where(i => Matches(i, mine)).ToList();
        return hits.FirstOrDefault(i => J.Str(i, "type") == "hoathinh") ?? hits.FirstOrDefault();
    }

    private static void Remember(string film, string slug)
    {
        lock (Lock) Slugs[film] = slug;
    }

    /// Chi tiết phim trên KKPhim; null nếu không có slug này (KKPhim trả 404).
    private static async Task<JObj?> Movie(string slug)
    {
        try
        {
            var o = await GetJson($"{Api}/phim/" + slug.FormEncoded());
            return J.Bool(o, "status") && J.Obj(o, "movie") != null ? o : null;
        }
        catch (Http.HttpError e) when (e.Code == 404)
        {
            return null;
        }
    }

    private static async Task<JObj> GetJson(string url)
    {
        var r = await Http.Get(url, new() { ["Accept"] = "application/json" });
        return JsonScan.ParseObject(r.Data) ?? throw new AppError("KKPhim trả dữ liệu lạ");
    }

    private static readonly Rx SlugRx = new(@"^[a-z0-9-]+$");

    /// Slug của phim trong địa chỉ trang: ".../tu-tien/tien-nghich" → "tien-nghich".
    public static string? SlugOf(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var u)) return null;
        var s = u.AbsolutePath.TrimEnd('/').AfterLast('/');
        if (s.EndsWith(".html")) s = s[..^5];
        return SlugRx.Matches(s) ? s : null;
    }

    public static bool Matches(JObj m, HashSet<string> mine) => Names(m).Overlaps(mine);

    private static readonly Rx Paren = new(@"^(.+?)\s*\((.+)\)\s*$");
    private static readonly Rx Digits = new(@"^\d+$");

    /// Các tên của một phim KKPhim (đã chuẩn hoá): tên, tên gốc; tên có phần trong ngoặc thì thêm phần ngoài / trong ngoặc
    /// ("Thần Y Cổ Đại Ở Đô Thị (Đô Thị Cổ Tiên Y)") — trừ khi trong ngoặc chỉ là số phần ("Đấu Phá Thương Khung (Phần 5)").
    public static HashSet<string> Names(JObj m)
    {
        var output = new HashSet<string>();
        foreach (var raw in new[] { J.Str(m, "name"), J.Str(m, "origin_name") })
        {
            output.Add(Norm(raw));
            if (Paren.Groups(raw) is { } g)
            {
                var inner = Norm(g[2]);
                if (!Digits.Matches(inner))
                {
                    output.Add(Norm(g[1]));
                    output.Add(inner);
                }
            }
        }
        output.Remove("");
        return output;
    }

    private static readonly Rx NonAlnum = new(@"[^a-z0-9]+");
    private static readonly Rx Season = new(@"\b(phan|season|mua|part|ss)\s*(\d+)");
    private static readonly Rx Spaces = new(@"\s+");

    /// Tên để so: bỏ dấu, chữ thường, chỉ chữ + số; "Phần 5" / "Season 5" / "SS5" → "5".
    public static string Norm(string s)
    {
        var t = NonAlnum.Replace(Plain(s), " ");
        t = Season.Replace(t, "$2");
        return Spaces.Replace(t, " ").Trim();
    }
}
