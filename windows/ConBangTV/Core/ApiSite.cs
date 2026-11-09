using System.Text.RegularExpressions;

namespace ConBangTV.Core;

using JObj = Dictionary<string, object?>;

/// Trang phim dựng bằng React (trang trắng + JS, vd mephim.ddns.net) có API JSON riêng ở /api/movies/… (nguồn KKPhim,
/// định dạng của phimapi.com) — bản Android: ApiSite.kt, iPad: ApiSite.swift (giữ cùng tên hàm để sửa song song).
///
/// Địa chỉ trong app theo đường dẫn của chính trang: phim = /phim/&lt;slug&gt;, tập = /phim/&lt;slug&gt;/&lt;tập&gt;?sv=&lt;số thứ tự bản&gt;
/// (Vietsub / Thuyết Minh cùng tên tập nên phải ghi bản). Danh sách = địa chỉ API.
public static class ApiSite
{
    /// Trang KKPhim (www.kkphim.com, từ 1.3): không có API /api/movies riêng — mọi dữ liệu gọi thẳng API công khai
    /// phimapi.com (cùng định dạng, qua Fallback). Địa chỉ phim / tập trong app vẫn theo tên miền kkphim.com.
    public const string KkHost = "kkphim.com";

    /// VSMOV (vsmov.com, từ 1.4): API riêng cùng định dạng KKPhim nhưng khác đường dẫn (/api/danh-sach/…, /api/phim/…,
    /// /api/tim-kiem). Tập chỉ có trình phát nhúng; app đọc luồng HLS trong trang trình phát (link có chữ ký) để tự phát.
    public const string VsHost = "vsmov.com";

    private static readonly object Lock = new();
    private static readonly HashSet<string> Known = new() { "mephim.ddns.net", KkHost, VsHost, NguonC.Host, Stremio.Host };
    private static readonly HashSet<string> NotApi = new();
    private const string Cdn = "https://phimimg.com";
    private const int Limit = 24;

    /// Menu cố định (API không có danh sách mục): hoạt hình Trung Quốc lên đầu như các trang khác của app.
    private static readonly (string, string)[] Menu =
    {
        ("Hoạt hình Trung Quốc", "category/hoat-hinh?country=trung-quoc"),
        ("Hoạt hình", "category/hoat-hinh"),
        ("Phim bộ", "category/phim-bo"),
        ("Phim lẻ", "category/phim-le"),
        ("TV Shows", "category/tv-shows"),
        ("Phim thuyết minh", "category/phim-thuyet-minh"),
        ("Phim lồng tiếng", "category/phim-long-tieng"),
        ("Phim Trung Quốc", "quoc-gia/trung-quoc"),
        ("Phim Hàn Quốc", "quoc-gia/han-quoc"),
        ("Phim Âu Mỹ", "quoc-gia/au-my"),
        ("Phim Nhật Bản", "quoc-gia/nhat-ban"),
        ("Phim Thái Lan", "quoc-gia/thai-lan"),
    };

    private static readonly (string, string)[] Genres =
    {
        ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Viễn Tưởng", "vien-tuong"), ("Kinh Dị", "kinh-di"),
        ("Tình Cảm", "tinh-cam"), ("Hài Hước", "hai-huoc"), ("Võ Thuật", "vo-thuat"), ("Tâm Lý", "tam-ly"),
        ("Phiêu Lưu", "phieu-luu"), ("Hình Sự", "hinh-su"), ("Chiến Tranh", "chien-tranh"), ("Thần Thoại", "than-thoai"),
        ("Bí Ẩn", "bi-an"), ("Học Đường", "hoc-duong"), ("Gia Đình", "gia-dinh"), ("Thể Thao", "the-thao"),
        ("Âm Nhạc", "am-nhac"), ("Chính Kịch", "chinh-kich"), ("Khoa Học", "khoa-hoc"), ("Tài Liệu", "tai-lieu"),
        ("Lịch Sử", "lich-su"), ("Kinh Điển", "kinh-dien"), ("Phim 18+", "phim-18"),
    };

