using System.Globalization;

namespace ConBangTV.Core;

// Đọc trang phim, nguồn dự phòng, phim liên quan, IMDb, chặn quảng cáo — chạy trong lõi Rust (linux/core: site.rs, api_site.rs,
// next_site.rs, kk_phim.rs, nguon_c.rs, stremio.rs, related.rs, imdb_top.rs, feed.rs, ad_block.rs), gọi qua Native. Giữ tên lớp /
// hàm của lõi C# cũ để giao diện khỏi sửa.

/// Điểm IMDb của một phim (lõi lấy từ dữ liệu KKPhim; ít hơn MinVotes lượt chấm thì coi như chưa có điểm).
public sealed record Imdb(double Score, int Votes)
{
    public const int MinVotes = 100;

    /// Nhãn trên thẻ phim, vd "IMDb 8.5".
    public string Label => "IMDb " + Score.ToString("0.0", CultureInfo.InvariantCulture);

    /// Dòng thông tin ở trang phim, vd "IMDb 8.5 (378 nghìn lượt chấm)".
    public string Detail => $"{Label} ({VotesText(Votes)} lượt chấm)";

    public static string VotesText(int v)
    {
        if (v >= 1_000_000)
        {
            var s = (v / 1_000_000.0).ToString("0.0", CultureInfo.InvariantCulture).Replace('.', ',');
            return (s.EndsWith(",0") ? s[..^2] : s) + " triệu";
        }
        return v >= 1000 ? $"{v / 1000} nghìn" : v.ToString(CultureInfo.InvariantCulture);
    }
}

/// Thể loại / quốc gia / loại phim theo slug của KKPhim.
public sealed record Tags(List<string> Genres, List<string> Countries, string? Type);

/// Một phim ở một nguồn của trang chủ tổng hợp (Host: tên miền nguồn).
public sealed record FeedItem(string Url, string Title, string? Poster, string? Badge, HashSet<string> Names, int? Year, long Updated,
    double? Ep, Tags? Tags, Imdb? Imdb = null)
{
    public string Host { get; init; } = "";
}

public static class Site
{
    public static FilmDetail? CachedDetail(string url) => Native.Sync<FilmDetail?>("site.cachedDetail", new { url });

    public static Task<Home> HomeOf(string? baseUrl = null) => Native.Call<Home>("site.homeOf", new { baseUrl });

    /// Đo tốc độ trang (Cài đặt → Trang phim).
    public static Task<SiteCheck> Check(string baseUrl) => Native.Call<SiteCheck>("site.check", new { baseUrl });

    public static Task<FilmPage> List(string url) => Native.Call<FilmPage>("site.list", new { url });

    public static Task<FilmPage> Search(string query, string? baseUrl = null) => Native.Call<FilmPage>("site.search", new { query, baseUrl });

    /// Tìm trên mọi trang trong danh sách.
    public static Task<FilmPage> SearchAll(string query) => Native.Call<FilmPage>("site.searchAll", new { query });

    public static Task<FilmDetail> Detail(string url) => Native.Call<FilmDetail>("site.detail", new { url });

    public static Task<Video> VideoOf(EpisodeSource src, Server? server) => Native.Call<Video>("site.videoOf", new { src, server });

    public static string DisplayName(string host) => Native.Sync<string>("site.displayName", new { host });

    public static string Tag(string host, IEnumerable<string> sites) => Native.Sync<string>("site.tag", new { host, sites = sites.ToList() });

    public static List<Category>? Menu(string baseUrl) => Native.Sync<List<Category>?>("site.menu", new { baseUrl });

    /// Tên miền (bỏ «www.»).
    public static string? HostOf(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var u) || string.IsNullOrEmpty(u.Host)) return null;
        var h = u.Host.ToLowerInvariant();
        return h.StartsWith("www.") ? h[4..] : h;
    }

    public static string? OriginOf(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var u) || string.IsNullOrEmpty(u.Host)) return null;
        return $"{u.Scheme}://{u.Host}" + (u.IsDefaultPort ? "" : $":{u.Port}");
    }
}

/// Trang dạng API kiểu KKPhim / VSMOV (kkphim.com, mephim, vsmov.com…).
public static class ApiSite
{
    public const string KkHost = "kkphim.com";
    public const string VsHost = "vsmov.com";

