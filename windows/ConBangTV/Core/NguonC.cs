using System.Text.RegularExpressions;

namespace ConBangTV.Core;

using JObj = Dictionary<string, object?>;

/// NguonC (phim.nguonc.com, từ 1.4) — bản Android: NguonC.kt, iPad: NguonC.swift (giữ cùng tên hàm). API công khai
/// /api/films/… (định dạng riêng, không phải KKPhim). Dùng hai cách:
/// - Trang riêng: trang chủ / danh sách / tìm kiếm / chi tiết / tập đọc từ API. Địa chỉ phim = /phim/&lt;slug&gt;, tập =
///   /phim/&lt;slug&gt;/&lt;tập&gt;?sv=&lt;thứ tự server&gt;; danh sách = địa chỉ API.
/// - Server dự phòng thứ hai (cạnh KKPhim) cho yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw.
/// Video chỉ có trình phát nhúng (embedN.streamc.xyz) → phát qua iframe, agent tự bỏ quảng cáo của JW.
public static class NguonC
{
    public const string Host = "phim.nguonc.com";
    public const string ApiBase = "https://phim.nguonc.com/api";

    /// Đầu tên loại server dự phòng NguonC, vd "nc-vs", "nc-tm".
    public const string Prefix = "nc-";

    private const long Ttl = 10 * 60_000;
    private const long MissTtl = 30 * 60_000;

    private static readonly (string, string)[] Menu =
    {
        ("Hoạt hình", "films/danh-sach/hoat-hinh"),
        ("Phim bộ", "films/danh-sach/phim-bo"),
        ("Phim lẻ", "films/danh-sach/phim-le"),
        ("TV Shows", "films/danh-sach/tv-shows"),
        ("Phim thuyết minh", "films/ngon-ngu/thuyet-minh"),
        ("Phim lồng tiếng", "films/ngon-ngu/long-tieng"),
        ("Phim Trung Quốc", "films/quoc-gia/trung-quoc"),
        ("Phim Hàn Quốc", "films/quoc-gia/han-quoc"),
        ("Phim Mỹ", "films/quoc-gia/my"),
        ("Phim Nhật Bản", "films/quoc-gia/nhat-ban"),
        ("Phim Thái Lan", "films/quoc-gia/thai-lan"),
    };

    private static readonly (string, string)[] Genres =
    {
        ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Hoạt Hình", "hoat-hinh"), ("Giả Tưởng", "gia-tuong"),
        ("Khoa Học Viễn Tưởng", "khoa-hoc-vien-tuong"), ("Kinh Dị", "kinh-di"), ("Tình Cảm", "tinh-cam"), ("Lãng Mạn", "lang-man"),
        ("Hài", "phim-hai"), ("Tâm Lý", "tam-ly"), ("Phiêu Lưu", "phieu-luu"), ("Hình Sự", "hinh-su"), ("Gây Cấn", "gay-can"),
        ("Bí Ẩn", "bi-an"), ("Chiến Tranh", "chien-tranh"), ("Chính Kịch", "chinh-kich"), ("Gia Đình", "gia-dinh"),
        ("Lịch Sử", "lich-su"), ("Tài Liệu", "tai-lieu"), ("Nhạc", "phim-nhac"), ("Miền Tây", "mien-tay"), ("Phim 18+", "phim-18"),
    };

    private static readonly (string, string)[] Countries =
    {
        ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Mỹ", "my"), ("Anh", "anh"),
        ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
        ("Việt Nam", "viet-nam"), ("Pháp", "phap"), ("Đức", "duc"), ("Tây Ban Nha", "tay-ban-nha"), ("Nga", "nga"),
        ("Canada", "canada"), ("Úc", "uc"), ("Italy", "italy"), ("Philippines", "philippines"), ("Brazil", "brazil"), ("Bỉ", "bi"),
    };

    public static bool IsSite(string url) => Site.HostOf(url) == Host;

    // ---- Trang riêng ----