    private static readonly (string, string)[] Countries =
    {
        ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Âu Mỹ", "au-my"),
        ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
        ("Việt Nam", "viet-nam"), ("Anh", "anh"), ("Pháp", "phap"), ("Canada", "canada"), ("Philippines", "philippines"),
        ("Indonesia", "indonesia"), ("Malaysia", "malaysia"), ("Nga", "nga"), ("Đức", "duc"), ("Tây Ban Nha", "tay-ban-nha"),
        ("Thổ Nhĩ Kỳ", "tho-nhi-ky"), ("Úc", "uc"), ("Quốc Gia Khác", "quoc-gia-khac"),
    };

    /// Trang KKPhim (gọi thẳng phimapi.com) thêm vài mục theo các danh sách riêng của KKPhim.
    private static readonly (string, string)[] KkMenu = Menu.Take(1)
        .Append(("Hoạt hình Trung Quốc thuyết minh", "category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh"))
        .Concat(Menu.Skip(1)).Append(("Phim chiếu rạp", "category/phim-chieu-rap")).ToArray();

    public static bool IsKk(string url) => Site.HostOf(url) == KkHost;

    public static bool IsVs(string url) => Site.HostOf(url) == VsHost;

    /// Menu VSMOV: lọc hoạt hình theo nước có trên /api/the-loai/…?country=.
    private static readonly (string, string)[] VsMenu =
    {
        ("Hoạt hình Trung Quốc", "the-loai/hoat-hinh?country=trung-quoc"),
        ("Hoạt hình", "the-loai/hoat-hinh"),
        ("Phim bộ", "danh-sach/phim-bo"),
        ("Phim lẻ", "danh-sach/phim-le"),
        ("Phim nhóm sub VSMOV", "danh-sach/subteam"),
        ("Phim Trung Quốc", "quoc-gia/trung-quoc"),
        ("Phim Hàn Quốc", "quoc-gia/han-quoc"),
        ("Phim Âu Mỹ", "quoc-gia/au-my"),
        ("Phim Nhật Bản", "quoc-gia/nhat-ban"),
        ("Phim Thái Lan", "quoc-gia/thai-lan"),
    };

    /// Thể loại / quốc gia (tên, slug KKPhim) — chọn cho hàng tự thêm của trang chủ tổng hợp.
    public static IEnumerable<(string Name, string Slug)> GenreList => Genres.Where(g => g.Item2 != "phim-18");
    public static IEnumerable<(string Name, string Slug)> CountryList => Countries;

    private static readonly (string, string)[] VsGenres =
    {
        ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Kiếm Hiệp", "kiem-hiep"), ("Tiên Hiệp", "tien-hiep"),
        ("Võ Thuật", "vo-thuat"), ("Viễn Tưởng", "vien-tuong"), ("Giả Tưởng", "gia-tuong"), ("Kinh Dị", "kinh-di"),
        ("Lãng Mạn", "lang-man"), ("Hài", "hai"), ("Chính Kịch", "chinh-kich"), ("Phiêu Lưu", "phieu-luu"),
        ("Hình Sự", "hinh-su"), ("Tội Phạm", "toi-pham"), ("Chiến Tranh", "chien-tranh"), ("Bí Ẩn", "bi-an"),
        ("Học Đường", "hoc-duong"), ("Gia Đình", "gia-dinh"), ("Thiếu Nhi", "thieu-nhi"), ("Thanh Xuân", "thanh-xuan"),
        ("Thương Trường", "thuong-truong"), ("Xã Hội Đen", "xa-hoi-den"), ("Truyền Hình Thực Tế", "truyen-hinh-thuc-te"),
        ("Phim Nhạc", "phim-nhac"),
    };

    private static readonly (string, string)[] VsCountries =
    {
        ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Âu Mỹ", "au-my"), ("Mỹ", "my"),
        ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
        ("Việt Nam", "viet-nam"), ("Anh", "anh"), ("Pháp", "phap"), ("Canada", "canada"), ("Úc", "uc"), ("Đức", "duc"),
        ("Tây Ban Nha", "tay-ban-nha"), ("Thổ Nhĩ Kỳ", "tho-nhi-ky"), ("Philippines", "philippines"), ("Indonesia", "indonesia"),
    };

    /// Địa chỉ API của VSMOV: "danh-sach/phim-bo" → https://vsmov.com/api/danh-sach/phim-bo?page=1.
    private static string VsApi(string origin, string path, int page)
    {
        var parts = path.Split('?', 2);
        return $"{origin}/api/{parts[0]}?" + (parts.Length > 1 ? parts[1] + "&" : "") + $"page={page}";
    }

