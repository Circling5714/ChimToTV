using System.Globalization;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace ConBangTV.Core;

using JObj = Dictionary<string, object?>;

// Trang chủ tổng hợp (từ Windows 1.5) — bản Android: HomeFeed.kt, iPad: HomeFeed.swift (giữ cùng tên hàm để sửa song song).

/// Thẻ phim của trang chủ tổng hợp. Links: các nguồn có phim này, nguồn mở trước đứng đầu.
public sealed record Card(Film Film, string? Chip, List<FeedItem> Links);

/// Một hàng. More: địa chỉ danh sách đầy đủ (null = «Xem thêm» hiện mọi phim đã có trong Cards); Slot: loại hàng trong cài
/// đặt của người xem (HomeFeed.Rows: "similar", "interest", "newest"…; "custom-<id>" = hàng tự thêm; "topics" = bản cũ).
public sealed record FeedRow(string Id, string Title, List<Card> Cards, string? More = null, string Slot = "");

/// Hàng người xem tự thêm (Cài đặt → Trang chủ → Các hàng): chủ đề có sẵn (Topic — HomeFeed.Topics) hoặc lọc theo loại phim
/// (hoathinh / series / single / tvshows) + nước + thể loại (slug KKPhim, rỗng = mọi) + ngôn ngữ (thuyet-minh / long-tieng).
public sealed record CustomRow(string Id, string? Topic = null, string Type = "", string Country = "", string Genre = "", string Lang = "")
{
    /// Mã trong cài đặt thứ tự các hàng.
    [System.Text.Json.Serialization.JsonIgnore]
    public string Key => $"custom-{Id}";

    /// Cùng nội dung (bỏ qua Id) — khỏi thêm trùng.
    public bool Same(CustomRow o) => this with { Id = "" } == o with { Id = "" };
}

/// Phim đang xem dở vừa có tập mới (Key: tên đã chuẩn hoá; Ep: số tập mới nhất).
public sealed record FreshCard(Card Card, string Key, double Ep);

public static class HomeFeed
{
    private const long Day = 86_400_000;
    /// Phim xem trong 30 ngày qua mới được tự báo tập mới; tập mới phải ra trong 7 ngày qua.
    private const long FreshWatched = 30 * Day;
    private const long FreshNew = 7 * Day;
    /// Số phim tối đa mỗi hàng (giữ cho «Xem thêm»).
    private const int RowMax = 40;

    public enum Role { Web, New, Similar, Interest, Dominant, Dub, Kids, Movies, Custom, Tags }

    /// Một phần cần tải: vai trò + tiêu đề hàng / địa chỉ «Xem thêm» (nếu có).
    public sealed record Part(Role Role, Feed.Req Req, string? Title = null, string? More = null, string Id = "", string Slot = "")
    {
        public string Key => Req.Key;
        public string RowId => Id.Length == 0 ? Req.Key : Id;
    }

    // ---- Các hàng người xem bật / tắt, sắp thứ tự, tự thêm (Cài đặt → Trang chủ → Các hàng) ----

    /// Theo thứ tự mặc định. «Có tập mới» / «Xem tiếp» (HeadRows) chỉ bật / tắt — luôn ở đầu trang; hàng trẻ em (KidsRows)
    /// chỉ có ở hồ sơ trẻ em.
    public static readonly (string Id, string Label)[] Rows =
    {
        ("new-eps", "Có tập mới"),
        ("continue", "Xem tiếp"),
        ("similar", "Giống «phim vừa xem»"),
        ("interest", "Vì … hay xem <thể loại> (sở thích 1)"),
        ("dominant", "Loại phim hay xem · mới cập nhật"),
        ("dub", "Thuyết minh / lồng tiếng mới"),
        ("kids-young", "Hoạt hình thiếu nhi"),
        ("kids-family", "Hoạt hình gia đình"),
        ("kids-dub", "Hoạt hình lồng tiếng"),
        ("interest2", "Vì … hay xem <thể loại> (sở thích 2)"),
        ("newest", "Mới cập nhật · mọi nguồn"),
        ("movies", "Phim lẻ mới"),
    };
    public static readonly HashSet<string> HeadRows = new() { "new-eps", "continue" };
    public static readonly HashSet<string> KidsRows = new() { "kids-young", "kids-family", "kids-dub" };
    /// Hàng hồ sơ trẻ em không có (chỉ hoạt hình hợp trẻ em).
    public static readonly HashSet<string> AdultRows = new() { "dominant", "dub", "newest", "movies" };
    /// Tối đa số hàng tự thêm.
    public const int CustomMax = 6;
    /// Chủ đề hợp hồ sơ trẻ em (chỉ hoạt hình).
    public static readonly HashSet<string> KidsTopics = new() { "cn", "jp", "kid" };
    /// Loại phim / ngôn ngữ cho hàng tự thêm (type, sort_lang của KKPhim).
    public static readonly (string Id, string Label)[] Types = { ("hoathinh", "Hoạt hình"), ("series", "Phim bộ"), ("single", "Phim lẻ"), ("tvshows", "TV show") };
    public static readonly (string Id, string Label)[] Langs = { ("", "Mọi bản"), ("thuyet-minh", "Thuyết minh"), ("long-tieng", "Lồng tiếng") };
    /// Thể loại không hợp trẻ em (hàng tự thêm của hồ sơ trẻ em không chọn được).
    public static IReadOnlySet<string> AdultGenres => Adult;

    /// Tên hàng tự thêm: «Hoạt hình Trung Quốc · phiêu lưu · thuyết minh», chủ đề thì tên chủ đề.
    public static string CustomTitle(CustomRow r)
    {
        if (r.Topic is { } t) return Topics.FirstOrDefault(x => x.Id == t).Label ?? t;
        var head = Types.FirstOrDefault(x => x.Id == r.Type).Label ?? "Phim";
        if (r.Country.Length > 0) head += " " + ApiSite.CountryName(r.Country);
        var parts = new List<string> { head };
        if (r.Genre.Length > 0) parts.Add(ApiSite.GenreName(r.Genre));
        if (r.Lang.Length > 0 && Langs.FirstOrDefault(x => x.Id == r.Lang).Label is { } l) parts.Add(l.ToLowerInvariant());
        return string.Join(" · ", parts);
    }

    /// Thứ tự mặc định: «Có tập mới», «Xem tiếp», các hàng tự thêm (theo lúc thêm), rồi các hàng app tự chọn.
    public static List<string> RowDefaults(List<CustomRow>? custom = null) =>
        Rows.Take(2).Select(x => x.Id).Concat((custom ?? Prefs.HomeCustom).Select(c => c.Key)).Concat(Rows.Skip(2).Select(x => x.Id)).ToList();

    /// Ghép thứ tự đã lưu ("id" bật, "-id" tắt) với các hàng mặc định: bỏ id lạ / trùng, hàng mặc định chưa có (hàng mới ở bản
    /// sau, hàng vừa tự thêm) chèn ngay sau hàng đứng trước nó trong thứ tự mặc định.
    public static List<(string Id, bool On)> MergeRows(List<string> saved, List<string>? defaults = null)
    {
        defaults ??= Rows.Select(x => x.Id).ToList();
        var o = new List<(string Id, bool On)>();
        foreach (var s in saved)
        {
            var on = !s.StartsWith('-');
            var id = on ? s : s[1..];
            if (defaults.Contains(id) && o.All(x => x.Id != id)) o.Add((id, on));
        }
        for (var i = 0; i < defaults.Count; i++)
        {
            var id = defaults[i];
            if (o.Any(x => x.Id == id)) continue;
            var prev = defaults.Take(i).LastOrDefault(p => o.Any(x => x.Id == p));
            var at = prev == null ? 0 : o.FindIndex(x => x.Id == prev) + 1;
            o.Insert(at, (id, true));
        }
        return o;
    }

