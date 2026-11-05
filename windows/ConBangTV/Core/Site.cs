using System.Text.RegularExpressions;
using AngleSharp.Dom;

namespace ConBangTV.Core;

// Đọc dữ liệu từ trang phim (bản Android: Site.kt, iPad: Site.swift — giữ cùng tên hàm, cùng bộ chọn CSS để sửa song song).
// Không viết riêng cho từng trang: nhận dạng theo mẫu giao diện ("flw-item" kiểu yanhh3d, "halim-item" của theme HaLim).
// Trang dựng bằng Next.js (kiểu PhimMoi) thì chuyển cho NextSite; trang ứng dụng JS có API JSON (mephim) cho ApiSite.
public static class Site
{
    /// Thẻ phim ở lưới / trượt / bảng xếp hạng.
    public const string Card = ".flw-item, li.item-top, .halim-item, a.halim-trending-link, a.t10-card";

    /// Tiêu đề khối trên trang chủ.
    public const string Heading = ".cat-heading, .section-title";

    // ---- Bộ nhớ chi tiết phim (30 phim gần nhất) ----

    private static readonly object CacheLock = new();
    private static readonly Dictionary<string, FilmDetail> DetailCache = new();
    private static readonly List<string> CacheOrder = new();

    public static FilmDetail? CachedDetail(string url)
    {
        lock (CacheLock) return DetailCache.GetValueOrDefault(url);
    }

    private static void Cache(FilmDetail d, string url)
    {
        lock (CacheLock)
        {
            DetailCache[url] = d;
            CacheOrder.Remove(url);
            CacheOrder.Add(url);
            if (CacheOrder.Count > 30)
            {
                DetailCache.Remove(CacheOrder[0]);
                CacheOrder.RemoveAt(0);
            }
        }
    }

    public static async Task<IDocument> Fetch(string url, Dictionary<string, string>? headers = null)
    {
        var r = await Http.Get(url, headers);
        return Html.Parse(r.Body, r.Url);
    }

    // ---- Trang chủ ----

    public static async Task<Home> HomeOf(string? baseUrl = null)
    {
        var b = baseUrl ?? Prefs.BaseUrl;
        if (ApiSite.IsKnown(b)) return await ApiSite.Home(b);
        var doc = await Fetch(b + "/");
        var o = OriginOf(doc.Location());
        var moved = o != null && HostOf(o) != HostOf(b) ? o : null;
        Home home;
        var data = NextSite.Data(doc);
        home = data != null ? await NextSite.Home(doc, data) : ParseHome(doc);
        // Trang trắng của ứng dụng JS (React…): thử API kiểu mephim
        if (home.Sections.Count == 0 && IsAppShell(doc) && await ApiSite.Probe(b)) return await ApiSite.Home(b);
        home.MovedTo = moved;
        return home;
    }

    /// Trang chỉ có khung rỗng cho ứng dụng JS (vd &lt;div id="root"&gt;&lt;/div&gt;) — dữ liệu không nằm trong HTML.
    public static bool IsAppShell(IDocument doc)
    {
        var root = doc.Q1("#root, #app, #__nuxt");
        return root != null && root.Children.Length == 0 && doc.Q("a[href]").Count < 5;
    }

    /// Thời gian tính phần tải qua mạng, không tính lúc đọc HTML.
    public static async Task<SiteCheck> Check(string baseUrl)
    {
        var t0 = DateTime.UtcNow;
        if (ApiSite.IsKnown(baseUrl))
        {
            var n = (await ApiSite.List(ApiSite.NewestUrl(baseUrl))).Films.Count;
            return new SiteCheck((int)(DateTime.UtcNow - t0).TotalMilliseconds, n);
        }
        var r = await Http.Get(baseUrl + "/");
        var ms = (int)(DateTime.UtcNow - t0).TotalMilliseconds;
        var doc = Html.Parse(r.Body, r.Url);
        return new SiteCheck(ms, ParseHome(doc).Sections.Sum(s => s.Films.Count));
    }

