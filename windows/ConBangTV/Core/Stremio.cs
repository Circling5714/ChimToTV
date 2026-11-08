using System.Globalization;
using System.Text;

namespace ConBangTV.Core;

using JObj = Dictionary<string, object?>;

/// CLB Phim Xưa qua add-on Stremio «K20 Phim Tổng Hợp» (sc.k-20.xyz, từ 1.4) — bản Android: Stremio.kt, iPad: Stremio.swift
/// (giữ cùng tên hàm). Giao thức Stremio: /catalog/movie/clbpx-movie[/genre=…&amp;search=…&amp;skip=N].json (30 phim mỗi lần),
/// /meta/movie/clbpx:&lt;slug&gt;.json (videos = các tập; phim lẻ không có), /stream/movie/&lt;mã tập&gt;.json (link MP4 qua máy chủ
/// trung gian của k-20: mở chậm, tua xa có khi không được). Chỉ trả dữ liệu cho User-Agent kiểu trình duyệt.
///
/// Địa chỉ trong app: danh sách = địa chỉ catalog; phim = https://sc.k-20.xyz/clbpx/&lt;slug&gt;; tập = …/clbpx/&lt;slug&gt;/&lt;mã tập&gt;
/// (phim lẻ: …/clbpx/&lt;slug&gt;/full).
public static class Stremio
{
    public const string Host = "sc.k-20.xyz";

    /// Tên hiện trong app (tên miền của add-on không nói lên gì).
    public const string Name = "CLB Phim Xưa";

    private const string Type = "movie";
    private const string CatalogId = "clbpx-movie";
    private const string IdPrefix = "clbpx";
    private const string Full = "full";

    /// Thể loại của catalog (manifest: extra genre). Mục đầu = mới cập nhật.
    private static readonly string[] GenreList =
    {
        "Mới Cập Nhật", "Kiếm Hiệp", "Tiên Hiệp", "Tâm Lý", "Ma Kinh Dị", "Điện Ảnh Châu Á", "Điện Ảnh Âu Mỹ", "Hàn Quốc", "Anime",
        "TV Series", "Thập Niên 60", "Thập Niên 70", "Thập Niên 80", "Thập Niên 90", "Thập Niên 2000",
    };

    public static bool IsSite(string url) => Site.HostOf(url) == Host;

    public static async Task<Home> Home(string baseUrl)
    {
        var o = ApiSite.Origin(baseUrl);
        var blocks = new[] { "Mới cập nhật", "Kiếm Hiệp", "TV Series", "Thập Niên 90", "Thập Niên 80" }
            .Select(t => (t, Catalog(o, genre: t == "Mới cập nhật" ? null : t))).ToArray();
        var sections = await ApiSite.Sections(blocks, async u => ParseList(await GetData(u), u).Films);
        return new Home { Sections = sections, Categories = MenuOf(baseUrl) };
    }

    public static List<Category> MenuOf(string baseUrl)
    {
        var o = ApiSite.Origin(baseUrl);
        return GenreList.Skip(1).Select(g => new Category(g, Catalog(o, genre: g))).ToList();
    }

    public static string NewestUrl(string baseUrl) => Catalog(ApiSite.Origin(baseUrl));

    public static string SearchUrl(string baseUrl, string query) => Catalog(ApiSite.Origin(baseUrl), search: query.Trim());

    public static (List<Category> Genres, List<Category> Countries) Filters(string baseUrl)
    {
        var o = ApiSite.Origin(baseUrl);
        return (GenreList.Select(g => new Category(g, Catalog(o, genre: g))).ToList(), new List<Category>());
    }

    /// Địa chỉ catalog; tham số kiểu Stremio: /catalog/movie/clbpx-movie/genre=…&amp;skip=30.json.
    public static string Catalog(string origin, string? genre = null, string? search = null, int skip = 0)
    {
        var extra = new List<string>();
        if (genre != null) extra.Add("genre=" + Enc("Thể loại: " + genre));
        if (search != null) extra.Add("search=" + Enc(search));
        if (skip > 0) extra.Add($"skip={skip}");
        return $"{origin}/catalog/{Type}/{CatalogId}" + (extra.Count == 0 ? "" : "/" + string.Join("&", extra)) + ".json";
    }

