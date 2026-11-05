using System.Text;
using AngleSharp.Dom;

namespace ConBangTV.Core;

using JObj = Dictionary<string, object?>;

/// Dữ liệu Next.js của một trang (các chuỗi self.__next_f.push nối lại).
public sealed class NextData
{
    public readonly string Text;

    public NextData(string text)
    {
        Text = text;
    }

    public int Length => Text.Length;

    public int? IndexOf(string needle, int from = 0)
    {
        if (from < 0 || from >= Text.Length) return null;
        var i = Text.IndexOf(needle, from, StringComparison.Ordinal);
        return i < 0 ? null : i;
    }

    /// Giá trị JSON tại vị trí; không đọc được → null.
    public object? ValueAt(int i)
    {
        var v = JsonScan.Value(Text, i);
        return ReferenceEquals(v, JsonScan.Fail) ? null : v;
    }
}

/// Trang phim dựng bằng Next.js, giao diện kiểu "PhimMoi" (vd phimmois.fun) — bản Android: NextSite.kt, iPad: NextSite.swift.
/// Không đọc HTML: mọi dữ liệu (phim, đủ danh sách tập, server, link video) nằm sẵn trong các đoạn push của trang.
public static class NextSite
{
    private const string Push = "self.__next_f.push([1,";
    private const string EpisodeKey = "\"episode\":{\"episodes\"";