    public static bool IsKnown(string url)
    {
        var h = Site.HostOf(url);
        if (h == null) return false;
        lock (Lock) return Known.Contains(h);
    }

    /// Trang chưa biết mà HTML là trang trắng của ứng dụng JS: thử API (một lần mỗi trang).
    public static async Task<bool> Probe(string baseUrl)
    {
        var host = Site.HostOf(baseUrl);
        if (host == null) return false;
        lock (Lock)
        {
            if (Known.Contains(host)) return true;
            if (NotApi.Contains(host)) return false;
        }
        bool ok;
        try
        {
            ok = J.Arr(await GetJson($"{Origin(baseUrl)}/api/movies/newest?page=1&limit=1", false), "items") != null;
        }
        catch (Exception)
        {
            ok = false;
        }
        lock (Lock)
        {
            if (ok) Known.Add(host);
            else NotApi.Add(host);
        }
        return ok;
    }

    // ---- Trang chủ / danh sách ----

    /// Trang chủ: các khối tải song song; khối nào lỗi thì bỏ.
    public static async Task<Home> Home(string baseUrl)
    {
        if (NguonC.IsSite(baseUrl)) return await NguonC.Home(baseUrl);
        if (Stremio.IsSite(baseUrl)) return await Stremio.Home(baseUrl);
        var o = Origin(baseUrl);
        var kk = IsKk(baseUrl);
        var vs = IsVs(baseUrl);
        // KKPhim không có khối thịnh hành / song ngữ: thêm hoạt hình thuyết minh, phim bộ, phim lẻ mới
        var blocks = vs
            ? new[]
            {
                ("Hoạt hình Trung Quốc", VsApi(o, "the-loai/hoat-hinh?country=trung-quoc", 1)),
                ("Phim mới cập nhật", NewestUrl(o)),
                ("Phim bộ mới", VsApi(o, "danh-sach/phim-bo", 1)),
                ("Phim lẻ mới", VsApi(o, "danh-sach/phim-le", 1)),
                ("Phim nhóm sub VSMOV", VsApi(o, "danh-sach/subteam", 1)),
            }
            : kk
            ? new[]
            {
                ("Hoạt hình Trung Quốc mới cập nhật", Api(o, "category/hoat-hinh?country=trung-quoc", 1)),
                ("Hoạt hình Trung Quốc thuyết minh", Api(o, "category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh", 1)),
                ("Phim mới cập nhật", NewestUrl(o)),
                ("Phim bộ mới", Api(o, "category/phim-bo", 1)),
                ("Phim lẻ mới", Api(o, "category/phim-le", 1)),
            }
            : new[]
            {
                ("Hoạt hình Trung Quốc mới cập nhật", Api(o, "category/hoat-hinh?country=trung-quoc", 1)),
                ("Phim mới cập nhật", NewestUrl(o)),
                ("Thịnh hành", $"{o}/api/movies/trending"),
                ("Song ngữ / Thuyết minh", $"{o}/api/movies/song-ngu"),
            };
        var sections = await Sections(blocks, async u => ParseItems(await GetJson(u), o));
        return new Home { Sections = sections, Categories = MenuOf(baseUrl) };
    }

    /// Mục menu của trang (cố định — không cần tải trang chủ; trang chủ tổng hợp dùng).
    public static List<Category> MenuOf(string baseUrl)
    {
        if (NguonC.IsSite(baseUrl)) return NguonC.MenuOf(baseUrl);
        if (Stremio.IsSite(baseUrl)) return Stremio.MenuOf(baseUrl);
        var o = Origin(baseUrl);
        return IsVs(baseUrl)
            ? VsMenu.Select(m => new Category(m.Item1, VsApi(o, m.Item2, 1))).ToList()
            : (IsKk(baseUrl) ? KkMenu : Menu).Select(m => new Category(m.Item1, Api(o, m.Item2, 1))).ToList();
    }

    /// Tên thể loại theo slug ("vo-thuat" → "võ thuật") — tiêu đề hàng gợi ý của trang chủ tổng hợp.
    public static string GenreName(string slug) =>
        Genres.Concat(VsGenres).FirstOrDefault(g => g.Item2 == slug).Item1?.ToLowerInvariant() ?? slug.Replace('-', ' ');