    /// Các hàng của người xem đang dùng (thứ tự, bật / tắt), kể cả hàng tự thêm.
    public static List<(string Id, bool On)> RowPrefs() => MergeRows(Prefs.HomeRows, RowDefaults());

    public static bool RowOn(string id)
    {
        foreach (var p in RowPrefs()) if (p.Id == id) return p.On;
        return true;
    }

    /// Loại hàng của một hàng lưu từ bản cũ (chưa ghi Slot): theo id; hai hàng «Vì … hay xem» theo thứ tự xuất hiện.
    private static string LegacySlot(string id, int interestBefore) =>
        id.StartsWith("interest-") ? (interestBefore == 0 ? "interest" : "interest2") : id.StartsWith("topic-") ? "topics" : id;

    /// Xếp các hàng theo cài đặt của người xem: bỏ hàng đã tắt, theo thứ tự đã chọn; hàng chủ đề bản cũ ("topics") đầu, hàng
    /// lạ cuối.
    public static List<FeedRow> Arrange(List<FeedRow> rows, List<(string Id, bool On)>? prefs = null) =>
        ArrangeSlots(rows, prefs).Select(x => x.Row).ToList();

    /// Như Arrange, kèm loại hàng (id trong cài đặt) của từng hàng — kể cả hàng lưu từ bản cũ (chưa ghi Slot).
    public static List<(FeedRow Row, string Slot)> ArrangeSlots(List<FeedRow> rows, List<(string Id, bool On)>? prefs = null)
    {
        prefs ??= RowPrefs();
        var pos = new Dictionary<string, int>();
        for (var i = 0; i < prefs.Count; i++) pos[prefs[i].Id] = prefs[i].On ? i : -1;
        var interest = 0;
        var keyed = new List<(FeedRow Row, string Slot, int Index)>();
        foreach (var r in rows)
        {
            var slot = r.Slot.Length == 0 ? LegacySlot(r.Id, interest) : r.Slot;
            if (slot.StartsWith("interest")) interest++;
            keyed.Add((r, slot, keyed.Count));
        }
        int Rank(string slot) => slot == "topics" ? -1 : pos.TryGetValue(slot, out var p) ? p : int.MaxValue;
        return keyed.Where(x => !pos.TryGetValue(x.Slot, out var p) || p != -1)
            .OrderBy(x => Rank(x.Slot)).ThenBy(x => x.Index).Select(x => (x.Row, x.Slot)).ToList();
    }

    /// Lưu cài đặt các hàng ("id" bật, "-id" tắt).
    public static void SaveRows(List<(string Id, bool On)> prefs) =>
        Prefs.HomeRows = prefs.Select(p => p.On ? p.Id : "-" + p.Id).ToList();

    /// Tắt một hàng (sửa nhanh trên trang chủ: «Ẩn hàng này»).
    public static List<(string Id, bool On)> Hide(List<(string Id, bool On)> prefs, string slot) =>
        prefs.Select(p => p.Id == slot ? (slot, false) : p).ToList();

    /// Sửa nhanh trên trang chủ: chuyển hàng slot lên / xuống qua hàng đang hiện kề nó (shown = loại hàng đang hiện, theo thứ
    /// tự trên trang — hàng tắt / không đủ phim nằm giữa bị vượt qua). null: không chuyển được (hàng đầu / cuối, «Có tập
    /// mới» / «Xem tiếp», hàng chủ đề bản cũ).
    public static List<(string Id, bool On)>? MoveShown(List<(string Id, bool On)> prefs, List<string> shown, string slot, bool up)
    {
        bool Fixed(string s) => HeadRows.Contains(s) || s == "topics";
        var i = shown.IndexOf(slot);
        if (i < 0 || Fixed(slot)) return null;
        var j = up ? i - 1 : i + 1;
        if (j < 0 || j >= shown.Count || Fixed(shown[j])) return null;
        var other = shown[j];
        var output = prefs.ToList();
        var at = output.FindIndex(p => p.Id == slot);
        if (at < 0) return null;
        var me = output[at];
        output.RemoveAt(at);
        var k = output.FindIndex(p => p.Id == other);
        if (k < 0) return null;
        output.Insert(up ? k : k + 1, me);
        return output;
    }

    /// Chủ đề cho người xem chưa có lịch sử (id lưu trong Prefs.HomeTopics).
    public static readonly (string Id, string Label)[] Topics =
    {
        ("cn", "Hoạt hình Trung Quốc"), ("kh", "Kiếm hiệp"), ("kr", "Phim Hàn"), ("mv", "Phim lẻ Âu Mỹ"),
        ("hk", "Phim xưa Hồng Kông"), ("kid", "Hoạt hình cho bé"), ("jp", "Anime Nhật"), ("tm", "Thuyết minh / lồng tiếng"),
    };

    /// Nguồn trong danh sách trang (gốc địa chỉ); Web: trang web đang dùng + trang hay xem gần đây.
    public sealed record Sources(string? Kk, string? Nc, string? Vs, string? K20, List<string> Web)
    {
        public int Count => new[] { Kk, Nc, Vs, K20 }.Count(x => x != null) + Web.Count;

        /// id (trong HomeFeed.SourceNames) của các nguồn có mặt.
        public List<string> Ids =>
            new[] { Kk != null ? "kk" : null, Nc != null ? "nc" : null, Vs != null ? "vs" : null, K20 != null ? "k20" : null, Web.Count > 0 ? "web" : null }
                .OfType<string>().ToList();
    }

    /// Nguồn của trang chủ tổng hợp người xem bật / tắt được (Cài đặt → «Nguồn cho trang chủ…», riêng từng người; khoá
    /// Prefs.HomeSrcOff), theo thứ tự hiện. «web» = trang đang dùng + trang xem gần đây.
    public static readonly (string Id, string Label)[] SourceNames =
    {
        ("kk", "KKPhim"), ("nc", "NguonC"), ("vs", "VSMOV"), ("k20", "CLB Phim Xưa"), ("web", "Trang đang dùng và trang xem gần đây"),
    };

    /// Bỏ các nguồn người xem đã tắt. Tắt hết các nguồn đang có (vd nguồn còn bật vừa bị xoá khỏi danh sách trang) thì coi như
    /// bật hết — trang chủ không bao giờ trống vì cài đặt nguồn.
    public static Sources WithoutOff(Sources all, ICollection<string> off)
    {
        if (all.Ids.All(off.Contains)) return all;
        return new Sources(off.Contains("kk") ? null : all.Kk, off.Contains("nc") ? null : all.Nc, off.Contains("vs") ? null : all.Vs,
            off.Contains("k20") ? null : all.K20, off.Contains("web") ? new() : all.Web);
    }

    /// Nguồn trang chủ của người xem đang dùng (đã bỏ nguồn tắt).
    public static Sources GetSources() => WithoutOff(AllSources(), Prefs.HomeSrcOff);

    /// Mọi nguồn có trong danh sách trang (cả nguồn người xem đã tắt).
    public static Sources AllSources()
    {
        var sites = Prefs.Sites;
        var baseUrl = Prefs.BaseUrl;
        string? Find(Func<string, bool> f) => sites.FirstOrDefault(f) is { } s ? ApiSite.Origin(s) : null;
        var kk = Find(ApiSite.IsKk);
        var nc = Find(NguonC.IsSite);
        var vs = Find(ApiSite.IsVs);
        var k20 = Find(Stremio.IsSite);
        var api = new[] { kk, nc, vs, k20 }.OfType<string>().Select(Site.HostOf).OfType<string>().ToHashSet();
        var now = Util.NowMs();
        var recent = Prefs.History.Where(h => now - h.Time < FreshWatched).Select(h => Site.HostOf(h.FilmUrl)).OfType<string>().Distinct();
        // Trang web đọc HTML: tối đa 2 trang (trang đang dùng + trang xem gần nhất)
        var web = new[] { baseUrl }.Concat(recent.Select(h => sites.FirstOrDefault(s => Site.HostOf(s) == h)).OfType<string>())
            .Where(s => Site.HostOf(s) is { } h && !api.Contains(h)).DistinctBy(s => Site.HostOf(s)).Take(2).ToList();
        return new Sources(kk, nc, vs, k20, web);
    }