    private sealed record Named(string Name, string Slug);

    private sealed record FilterLists(List<Category> Genres, List<Category> Countries);

    public static bool IsKnown(string url) => Native.Sync<bool>("api.isKnown", new { url });

    public static string CountryName(string slug) => Native.Sync<string>("api.countryName", new { slug });

    public static string GenreName(string slug) => Native.Sync<string>("api.genreName", new { slug });

    public static (List<Category> Genres, List<Category> Countries) Filters(string baseUrl)
    {
        var f = Native.Sync<FilterLists>("api.filters", new { baseUrl });
        return (f.Genres, f.Countries);
    }

    public static IEnumerable<(string Name, string Slug)> GenreList =>
        Native.Sync<List<Named>>("api.genreList").Select(n => (n.Name, n.Slug));

    public static IEnumerable<(string Name, string Slug)> CountryList =>
        Native.Sync<List<Named>>("api.countryList").Select(n => (n.Name, n.Slug));
}

/// Trang Next.js kiểu PhimMoi (phimmois.fun…).
public static class NextSite
{
    private sealed record FilterLists(List<Category> Genres, List<Category> Countries);

    public static bool IsKnown(string baseUrl) => Native.Sync<bool>("next.isKnown", new { url = baseUrl });

    public static async Task<(List<Category> Genres, List<Category> Countries)> Filters(string baseUrl)
    {
        var f = await Native.Call<FilterLists>("next.filters", new { baseUrl });
        return (f.Genres, f.Countries);
    }
}

/// Server dự phòng KKPhim / NguonC cho các trang streamfree.
public static class KkPhim
{
    /// Ngôn ngữ của một bản (Vietsub / Thuyết Minh / Lồng Tiếng) — null = không rõ.
    public static string? Lang(string label) => Native.Sync<string?>("kk.lang", new { label });

    /// Tên phim chuẩn hoá (so trùng phim giữa các nguồn).
    public static string Norm(string s) => Native.Sync<string>("kk.norm", new { s });

    /// Server của tập: của trang + KKPhim / NguonC dự phòng.
    public static Task<EpisodeSource> Source(FilmDetail d, EpisodeGroup? group, Episode ep) =>
        Native.Call<EpisodeSource>("kk.source", new { detail = d, group, ep });
}

/// Phim liên quan ở trang phim: «Cùng bộ», «Cùng thể loại».
public static class Related
{
    public sealed record Row(string Id, string Title, List<Film> Films);

    public static Task<List<Row>> Load(string title, string url) => Native.Call<List<Row>>("related.load", new { title, url });
}

/// «Điểm IMDb cao» (menu) và xếp theo điểm IMDb ở danh sách KKPhim.
public static class ImdbTop
{
    private static string? _cacheFile;

    /// Tệp lưu danh sách đã xếp (lõi đọc / ghi).
    public static string? CacheFile
    {
        get => _cacheFile;
        set
        {
            _cacheFile = value;
            Native.SyncVoid("imdb.setCacheFile", new { path = value });
        }
    }

    public static bool CanRank(string url) => Native.Sync<bool>("imdb.canRank", new { url });

    public static Task<FilmPage> Page(bool kids) => Native.Call<FilmPage>("imdb.page", new { kids });

    public static Task<FilmPage> ForList(string url) => Native.Call<FilmPage>("imdb.forList", new { url });
}

/// Nguồn của trang chủ tổng hợp đang «nghỉ» (lỗi liên tiếp — Cài đặt → Trang chủ).
public static class Feed
{
    public static Dictionary<string, long> RestingHosts() => Native.Sync<Dictionary<string, long>>("feed.restingHosts");

    public static void ClearRest() => Native.SyncVoid("feed.clearRest");
}

/// Tên miền quảng cáo / theo dõi bị chặn trong trình phát (WebView2) — cùng danh sách với cổng nội bộ của lõi.
public static class AdBlock
{
    private static List<string>? _filters;

    public static bool IsAd(string? host) => Native.Sync<bool>("ad.isAd", new { host });

    public static IEnumerable<string> Filters() => _filters ??= Native.Sync<List<string>>("ad.filters");
}