    private static HashSet<IElement> IdSet(IParentNode root, string css) => new(root.Q(css), ReferenceEqualityComparer.Instance);

    private static bool HasAncestor(IElement el, HashSet<IElement> set, bool includeSelf = false)
    {
        for (var e = includeSelf ? el : el.ParentElement; e != null; e = e.ParentElement)
            if (set.Contains(e)) return true;
        return false;
    }

    private static IElement? Closest(IElement el, HashSet<IElement> set)
    {
        for (var e = el; e != null; e = e.ParentElement)
            if (set.Contains(e)) return e;
        return null;
    }

    /// Gom thẻ phim theo tiêu đề khối đứng trước nó (theo thứ tự trong trang).
    public static Home ParseHome(IDocument doc)
    {
        var data = NextSite.Data(doc);
        if (data != null) return NextSite.ParseHome(doc, data);
        var cards = IdSet(doc, Card);
        var realtime = IdSet(doc, ".block_area-realtime");
        var schedule = IdSet(doc, ".block_area-schedule");
        var order = new List<IElement?>();
        var groups = new Dictionary<object, List<IElement>>(ReferenceEqualityComparer.Instance);
        var noHeading = new object();
        IElement? heading = null;
        foreach (var el in doc.Q($"{Heading}, {Card}"))
        {
            if (HasAncestor(el, cards)) continue;
            if (cards.Contains(el))
            {
                object k = (object?)heading ?? noHeading;
                if (!groups.ContainsKey(k))
                {
                    groups[k] = new();
                    order.Add(heading);
                }
                groups[k].Add(el);
            }
            else
            {
                heading = el;
            }
        }
        var sections = new List<FilmSection>();
        foreach (var h in order)
        {
            var films = groups[(object?)h ?? noHeading].Select(ParseCard).OfType<Film>().DistinctBy(f => f.Url).ToList();
            if (films.Count == 0) continue;
            var text = h?.Txt() ?? "";
            string title;
            if (h == null) title = "Nổi bật";
            else if (text.Length == 0) title = HasAncestor(h, realtime, true) ? "Xem nhiều" : "Phim";
            else if (HasAncestor(h, schedule, true)) title = $"{text} hôm nay";
            else title = text;
            sections.Add(new FilmSection(title, films));
        }
        return new Home { Sections = sections, Categories = ParseCategories(doc) };
    }

    private static readonly Rx SkipCategory = new("account|login|dang-nhap|lich-su|history|bookmark|theo-doi|sitemap|feed");

    private static List<Category> ParseCategories(IDocument doc)
    {
        var o = OriginOf(doc.Location());
        var home = o != null ? HostOf(o) : null;
        var outList = new List<Category>();
        var seen = new HashSet<string>();
        var links = doc.Q("a.nav-link[href], li.menu-item > a[href], #header nav a[href], .navbar a[href], #menu a[href]");
        if (links.Count == 0) links = doc.Q(".block_area-genres a[href]");
        foreach (var a in links)
        {
            // Bỏ mục cha của menu thả xuống (href="#")
            var href = a.At("href").Trim();
            if (href.StartsWith('#') || href.StartsWith("javascript", StringComparison.OrdinalIgnoreCase)) continue;
            var url = a.Abs("href").TrimEnd('/');
            var title = a.Txt().Length == 0 ? a.At("title").Trim() : a.Txt();
            if (url.Length == 0 || title.Length == 0 || seen.Contains(url)) continue;
            if (HostOf(url) != home || !Uri.TryCreate(url, UriKind.Absolute, out var u) || u.AbsolutePath.Trim('/').Length == 0) continue;
            if (SkipCategory.Matches(url)) continue;
            seen.Add(url);
            outList.Add(new Category(title, url));
        }
        return outList;
    }