    // ---- Sở thích ----

    /// Thể loại quá chung (không dùng để gợi ý); chinh-kich / tam-ly tính nửa điểm.
    private static readonly HashSet<string> Generic = new() { "hoat-hinh", "phim-18", "tv-shows", "phim-bo", "phim-le" };
    private static readonly HashSet<string> Half = new() { "chinh-kich", "tam-ly" };
    /// Không hợp trẻ em (hồ sơ trẻ em bỏ các phim này và mọi phim không phải hoạt hình).
    private static readonly HashSet<string> Adult = new() { "phim-18", "kinh-di", "hinh-su", "chien-tranh" };
    /// Tên phim ma / kinh dị mà nguồn không gắn thể loại.
    private static readonly Regex Scary = new("(?i)horror|kinh dị|xác sống|zombie|ma quỷ|quỷ dữ|ác quỷ|kyoufu");

    public sealed record Combo(string Type, string Country, string Genre);

    /// Điểm sở thích của một người xem.
    public sealed class Taste
    {
        public Dictionary<string, double> Genre = new();
        public Dictionary<Combo, double> Combos = new();
        public Dictionary<(string Type, string Country), double> Pair = new();
        public double Total;

        public double Affinity(Tags? t)
        {
            if (t == null) return 0;
            var p = Pair.GetValueOrDefault((t.Type ?? "", t.Countries.FirstOrDefault() ?? ""));
            return t.Genres.Sum(g => Genre.GetValueOrDefault(g)) + p;
        }
    }

    /// Điểm của mỗi phim = số tập đã xem (1–20) × ½ mũ (số ngày từ lần xem cuối ÷ 14), cộng vào thể loại, cặp (loại, nước)
    /// và bộ ba (loại, nước, thể loại). Phim chưa biết thể loại thì bỏ qua.
    public static Taste GetTaste(IEnumerable<HistoryItem> history, Func<string, Tags?> tagsOf, Func<string, int> watched, long now)
    {
        var t = new Taste();
        foreach (var h in history)
        {
            var days = Math.Max(0, now - h.Time) / (double)Day;
            if (days > 120 || tagsOf(h.Title) is not { } tags) continue;
            var w = Math.Clamp(watched(h.FilmUrl), 1, 20) * Math.Pow(0.5, days / 14);
            t.Total += w;
            var type = tags.Type ?? "";
            var country = tags.Countries.FirstOrDefault() ?? "";
            t.Pair[(type, country)] = t.Pair.GetValueOrDefault((type, country)) + w;
            foreach (var g in tags.Genres.Where(g => !Generic.Contains(g)))
            {
                var gw = Half.Contains(g) ? w / 2 : w;
                t.Genre[g] = t.Genre.GetValueOrDefault(g) + gw;
                if (type.Length > 0 && country.Length > 0)
                {
                    var c = new Combo(type, country, g);
                    t.Combos[c] = t.Combos.GetValueOrDefault(c) + gw;
                }
            }
        }
        return t;
    }

    private static string? TypeSlug(string type) => type switch
    {
        "hoathinh" => "hoat-hinh",
        "series" => "phim-bo",
        "single" => "phim-le",
        "tvshows" => "tv-shows",
        _ => null,
    };

    public static string TypeName(string type) => type switch
    {
        "hoathinh" => "hoạt hình",
        "series" => "phim bộ",
        "single" => "phim lẻ",
        "tvshows" => "TV show",
        _ => "phim",
    };

    // ---- Lập danh sách cần tải ----

    /// Pid: người xem lúc lập kế hoạch; Kids: hồ sơ trẻ em.
    public sealed record Plan(List<Part> Parts, Sources Src, HistoryItem? Similar = null, Tags? SimilarTags = null, Taste? Taste = null,
        HashSet<string>? Topics = null, string Pid = "", bool Kids = false);

    private static readonly Dictionary<string, string> JsonHeader = new() { ["Accept"] = "application/json" };

    private static async Task<JObj> GetRoot(string url) =>
        JsonScan.ParseObject((await Http.Get(url, JsonHeader)).Data) ?? throw new AppError("Nguồn trả dữ liệu lạ");