    public static async Task<Home> Home(string baseUrl)
    {
        var o = ApiSite.Origin(baseUrl);
        var blocks = new[]
        {
            ("Hoạt hình mới cập nhật", ApiUrl(o, "films/danh-sach/hoat-hinh")),
            ("Phim mới cập nhật", NewestUrl(o)),
            ("Phim bộ mới", ApiUrl(o, "films/danh-sach/phim-bo")),
            ("Phim lẻ mới", ApiUrl(o, "films/danh-sach/phim-le")),
            ("Phim thuyết minh", ApiUrl(o, "films/ngon-ngu/thuyet-minh")),
        };
        var sections = await ApiSite.Sections(blocks, async u => ParseList(await GetJson(u), u).Films);
        return new Home { Sections = sections, Categories = MenuOf(baseUrl) };
    }

    public static List<Category> MenuOf(string baseUrl)
    {
        var o = ApiSite.Origin(baseUrl);
        return Menu.Select(m => new Category(m.Item1, ApiUrl(o, m.Item2))).ToList();
    }

    public static string NewestUrl(string baseUrl) => ApiUrl(ApiSite.Origin(baseUrl), "films/phim-moi-cap-nhat");

    public static string SearchUrl(string baseUrl, string query) =>
        $"{ApiSite.Origin(baseUrl)}/api/films/search?keyword=" + query.Trim().FormEncoded() + "&page=1";

    public static (List<Category> Genres, List<Category> Countries) Filters(string baseUrl)
    {
        var o = ApiSite.Origin(baseUrl);
        return (Genres.Select(g => new Category(g.Item1, ApiUrl(o, $"films/the-loai/{g.Item2}"))).ToList(),
            Countries.Select(c => new Category(c.Item1, ApiUrl(o, $"films/quoc-gia/{c.Item2}"))).ToList());
    }

    private static string ApiUrl(string origin, string path) => $"{origin}/api/{path}?page=1";

    public static async Task<FilmPage> List(string url) => ParseList(await GetJson(url), url);

    /// {status, paginate:{current_page, total_page}, items:[{name, slug, thumb_url (dọc), poster_url (ngang), current_episode}]}.
    public static FilmPage ParseList(JObj root, string url)
    {
        var o = ApiSite.Origin(url);
        var films = J.Objects(J.Arr(root, "items")).Select(m =>
        {
            var slug = J.Str(m, "slug").NonEmpty();
            var name = J.Str(m, "name").NonEmpty();
            if (slug == null || name == null) return null;
            var p = J.Str(m, "thumb_url").NonEmpty() ?? J.Str(m, "poster_url");
            return new Film($"{o}/phim/{slug}", name, p.StartsWith("http") ? p : null, NextSite.Badge(J.Str(m, "current_episode")));
        }).OfType<Film>().DistinctBy(f => f.Url).ToList();
        var pg = J.Obj(root, "paginate");
        int page = J.Int(pg, "current_page"), pages = J.Int(pg, "total_page");
        return new FilmPage { Films = films, NextUrl = films.Count > 0 && page >= 1 && page < pages ? ApiSite.WithPage(url, page + 1) : null };
    }

    public static async Task<FilmDetail> Detail(string url)
    {
        var film = FilmUrl(url);
        return ParseDetail(await GetJson(DetailApi(film)), film);
    }

    public static FilmDetail ParseDetail(JObj root, string url)
    {
        var m = J.Obj(root, "movie") ?? throw new AppError("Không đọc được thông tin phim");
        var content = J.Str(m, "description").NonEmpty();
        var desc = content != null ? Html.Parse(content, url).Body?.TextContent.CollapseSpaces().NonEmpty() : null;
        var cats = Categories(m);
        var year = J.Any(m, "year");
        if (year == "0") year = "";
        var info = string.Join("  •  ", new[]
        {
            J.Str(m, "current_episode"), J.Str(m, "time"), J.Str(m, "language"), cats.GetValueOrDefault("Quốc gia", ""), year,
            cats.GetValueOrDefault("Thể loại", ""),
        }.Where(s => s.Length > 0)).NonEmpty();
        var groups = new List<EpisodeGroup>();
        var servers = J.Objects(J.Arr(m, "episodes"));
        for (var i = 0; i < servers.Count; i++)
        {
            var s = servers[i];
            var eps = J.Objects(J.Arr(s, "items")).Select(e =>
            {
                var slug = J.Str(e, "slug").NonEmpty();
                if (slug == null) return null;
                if (!J.Str(e, "embed").StartsWith("http") && !J.Str(e, "m3u8").StartsWith("http")) return null;
                return new Episode(ApiSite.Title(J.Str(e, "name").NonEmpty() ?? slug), $"{url}/{slug}?sv={i}");
            }).OfType<Episode>().DistinctBy(e => e.Url).ToList();
            if (eps.Count > 0) groups.Add(new EpisodeGroup(J.Str(s, "server_name").NonEmpty() ?? $"Server {i + 1}", eps));
        }
        var poster = J.Str(m, "thumb_url").NonEmpty() ?? J.Str(m, "poster_url");
        return new FilmDetail
        {
            Url = url, Title = J.Str(m, "name").NonEmpty() ?? url, OtherTitle = J.Str(m, "original_name").NonEmpty(),
            Poster = poster.StartsWith("http") ? poster : null, Description = desc, Info = info, Groups = groups,
        };
    }