    /// Tên nước theo slug ("trung-quoc" → "Trung Quốc").
    public static string CountryName(string slug) =>
        Countries.Concat(VsCountries).FirstOrDefault(c => c.Item2 == slug).Item1 ?? slug.Replace('-', ' ');

    /// JSON của một API (KKPhim: gọi thẳng phimapi.com; trang lỗi thì phimapi.com) — trang chủ tổng hợp dùng.
    public static Task<JObj> Json(string url) => GetJson(url);

    /// Các khối trang chủ tải song song; khối nào lỗi / rỗng thì bỏ, lỗi hết thì báo lỗi.
    public static async Task<List<FilmSection>> Sections((string, string)[] blocks, Func<string, Task<List<Film>>> load)
    {
        var tasks = blocks.Select(b => load(b.Item2)).ToList();
        try
        {
            await Task.WhenAll(tasks);
        }
        catch (Exception)
        {
        }
        var sections = new List<FilmSection>();
        Exception? firstError = null;
        for (var i = 0; i < blocks.Length; i++)
        {
            if (!tasks[i].IsCompletedSuccessfully)
            {
                firstError ??= tasks[i].Exception?.InnerException;
                continue;
            }
            if (tasks[i].Result.Count > 0) sections.Add(new FilmSection(blocks[i].Item1, tasks[i].Result));
        }
        if (sections.Count == 0) throw firstError ?? new AppError("Không tải được danh sách phim");
        return sections;
    }

    public static async Task<FilmPage> List(string url)
    {
        if (NguonC.IsSite(url)) return await NguonC.List(url);
        if (Stremio.IsSite(url)) return await Stremio.List(url);
        return ParseList(await GetJson(url), url);
    }

    /// Danh sách có phân trang (data.params.pagination; VSMOV: pagination ở gốc) → trang sau = page + 1.
    public static FilmPage ParseList(JObj root, string url)
    {
        var films = ParseItems(root, Origin(url));
        var data = J.Obj(root, "data") ?? root;
        var p = J.Obj(J.Obj(data, "params"), "pagination") ?? J.Obj(root, "pagination");
        int page = J.Int(p, "currentPage"), pages = J.Int(p, "totalPages");
        var next = films.Count > 0 && page >= 1 && page < pages ? WithPage(url, page + 1) : null;
        return new FilmPage { Films = films, NextUrl = next };
    }

    /// Phim mới cập nhật (đo nhanh trang trong Cài đặt).
    public static string NewestUrl(string baseUrl) =>
        NguonC.IsSite(baseUrl) ? NguonC.NewestUrl(baseUrl)
        : Stremio.IsSite(baseUrl) ? Stremio.NewestUrl(baseUrl)
        : IsVs(baseUrl) ? $"{Origin(baseUrl)}/api/danh-sach/phim-moi-cap-nhat?page=1"
        : $"{Origin(baseUrl)}/api/movies/newest?page=1&limit={Limit}";

    public static string SearchUrl(string baseUrl, string query) =>
        NguonC.IsSite(baseUrl) ? NguonC.SearchUrl(baseUrl, query)
        : Stremio.IsSite(baseUrl) ? Stremio.SearchUrl(baseUrl, query)
        : IsVs(baseUrl) ? $"{Origin(baseUrl)}/api/tim-kiem?keyword=" + query.Trim().FormEncoded() + $"&page=1&limit={Limit}"
        : $"{Origin(baseUrl)}/api/movies/search?keyword=" + query.Trim().FormEncoded() + $"&page=1&limit={Limit}";

    public static (List<Category> Genres, List<Category> Countries) Filters(string baseUrl)
    {
        if (NguonC.IsSite(baseUrl)) return NguonC.Filters(baseUrl);
        if (Stremio.IsSite(baseUrl)) return Stremio.Filters(baseUrl);
        var o = Origin(baseUrl);
        if (IsVs(baseUrl))
        {
            return (VsGenres.Select(g => new Category(g.Item1, VsApi(o, $"the-loai/{g.Item2}", 1))).ToList(),
                VsCountries.Select(c => new Category(c.Item1, VsApi(o, $"quoc-gia/{c.Item2}", 1))).ToList());
        }
        return (Genres.Select(g => new Category(g.Item1, Api(o, $"the-loai/{g.Item2}", 1))).ToList(),
            Countries.Select(c => new Category(c.Item1, Api(o, $"quoc-gia/{c.Item2}", 1))).ToList());
    }