    /// Thẻ phim → Film. Hỗ trợ flw-item (yanhh3d), halim-item (HaLim), item-top (bảng xếp hạng).
    public static Film? ParseCard(IElement el)
    {
        var a = el.Q1("a.film-poster-ahref[href], a.halim-thumb[href], .film-name a[href]") ?? el.Q1("a[href]")
            ?? (el.LocalName == "a" && el.HasAttribute("href") ? el : null);
        if (a == null) return null;
        var url = a.Abs("href");
        if (url.Length == 0) return null;
        var img = el.Q1("img");
        var candidates = new[] { el.Q1(".film-name a, .entry-title, .halim-trending-title-text, h4, .t10-movie-title")?.Txt(), a.At("title"), img?.At("alt") };
        var title = candidates.Select(c => c.NonEmpty()).FirstOrDefault(c => c != null);
        if (title == null) return null;
        var badge = el.Q1(".tick-rate, .fdi-item, .episode")?.Txt().NonEmpty();
        if (badge == null && el.Q1(".t10-rank-num")?.Txt().NonEmpty() is { } rank) badge = $"Top {rank}";
        return new Film(url, title, img != null ? ImageUrl(img) : null, badge);
    }

    public static List<Film> ParseCards(IParentNode root)
    {
        var all = root.Q(Card);
        var set = new HashSet<IElement>(all, ReferenceEqualityComparer.Instance);
        return all.Where(e => !HasAncestor(e, set)).Select(ParseCard).OfType<Film>().DistinctBy(f => f.Url).ToList();
    }

    private static string? ImageUrl(IElement img) =>
        new[] { "data-src", "data-lazy-src", "data-original", "src" }.Select(k => img.Abs(k)).FirstOrDefault(u => u.StartsWith("http"));

    /// Trang danh sách kiểu mới của hoathinh3d.pw (11/2026): HTML chỉ có khung trống, phim nạp bằng API JSON —
    /// /wp-json/halim/v1/{movies|ongoing|completed|top-rated}?page=N&amp;per_page=20 và lịch chiếu
    /// /wp-json/movie-schedule/v1/daily?day=thu-hai… Đọc địa chỉ API ngay trong mã JS của trang (API_URL).
    public static class HalimApi
    {
        public const string Path = "/wp-json/halim/v1/";
        public const int PerPage = 20;
        private static readonly Rx Api = new(@"API_URL\s*=\s*[""']([^""']*wp-json\\?/halim\\?/v1\\?/[a-z0-9-]+)[""']");
        private static readonly Rx Sort = new(@"SORTBY\s*=\s*[""']([a-z0-9_-]+)[""']");
        private static readonly Rx ScheduleRx = new(@"API_URL\s*=\s*[""']([^""']*wp-json\\?/movie-schedule\\?/v1\\?/daily)[""']");
        private static readonly Rx PageRx = new(@"[?&]page=(\d+)");

        /// Trang 1 của API trong HTML (null = không phải trang kiểu này).
        public static string? FirstPage(string html)
        {
            var a = Api.Group(1, html)?.Replace("\\/", "/");
            if (a == null) return null;
            var s = Sort.Group(1, html) is { } so ? $"sortby={so}&" : "";
            return $"{a}?{s}page=1&per_page={PerPage}";
        }

        public static string? ScheduleApi(string html) => ScheduleRx.Group(1, html)?.Replace("\\/", "/");

        /// {success, has_more, items:[{title, permalink, poster_url, latest_episode?, rank?}]}
        public static FilmPage Parse(byte[] data, string url)
        {
            var o = JsonScan.ParseObject(data);
            var items = J.Objects(J.Arr(o, "items"));
            if (o == null) return new FilmPage();
            var films = items.Select(FilmOf).OfType<Film>().DistinctBy(f => f.Url).ToList();
            var page = int.TryParse(PageRx.Group(1, url), out var p) ? p : 1;
            string? next = null;
            var m = Regex.Match(url, @"([?&]page=)\d+");
            if (J.Bool(o, "has_more") && films.Count > 0 && m.Success)
                next = url[..m.Index] + m.Groups[1].Value + (page + 1) + url[(m.Index + m.Length)..];
            return new FilmPage { Films = films, NextUrl = next };
        }