    /// Lập kế hoạch tải cho người xem đang dùng (đọc Prefs, không chạm mạng).
    public static Plan MakePlan(bool tagsKnownOnly = false)
    {
        var s = GetSources();
        var kids = Prefs.Kids;
        var parts = new List<Part>();
        var now = Util.NowMs();
        var history = Prefs.History;
        string HostOf(string o) => Site.HostOf(o) ?? "";
        // Hàng người xem đã tắt: khỏi tải. «Mới cập nhật» vẫn tải danh sách mới của các nguồn — cần để gộp phim, tìm «Có tập mới»
        var off = RowPrefs().Where(x => !x.On).Select(x => x.Id).ToHashSet();

        // Hồ sơ trẻ em: không lấy danh sách «mới cập nhật» chung, chỉ các danh sách hoạt hình
        if (!kids)
        {
            if (s.Kk is { } kkO)
                parts.Add(new Part(Role.New, new Feed.Req($"new|{kkO}", HostOf(kkO),
                    async () => new Feed.Loaded(Feed.ParseKk(await ApiSite.Json($"{kkO}/api/movies/newest?page=1&limit=40"), kkO)))));
            if (s.Nc is { } ncO)
                parts.Add(new Part(Role.New, new Feed.Req($"new|{ncO}", HostOf(ncO),
                    async () => new Feed.Loaded(Feed.ParseNc(await GetRoot(NguonC.NewestUrl(ncO)), ncO)))));
            if (s.Vs is { } vsO)
                parts.Add(new Part(Role.New, new Feed.Req($"new|{vsO}", HostOf(vsO),
                    async () => new Feed.Loaded(Feed.ParseKk(await ApiSite.Json($"{vsO}/api/danh-sach/phim-moi-cap-nhat?page=1"), vsO)))));
            // Trang web (trang đang dùng trước): phim mới cập nhật + menu mục của trang
            foreach (var w in s.Web)
            {
                parts.Add(new Part(Role.Web, new Feed.Req($"web|{w}", HostOf(w), async () =>
                {
                    var h = await Site.HomeOf(w);
                    return new Feed.Loaded(Feed.FromFilms(NewestOf(h)).Take(RowMax).ToList(), h.Categories, h.MovedTo);
                })));
            }
            if (s.K20 is { } k20O)
                parts.Add(new Part(Role.New, new Feed.Req($"new|{k20O}", HostOf(k20O),
                    async () => new Feed.Loaded(Feed.FromFilms((await Stremio.List(Stremio.NewestUrl(k20O))).Films)))));
        }

        // Thể loại của phim đã xem chưa biết: tra KKPhim (mỗi lượt tối đa 6 phim; không thấy thì 3 ngày sau mới tra lại)
        if (!tagsKnownOnly && (s.Kk != null || Prefs.KkBackup))
        {
            foreach (var h in history.Where(h => now - h.Time < 120 * Day).DistinctBy(h => KkPhim.Norm(h.Title))
                         .Where(h => FilmTags.NeedsLookup(h.Title)).Take(6))
            {
                var title = h.Title;
                parts.Add(new Part(Role.Tags, new Feed.Req($"tags|{KkPhim.Norm(title)}", "phimapi.com", async () =>
                {
                    await FilmTags.Lookup(title);
                    return new Feed.Loaded(new());
                }, Keep: false)));
            }
        }

        var counts = Prefs.WatchedCounts();
        var taste = history.Count == 0 ? null : GetTaste(history, FilmTags.Of, u => counts.GetValueOrDefault(u), now);
        var topics = Prefs.HomeTopics;
        HistoryItem? similar = null;
        Tags? similarTags = null;
        (string Type, string Country)? top = null;
        if (taste != null && taste.Total > 0)
        {
            // Giống «phim vừa xem»: cùng loại phim, nước và thể loại riêng nhất (trẻ em: chỉ phim hoạt hình)
            similar = history.FirstOrDefault(h => FilmTags.Of(h.Title) is { } t && t.Type != null && t.Countries.Count > 0 && (!kids || t.Type == "hoathinh"));
            similarTags = similar != null ? FilmTags.Of(similar.Title) : null;
            var used = new HashSet<string>();
            if (similar != null && similarTags != null)
            {
                var g = similarTags.Genres.Where(x => !Generic.Contains(x) && !(kids && Adult.Contains(x))).OrderBy(x => Half.Contains(x) ? 1 : 0).FirstOrDefault();
                if (FilterUrl(s, similarTags.Type!, similarTags.Countries[0], g) is { } url)
                {
                    if (g != null) used.Add(g);
                    if (!off.Contains("similar")) parts.Add(new Part(Role.Similar, Req(url), $"Giống «{similar.Title}»", url, "similar", "similar"));
                }
            }
            // Vì … hay xem <thể loại>: hai bộ ba điểm cao nhất, khác thể loại với hàng trên
            var n = 0;
            foreach (var cb in taste.Combos.OrderByDescending(kv => kv.Value).Select(kv => kv.Key))
            {
                if (n == 2) break;
                if (used.Contains(cb.Genre) || Half.Contains(cb.Genre)) continue;
                if (kids && (cb.Type != "hoathinh" || Adult.Contains(cb.Genre))) continue;
                if (FilterUrl(s, cb.Type, cb.Country, cb.Genre) is not { } url) continue;
                used.Add(cb.Genre);
                n++;
                var slot = n == 1 ? "interest" : "interest2";
                if (off.Contains(slot)) continue;
                var title = $"Vì {Prefs.CurrentProfile.Name} hay xem {ApiSite.GenreName(cb.Genre)} · {TypeName(cb.Type)} {ApiSite.CountryName(cb.Country)}";
                parts.Add(new Part(Role.Interest, Req(url), title, url, $"interest-{cb.Genre}", slot));
            }
            // Loại phim + nước xem nhiều nhất (≥ 40%): hàng mới cập nhật riêng, gộp mọi nguồn
            var best = taste.Pair.Where(kv => kv.Key.Type.Length > 0 && kv.Key.Country.Length > 0).OrderByDescending(kv => kv.Value).FirstOrDefault();
            if (best.Key.Type != null && best.Value >= taste.Total * 0.4) top = best.Key;
            if (top is { } tp && !kids && !off.Contains("dominant"))
            {
                var tn = TypeName(tp.Type);
                var title = $"{char.ToUpper(tn[0])}{tn[1..]} {ApiSite.CountryName(tp.Country)} mới cập nhật · mọi nguồn";
                if (FilterUrl(s, tp.Type, tp.Country, null) is { } url) parts.Add(new Part(Role.Dominant, Req(url), title, null, "dominant", "dominant"));
            }
        }
        // Hàng tự thêm (chủ đề người mới chọn cũng là hàng tự thêm — vẫn còn khi đã có lịch sử). Hồ sơ trẻ em: chỉ hoạt hình
        foreach (var c in Prefs.HomeCustom)
        {
            if (off.Contains(c.Key)) continue;
            if (kids && (c.Topic != null ? !KidsTopics.Contains(c.Topic) : c.Type != "hoathinh")) continue;
            var found = c.Topic != null ? TopicReq(s, c.Topic) : CustomReq(s, c);
            if (found is not { } tr) continue;
            parts.Add(new Part(Role.Custom, tr.Req, CustomTitle(c), tr.More, c.Key, c.Key));
        }
        if (kids)
        {
            parts.AddRange(KidsParts(s).Where(p => !off.Contains(p.Slot)));
        }
        else
        {
            if (!off.Contains("dub") && DubPart(s, top) is { } d) parts.Add(d);
            // Phim lẻ mới (xếp theo thể loại người xem hay xem)
            if (off.Contains("movies"))
            {
                // người xem đã tắt hàng này
            }
            else if (s.Kk != null)
            {
                var u = $"{s.Kk}/api/movies/category/phim-le?page=1&limit=30";
                parts.Add(new Part(Role.Movies, Req(u), "Phim lẻ mới", u, "movies", "movies"));
            }
            else if (s.Vs != null)
            {
                var u = $"{s.Vs}/api/danh-sach/phim-le?page=1";
                parts.Add(new Part(Role.Movies, Req(u), "Phim lẻ mới", u, "movies", "movies"));
            }
        }
        return new Plan(parts, s, similar, similarTags, taste, topics, Prefs.ProfileId, kids);
    }

    /// Người hay xem bản thuyết minh / lồng tiếng (từ nửa số lần mở tập gần đây, ít nhất 5 lần): hàng riêng.
    private static Part? DubPart(Sources s, (string Type, string Country)? top)
    {
        var langs = Prefs.WatchLangs;
        var tm = langs.Count(l => l == "tm");
        var lt = langs.Count(l => l == "lt");
        if (langs.Count < 5 || (tm + lt) * 2 < langs.Count) return null;
        var slug = lt > tm ? "long-tieng" : "thuyet-minh";
        var name = lt > tm ? "Lồng tiếng" : "Thuyết minh";
        if (s.Kk is { } o)
        {
            if (top is { } tp && TypeSlug(tp.Type) is { } t)
            {
                var url = $"{o}/api/movies/category/{t}?country={tp.Country}&sort_lang={slug}&page=1&limit=30";
                return new Part(Role.Dub, Req(url), $"{name} mới · {TypeName(tp.Type)} {ApiSite.CountryName(tp.Country)}", url, "dub");
            }
            var u = $"{o}/api/movies/category/phim-{slug}?page=1&limit=30";
            return new Part(Role.Dub, Req(u), $"Phim {name.ToLowerInvariant()} mới", u, "dub");
        }
        if (s.Nc is { } nc)
        {
            var url = $"{nc}/api/films/ngon-ngu/{slug}?page=1";
            return new Part(Role.Dub, new Feed.Req($"list|{url}", Site.HostOf(nc) ?? "", async () => new Feed.Loaded(Feed.ParseNc(await GetRoot(url), nc))),
                $"Phim {name.ToLowerInvariant()} mới", url, "dub");
        }
        return null;
    }

    /// Hồ sơ trẻ em: hoạt hình thiếu nhi (KKPhim «trẻ em» + VSMOV «thiếu nhi»), gia đình, lồng tiếng.
    private static List<Part> KidsParts(Sources s)
    {
        var o = new List<Part>();
        if (s.Kk != null)
        {
            var u = $"{s.Kk}/api/movies/category/hoat-hinh?category=tre-em&page=1&limit=30";
            o.Add(new Part(Role.Kids, Req(u), "Hoạt hình thiếu nhi", u, "kids-young", "kids-young"));
        }
        if (s.Vs != null)
        {
            var u = $"{s.Vs}/api/the-loai/thieu-nhi?page=1";
            o.Add(new Part(Role.Kids, Req(u), "Hoạt hình thiếu nhi", u, "kids-young", "kids-young"));
        }
        if (s.Kk != null)
        {
            var fam = $"{s.Kk}/api/movies/category/hoat-hinh?category=gia-dinh&page=1&limit=30";
            o.Add(new Part(Role.Kids, Req(fam), "Hoạt hình gia đình", fam, "kids-family", "kids-family"));
            var dub = $"{s.Kk}/api/movies/category/hoat-hinh?sort_lang=long-tieng&page=1&limit=30";
            o.Add(new Part(Role.Kids, Req(dub), "Hoạt hình lồng tiếng", dub, "kids-dub", "kids-dub"));
        }
        return o;
    }