    public static async Task<FilmPage> List(string url) => ParseList(await GetData(url), url);

    private static readonly Rx TapCount = new(@"(\d+)\s*Tập\s*$", System.Text.RegularExpressions.RegexOptions.IgnoreCase);

    /// {metas:[{id:"clbpx:&lt;slug&gt;", name, poster}]}; trang sau = skip + số phim (hết khi rỗng; tìm kiếm chỉ một trang).
    public static FilmPage ParseList(byte[] data, string url)
    {
        var o = ApiSite.Origin(url);
        var root = JsonScan.ParseObject(data) ?? new JObj();
        var films = J.Objects(J.Arr(root, "metas")).Select(m =>
        {
            var id = J.Str(m, "id");
            var name = J.Str(m, "name").NonEmpty();
            if (!id.StartsWith(IdPrefix + ":") || name == null) return null;
            var slug = id[(IdPrefix.Length + 1)..];
            if (slug.Length == 0) return null;
            var poster = J.Str(m, "poster").StartsWith("http") ? J.Str(m, "poster") : null;
            var count = TapCount.Group(1, name);
            return new Film($"{o}/{IdPrefix}/{slug}", name, poster, count != null ? $"{count} tập" : null);
        }).OfType<Film>().DistinctBy(f => f.Url).ToList();
        var extra = ExtraOf(url);
        string? next = null;
        if (films.Count > 0 && !extra.ContainsKey("search"))
        {
            var skip = (int.TryParse(extra.GetValueOrDefault("skip"), out var s) ? s : 0) + films.Count;
            var g = extra.GetValueOrDefault("genre");
            if (g != null && g.StartsWith("Thể loại: ")) g = g["Thể loại: ".Length..];
            next = Catalog(o, g, null, skip);
        }
        return new FilmPage { Films = films, NextUrl = next };
    }

    /// Tham số trong đoạn cuối địa chỉ catalog: "genre=Th%E1…&amp;skip=30.json" → {genre: "Thể loại: …", skip: "30"}.
    private static Dictionary<string, string> ExtraOf(string url)
    {
        var last = Uri.TryCreate(url, UriKind.Absolute, out var u) ? u.AbsolutePath.AfterLast('/') : "";
        if (last.EndsWith(".json")) last = last[..^5];
        var output = new Dictionary<string, string>();
        if (!last.Contains('=')) return output;
        foreach (var p in last.Split('&'))
        {
            var kv = p.Split('=', 2);
            if (kv.Length == 2) output[kv[0]] = Uri.UnescapeDataString(kv[1]);
        }
        return output;
    }

    public static async Task<FilmDetail> Detail(string url)
    {
        var film = FilmUrl(url);
        return ParseDetail(await GetData(MetaUrl(film)), film);
    }

    private static readonly Rx LeadingZero = new(@"^(Tập\s*)0+(\d)", System.Text.RegularExpressions.RegexOptions.IgnoreCase);

    /// {meta:{name, poster, description, releaseInfo, genres, videos:[{id, title, season, episode}]}}.
    public static FilmDetail ParseDetail(byte[] data, string url)
    {
        var m = J.Obj(JsonScan.ParseObject(data), "meta") ?? throw new AppError("Không đọc được thông tin phim");
        var prefix = J.Str(m, "id") + ":";
        var videos = J.Objects(J.Arr(m, "videos")).OrderBy(v => J.Int(v, "season", 1)).ThenBy(v => J.Int(v, "episode")).ToList();
        var eps = videos.Count == 0
            ? new List<Episode> { new("Xem phim", $"{url}/{Full}") }
            : videos.Select(v =>
            {
                var id = J.Str(v, "id");
                if (!id.StartsWith(prefix) || id.Length == prefix.Length) return null;
                var title = LeadingZero.Replace(J.Str(v, "title").NonEmpty() ?? $"Tập {J.Int(v, "episode")}", "${1}${2}");
                return new Episode(title, $"{url}/{Enc(id[prefix.Length..])}");
            }).OfType<Episode>().DistinctBy(e => e.Url).ToList();
        var genres = (J.Arr(m, "genres") ?? new List<object?>()).OfType<string>();
        var info = string.Join("  •  ", new[] { J.Str(m, "releaseInfo"), string.Join(", ", genres) }.Where(s => s.Length > 0)).NonEmpty();
        return new FilmDetail
        {
            Url = url, Title = J.Str(m, "name").NonEmpty() ?? url,
            Poster = J.Str(m, "poster").StartsWith("http") ? J.Str(m, "poster") : null,
            Description = J.Str(m, "description").NonEmpty(), Info = info, Groups = { new EpisodeGroup("Tập phim", eps) },
        };
    }