    /// Phim trong một phản hồi danh sách (items ở gốc hoặc trong data).
    public static List<Film> ParseItems(JObj root, string origin)
    {
        var data = J.Obj(root, "data") ?? root;
        var b = J.Str(data, "APP_DOMAIN_CDN_IMAGE").NonEmpty() ?? J.Str(root, "cdnDomain").NonEmpty() ?? Cdn;
        return J.Objects(J.Arr(data, "items")).Select(o => FilmOf(o, origin, b)).OfType<Film>().DistinctBy(f => f.Url).ToList();
    }

    private static Film? FilmOf(JObj o, string origin, string b)
    {
        var slug = J.Str(o, "slug").NonEmpty();
        var name = J.Str(o, "name").NonEmpty();
        if (slug == null || name == null) return null;
        var poster = Image(J.Str(o, "poster_url").NonEmpty() ?? J.Str(o, "thumb_url"), J.Str(o, "cdnDomain").NonEmpty() ?? b);
        return new Film($"{origin}/phim/{slug}", name, poster, NextSite.Badge(J.Str(o, "episode_current")));
    }

    // ---- Chi tiết phim ----

    public static async Task<FilmDetail> Detail(string url)
    {
        if (NguonC.IsSite(url)) return await NguonC.Detail(url);
        if (Stremio.IsSite(url)) return await Stremio.Detail(url);
        var film = FilmUrl(url);
        return ParseDetail(await GetJson(DetailApi(film)), film);
    }

    public static FilmDetail ParseDetail(JObj root, string url)
    {
        var m = J.Obj(root, "movie") ?? throw new AppError("Không đọc được thông tin phim");
        var content = J.Str(m, "content").NonEmpty();
        var desc = content != null ? Html.Parse(content, url).Body?.TextContent.CollapseSpaces().NonEmpty() : null;
        var year = J.Any(m, "year");
        if (year == "0") year = "";
        var info = string.Join("  •  ", new[]
        {
            J.Str(m, "episode_current"), J.Str(m, "time"), J.Str(m, "lang"), Names(J.Arr(m, "country")), year, Names(J.Arr(m, "category")),
        }.Where(s => s.Length > 0)).NonEmpty();
        var groups = new List<EpisodeGroup>();
        var servers = J.Objects(J.Arr(root, "episodes"));
        for (var i = 0; i < servers.Count; i++)
        {
            var s = servers[i];
            var eps = J.Objects(J.Arr(s, "server_data")).Select(e =>
            {
                var slug = J.Str(e, "slug").NonEmpty();
                if (slug == null) return null;
                if (J.Str(e, "link_m3u8").Length == 0 && J.Str(e, "link_embed").Length == 0) return null;
                return new Episode(Title(J.Str(e, "name").NonEmpty() ?? slug), $"{url}/{slug}?sv={i}");
            }).OfType<Episode>().DistinctBy(e => e.Url).ToList();
            // VSMOV: "Vietsub\r\n      #1" → gộp khoảng trắng
            var name = Regex.Replace(J.Str(s, "server_name"), @"\s+", " ").NonEmpty();
            if (eps.Count > 0) groups.Add(new EpisodeGroup(name ?? $"Bản {i + 1}", eps));
        }
        return new FilmDetail
        {
            Url = url, Title = J.Str(m, "name").NonEmpty() ?? url, OtherTitle = J.Str(m, "origin_name").NonEmpty(),
            Poster = Image(J.Str(m, "poster_url").NonEmpty() ?? J.Str(m, "thumb_url"), Cdn),
            Description = desc, Info = info, Groups = groups,
        };
    }

    private static readonly Rx LeadingZero = new(@"^(Tập\s*)0+(\d)", RegexOptions.IgnoreCase);

    private static readonly Rx BareNumber = new(@"^\d+(\.\d+)?$");