    /// category: {"1": {group:{name:"Định dạng"}, list:[{name}]}, …} → tên nhóm → "a, b".
    private static Dictionary<string, string> Categories(JObj m)
    {
        var output = new Dictionary<string, string>();
        foreach (var (_, v) in J.Obj(m, "category") ?? new JObj())
        {
            if (v is not JObj g) continue;
            var name = J.Str(J.Obj(g, "group"), "name").NonEmpty();
            if (name == null) continue;
            output[name] = string.Join(", ", J.Objects(J.Arr(g, "list")).Select(x => J.Str(x, "name")).Where(s => s.Length > 0));
        }
        return output;
    }

    public static async Task<EpisodeSource> Source(string epUrl)
    {
        var movie = J.Obj(await GetJson(DetailApi(FilmUrl(epUrl))), "movie") ?? throw new AppError("Không đọc được thông tin phim");
        var u = new Uri(epUrl);
        var sv = int.TryParse(Regex.Match(u.Query, @"[?&]sv=(\d+)").Groups[1].Value, out var n) ? n : 0;
        var tap = u.AbsolutePath.TrimEnd('/').AfterLast('/');
        var servers = J.Objects(J.Arr(movie, "episodes"));
        var order = (sv < servers.Count ? new[] { servers[sv] } : Array.Empty<JObj>()).Concat(servers);
        var e = order.Select(s => J.Objects(J.Arr(s, "items")).FirstOrDefault(x => J.Str(x, "slug") == tap)).FirstOrDefault(x => x != null)
            ?? throw new AppError("Không tìm thấy tập này trên trang");
        var links = new Dictionary<string, Video>();
        var list = new List<Server>();
        if (J.Str(e, "m3u8").StartsWith("http"))
        {
            links["m3u8"] = new Video(J.Str(e, "m3u8"));
            list.Add(new Server("HLS", "m3u8"));
        }
        if (J.Str(e, "embed").StartsWith("http"))
        {
            links["embed"] = new Video(J.Str(e, "embed"));
            list.Add(new Server("Trình phát NguonC", "embed"));
        }
        if (links.Count == 0) throw new AppError("Tập này chưa có video");
        return new EpisodeSource { EpUrl = epUrl, Servers = list, Ep = tap, Sv = sv.ToString(), Links = links, Next = true };
    }

    /// ".../phim/muc-than-ky/tap-1?sv=0" → ".../phim/muc-than-ky".
    public static string FilmUrl(string url)
    {
        var parts = Uri.TryCreate(url, UriKind.Absolute, out var u) ? u.AbsolutePath.Split('/', StringSplitOptions.RemoveEmptyEntries) : Array.Empty<string>();
        var i = Array.IndexOf(parts, "phim");
        if (i < 0 || i + 1 >= parts.Length) throw new AppError($"Địa chỉ phim lạ: {url}");
        return $"{ApiSite.Origin(url)}/phim/{parts[i + 1]}";
    }

    private static string DetailApi(string film) => $"{ApiSite.Origin(film)}/api/film/" + film.AfterLast('/');

    // ---- Server dự phòng ----

    private static readonly object Lock = new();
    private static readonly Dictionary<string, (long At, JObj? Movie)> Cache = new();
    private static readonly Dictionary<string, string> Slugs = new();

    /// Trang dùng server dự phòng NguonC: bật trong Cài đặt, không phải trang đã có NguonC (NguonC, phimmoi) / trang API.
    public static bool AppliesTo(string url) => Prefs.NcBackup && !ApiSite.IsKnown(url) && !NextSite.IsKnown(url);