    /// Danh sách lọc theo loại phim + nước (+ thể loại): KKPhim lọc được cả ba; không có KKPhim thì VSMOV.
    private static string? FilterUrl(Sources s, string type, string country, string? genre)
    {
        if (s.Kk is { } o && TypeSlug(type) is { } t)
            return $"{o}/api/movies/category/{t}?country={country}" + (genre != null ? $"&category={genre}" : "") + "&page=1&limit=30";
        if (s.Vs is { } v)
        {
            var p = genre != null ? $"the-loai/{genre}?country={country}" : type == "hoathinh" ? $"the-loai/hoat-hinh?country={country}" : $"quoc-gia/{country}";
            return Page($"{v}/api/{p}");
        }
        return null;
    }

    private static string Page(string url) => url + (url.Contains('?') ? "&" : "?") + "page=1";

    /// Danh sách của một hàng tự thêm: KKPhim lọc được loại + nước + thể loại + ngôn ngữ; không có KKPhim thì VSMOV (thể loại /
    /// nước / loại, không lọc ngôn ngữ). (yêu cầu, địa chỉ «Xem thêm»).
    private static (Feed.Req Req, string More)? CustomReq(Sources s, CustomRow c)
    {
        var q = new List<string>();
        if (c.Country.Length > 0) q.Add($"country={c.Country}");
        if (c.Genre.Length > 0) q.Add($"category={c.Genre}");
        if (c.Lang.Length > 0) q.Add($"sort_lang={c.Lang}");
        if (s.Kk is { } o && TypeSlug(c.Type) is { } t)
        {
            var url = $"{o}/api/movies/category/{t}?" + string.Join("&", q.Concat(new[] { "page=1", "limit=30" }));
            return (Req(url), url);
        }
        if (s.Vs is { } v)
        {
            var cq = c.Country.Length > 0 ? $"?country={c.Country}" : "";
            string? p = c.Genre.Length > 0 ? $"the-loai/{c.Genre}{cq}"
                : c.Type == "hoathinh" ? $"the-loai/hoat-hinh{cq}"
                : c.Country.Length > 0 ? $"quoc-gia/{c.Country}"
                : TypeSlug(c.Type) is { } ts ? $"danh-sach/{ts}" : null;
            if (p == null) return null;
            var url = Page($"{v}/api/{p}");
            return (Req(url), url);
        }
        return null;
    }

    /// Đọc một danh sách KKPhim / VSMOV theo địa chỉ (định dạng KKPhim).
    private static Feed.Req Req(string url)
    {
        var o = ApiSite.Origin(url);
        return new Feed.Req($"list|{url}", Site.HostOf(o) ?? "", async () => new Feed.Loaded(Feed.ParseKk(await ApiSite.Json(url), o)));
    }

    /// Danh sách cho một chủ đề: nguồn đầu tiên có trong danh sách trang. (yêu cầu, địa chỉ «Xem thêm»).
    private static (Feed.Req Req, string More)? TopicReq(Sources s, string id)
    {
        string? Kk(string p) => s.Kk != null ? Page($"{s.Kk}/api/movies/{p}") + "&limit=30" : null;
        string? Vs(string p) => s.Vs != null ? Page($"{s.Vs}/api/{p}") : null;
        var url = id switch
        {
            "cn" => Kk("category/hoat-hinh?country=trung-quoc") ?? Vs("the-loai/hoat-hinh?country=trung-quoc"),
            "kh" => Vs("the-loai/kiem-hiep") ?? (s.K20 != null ? Stremio.Catalog(s.K20, genre: "Kiếm Hiệp") : null),
            "kr" => Kk("category/phim-bo?country=han-quoc") ?? Vs("quoc-gia/han-quoc"),
            "mv" => Kk("category/phim-le?country=au-my") ?? Vs("danh-sach/phim-le"),
            "hk" => (s.K20 != null ? Stremio.NewestUrl(s.K20) : null) ?? Kk("quoc-gia/hong-kong"),
            "kid" => Vs("the-loai/thieu-nhi") ?? Kk("category/hoat-hinh?category=gia-dinh"),
            "jp" => Kk("category/hoat-hinh?country=nhat-ban") ?? Vs("the-loai/hoat-hinh?country=nhat-ban"),
            "tm" => Kk("category/phim-thuyet-minh") ?? (s.Nc != null ? $"{s.Nc}/api/films/ngon-ngu/thuyet-minh?page=1" : null),
            _ => null,
        };
        if (url == null) return null;
        var o = ApiSite.Origin(url);
        var req = new Feed.Req($"list|{url}", Site.HostOf(url) ?? "", async () =>
        {
            if (Stremio.IsSite(url)) return new Feed.Loaded(Feed.FromFilms((await Stremio.List(url)).Films));
            if (NguonC.IsSite(url)) return new Feed.Loaded(Feed.ParseNc(await GetRoot(url), o));
            return new Feed.Loaded(Feed.ParseKk(await ApiSite.Json(url), o));
        });
        return (req, url);
    }

    private static readonly Regex NewRx = new("(?i)mới|cập nhật|update|new");

    /// Khối «mới cập nhật» của trang web (không có thì khối lớn nhất / mọi khối).
    private static List<Film> NewestOf(Home h)
    {
        var pick = h.Sections.Where(x => NewRx.IsMatch(x.Title)).ToList();
        if (pick.Count == 0) pick = h.Sections.Where(x => x.Films.Count >= 6).Take(1).ToList();
        if (pick.Count == 0) pick = h.Sections;
        return pick.SelectMany(x => x.Films).DistinctBy(f => f.Url).ToList();
    }

    // ---- Dựng trang ----

    /// Một lần dựng trang chủ. Note: dòng trạng thái; NoteGaveUp: khi thôi chờ nguồn chậm; Pending: còn nguồn chậm có thể về
    /// muộn — Late() dựng lại khi chúng về.
    public sealed class Snapshot
    {
        public List<FeedRow> Rows = new();
        public List<FreshCard> Fresh = new();
        public List<Category>? Categories;
        public (string Old, string New)? MovedTo;
        public string Note = "", NoteGaveUp = "";
        public bool Empty, FromCache, Pending;
        public List<(Part Part, Feed.Result Result)> Results = new();
        public Plan? Plan;
        public long StartedAt;
    }

    /// Nguồn chậm (quá hạn / chưa tới lượt) — khác nguồn lỗi hẳn: có thể về muộn.
    private static bool Slow(Feed.Result r) => r.State != Feed.State.Ok && !r.Resting && (r.Error == null || r.Error.StartsWith("quá"));

    /// Nguồn chậm của lần dựng trước đã về (bản lưu mới hơn lúc bắt đầu tải): dựng lại; chưa có gì mới thì null.
    public static Snapshot? Late(Snapshot s)
    {
        if (s.Plan is not { } plan) return null;
        var changed = false;
        var res = s.Results.Select(pr =>
        {
            var (p, r) = pr;
            if (p.Role == Role.Tags || !Slow(r) || !p.Req.Keep || Feed.ReadCache(p.Key) is not { } c || c.At < s.StartedAt) return pr;
            changed = true;
            return (p, new Feed.Result(p.Req, Feed.State.Ok, c.Loaded.Items, c.Loaded.Categories, null, false, r.Ms, c.At));
        }).ToList();
        if (!changed) return null;
        var snap = Build(plan, res, false, s.StartedAt);
        SaveLast(plan.Pid, snap);
        return snap;
    }

    /// Bản lưu của mọi phần; null nếu chưa có bản lưu nào.
    public static Snapshot? Cached(Plan plan)
    {
        var results = plan.Parts.Select(p => Feed.ReadCache(p.Key) is { } c
            ? (p, new Feed.Result(p.Req, Feed.State.Cached, c.Loaded.Items, c.Loaded.Categories, null, false, 0, c.At))
            : (p, new Feed.Result(p.Req, Feed.State.NoData, new(), null, null, false, 0, 0))).ToList();
        return results.Any(x => x.p.Role != Role.Tags && x.Item2.Items.Count > 0) ? Build(plan, results, true) : null;
    }