        private static Film? FilmOf(Dictionary<string, object?> o)
        {
            var url = J.Str(o, "permalink").NonEmpty();
            var title = J.Str(o, "title").NonEmpty();
            if (url == null || title == null) return null;
            var poster = J.Str(o, "poster_url").NonEmpty() ?? J.Str(o, "thumbnail").NonEmpty();
            string? badge = null;
            if (J.Str(o, "latest_episode").NonEmpty() is { } ep) badge = ep.All(char.IsDigit) ? $"Tập {ep}" : ep;
            else if (J.Int(o, "rank") > 0) badge = $"Top {J.Int(o, "rank")}";
            var early = J.Bool(o, "is_early_screening") ? J.Str(o, "early_screening_time").NonEmpty() : null;
            var b = string.Join(" · ", new[] { early, badge }.OfType<string>());
            return new Film(url, title, poster, b.Length == 0 ? null : b);
        }

        private static readonly (string Slug, string Name)[] Days =
        {
            ("chu-nhat", "Chủ nhật"), ("thu-hai", "Thứ 2"), ("thu-ba", "Thứ 3"), ("thu-tu", "Thứ 4"),
            ("thu-nam", "Thứ 5"), ("thu-sau", "Thứ 6"), ("thu-bay", "Thứ 7"),
        };

        /// Một ngày của lịch chiếu: phim chiếu sớm (kèm giờ) trước, rồi phim thường.
        public static List<Film> ParseDay(byte[] data)
        {
            var o = JsonScan.ParseObject(data);
            if (o == null) return new();
            return new[] { "early_movies", "regular_movies" }.SelectMany(k => J.Objects(J.Arr(o, k)))
                .Select(FilmOf).OfType<Film>().DistinctBy(f => f.Url).ToList();
        }

        /// Cả tuần, bắt đầu từ hôm nay (giờ Việt Nam).
        public static async Task<List<FilmSection>> Schedule(string api)
        {
            var vn = TimeZoneInfo.ConvertTimeBySystemTimeZoneId(DateTime.UtcNow, "SE Asia Standard Time");
            var today = (int)vn.DayOfWeek;
            var outList = new List<FilmSection>();
            for (var i = 0; i < 7; i++)
            {
                var (slug, name) = Days[(today + i) % 7];
                try
                {
                    var r = await Http.Get($"{api}?day={slug}");
                    var films = ParseDay(r.Data);
                    if (films.Count > 0) outList.Add(new FilmSection(name + (i == 0 ? " (hôm nay)" : ""), films));
                }
                catch (Exception)
                {
                }
            }
            return outList;
        }
    }

    // ---- Danh sách / tìm kiếm ----

    public static async Task<FilmPage> List(string url)
    {
        if (ApiSite.IsKnown(url)) return await ApiSite.List(url);
        // hoathinh3d.pw (11/2026): Phim Lẻ / Đang Chiếu / Hoàn Thành / Đánh Giá Cao nạp phim bằng API JSON
        if (url.Contains(HalimApi.Path))
            return HalimApi.Parse((await Http.Get(url, new() { ["Accept"] = "application/json" })).Data, url);
        var doc = await Fetch(url);
        var html = doc.HtmlText();
        if (HalimApi.FirstPage(html) is { } first) return await List(first);
        if (HalimApi.ScheduleApi(html) is { } api) return new FilmPage { Sections = await HalimApi.Schedule(api) };
        var page = ParseList(doc, url);
        if (page.Films.Count == 0)
        {
            var days = doc.Q(".day-tabs .day-tab[data-id]");
            if (days.Count > 0) return new FilmPage { Sections = await ScheduleOf(doc, days) };
        }
        return page;
    }