    /// Server NguonC cho một tập (rỗng nếu NguonC không có phim / tập này). Lỗi mạng → ném lỗi.
    public static async Task<List<(Server, Video)>> Servers(FilmDetail d, EpisodeGroup? group, Episode ep)
    {
        var num = Site.EpNumber(ep.Title) ?? Site.EpNumber(ep.Url.TrimEnd('/').AfterLast('/'));
        if (num == null) return new();
        var movie = await MovieFor(d);
        return movie == null ? new() : Pick(movie, num.Value, group == null ? null : KkPhim.Lang(group.Name));
    }

    /// Server có tập số num trong phim NguonC ({name, episodes:[{server_name, items:[{name, slug, embed}]}]}).
    public static List<(Server, Video)> Pick(JObj movie, double num, string? lang)
    {
        var output = new List<(Server, Video)>();
        var used = new HashSet<string>();
        foreach (var s in J.Objects(J.Arr(movie, "episodes")))
        {
            var l = KkPhim.Lang(J.Str(s, "server_name")) ?? "vs";
            if (lang != null && l != lang) continue;
            var e = J.Objects(J.Arr(s, "items"))
                .FirstOrDefault(x => (Site.EpNumber(J.Str(x, "name")) ?? Site.EpNumber(J.Str(x, "slug"))) == num);
            if (e == null) continue;
            var embed = J.Str(e, "embed");
            if (!embed.StartsWith("http")) continue;
            var type = Prefix + l;
            var n = 1;
            while (!used.Add(type)) type = Prefix + l + "-" + ++n;
            var name = l == "tm" ? "Thuyết Minh" : l == "lt" ? "Lồng Tiếng" : "Vietsub";
            output.Add((new Server($"NguonC {name}" + (n > 1 ? $" {n}" : ""), type), new Video(embed)));
        }
        return output;
    }

    private static async Task<JObj?> MovieFor(FilmDetail d)
    {
        var now = Util.NowMs();
        lock (Lock)
        {
            if (Cache.TryGetValue(d.Url, out var hit) && now - hit.At < (hit.Movie == null ? MissTtl : Ttl)) return hit.Movie;
        }
        var found = await Find(d);
        lock (Lock)
        {
            Cache[d.Url] = (now, found);
            if (Cache.Count > 40) Cache.Remove(Cache.MinBy(x => x.Value.At).Key);
        }
        return found;
    }

    /// Phim NguonC khớp tên với phim của trang (slug của trang trước, rồi tìm theo tên); null = NguonC không có.
    private static async Task<JObj?> Find(FilmDetail d)
    {
        var mine = new[] { d.Title, d.OtherTitle }.OfType<string>().Select(KkPhim.Norm).Where(s => s.Length > 0).ToHashSet();
        if (mine.Count == 0) return null;
        string? known;
        lock (Lock) Slugs.TryGetValue(d.Url, out known);
        if (known != null && await Movie(known) is { } k) return k;
        if (KkPhim.SlugOf(d.Url) is { } own && await Movie(own) is { } m && KkPhim.Matches(m, mine))
        {
            Remember(d.Url, own);
            return m;
        }
        var q = $"{ApiBase}/films/search?keyword=" + (d.Title.Length > 80 ? d.Title[..80] : d.Title).FormEncoded();
        var hit = J.Objects(J.Arr(await GetJson(q), "items")).FirstOrDefault(i => KkPhim.Matches(i, mine));
        var slug = hit == null ? null : J.Str(hit, "slug").NonEmpty();
        if (slug == null) return null;
        var r = await Movie(slug);
        if (r != null) Remember(d.Url, slug);
        return r;
    }

    private static void Remember(string film, string slug)
    {
        lock (Lock) Slugs[film] = slug;
    }

    /// Phim trên NguonC (movie); null nếu không có slug này (404).
    private static async Task<JObj?> Movie(string slug)
    {
        try
        {
            return J.Obj(await GetJson($"{ApiBase}/film/" + slug.FormEncoded()), "movie");
        }
        catch (Http.HttpError e) when (e.Code == 404)
        {
            return null;
        }
    }

    private static async Task<JObj> GetJson(string url)
    {
        var r = await Http.Get(url, new() { ["Accept"] = "application/json" });
        return JsonScan.ParseObject(r.Data) ?? throw new AppError("NguonC trả dữ liệu lạ");
    }
}