    /// Tải mọi phần rồi dựng trang. Lượt đầu chưa biết thể loại phim đã xem: tra xong thì lập lại kế hoạch, tải thêm hàng gợi ý.
    public static async Task<Snapshot> Load(Plan plan)
    {
        var started = Util.NowMs();
        var p = plan;
        var results = p.Parts.Zip(await Feed.Load(p.Parts.Select(x => x.Req).ToList()), (a, b) => (a, b)).ToList();
        if (p.Parts.Any(x => x.Role == Role.Tags))
        {
            var again = MakePlan(true);
            var have = results.Select(x => x.a.Key).ToHashSet();
            var extra = again.Parts.Where(x => !have.Contains(x.Key)).ToList();
            if (extra.Count > 0) results.AddRange(extra.Zip(await Feed.Load(extra.Select(x => x.Req).ToList()), (a, b) => (a, b)));
            var byKey = results.ToDictionary(x => x.a.Key, x => x.b);
            results = again.Parts.Where(a => byKey.ContainsKey(a.Key)).Select(a => (a, byKey[a.Key]))
                .Concat(results.Where(x => x.a.Role == Role.Tags)).ToList();
            p = again;
        }
        var snap = Build(p, results.Select(x => (x.a, x.b)).ToList(), false, started);
        SaveLast(p.Pid, snap);
        return snap;
    }

    /// Dữ liệu người xem dùng khi dựng trang (tách khỏi Prefs để test).
    public sealed record Ctx(List<HistoryItem> History, HashSet<string> Follows, Func<string, double?> Seen, string Base, List<string> Sites, long Now);

    public static Snapshot Build(Plan plan, List<(Part Part, Feed.Result Result)> results, bool fromCache, long startedAt = 0)
    {
        var now = Util.NowMs();
        var ctx = new Ctx(Prefs.History, Prefs.Follows.Select(f => f.FilmUrl).ToHashSet(), Prefs.SeenEp, Site.HostOf(Prefs.BaseUrl) ?? "",
            Prefs.Sites, now);
        var (rows, fresh) = BuildRows(plan, results, ctx);
        var webBase = results.FirstOrDefault(x => x.Part.Role == Role.Web && x.Part.Key == $"web|{Prefs.BaseUrl}").Result;
        var cats = webBase?.Categories ?? Site.Menu(Prefs.BaseUrl);
        var moved = results.Where(x => x.Part.Role == Role.Web && x.Result.State == Feed.State.Ok && x.Result.MovedTo != null)
            .Select(x => ((string Old, string New)?)(x.Part.Key["web|".Length..], x.Result.MovedTo!)).FirstOrDefault();
        var content = fromCache ? new() : results.Where(x => x.Part.Role != Role.Tags).ToList();
        string Names(IEnumerable<(Part Part, Feed.Result Result)> l) =>
            string.Join(", ", l.Select(x => x.Part.Req.Host).Distinct().Select(h => Site.Tag(h, ctx.Sites)));
        var slow = content.Where(x => Slow(x.Result)).ToList();
        var failed = content.Where(x => x.Result.State != Feed.State.Ok && !Slow(x.Result) && !x.Result.Resting).ToList();
        var rest = content.Where(x => x.Result.Resting).ToList();
        var time = DateTimeOffset.FromUnixTimeMilliseconds(results.Count == 0 ? now : Math.Max(results.Max(x => x.Result.SavedAt), 1)).ToLocalTime()
            .ToString("HH:mm", CultureInfo.InvariantCulture);
        var tail = (failed.Count > 0 ? $" · lỗi: {Names(failed)}" : "") + (rest.Count > 0 ? $" · tạm nghỉ: {Names(rest)}" : "");
        var note = fromCache ? $"Bản lưu lúc {time} · đang làm mới…"
            : slow.Count > 0 ? $"Cập nhật lúc {time} · đang chờ {Names(slow)}…{tail}" : $"Cập nhật lúc {time}{tail}";
        var gaveUp = $"Cập nhật lúc {time}" + (slow.Count > 0 ? $" · chậm, dùng bản lưu: {Names(slow)}" : "") + tail;
        return new Snapshot
        {
            Rows = rows, Fresh = fresh, Categories = cats, MovedTo = moved, Note = note, NoteGaveUp = gaveUp,
            Empty = rows.Count == 0 && fresh.Count == 0, FromCache = fromCache, Pending = slow.Count > 0, Results = results, Plan = plan,
            StartedAt = startedAt,
        };
    }