    public static FilmPage ParseList(IDocument doc, string url)
    {
        var data = NextSite.Data(doc);
        if (data != null) return NextSite.ParseList(doc, data, url);
        var main = doc.Q("#main-content, #main-contents, main").FirstOrDefault(m => ParseCards(m).Count > 0);
        // Trang lịch chiếu yanhh3d: khung chính trống, chỉ có bảng xếp hạng ở cột phụ → để List() nạp theo ngày
        List<Film> films;
        if (main != null) films = ParseCards(main);
        else if (doc.Q(".day-tabs .day-tab[data-id]").Count > 0) films = new();
        else films = ParseCards(doc);
        var next = doc.Q1(".pagination a.next[href], a.next.page-numbers[href], a[rel=next][href]")?.Abs("href");
        return new FilmPage { Films = films, NextUrl = !string.IsNullOrEmpty(next) && next != url ? next : null };
    }

    private static readonly Rx AjaxUrl = new(@"ajax_url\s*=\s*['""]([^'""]+)['""]");

    /// Trang lịch chiếu yanhh3d: mỗi ngày nạp riêng qua admin-ajax (action yan_ajax_schedule), bắt đầu từ hôm nay.
    private static async Task<List<FilmSection>> ScheduleOf(IDocument doc, List<IElement> days)
    {
        var scripts = string.Join("\n", doc.Q("script").Select(s => s.ScriptText()));
        var ajax = AjaxUrl.Group(1, scripts) ?? Resolve(doc.Location(), "/wp-admin/admin-ajax.php");
        var today = Math.Max(0, days.FindIndex(d => d.ClassList.Contains("active")));
        var ordered = days.Skip(today).Concat(days.Take(today)).ToList();
        var outList = new List<FilmSection>();
        for (var i = 0; i < ordered.Count; i++)
        {
            var day = ordered[i];
            try
            {
                var r = await Http.Post(ajax, new() { ["action"] = "yan_ajax_schedule", ["day"] = day.At("data-id") },
                    new() { ["Referer"] = doc.Location(), ["X-Requested-With"] = "XMLHttpRequest" });
                var o = JsonScan.ParseObject(r.Data);
                var html = J.Str(o, "html");
                if (html.Length == 0) continue;
                var films = ParseCards(Html.Parse(html, doc.Location()));
                if (films.Count > 0) outList.Add(new FilmSection(day.Txt() + (i == 0 ? " (hôm nay)" : ""), films));
            }
            catch (Exception)
            {
            }
        }
        return outList;
    }

    /// Trang WordPress nhận ?s=; trang Next.js tìm ở /tim-kiem (chưa biết loại trang thì thử ?s= trước).
    public static async Task<FilmPage> Search(string query, string? baseUrl = null)
    {
        var b = baseUrl ?? Prefs.BaseUrl;
        if (ApiSite.IsKnown(b)) return await ApiSite.List(ApiSite.SearchUrl(b, query));
        if (!NextSite.IsKnown(b))
        {
            var page = await List($"{b}/?s=" + query.Trim().FormEncoded());
            if (!NextSite.IsKnown(b)) return page;
        }
        return await List(NextSite.SearchUrl(b, query));
    }