    /// "Tập 01" → "Tập 1", "164" → "Tập 164" (VSMOV ghi số trần); "Full" giữ nguyên.
    public static string Title(string name)
    {
        var s = name.Trim();
        if (BareNumber.Matches(s))
        {
            var n = s.TrimStart('0');
            return "Tập " + (n.Length == 0 || n.StartsWith('.') ? "0" + n : n);
        }
        return LeadingZero.Replace(s, "${1}${2}");
    }

    // ---- Tập phim ----

    /// Hai server cho mỗi tập: luồng HLS (app tự phát, bỏ quảng cáo) và trình phát nhúng của KKPhim (dự phòng).
    public static async Task<EpisodeSource> Source(string epUrl)
    {
        if (NguonC.IsSite(epUrl)) return await NguonC.Source(epUrl);
        if (Stremio.IsSite(epUrl)) return await Stremio.Source(epUrl);
        var root = await GetJson(DetailApi(FilmUrl(epUrl)));
        var u = new Uri(epUrl);
        var svText = Regex.Match(u.Query, @"[?&]sv=(\d+)").Groups[1].Value;
        var sv = int.TryParse(svText, out var n) ? n : 0;
        var tap = u.AbsolutePath.TrimEnd('/').AfterLast('/');
        var servers = J.Objects(J.Arr(root, "episodes"));
        var order = (sv < servers.Count ? new[] { servers[sv] } : Array.Empty<JObj>()).Concat(servers);
        var e = order.Select(s => J.Objects(J.Arr(s, "server_data")).FirstOrDefault(x => J.Str(x, "slug") == tap)).FirstOrDefault(x => x != null)
            ?? throw new AppError("Không tìm thấy tập này trên trang");
        var links = new Dictionary<string, Video>();
        var list = new List<Server>();
        var vs = IsVs(epUrl);
        var embed = J.Str(e, "link_embed").StartsWith("http") ? J.Str(e, "link_embed") : null;
        var hls = J.Str(e, "link_m3u8").StartsWith("http") ? J.Str(e, "link_m3u8") : null;
        if (hls == null && vs && embed != null) hls = await VsHls(embed);
        if (hls != null)
        {
            links["m3u8"] = new Video(hls);
            list.Add(new Server(vs ? "HLS" : "HLS (bỏ quảng cáo)", "m3u8"));
        }
        if (embed != null)
        {
            links["embed"] = new Video(embed);
            list.Add(new Server(vs ? "Trình phát VSMOV" : "Trình phát KKPhim", "embed"));
        }
        if (links.Count == 0) throw new AppError("Tập này chưa có video");
        return new EpisodeSource { EpUrl = epUrl, Servers = list, Ep = tap, Sv = sv.ToString(), Links = links, Next = true };
    }