    public static (List<FeedRow> Rows, List<FreshCard> Fresh) BuildRows(Plan plan, List<(Part Part, Feed.Result Result)> results, Ctx ctx)
    {
        var content = results.Where(x => x.Part.Role != Role.Tags).ToList();
        // Gộp mọi danh sách (trang web đang dùng trước, rồi các nguồn API) — một phim chỉ thuộc một nhóm
        var ordered = content.Select((x, i) => (x, i)).OrderBy(y => (int)y.x.Part.Role).ThenBy(y => y.i).Select(y => y.x).ToList();
        var merged = Feed.Merge(ordered.Select(x => x.Result.Items), ctx.Now);
        var groupOf = new Dictionary<string, FeedFilm>();
        foreach (var f in merged)
            foreach (var i in f.Items) groupOf[i.Url] = f;
        var down = content.Where(x => x.Result.State != Feed.State.Ok).Select(x => x.Part.Req.Host).ToHashSet();

        // Thứ tự nguồn khi mở phim: trang đang dùng, trang hay xem gần đây, rồi KKPhim / NguonC / VSMOV / CLB Phim Xưa
        var prefer = new[] { ctx.Base }.Concat(ctx.History.Select(h => Site.HostOf(h.FilmUrl)).OfType<string>())
            .Concat(new[] { plan.Src.Kk, plan.Src.Nc, plan.Src.Vs, plan.Src.K20 }.OfType<string>().Select(Site.HostOf).OfType<string>())
            .Distinct().ToList();
        var histUrl = new Dictionary<string, HistoryItem>();
        var histName = new Dictionary<string, HistoryItem>();
        foreach (var h in ctx.History)
        {
            histUrl.TryAdd(h.FilmUrl, h);
            foreach (var n in KkPhim.Names(new JObj { ["name"] = h.Title })) histName.TryAdd(n, h);
        }
        var memo = new Dictionary<FeedFilm, HistoryItem?>(ReferenceEqualityComparer.Instance);
        HistoryItem? HistoryOf(FeedFilm f)
        {
            if (memo.TryGetValue(f, out var m)) return m;
            var h = f.Items.Select(i => histUrl.GetValueOrDefault(i.Url)).FirstOrDefault(x => x != null) ??
                    f.Names.Select(n => histName.GetValueOrDefault(n)).FirstOrDefault(x => x != null);
            memo[f] = h;
            return h;
        }
        bool Watched(FeedFilm f) => HistoryOf(f) != null;
        var tags = new Dictionary<string, string>();
        string Tag(string host) => tags.TryGetValue(host, out var t) ? t : tags[host] = Site.Tag(host, ctx.Sites);

        Card MakeCard(FeedFilm f)
        {
            var own = HistoryOf(f) is { } hh ? Site.HostOf(hh.FilmUrl) : null;
            var o = f.Ordered((own != null ? new[] { own } : Array.Empty<string>()).Concat(prefer).ToList());
            var live = o.Items.Where(i => !down.Contains(i.Host)).ToList();
            var first = live.FirstOrDefault() ?? o.Items[0];
            var links = new[] { first }.Concat(o.Items.Where(i => i.Url != first.Url)).ToList();
            var badge = f.Latest?.Ep is { } ep ? $"Tập {Num(ep)}" : first.Badge;
            var chip = live.Count == 0 ? "bản lưu" : live.Count > 1 ? $"{live.Count} nguồn" : Tag(first.Host);
            return new Card(new Film(first.Url, first.Title, links.Select(l => l.Poster).FirstOrDefault(p => p != null), badge), chip, links);
        }

        // Hồ sơ trẻ em: chỉ phim hoạt hình, không có thể loại không hợp trẻ em (phim chưa rõ thể loại — vd danh sách «thiếu
        // nhi» của VSMOV — giữ lại vì đã đến từ danh sách dành cho trẻ em)
        bool KidsOk(FeedFilm f)
        {
            if (!plan.Kids) return true;
            if (f.Items.Any(i => Scary.IsMatch(i.Title))) return false;
            var t = f.Tags;
            return t == null || ((t.Type == null || t.Type == "hoathinh") && !t.Genres.Any(Adult.Contains));
        }

        var rows = new List<FeedRow>();
        var claimed = new HashSet<FeedFilm>(ReferenceEqualityComparer.Instance);
        List<FeedFilm> Films(Feed.Result r) => r.Items.Select(i => groupOf.GetValueOrDefault(i.Url)).OfType<FeedFilm>().Distinct(ReferenceEqualityComparer.Instance).Cast<FeedFilm>().ToList();
        void Add(string slot, string id, string title, IEnumerable<FeedFilm> list, string? more, bool claim, int min = 3)
        {
            var l = list.Where(f => !claimed.Contains(f) && KidsOk(f)).Take(RowMax).ToList();
            if (l.Count < min) return;
            if (claim) foreach (var f in l.Take(6)) claimed.Add(f);
            rows.Add(new FeedRow(id, title, l.Select(MakeCard).ToList(), more, slot));
        }
        List<(Part Part, Feed.Result Result)> ByRole(Role r) => content.Where(x => x.Part.Role == r).ToList();
        IEnumerable<FeedFilm> NewestFirst(IEnumerable<FeedFilm> l) => l.Select((f, i) => (f, i)).OrderByDescending(x => x.f.Updated).ThenBy(x => x.i).Select(x => x.f);
        IEnumerable<FeedFilm> ByAffinity(IEnumerable<FeedFilm> l) => l.Select((f, i) => (f, i))
            .OrderByDescending(x => plan.Taste?.Affinity(x.f.Tags) ?? 0).ThenByDescending(x => x.f.Updated).ThenBy(x => x.i).Select(x => x.f);

        // Có tập mới: phim đang xem dở (xem trong 30 ngày, đã xem kịp tập mới nhất lúc đó) vừa có tập mới ở bất kỳ nguồn nào
        var fresh = new List<FreshCard>();
        foreach (var same in ctx.History.GroupBy(h => KkPhim.Norm(h.Title)))
        {
            var key = same.Key;
            var h = same.First(); // xem gần nhất
            if (ctx.Now - h.Time > FreshWatched || ctx.Follows.Contains(h.FilmUrl)) continue;
            var eps = same.Select(x => Feed.EpOf(x.EpTitle)).OfType<double>().ToList();
            if (eps.Count == 0) continue;
            var watchedEp = eps.Max();
            var f = merged.FirstOrDefault(g => g.Items.Any(i => i.Url == h.FilmUrl) || g.Names.Contains(key));
            if (f?.Latest is not { Ep: { } ep } latest || latest.Updated < ctx.Now - FreshNew) continue;
            var seen = ctx.Seen(key);
            if (seen != null && watchedEp < seen - 1) continue; // chưa xem kịp — không phải «tập mới»
            var known = Math.Max(watchedEp, seen ?? watchedEp);
            if (ep <= known) continue;
            // Xem từ trước bản này: số tập ra thêm phải hợp với thời gian từ lần xem cuối (≤ 2 tập + 3 tập mỗi tuần)
            if (seen == null && ep - watchedEp > 2 + (int)((ctx.Now - h.Time) / (double)Day / 7 * 3)) continue;
            var ownHost = Site.HostOf(h.FilmUrl) ?? "";
            var badge = ep - known >= 2 ? $"+{Num(ep - known)} · Tập {Num(ep)}" : $"Tập {Num(ep)} mới";
            var chip = down.Contains(latest.Host) ? "lần kiểm trước" : latest.Host != ownHost ? $"có ở {Site.Tag(latest.Host, ctx.Sites)}" : null;
            var open = latest.Host == ownHost ? f.Items.FirstOrDefault(i => i.Url == h.FilmUrl) ?? latest : latest;
            var links = new[] { open }.Concat(f.Items.Where(i => i.Url != open.Url)).ToList();
            fresh.Add(new FreshCard(new Card(new Film(open.Url, h.Title, h.Poster ?? open.Poster, badge), chip, links), key, ep));
        }

        // Hàng tự thêm (và chủ đề người mới chọn): phim chưa xem, mới cập nhật trước
        foreach (var (p, r) in ByRole(Role.Custom)) Add(p.Slot, p.RowId, p.Title ?? "", NewestFirst(Films(r).Where(f => !Watched(f))), p.More, true);

        // Giống «phim vừa xem»: xếp theo số thể loại chung
        var simGenres = (plan.SimilarTags?.Genres ?? new()).ToHashSet();
        var me = plan.Similar != null ? KkPhim.Names(new JObj { ["name"] = plan.Similar.Title }) : new HashSet<string>();
        foreach (var (p, r) in ByRole(Role.Similar))
        {
            var list = Films(r).Where(f => !Watched(f) && !f.Matches(me)).Select((f, i) => (f, i))
                .OrderByDescending(x => x.f.Tags?.Genres.Count(simGenres.Contains) ?? 0).ThenByDescending(x => x.f.Updated).ThenBy(x => x.i).Select(x => x.f);
            Add("similar", p.RowId, p.Title ?? "", list, p.More, true, 4);
        }
        var interest = ByRole(Role.Interest);
        void InterestRow(string slot)
        {
            if (interest.FirstOrDefault(x => x.Part.Slot == slot) is not { Part: { } p, Result: { } r }) return;
            Add(slot, p.RowId, p.Title ?? "", ByAffinity(Films(r).Where(f => !Watched(f))), p.More, true);
        }
        InterestRow("interest");

        // Loại phim hay xem, mới cập nhật · mọi nguồn
        var dominant = ByRole(Role.Dominant);
        if (dominant.Count > 0)
        {
            var set = dominant.SelectMany(x => Films(x.Result)).Distinct(ReferenceEqualityComparer.Instance).Cast<FeedFilm>();
            Add("dominant", "dominant", dominant[0].Part.Title ?? "", NewestFirst(set.Where(f => !Watched(f))), null, true);
        }
        // Thuyết minh / lồng tiếng mới
        foreach (var (p, r) in ByRole(Role.Dub)) Add("dub", p.RowId, p.Title ?? "", NewestFirst(Films(r).Where(f => !Watched(f))), p.More, true);
        // Hồ sơ trẻ em: hoạt hình thiếu nhi (KKPhim + VSMOV gộp một hàng), gia đình, lồng tiếng
        foreach (var g in ByRole(Role.Kids).GroupBy(x => x.Part.RowId))
        {
            var ps = g.ToList();
            var set = ps.SelectMany(x => Films(x.Result)).Distinct(ReferenceEqualityComparer.Instance).Cast<FeedFilm>();
            Add(g.Key, g.Key, ps[0].Part.Title ?? "", NewestFirst(set.Where(f => !Watched(f))), ps.Select(x => x.Part.More).FirstOrDefault(m => m != null), true);
        }
        InterestRow("interest2");

        // Mới cập nhật · mọi nguồn
        var newest = ByRole(Role.Web).Concat(ByRole(Role.New)).SelectMany(x => Films(x.Result)).Distinct(ReferenceEqualityComparer.Instance).Cast<FeedFilm>();
        Add("newest", "newest", "Mới cập nhật · mọi nguồn", NewestFirst(newest.Where(f => !Watched(f))), null, false);

        foreach (var (p, r) in ByRole(Role.Movies)) Add("movies", p.RowId, p.Title ?? "", ByAffinity(Films(r).Where(f => !Watched(f))), p.More, false);
        // Thứ tự / bật tắt theo cài đặt của người xem (Cài đặt → Trang chủ → Các hàng)
        return (Arrange(rows), fresh);
    }