    /// Tìm cùng lúc trên mọi trang trong danh sách: mỗi trang một nhóm, trang đang dùng đứng đầu; lỗi / quá 20 giây thì bỏ.
    public static async Task<FilmPage> SearchAll(string query)
    {
        var sites = new[] { Prefs.BaseUrl }.Concat(Prefs.Sites).Distinct().ToList();
        var tasks = sites.Select(b => Util.WithTimeout(20, () => Search(query, b))).ToList();
        try
        {
            await Task.WhenAll(tasks);
        }
        catch (Exception)
        {
        }
        var sections = new List<FilmSection>();
        Exception? firstError = null;
        for (var i = 0; i < sites.Count; i++)
        {
            var t = tasks[i];
            if (t.IsCompletedSuccessfully)
            {
                var films = t.Result.Films.Concat(t.Result.Sections.SelectMany(s => s.Films)).DistinctBy(f => f.Url).ToList();
                if (films.Count > 0) sections.Add(new FilmSection($"{HostOf(sites[i]) ?? sites[i]} — {films.Count} phim", films));
            }
            else firstError ??= t.Exception?.InnerException;
        }
        if (firstError != null && tasks.All(t => !t.IsCompletedSuccessfully)) throw firstError;
        return new FilmPage { Sections = sections };
    }

    // ---- Chi tiết phim ----

    public static async Task<FilmDetail> Detail(string url)
    {
        if (ApiSite.IsKnown(url))
        {
            var ad = await ApiSite.Detail(url);
            Cache(ad, url);
            return ad;
        }
        var doc = await Fetch(url);
        if (IsAppShell(doc) && await ApiSite.Probe(url)) return await Detail(url);
        var data = NextSite.Data(doc);
        var d = data != null ? await NextSite.Detail(doc, data, url) : ParseDetail(doc, url);
        Cache(d, url);
        return d;
    }

    public static FilmDetail ParseDetail(IDocument doc, string url)
    {
        var data = NextSite.Data(doc);
        if (data != null)
        {
            try
            {
                return NextSite.ParseDetail(doc, data, url);
            }
            catch (AppError)
            {
            }
        }
        var title = doc.Q1(".anisc-detail .film-name, h1.film-name, h1")?.Txt().NonEmpty() ?? Meta(doc, "og:title") ?? url;
        var other = doc.Q1(".film-name-real, .info-hero__alt, .movie-detail .original_title, h2.org_title")?.Txt().NonEmpty();
        if (other == title) other = null;
        var posterEl = doc.Q1(".anisc-poster img, .info-hero__poster img, .movie-thumb, .halim-movie-wrapper .movie-poster img");
        var poster = (posterEl != null ? ImageUrl(posterEl) : null) ?? Meta(doc, "og:image");
        var descEl = doc.Q1(".film-description .text, .film-description, .short-content, .info-hero__desc, #film-content, .film-content, .entry-content .video-item");
        var desc = descEl != null && descEl.Txt().Length > 20 ? descEl.Txt() : Meta(doc, "og:description");
        var infoParts = new List<string>();
        if (doc.Q1(".movie-status, .info-hero__status, .status-badge")?.Txt() is { } st) infoParts.Add(st);
        if (doc.Q(".anisc-info .item").FirstOrDefault(e => e.Txt().StartsWith("Thời lượng"))?.Q1(".name")?.Txt() is { } dur) infoParts.Add(dur);
        infoParts.Add(string.Join(", ", doc.Q(".item-cate a, .info-hero__tags a, .category a[rel~=tag]").Select(e => e.Txt())));
        var info = string.Join("  •  ", infoParts.Where(p => p.Length > 0).Distinct()).NonEmpty();

        var groups = ParseEpisodeGroups(doc);
        if (groups.Count == 0 && doc.Q1("a.btn-play[href], a.watch-btn[href], a.watch-movie[href]")?.Abs("href").NonEmpty() is { } play)
            groups = new() { new EpisodeGroup("Tập phim", new() { new Episode("Xem phim", play) }) };
        return new FilmDetail { Url = url, Title = title, OtherTitle = other, Poster = poster, Description = desc, Info = info, Groups = groups };
    }