    /// Luồng HLS trong trang trình phát VSMOV (vN.streamvsmov.com/video/&lt;mã&gt;): link có chữ ký (signedMasterUrl, hết hạn sau
    /// khoảng một giờ) hoặc /stream/&lt;mã&gt;/master.m3u8 khi máy chủ đó không ký. Đoạn video ngụy trang ảnh PNG — hls.html cắt
    /// phần ảnh. Không đọc được thì null (vẫn còn trình phát nhúng).
    private static async Task<string?> VsHls(string embed)
    {
        try
        {
            var r = await Http.Get(embed, new() { ["Referer"] = "https://vsmov.com/" });
            return VsHlsFrom(r.Body);
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static readonly Rx SignedRx = new(@"signedMasterUrl\s*:\s*""(https?://[^""]+)""");
    private static readonly Rx BaseRx = new(@"baseUrl\s*=\s*""(https?://[^""]+)""");
    private static readonly Rx HashRx = new(@"videoHash\s*=\s*""([0-9a-zA-Z-]+)""");

    public static string? VsHlsFrom(string page)
    {
        if (SignedRx.Group(1, page) is { } signed) return signed;
        var b = BaseRx.Group(1, page);
        var h = HashRx.Group(1, page);
        return b != null && h != null ? $"{b}/stream/{h}/master.m3u8" : null;
    }

    // ---- Tiện ích ----

    /// Địa chỉ phim từ địa chỉ phim / tập: ".../phim/tien-nghich/tap-01?sv=1" → ".../phim/tien-nghich".
    public static string FilmUrl(string url)
    {
        var parts = Uri.TryCreate(url, UriKind.Absolute, out var u) ? u.AbsolutePath.Split('/', StringSplitOptions.RemoveEmptyEntries) : Array.Empty<string>();
        var i = Array.IndexOf(parts, "phim");
        if (i < 0 || i + 1 >= parts.Length) throw new AppError($"Địa chỉ phim lạ: {url}");
        return $"{Origin(url)}/phim/{parts[i + 1]}";
    }

    private static string DetailApi(string url) =>
        IsVs(url) ? $"{Origin(url)}/api/phim/" + FilmUrl(url).AfterLast('/') : $"{Origin(url)}/api/movies/detail/" + FilmUrl(url).AfterLast('/');

    private static string Api(string origin, string path, int page)
    {
        var parts = path.Split('?', 2);
        return $"{origin}/api/movies/{parts[0]}?" + (parts.Length > 1 ? parts[1] + "&" : "") + $"page={page}&limit={Limit}";
    }

    private static readonly Rx PageRx = new(@"([?&]page=)\d+");

    public static string WithPage(string url, int page)
    {
        if (PageRx.Matches(url)) return PageRx.Re.Replace(url, "${1}" + page, 1);
        return url + (url.Contains('?') ? "&" : "?") + $"page={page}";
    }

    /// Gọi API của trang; trang lỗi (máy chủ của trang hay báo 500) thì gọi thẳng nguồn KKPhim (phimapi.com — cùng định
    /// dạng), như chính trang web làm.
    private static async Task<JObj> GetJson(string url, bool useFallback = true)
    {
        // Trang KKPhim: gọi thẳng phimapi.com (kkphim.com không có /api/movies)
        if (IsKk(url)) return await FetchJson(Fallback(url) ?? url);
        try
        {
            return await FetchJson(url);
        }
        catch (Exception)
        {
            var alt = useFallback ? Fallback(url) : null;
            if (alt == null) throw;
            try
            {
                return await FetchJson(alt);
            }
            catch (Exception)
            {
            }
            throw;
        }
    }

    private static async Task<JObj> FetchJson(string url)
    {
        var r = await Http.Get(url, new() { ["Accept"] = "application/json" });
        var o = JsonScan.ParseObject(r.Data) ?? throw new AppError("Trang trả dữ liệu lạ");
        if (o.TryGetValue("error", out var e) && e is string es && !o.ContainsKey("items") && !o.ContainsKey("data") && !o.ContainsKey("movie"))
            throw new AppError(es);
        return o;
    }

    /// Địa chỉ tương ứng trên phimapi.com của một API của trang (không có thì null, vd thịnh hành / song ngữ).
    public static string? Fallback(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var u)) return null;
        var i = u.AbsolutePath.IndexOf("/api/movies/", StringComparison.Ordinal);
        if (i < 0) return null;
        var path = u.AbsolutePath[(i + "/api/movies/".Length)..];
        var q = url.Contains('?') ? "?" + url[(url.IndexOf('?') + 1)..] : "";
        const string kk = "https://phimapi.com";
        if (path == "newest") return $"{kk}/v1/api/danh-sach/phim-moi-cap-nhat{q}";
        if (path.StartsWith("category/")) return $"{kk}/v1/api/danh-sach/{path["category/".Length..]}{q}";
        if (path.StartsWith("the-loai/") || path.StartsWith("quoc-gia/")) return $"{kk}/v1/api/{path}{q}";
        if (path == "search") return $"{kk}/v1/api/tim-kiem{q}";
        if (path.StartsWith("detail/")) return $"{kk}/phim/{path["detail/".Length..]}";
        return null;
    }

    public static string Origin(string url) => Site.OriginOf(url) ?? url.TrimEnd('/');

    /// Ảnh: đường dẫn đầy đủ giữ nguyên; "uploads/movies/…" → nối vào máy chủ ảnh.
    public static string? Image(string reference, string b)
    {
        if (reference.Length == 0) return null;
        if (reference.StartsWith("http")) return reference;
        var host = Site.OriginOf(b) ?? Cdn;
        return $"{host}/{reference.TrimStart('/')}";
    }

    private static string Names(List<object?>? a) =>
        string.Join(", ", J.Objects(a).Select(o => J.Str(o, "name")).Where(s => s.Length > 0));
}