    /// Người xem vừa mở trang phim / một tập: ghi tập mới nhất đã thấy (theo tên — chung mọi nguồn).
    public static void MarkSeen(FilmDetail d)
    {
        var eps = d.Groups.SelectMany(g => g.Episodes).Select(e => Feed.EpOf(e.Title)).OfType<double>().ToList();
        if (eps.Count > 0) Prefs.SetSeenEp(KkPhim.Norm(d.Title), eps.Max());
    }

    /// 108.0 → "108", 10.5 → "10.5".
    public static string Num(double d) => d == Math.Floor(d) ? ((long)d).ToString(CultureInfo.InvariantCulture) : d.ToString(CultureInfo.InvariantCulture);

    // ---- Trang chủ lần trước (mở app là hiện ngay) ----

    private static string LastKey(string pid) => $"last|{pid}";

    public static void SaveLast(string pid, Snapshot s)
    {
        if (s.FromCache || s.Empty) return;
        var o = new JObj
        {
            ["at"] = Util.NowMs(),
            ["rows"] = s.Rows.Select(r => (object?)new JObj { ["id"] = r.Id, ["t"] = r.Title, ["m"] = r.More ?? "", ["s"] = r.Slot, ["c"] = r.Cards.Select(c => (object?)CardJson(c)).ToList() }).ToList(),
            ["fresh"] = s.Fresh.Select(f => (object?)new JObj { ["c"] = CardJson(f.Card), ["k"] = f.Key, ["e"] = f.Ep }).ToList(),
        };
        if (s.Categories != null) o["cats"] = s.Categories.Select(c => (object?)new JObj { ["t"] = c.Title, ["u"] = c.Url }).ToList();
        Feed.WriteText(LastKey(pid), JsonSerializer.Serialize(o));
    }

    /// Trang chủ lần trước của người xem pid (dưới 24 giờ); null nếu chưa có.
    public static Snapshot? Last(string pid)
    {
        try
        {
            if (Feed.ReadText(LastKey(pid)) is not { } text || JsonScan.Parse(text) is not JObj o) return null;
            var at = Feed.Num64(o.GetValueOrDefault("at"));
            if (Util.NowMs() - at > Feed.CacheMax) return null;
            var rows = J.Objects(J.Arr(o, "rows")).Select(r => new FeedRow(J.Str(r, "id"), J.Str(r, "t"),
                J.Objects(J.Arr(r, "c")).Select(CardOf).OfType<Card>().ToList(), J.Str(r, "m").NonEmpty(), J.Str(r, "s"))).ToList();
            var fresh = J.Objects(J.Arr(o, "fresh")).Select(f => J.Obj(f, "c") is { } c && CardOf(c) is { } card
                ? new FreshCard(card, J.Str(f, "k"), Feed.Dbl(f.GetValueOrDefault("e")) ?? 0) : null).OfType<FreshCard>().ToList();
            var cats = J.Arr(o, "cats") is { } cs ? J.Objects(cs).Select(x => new Category(J.Str(x, "t"), J.Str(x, "u"))).ToList() : null;
            var time = DateTimeOffset.FromUnixTimeMilliseconds(at).ToLocalTime().ToString("HH:mm", CultureInfo.InvariantCulture);
            var note = $"Bản lưu lúc {time} · đang làm mới…";
            return new Snapshot { Rows = rows, Fresh = fresh, Categories = cats, Note = note, NoteGaveUp = note, Empty = rows.Count == 0 && fresh.Count == 0, FromCache = true };
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static JObj CardJson(Card c) => new()
    {
        ["u"] = c.Film.Url, ["n"] = c.Film.Title, ["p"] = c.Film.Poster ?? "", ["b"] = c.Film.Badge ?? "", ["x"] = c.Chip ?? "",
        ["l"] = c.Links.Select(l => (object?)(l with { Names = new(), Tags = null }).Json()).ToList(),
    };

    private static Card? CardOf(JObj o)
    {
        if (J.Str(o, "u").NonEmpty() is not { } url) return null;
        return new Card(new Film(url, J.Str(o, "n"), J.Str(o, "p").NonEmpty(), J.Str(o, "b").NonEmpty()), J.Str(o, "x").NonEmpty(),
            J.Objects(J.Arr(o, "l")).Select(FeedItem.Of).OfType<FeedItem>().ToList());
    }
}

/// Thể loại / quốc gia / loại phim của phim đã xem (tra KKPhim theo tên, lưu chung cả máy). Khoá = tên đã chuẩn hoá;
/// {g, c, t, at} hoặc {miss: lúc tra không thấy}.
public static class FilmTags
{
    private const int MaxCount = 400;
    private const long MissRetry = 3 * 86_400_000L;
    private static readonly object Lock = new();
    private static Dictionary<string, JObj>? _mem;
    private static string? _memOf;

    private static Dictionary<string, JObj> Store()
    {
        var raw = Prefs.FilmTags;
        if (raw != _memOf || _mem == null)
        {
            _mem = (JsonScan.Parse(raw) as JObj ?? new()).Where(kv => kv.Value is JObj).ToDictionary(kv => kv.Key, kv => (JObj)kv.Value!);
            _memOf = raw;
        }
        return _mem;
    }

    public static Tags? Of(string title)
    {
        lock (Lock) return Store().TryGetValue(KkPhim.Norm(title), out var e) && !e.ContainsKey("miss") ? Tags.Of(e) : null;
    }

    public static bool NeedsLookup(string title)
    {
        lock (Lock)
        {
            if (!Store().TryGetValue(KkPhim.Norm(title), out var e)) return true;
            return e.ContainsKey("miss") && Util.NowMs() - Feed.Num64(e["miss"]) > MissRetry;
        }
    }

    public static void Remember(string key, Tags? tags)
    {
        if (key.Length == 0) return;
        lock (Lock)
        {
            var s = new Dictionary<string, JObj>(Store());
            if (tags != null)
            {
                var o = tags.Json();
                o["at"] = Util.NowMs();
                s[key] = o;
            }
            else
            {
                s[key] = new JObj { ["miss"] = Util.NowMs() };
            }
            if (s.Count > MaxCount)
            {
                long At(JObj o) => o.ContainsKey("at") ? Feed.Num64(o["at"]) : Feed.Num64(o.GetValueOrDefault("miss"));
                foreach (var k in s.OrderBy(kv => At(kv.Value)).Take(s.Count - MaxCount).Select(kv => kv.Key).ToList()) s.Remove(k);
            }
            var raw = JsonSerializer.Serialize(s);
            Prefs.FilmTags = raw;
            _mem = s;
            _memOf = raw;
        }
    }

    /// Tra KKPhim theo tên (lỗi mạng → ném lỗi, không ghi; không thấy → ghi «không có»).
    public static async Task Lookup(string title)
    {
        var q = KkPhim.Api + "/v1/api/tim-kiem?keyword=" + (title.Length > 80 ? title[..80] : title).FormEncoded() + "&limit=10";
        var root = JsonScan.ParseObject((await Http.Get(q, new() { ["Accept"] = "application/json" })).Data);
        var items = J.Objects(J.Arr(J.Obj(root, "data"), "items"));
        var hit = KkPhim.Choose(items, KkPhim.Names(new JObj { ["name"] = title }));
        Remember(KkPhim.Norm(title), hit != null ? Tags.FromKk(hit) : null);
    }
}