    /// Danh sách tập theo nhóm, mỗi nhóm sắp tăng dần (trang web thường xếp tập mới nhất lên đầu).
    private static List<EpisodeGroup> ParseEpisodeGroups(IDocument doc)
    {
        // HaLim: mỗi <ul class="halim-list-eps"> là danh sách tập của một server (Việt Sub, Thuyết Minh…)
        var lists = doc.Q("ul.halim-list-eps");
        if (lists.Count > 0)
        {
            var tabLabels = doc.Q(".hh3d-server-tab__label, .halim-server-tab .label");
            var servers = IdSet(doc, ".halim-server");
            static string? Clean(IElement? e) => e?.Txt().Trim('#', ':', ' ').NonEmpty();
            var groups = new List<EpisodeGroup>();
            for (var i = 0; i < lists.Count; i++)
            {
                var ul = lists[i];
                var eps = EpisodesFrom(ul.Q("a[href]"));
                if (eps.Count == 0) continue;
                var name = Clean(Closest(ul, servers)?.Q1(".halim-server-name")) ?? Clean(i < tabLabels.Count ? tabLabels[i] : null) ?? $"Server {i + 1}";
                groups.Add(new EpisodeGroup(name, eps));
            }
            return groups.DistinctBy(g => string.Join("\n", g.Episodes.Select(e => e.Url))).ToList();
        }
        var links = doc.Q("a.ep-item[href]");
        if (links.Count == 0) links = doc.Q(".ss-list a[href], .episodes a[href]");
        if (links.Count == 0) links = doc.Q("a[href*='/tap-']");
        var all = EpisodesFrom(links);
        return all.Count == 0 ? new() : new() { new EpisodeGroup("Tập phim", all) };
    }

    private static readonly Rx PlainNumber = new(@"^\d+(\.\d+)?$");

    private static List<Episode> EpisodesFrom(List<IElement> links)
    {
        var eps = new List<Episode>();
        var seen = new HashSet<string>();
        foreach (var a in links)
        {
            var u = a.Abs("href");
            if (u.Length == 0 || !seen.Add(u)) continue;
            var title = a.Txt().Length == 0 ? a.At("title").Trim() : a.Txt();
            if (PlainNumber.Matches(title)) title = $"Tập {title}";
            eps.Add(new Episode(title, u));
        }
        var nums = eps.Select(e => EpNumber(e.Title) ?? EpNumber(e.Url.AfterLast('/'))).ToList();
        if (nums.Count(n => n != null) >= eps.Count * 0.8)
        {
            // Sắp theo số tập (tập không có số xuống cuối), giữ thứ tự gốc khi bằng nhau
            return Enumerable.Range(0, eps.Count)
                .OrderBy(i => nums[i] == null ? 1 : 0).ThenBy(i => nums[i] ?? 0).ThenBy(i => i)
                .Select(i => eps[i]).ToList();
        }
        eps.Reverse();
        return eps;
    }

    private static readonly Rx EpWord = new(@"(?i)t[aậ]p[\s\-_]*(\d+(?:\.\d+)?)");
    private static readonly Rx AnyNumber = new(@"(\d+(?:\.\d+)?)");

    public static double? EpNumber(string s)
    {
        var g = EpWord.Group(1, s) ?? AnyNumber.Group(1, s);
        return g != null && double.TryParse(g, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var d) ? d : null;
    }

    // ---- Trang tập phim ----

    public static async Task<EpisodeSource> Source(string epUrl)
    {
        if (ApiSite.IsKnown(epUrl)) return await ApiSite.Source(epUrl);
        var doc = await Fetch(epUrl);
        if (IsAppShell(doc) && await ApiSite.Probe(epUrl)) return await ApiSite.Source(epUrl);
        return ParseSource(doc, epUrl);
    }

    private static readonly Rx MovieInfo = new(@"MovieInfo\s*=\s*\{\s*id:\s*(\d+)");
    private static readonly Rx PlayerPhp = new(@"url:\s*['""]([^'""]*player\.php)['""]");