    /// Mỗi link MP4 là một server: "720p", "1080p · 3,1 GB"… (cùng chất lượng có nhiều bản thì ghi dung lượng).
    public static async Task<EpisodeSource> Source(string epUrl)
    {
        var film = FilmUrl(epUrl);
        var code = Uri.UnescapeDataString(new Uri(epUrl).AbsolutePath.TrimEnd('/').AfterLast('/'));
        var slug = film.AfterLast('/');
        var id = code == Full ? $"{IdPrefix}:{slug}" : $"{IdPrefix}:{slug}:{code}";
        return ParseSource(await GetData($"{ApiSite.Origin(film)}/stream/{Type}/{Enc(id)}.json"), epUrl);
    }

    private static readonly Rx SizeRx = new(@"[?&]size=(\d+)");

    public static EpisodeSource ParseSource(byte[] data, string epUrl)
    {
        var streams = J.Objects(J.Arr(JsonScan.ParseObject(data), "streams")).Where(s => J.Str(s, "url").StartsWith("http")).ToList();
        if (streams.Count == 0) throw new AppError("Tập này chưa có video");
        var quality = streams.Select(s => J.Str(s, "title").Split('\n')[0].Trim().NonEmpty() ?? "MP4").ToList();
        var servers = new List<Server>();
        var links = new Dictionary<string, Video>();
        for (var i = 0; i < streams.Count; i++)
        {
            var url = J.Str(streams[i], "url");
            var size = double.TryParse(SizeRx.Group(1, url), NumberStyles.Float, CultureInfo.InvariantCulture, out var b) ? b : (double?)null;
            var dup = quality.Count(q => q == quality[i]) > 1;
            var label = quality[i] + (dup && size != null ? " · " + (size.Value / 1e9).ToString("0.0", CultureInfo.InvariantCulture).Replace('.', ',') + " GB" : "");
            var type = $"mp4-{i}";
            servers.Add(new Server(label, type));
            links[type] = new Video(url, null, true);
        }
        return new EpisodeSource { EpUrl = epUrl, Servers = servers, Links = links, Next = true };
    }

    /// ".../clbpx/nch1993/LM4bPc-3a" → ".../clbpx/nch1993".
    public static string FilmUrl(string url)
    {
        var parts = Uri.TryCreate(url, UriKind.Absolute, out var u) ? u.AbsolutePath.Split('/', StringSplitOptions.RemoveEmptyEntries) : Array.Empty<string>();
        var i = Array.IndexOf(parts, IdPrefix);
        if (i < 0 || i + 1 >= parts.Length) throw new AppError($"Địa chỉ phim lạ: {url}");
        return $"{ApiSite.Origin(url)}/{IdPrefix}/{parts[i + 1]}";
    }

    private static string MetaUrl(string film) => $"{ApiSite.Origin(film)}/meta/{Type}/" + Enc($"{IdPrefix}:" + film.AfterLast('/')) + ".json";

    private static async Task<byte[]> GetData(string url)
    {
        try
        {
            return (await Http.Get(url, new() { ["Accept"] = "application/json" })).Data;
        }
        catch (Http.HttpError e) when (e.Code == 404)
        {
            throw new AppError("CLB Phim Xưa không có phim / tập này");
        }
    }

    /// Mã hoá chặt (chỉ giữ chữ / số ASCII và "-._*"), dấu cách = %20 — như URLEncoder của bản Android + "%20".
    public static string Enc(string s)
    {
        var sb = new StringBuilder();
        foreach (var b in Encoding.UTF8.GetBytes(s))
        {
            var c = (char)b;
            if (c is >= 'a' and <= 'z' or >= 'A' and <= 'Z' or >= '0' and <= '9' or '-' or '.' or '_' or '*') sb.Append(c);
            else sb.Append('%').Append(b.ToString("X2"));
        }
        return sb.ToString();
    }
}