    /// Khối phim trang chủ: ["$","$L26","Trung Quốc Hot Nhất",{"movies":[…]}]
    private static readonly Rx SectionRx = new(@"\[""\$"",""\$L\w+"",""([^""]+)"",\{""movies"":\[");
    private static readonly Rx MovieRx = new(@"\{""id"":\d+,""name"":""");
    private static readonly Rx PagingRx = new(@"""total"":(\d+),""page"":(\d+),""limit"":(\d+)");

    /// Menu của trang chỉ có 10 thể loại → thêm mục cố định (đường dẫn bộ lọc của chính trang, mới cập nhật lên đầu).
    public static readonly (string Title, string Path)[] Menu =
    {
        ("Hoạt hình Trung Quốc", "/loc-phim?genre=hoat-hinh&country=trung-quoc&sort=updated_at&order=desc"),
        ("Hoạt hình", "/loc-phim?genre=hoat-hinh&sort=updated_at&order=desc"),
        ("Phim bộ mới", "/loc-phim?type=phim-bo&sort=updated_at&order=desc"),
        ("Phim lẻ mới", "/loc-phim?type=phim-le&sort=updated_at&order=desc"),
        ("Phim Hàn Quốc", "/loc-phim?country=han-quoc&sort=updated_at&order=desc"),
        ("Phim Trung Quốc", "/loc-phim?country=trung-quoc&sort=updated_at&order=desc"),
        ("Phim Âu Mỹ", "/loc-phim?country=au-my&sort=updated_at&order=desc"),
        ("Phim Nhật Bản", "/loc-phim?country=nhat-ban&sort=updated_at&order=desc"),
        ("Phim Thái Lan", "/loc-phim?country=thai-lan&sort=updated_at&order=desc"),
    };

    /// Trang đã gặp là Next.js (để tìm kiếm đi thẳng /tim-kiem thay cho ?s= của WordPress).
    private static readonly HashSet<string> Hosts = new();

    public static bool IsKnown(string baseUrl)
    {
        var h = Site.HostOf(baseUrl);
        lock (Hosts) return h != null && Hosts.Contains(h);
    }

    /// Dữ liệu Next.js của trang; null nếu không phải trang Next.js.
    public static NextData? Data(IDocument doc)
    {
        var sb = new StringBuilder();
        foreach (var s in doc.Q("script"))
        {
            var js = s.ScriptText();
            var i = js.IndexOf(Push, StringComparison.Ordinal);
            if (i < 0) continue;
            if (JsonScan.Value(js, i + Push.Length) is string str) sb.Append(str);
        }
        if (sb.Length == 0) return null;
        if (Site.HostOf(doc.Location()) is { } h)
            lock (Hosts) Hosts.Add(h);
        return new NextData(sb.ToString());
    }

    // ---- Trang chủ / danh sách ----

    /// Trang chủ, thêm khối hoạt hình Trung Quốc mới cập nhật lên đầu.
    public static async Task<Home> Home(IDocument doc, NextData data)
    {
        var h = ParseHome(doc, data);
        try
        {
            var hh3d = (await ListOf(OriginOf(doc) + Menu[0].Path)).Films;
            if (hh3d.Count > 0) h.Sections.Insert(0, new FilmSection("Hoạt hình Trung Quốc mới cập nhật", hh3d));
        }
        catch (Exception)
        {
        }
        return h;
    }

    public static Home ParseHome(IDocument doc, NextData data)
    {
        var origin = OriginOf(doc);
        var sections = new List<FilmSection>();
        var matches = SectionRx.All(data.Text).ToList();
        var first = matches.Count > 0 ? matches[0].Index : data.Length;
        var top = Movies(data, origin, 0, first);
        if (top.Count > 0) sections.Add(new FilmSection("Nổi bật", top));
        foreach (var m in matches)
        {
            var arrayAt = m.Index + m.Length - 1;
            if (data.ValueAt(arrayAt) is not List<object?> arr) continue;
            var films = J.Objects(arr).Select(o => FilmOf(o, origin)).OfType<Film>().DistinctBy(f => f.Url).ToList();
            if (films.Count > 0) sections.Add(new FilmSection(m.Groups[1].Value, films));
        }
        return new Home { Sections = sections, Categories = Categories(doc, origin) };
    }

    private static List<Category> Categories(IDocument doc, string origin)
    {
        var outList = new List<Category>();
        var seen = new HashSet<string>();
        foreach (var (title, path) in Menu)
            if (seen.Add(origin + path)) outList.Add(new Category(title, origin + path));
        foreach (var a in doc.Q("a[href^=\"/the-loai/\"]"))
        {
            var url = a.Abs("href").TrimEnd('/');
            var title = a.Txt();
            if (title.Length > 0 && seen.Add(url)) outList.Add(new Category(title, url));
        }
        return outList;
    }

    /// Bộ lọc của trang (/loc-phim): đủ thể loại và quốc gia. Bỏ mục rác (năm, mã hai chữ) và tên trùng.
    public static async Task<(List<Category> Genres, List<Category> Countries)> Filters(string baseUrl)
    {
        var r = await Http.Get(baseUrl + "/loc-phim");
        var doc = Html.Parse(r.Body, r.Url);
        var d = Data(doc) ?? throw new AppError("Trang không còn dạng Next.js");
        return ParseFilters(doc, d);
    }

    public static (List<Category> Genres, List<Category> Countries) ParseFilters(IDocument doc, NextData d)
    {
        var origin = OriginOf(doc);
        List<Category> Read(string key, string param)
        {
            // Phim trong trang cũng có "genres":[…] của riêng nó → lấy danh sách dài nhất (bộ lọc của trang)
            var tag = $"\"{key}\":[{{";
            var best = new List<object?>();
            var i = d.IndexOf(tag);
            while (i is { } at)
            {
                if (d.ValueAt(at + key.Length + 3) is List<object?> a && a.Count > best.Count) best = a;
                i = d.IndexOf(tag, at + tag.Length);
            }
            return J.Objects(best).Where(o => J.Bool(o, "is_active", true))
                .Select(o => (Name: J.Str(o, "name"), Slug: J.Str(o, "slug")))
                .Where(x => x.Slug.Length > 0 && x.Name.Any(char.IsLetter) && !(x.Name.Length <= 2 && x.Name.All(char.IsUpper)))
                .DistinctBy(x => x.Name.ToLowerInvariant())
                .Select(x => new Category(x.Name, $"{origin}/loc-phim?{param}={x.Slug.FormEncoded()}&sort=updated_at&order=desc"))
                .ToList();
        }
        return (Read("genres", "genre"), Read("countries", "country"));
    }

    private static async Task<FilmPage> ListOf(string url)
    {
        var r = await Http.Get(url);
        var doc = Html.Parse(r.Body, r.Url);
        var d = Data(doc) ?? throw new AppError("Trang không còn dạng Next.js");
        return ParseList(doc, d, url);
    }

    /// Danh sách / tìm kiếm: 21 phim một trang, trang sau = tham số page tiếp theo (dữ liệu có total/page/limit).
    public static FilmPage ParseList(IDocument doc, NextData data, string url)
    {
        var films = Movies(data, OriginOf(doc));
        string? next = null;
        var g = PagingRx.Groups(data.Text);
        if (g != null && int.TryParse(g[1], out var total) && int.TryParse(g[2], out var page) && int.TryParse(g[3], out var limit) &&
            films.Count > 0 && page * limit < total)
            next = WithPage(url, page + 1);
        return new FilmPage { Films = films, NextUrl = next };
    }

    public static string SearchUrl(string baseUrl, string query) => $"{baseUrl}/tim-kiem?keyword=" + query.Trim().FormEncoded();

    private static readonly Rx PageParam = new(@"([?&]page=)\d+");

    private static string WithPage(string url, int page)
    {
        if (PageParam.Matches(url)) return PageParam.Replace(url, "${1}" + page);
        return url + (url.Contains('?') ? "&" : "?") + $"page={page}";
    }

    // ---- Chi tiết phim ----

    /// Chi tiết + danh sách tập của các ngôn ngữ khác (tải song song).
    public static async Task<FilmDetail> Detail(IDocument doc, NextData data, string url)
    {
        var d = ParseDetail(doc, data, url);
        var ep = EpisodeData(data);
        if (ep == null) return d;
        var origin = OriginOf(doc);
        var slug = MovieOf(data, url) is { } m ? J.Str(m, "slug") : "";
        if (slug.Length == 0) return d;
        // Trang chỉ trả danh sách tập của một ngôn ngữ khi tập được hỏi có ở ngôn ngữ đó → hỏi tập đầu, rồi tập cuối
        var mainEps = d.Groups.FirstOrDefault()?.Episodes ?? new();
        var found0 = new[] { mainEps.FirstOrDefault(), mainEps.LastOrDefault() }.OfType<Episode>().Select(e => EpSlug(e.Url))
            .Where(s => s.Length > 0).Distinct().ToList();
        var probes = found0.Count == 0 ? new List<string> { "tap-1" } : found0;
        var extra = Languages(ep).Where(l => d.Groups.All(g => g.Name != l.Name)).ToList();
        var tasks = extra.Select(async lang =>
        {
            var server = BestServer(ep, lang.Slug);
            if (server == null) return null;
            foreach (var probe in probes)
            {
                try
                {
                    var r = await Http.Get($"{origin}/xem-phim/{slug}/{probe}/{lang.Slug}?server={server}");
                    var dd = Data(Html.Parse(r.Body, r.Url));
                    var e = dd != null ? EpisodeData(dd) : null;
                    if (e == null || J.Str(e, "current_language") != lang.Slug) continue;
                    var eps = Episodes(e, origin, slug, lang.Slug);
                    if (eps.Count > 0) return new EpisodeGroup(lang.Name, eps);
                }
                catch (Exception)
                {
                }
            }
            return (EpisodeGroup?)null;
        }).ToList();
        foreach (var g in await Task.WhenAll(tasks))
            if (g != null) d.Groups.Add(g);
        return d;
    }

    public static FilmDetail ParseDetail(IDocument doc, NextData data, string url)
    {
        var origin = OriginOf(doc);
        var m = MovieOf(data, url) ?? throw new AppError("Không đọc được thông tin phim");
        var slug = J.Str(m, "slug");
        var rawDesc = J.Str(m, "description");
        var desc = rawDesc.Length > 20 && !rawDesc.StartsWith('$') ? rawDesc
            : doc.Q1("meta[property=\"og:description\"], meta[name=\"description\"]")?.At("content").NonEmpty();
        var year = m.TryGetValue("year", out var y) ? y switch { long l => l.ToString(), double dd => ((long)dd).ToString(), _ => "" } : "";
        var info = string.Join("  •  ", new[]
        {
            J.Str(m, "status_text").NonEmpty() ?? J.Str(m, "display_status"),
            J.Str(J.Obj(m, "country"), "name"),
            year,
            Names(J.Arr(m, "genres")),
        }.Where(s => s.Length > 0)).NonEmpty();

        var groups = new List<EpisodeGroup>();
        if (EpisodeData(data) is { } ep)
        {
            var lang = J.Str(ep, "current_language");
            var eps = Episodes(ep, origin, slug, lang);
            if (eps.Count > 0)
                groups.Add(new EpisodeGroup(Languages(ep).FirstOrDefault(l => l.Slug == lang).Name ?? "Tập phim", eps));
        }
        var posterRef = J.Str(m, "poster_url").NonEmpty() ?? J.Str(m, "thumb_url").NonEmpty();
        return new FilmDetail
        {
            Url = url, Title = J.Str(m, "name").NonEmpty() ?? url, OtherTitle = J.Str(m, "origin_name").NonEmpty(),
            Poster = posterRef != null ? Abs(origin, posterRef) : null, Description = desc, Info = info, Groups = groups,
        };
    }

    /// Đối tượng phim của trang (trang có thể nhắc tới phim khác → ưu tiên phim trùng slug trong địa chỉ).
    private static JObj? MovieOf(NextData data, string url)
    {
        var slug = url.TrimEnd('/').AfterLast('/');
        JObj? first = null;
        const string tag = "\"movie\":{";
        var i = data.IndexOf(tag);
        while (i is { } at)
        {
            if (data.ValueAt(at + "\"movie\":".Length) is JObj o)
            {
                if (J.Str(o, "slug") == slug) return o;
                first ??= o;
            }
            i = data.IndexOf(tag, at + 1);
        }
        return first;
    }

    private static JObj? EpisodeData(NextData data)
    {
        var i = data.IndexOf(EpisodeKey);
        return i == null ? null : data.ValueAt(i.Value + "\"episode\":".Length) as JObj;
    }

    /// Ngôn ngữ có tập (theo thứ tự trang đưa): slug → tên hiển thị, vd "thuyetminh" → "Thuyết Minh".
    private static List<(string Slug, string Name)> Languages(JObj ep)
    {
        var outList = new List<(string Slug, string Name)>();
        foreach (var s in J.Objects(J.Arr(ep, "available_sources")))
        {
            var slug = J.Str(s, "language_slug");
            if (slug.Length > 0 && outList.All(l => l.Slug != slug)) outList.Add((slug, J.Str(s, "language_name").NonEmpty() ?? slug));
        }
        return outList;
    }

    /// Server có nhiều tập nhất của một ngôn ngữ (lấy danh sách tập đầy đủ nhất).
    private static string? BestServer(JObj ep, string lang) =>
        J.Objects(J.Arr(ep, "available_sources")).Where(s => J.Str(s, "language_slug") == lang)
            .MaxBy(s => J.Int(s, "total_episodes")) is { } best ? J.Str(best, "server_slug").NonEmpty() : null;

    private static List<Episode> Episodes(JObj ep, string origin, string slug, string lang) =>
        J.Objects(J.Arr(ep, "episodes")).Select(e =>
        {
            var s = J.Str(e, "slug").NonEmpty();
            if (s == null) return null;
            var l = J.Str(e, "language").NonEmpty() ?? lang;
            var title = J.Str(e, "episode_label").NonEmpty() ?? J.Str(e, "name").NonEmpty() ?? s;
            return new Episode(title, $"{origin}/xem-phim/{slug}/{s}/{l}");
        }).OfType<Episode>().DistinctBy(e => e.Url).ToList();

    // ---- Trang tập phim ----

    /// Server của tập theo ngôn ngữ đang xem; link video của server trang mở sẵn đọc luôn (khỏi tải lại).
    public static EpisodeSource ParseSource(IDocument doc, NextData data, string epUrl)
    {
        var ep = EpisodeData(data) ?? throw new AppError("Không đọc được trang tập phim");
        var lang = J.Str(ep, "current_language");
        var servers = J.Objects(J.Arr(ep, "episode_sources")).Select(s =>
        {
            var sv = J.Obj(s, "server");
            if (sv == null) return null;
            var l = J.Str(J.Obj(s, "language"), "slug");
            if (lang.Length > 0 && l.Length > 0 && l != lang) return null;
            return new Server(J.Str(sv, "name"), J.Str(sv, "slug"));
        }).OfType<Server>().ToList();
        if (servers.Count == 0)
            servers = J.Objects(J.Arr(ep, "available_sources")).Where(s => lang.Length == 0 || J.Str(s, "language_slug") == lang)
                .Select(s => new Server(J.Str(s, "server_name"), J.Str(s, "server_slug"))).ToList();
        servers = servers.Where(s => s.Type.Length > 0).DistinctBy(s => s.Type)
            .Select(s => s.Label.Length == 0 ? new Server(s.Type, s.Type) : s).ToList();
        var cur = CurrentServer(ep);
        var links = new Dictionary<string, Video>();
        if (cur.Length > 0 && VideoOfEp(ep, OriginOf(doc)) is { } v) links[cur] = v;
        return new EpisodeSource { EpUrl = epUrl, Servers = servers, Ep = EpSlug(epUrl), Links = links, Next = true };
    }

    /// ".../xem-phim/tien-nghich/tap-165/vietsub" → "tap-165".
    public static string EpSlug(string epUrl)
    {
        var parts = epUrl.TrimEnd('/').Split('/');
        return parts.Length >= 2 ? parts[^2] : "";
    }

    /// Link video của một server: đã có sẵn thì dùng, không thì mở trang tập với ?server=… để lấy.
    public static async Task<Video> VideoOf(EpisodeSource src, Server server)
    {
        if (src.Links.TryGetValue(server.Type, out var v)) return v;
        var r = await Http.Get(src.EpUrl + "?server=" + server.Type.FormEncoded());
        var doc = Html.Parse(r.Body, r.Url);
        var d = Data(doc);
        var ep = d != null ? EpisodeData(d) : null;
        if (ep == null) throw new AppError("Không đọc được trang tập phim");
        // Tập chưa có trên server này thì trang tự mở server khác → coi như chưa có
        if (CurrentServer(ep) != server.Type || VideoOfEp(ep, OriginOf(doc)) is not { } video)
            throw new AppError($"Server \"{server.Label}\" chưa có video cho tập này");
        return video;
    }

    private static string CurrentServer(JObj ep) =>
        ep.TryGetValue("current_server", out var v) ? v switch { JObj o => J.Str(o, "slug"), string s => s, _ => "" } : "";

    /// Server riêng của trang ("HD") cho link dạng /public/m3u8/… (cùng tên miền) và phụ đề rời.
    private static Video? VideoOfEp(JObj ep, string origin)
    {
        var v = J.Obj(ep, "video");
        var link = J.Str(v, "link_m3u8").NonEmpty() ?? J.Str(v, "link_embed").NonEmpty();
        if (v == null || link == null || Abs(origin, link) is not { } url) return null;
        return new Video(url, Subtitle(J.Arr(v, "subtitles"), origin));
    }

    /// Phụ đề rời: bản mặc định, không có thì tiếng Việt.
    private static string? Subtitle(List<object?>? a, string origin)
    {
        var subs = J.Objects(a).Where(s => J.Str(s, "url").Length > 0).ToList();
        var s = subs.FirstOrDefault(x => J.Bool(x, "isDefault")) ?? subs.FirstOrDefault(x => J.Str(x, "lang") == "vie") ?? subs.FirstOrDefault();
        return s != null ? Abs(origin, J.Str(s, "url")) : null;
    }

    // ---- Tiện ích ----

    private static List<Film> Movies(NextData data, string origin, int from = 0, int? to = null)
    {
        var end = to ?? data.Length;
        var outList = new List<Film>();
        var seen = new HashSet<string>();
        foreach (var m in MovieRx.All(data.Text, from))
        {
            if (m.Index >= end) break;
            if (data.ValueAt(m.Index) is not JObj o || FilmOf(o, origin) is not { } f) continue;
            if (seen.Add(f.Url)) outList.Add(f);
        }
        return outList;
    }

    /// Phim trong danh sách (đối tượng thể loại / quốc gia cũng có id + name + slug nhưng không có poster).
    private static Film? FilmOf(JObj o, string origin)
    {
        if (!o.ContainsKey("poster_url")) return null;
        var slug = J.Str(o, "slug").NonEmpty();
        var name = J.Str(o, "name").NonEmpty();
        if (slug == null || name == null) return null;
        var p = J.Str(o, "poster_url").NonEmpty() ?? J.Str(o, "thumb_url").NonEmpty();
        return new Film($"{origin}/phim/{slug}", name, p != null ? Abs(origin, p) : null, Badge(J.Str(o, "display_status")));
    }

    private static readonly Rx NumberRx = new(@"\d+");

    /// Nhãn trên thẻ phim (một dòng): "Thuyết Minh 107, Vietsub 165" → "Tập 165", "Vietsub Full" → "Full".
    public static string? Badge(string status)
    {
        var nums = NumberRx.All(status).Select(m => int.TryParse(m.Value, out var n) ? n : (int?)null).OfType<int>().ToList();
        if (nums.Count > 0) return $"Tập {nums.Max()}";
        if (status.Contains("full", StringComparison.OrdinalIgnoreCase)) return "Full";
        return status.NonEmpty();
    }

    private static string Names(List<object?>? a) =>
        string.Join(", ", J.Objects(a).Select(o => J.Str(o, "name")).Where(s => s.Length > 0));

    private static string OriginOf(IDocument doc) => Site.OriginOf(doc.Location()) ?? Prefs.BaseUrl;

    /// Đường dẫn tuyệt đối; dạng lạ (vd "data:…") thì null — bỏ một ảnh / link chứ không hỏng cả trang.
    private static string? Abs(string origin, string reference)
    {
        if (!Uri.TryCreate(origin + "/", UriKind.Absolute, out var b) || !Uri.TryCreate(b, reference, out var u)) return null;
        var s = u.AbsoluteUri;
        return s.StartsWith("http") ? s : null;
    }
}