    public static EpisodeSource ParseSource(IDocument doc, string epUrl)
    {
        var data = NextSite.Data(doc);
        if (data != null)
        {
            try
            {
                return NextSite.ParseSource(doc, data, epUrl);
            }
            catch (AppError)
            {
            }
        }
        var scripts = string.Join("\n", doc.Q("script").Select(s => s.ScriptText()));
        var servers = doc.Q("#list_sv [data-type], .get-eps[data-type], .btn3dsv[data-type]")
            .Select(e => new Server(e.Txt(), e.At("data-type")))
            .Where(s => s.Type.Length > 0).DistinctBy(s => s.Type)
            .Select(s => s.Label.Length == 0 ? new Server(s.Type, s.Type) : s).ToList();

        var active = doc.Q1("a.ep-item.active[data-post-id], .halim-episode.active a[data-post-id], .active > a[data-post-id]")
            ?? doc.Q1("[data-post-id][data-ep]");
        var postId = active?.At("data-post-id").NonEmpty() ?? MovieInfo.Group(1, scripts) ?? "";
        var tail = epUrl.TrimEnd('/').AfterLast('/');
        if (tail.EndsWith(".html")) tail = tail[..^5];
        var ep = active?.At("data-ep").NonEmpty() ?? tail;
        var sv = active?.At("data-sv").NonEmpty() ?? "1";

        // player.php kiểu "dox_ajax_player" (yanhh3d và các trang cùng nhóm)
        string? playerUrl = null;
        if (scripts.Contains("dox_ajax_player"))
            playerUrl = PlayerPhp.Group(1, scripts) is { } p ? Resolve(epUrl, p) : Resolve(epUrl, "/player/player.php");
        // Bỏ qua iframe giữ chỗ src=""
        var direct = doc.Q("#video-player iframe, .player-frame iframe, #halim-player-wrapper iframe")
            .FirstOrDefault(f => f.At("src").StartsWith("http") || f.At("src").StartsWith("//"))?.Abs("src").NonEmpty();
        var pageMode = (playerUrl == null || servers.Count == 0 || postId.Length == 0) && direct == null;
        return new EpisodeSource
        {
            EpUrl = epUrl, Servers = servers, PostId = postId, Ep = ep, Sv = sv, PlayerUrl = playerUrl,
            DirectEmbed = direct, PageMode = pageMode,
        };
    }

    /// Hỏi player.php địa chỉ trình phát nhúng (iframe) của một server (trang Next.js / API: link .m3u8 / iframe).
    public static async Task<Video> VideoOf(EpisodeSource src, Server? server)
    {
        if (src.Next)
        {
            if (server == null) throw new AppError("Không có server");
            return await NextSite.VideoOf(src, server);
        }
        if (server == null || src.PlayerUrl == null)
        {
            if (src.DirectEmbed == null) throw new AppError("Không có server");
            return new Video(src.DirectEmbed);
        }
        var url = src.PlayerUrl + "?action=dox_ajax_player&post_id=" + src.PostId.FormEncoded() + "&chapter_st=" + src.Ep.FormEncoded() +
                  "&type=" + server.Type.FormEncoded() + "&sv=" + src.Sv.FormEncoded();
        var r = await Http.Get(url, new() { ["Referer"] = src.EpUrl, ["X-Requested-With"] = "XMLHttpRequest" });
        var doc = Html.Parse(r.Body, src.EpUrl);
        var f = doc.Q1("iframe[src]")?.Abs("src");
        if (f != null && f.StartsWith("http")) return new Video(f);
        throw new AppError($"Server \"{server.Label}\" chưa có video cho tập này");
    }

    public static string? Meta(IDocument doc, string property) =>
        doc.Q1($"meta[property=\"{property}\"], meta[name=\"{property}\"]")?.At("content").NonEmpty();

    public static string Resolve(string baseUrl, string reference)
    {
        if (Uri.TryCreate(baseUrl, UriKind.Absolute, out var b) && Uri.TryCreate(b, reference, out var u)) return u.AbsoluteUri;
        return reference;
    }

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
