using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

using JObj = Dictionary<string, object?>;

/// Trang chủ tổng hợp — cùng ca với FeedTest.kt / FeedTests.swift (đọc danh sách, gộp phim trùng, sở thích, dựng hàng, tập mới,
/// hồ sơ trẻ em).
public class FeedTests : PrefsTest
{
    private const string Kk = "https://www.kkphim.com";
    private const string Nc = "https://phim.nguonc.com";
    private static readonly long Now = Feed.Time("2026-11-08T20:00:00+07:00");
    private const long Hour = 3_600_000;
    private const long Day = 24 * Hour;

    private static JObj Json(string s) => (JObj)JsonScan.Parse(s)!;

    private static List<FeedItem> KkItems() => Feed.ParseKk(Json("""
        {"status":"success","data":{"APP_DOMAIN_CDN_IMAGE":"https://phimimg.com","items":[
          {"name":"Mục Thần Ký","slug":"muc-than-ky","origin_name":"Tales Of Herding Gods","type":"hoathinh",
           "poster_url":"upload/vod/a.jpg","episode_current":"Tập 107","year":2024,"modified":{"time":"2026-11-08T11:55:29.000Z"},
           "category":[{"name":"Phiêu Lưu","slug":"phieu-luu"},{"name":"Viễn Tưởng","slug":"vien-tuong"}],
           "country":[{"name":"Trung Quốc","slug":"trung-quoc"}]},
          {"name":"Đấu Phá Thương Khung (Phần 5)","slug":"dau-pha-thuong-khung-phan-5","type":"hoathinh",
           "poster_url":"https://img.example/dp5.jpg","episode_current":"Tập 150","year":2022,"modified":{"time":"2026-11-08T10:00:00.000Z"}},
          {"name":"Thôn Phệ Tinh Không","slug":"thon-phe-tinh-khong","type":"hoathinh","episode_current":"Tập 248","year":2020,
           "modified":{"time":"2026-11-08T09:00:00.000Z"}},
          {"name":"Thiên Long Bát Bộ","slug":"thien-long-bat-bo-2021","type":"series","episode_current":"Hoàn Tất (50/50)","year":2021,
           "modified":{"time":"2026-11-01T09:00:00.000Z"}}
        ]}}
        """), Kk);

    private static List<FeedItem> NcItems() => Feed.ParseNc(Json("""
        {"status":"success","items":[
          {"name":"Mục Thần Ký","slug":"muc-than-ky","original_name":"Mu Shen Ji, Tales of Herding Gods","year":"2024",
           "current_episode":"Tập 108","thumb_url":"https://phim.nguonc.com/t.jpg","modified":"2026-11-08T12:30:00.000000Z"},
          {"name":"Đấu Phá Thương Khung 5","slug":"dau-pha-5","year":"2022","current_episode":"Tập 150","modified":"2026-11-08T08:00:00.000000Z"},
          {"name":"Thôn Phệ Tinh Không 2","slug":"thon-phe-2","year":"2024","current_episode":"Tập 222","modified":"2026-11-08T12:00:00.000000Z"}
        ]}
        """), Nc);

    private static List<FeedItem> Web(params (string Title, string? Badge)[] titles) =>
        Feed.FromFilms(titles.Select(t => new Film("https://yanhh3d.ee/" + KkPhim.Norm(t.Title).Replace(' ', '-'), t.Title, null, t.Badge)));

    [Fact]
    public void EpisodeNumbersAndTimes()
    {
        Assert.Equal(108, Feed.EpOf("Tập 108"));
        Assert.Equal(16, Feed.EpOf("Hoàn Tất (16/16)"));
        Assert.Equal(7, Feed.EpOf("7/??"));
        Assert.Null(Feed.EpOf("Full"));
        Assert.Null(Feed.EpOf("Top 1"));
        var utc = Feed.Time("2026-11-08T14:12:36Z");
        Assert.Equal(utc, Feed.Time("2026-11-08T21:12:36+07:00"));
        Assert.Equal(utc, Feed.Time("2026-11-08T14:12:36.000000Z"));
        Assert.Equal(utc, Feed.Time("2026-11-08 21:12:36"));
        Assert.Equal(0, Feed.Time("không phải giờ"));
    }

    [Fact]
    public void ParsesKkAndNguonC()
    {
        var m = KkItems()[0];
        Assert.Equal($"{Kk}/phim/muc-than-ky", m.Url);
        Assert.Equal("https://phimimg.com/upload/vod/a.jpg", m.Poster);
        Assert.Equal(107, m.Ep);
        Assert.Equal(new[] { "phieu-luu", "vien-tuong" }, m.Tags!.Genres);
        Assert.Equal("hoathinh", m.Tags.Type);
        Assert.Equal("kkphim.com", m.Host);
        var n = NcItems()[0];
        Assert.Equal(108, n.Ep);
        Assert.Contains(KkPhim.Norm("Mu Shen Ji"), n.Names);
        Assert.Equal(m, FeedItem.Of(Json(System.Text.Json.JsonSerializer.Serialize(m.Json()))));
    }

    [Fact]
    public void MergesSameFilmAcrossSources()
    {
        var y = Web(("Đấu Phá Thương Khung Phần 5", "Tập 150"), ("Mục Thần Ký", "Tập 107"), ("Thôn Phệ Tinh Không", "Tập 248"));
        var films = Feed.Merge(new[] { y, KkItems(), NcItems(), KkItems().Take(1).ToList() }, Now);
        List<FeedFilm> Find(string n) => films.Where(f => f.Names.Contains(KkPhim.Norm(n))).ToList();
        Assert.Single(Find("Đấu Phá Thương Khung Phần 5"));
        Assert.Equal(3, Find("Đấu Phá Thương Khung Phần 5")[0].Items.Count);
        Assert.Equal(3, Find("Mục Thần Ký").Single().Items.Count); // cùng phim trong hai danh sách KKPhim vẫn một nhóm
        Assert.Equal("phim.nguonc.com", Find("Mục Thần Ký")[0].Latest!.Host);
        Assert.Single(Find("Thôn Phệ Tinh Không"));
        Assert.Single(Find("Thôn Phệ Tinh Không 2"));
        var two = Feed.FromFilms(new[] { new Film("https://yanhh3d.ee/a", "Tiên Nghịch", null, null), new Film("https://yanhh3d.ee/b", "Tiên Nghịch", null, null) });
        Assert.Equal(2, Feed.Merge(new[] { two }, Now).Count);
    }

    [Fact]
    public void TasteFavoursRecentAndMuchWatched()
    {
        var tags = new Dictionary<string, Tags>
        {
            ["Tiên Nghịch"] = new(new() { "vo-thuat", "hoat-hinh", "chinh-kich" }, new() { "trung-quoc" }, "hoathinh"),
            ["Hạ Cánh Nơi Anh"] = new(new() { "tinh-cam" }, new() { "han-quoc" }, "series"),
        };
        var history = new List<HistoryItem>
        {
            new() { FilmUrl = "https://yanhh3d.ee/tien-nghich", Title = "Tiên Nghịch", EpTitle = "Tập 166", EpUrl = "e1", Time = Now - Day },
            new() { FilmUrl = "https://www.kkphim.com/phim/ha-canh", Title = "Hạ Cánh Nơi Anh", EpTitle = "Tập 3", EpUrl = "e2", Time = Now - 60 * Day },
        };
        var t = HomeFeed.GetTaste(history, s => tags.GetValueOrDefault(s), u => u.Contains("tien") ? 20 : 3, Now);
        Assert.True(t.Genre["vo-thuat"] > 15);
        Assert.False(t.Genre.ContainsKey("hoat-hinh"));
        Assert.Equal(t.Genre["vo-thuat"] / 2, t.Genre["chinh-kich"], 9);
        Assert.Equal(new HomeFeed.Combo("hoathinh", "trung-quoc", "vo-thuat"), t.Combos.MaxBy(kv => kv.Value).Key);
    }

    private static (HomeFeed.Part Part, Feed.Result Result) Part(HomeFeed.Role role, string key, string host, List<FeedItem> items,
        string? title = null, string id = "", Feed.State state = Feed.State.Ok)
    {
        var r = new Feed.Req(key, host, () => Task.FromResult(new Feed.Loaded(items)));
        return (new HomeFeed.Part(role, r, title, null, id), new Feed.Result(r, state, items, null, state == Feed.State.Ok ? null : "lỗi 502", false, 0, Now));
    }

    private static HomeFeed.Plan Plan(List<(HomeFeed.Part Part, Feed.Result Result)> parts, bool kids = false) =>
        new(parts.Select(p => p.Part).ToList(), new HomeFeed.Sources(Kk, Nc, "https://vsmov.com", null, new() { "https://yanhh3d.ee" }), Kids: kids);

    private static HomeFeed.Ctx Ctx(List<HistoryItem> history, Dictionary<string, double>? seen = null, HashSet<string>? follows = null) =>
        new(history, follows ?? new(), k => seen != null && seen.TryGetValue(k, out var v) ? v : null, "yanhh3d.ee",
            new() { "https://yanhh3d.ee", Kk, Nc }, Now);

    private static HistoryItem H(string slug, string title, string ep, long ago) =>
        new() { FilmUrl = "https://yanhh3d.ee/" + slug, Title = title, EpTitle = ep, EpUrl = "e-" + slug, Time = Now - ago };

    [Fact]
    public void NewEpisodesOfFilmsBeingWatched()
    {
        var parts = new List<(HomeFeed.Part, Feed.Result)>
        {
            Part(HomeFeed.Role.Web, "web|y", "yanhh3d.ee", Web(("Mục Thần Ký", "Tập 107"), ("Tiên Nghịch", "Tập 168"), ("Thiên Long Bát Bộ", "Tập 45"))),
            Part(HomeFeed.Role.New, "new|kk", "kkphim.com", KkItems()),
            Part(HomeFeed.Role.New, "new|nc", "phim.nguonc.com", NcItems()),
        };
        var history = new List<HistoryItem>
        {
            H("muc-than-ky", "Mục Thần Ký", "Tập 107", Day), H("tien-nghich", "Tiên Nghịch", "Tập 166", 2 * Day),
            H("thien-long-bat-bo", "Thiên Long Bát Bộ", "Tập 30", Day),
        };
        var fresh = HomeFeed.BuildRows(Plan(parts), parts, Ctx(history, new() { [KkPhim.Norm("Thiên Long Bát Bộ")] = 45 })).Fresh
            .ToDictionary(f => f.Card.Film.Title);
        Assert.Equal(new HashSet<string> { "Mục Thần Ký", "Tiên Nghịch" }, fresh.Keys.ToHashSet());
        Assert.Equal("Tập 108 mới", fresh["Mục Thần Ký"].Card.Film.Badge);
        Assert.Equal("có ở nguonc", fresh["Mục Thần Ký"].Card.Chip);
        Assert.Equal($"{Nc}/phim/muc-than-ky", fresh["Mục Thần Ký"].Card.Film.Url);
        Assert.Equal("+2 · Tập 168", fresh["Tiên Nghịch"].Card.Film.Badge);
        // Đang xem dở từ trước (tập 31 / 108): không phải «tập mới»
        Assert.Empty(HomeFeed.BuildRows(Plan(parts), parts, Ctx(new() { H("muc-than-ky", "Mục Thần Ký", "Tập 31", 3 * Day) })).Fresh);
        // Đã thấy tập 108 → thôi báo
        Assert.DoesNotContain(HomeFeed.BuildRows(Plan(parts), parts, Ctx(history, new() { [KkPhim.Norm("Mục Thần Ký")] = 108 })).Fresh,
            f => f.Card.Film.Title == "Mục Thần Ký");
    }

    [Fact]
    public void NewestRowMergesSourcesAndMarksBrokenOnes()
    {
        var parts = new List<(HomeFeed.Part, Feed.Result)>
        {
            Part(HomeFeed.Role.Web, "web|y", "yanhh3d.ee", Web(("Đấu Phá Thương Khung Phần 5", "Tập 150"))),
            Part(HomeFeed.Role.New, "new|kk", "kkphim.com", KkItems(), state: Feed.State.Cached), // KKPhim lỗi, dùng bản lưu
            Part(HomeFeed.Role.New, "new|nc", "phim.nguonc.com", NcItems()),
        };
        var newest = HomeFeed.BuildRows(Plan(parts), parts, Ctx(new())).Rows.Single(r => r.Id == "newest");
        Assert.Equal("bản lưu", newest.Cards.Single(c => c.Film.Title == "Thiên Long Bát Bộ").Chip);
        var dp = newest.Cards.Single(c => KkPhim.Norm(c.Film.Title) == KkPhim.Norm("Đấu Phá Thương Khung Phần 5"));
        Assert.Equal("2 nguồn", dp.Chip);
        Assert.Equal("https://yanhh3d.ee/dau-pha-thuong-khung-5", dp.Film.Url); // trang đang dùng mở trước
        Assert.Equal(3, dp.Links.Count);
    }

    [Fact]
    public void KidsProfileKeepsOnlyChildFriendlyAnimation()
    {
        static string Item(string slug, string type, params string[] genres) =>
            $"{{\"name\":\"{slug}\",\"slug\":\"{slug}\",\"type\":\"{type}\",\"modified\":{{\"time\":\"2026-11-08T10:00:00Z\"}}," +
            $"\"category\":[{string.Join(",", genres.Select(g => $"{{\"slug\":\"{g}\"}}"))}]}}";
        var list = Feed.ParseKk(Json("{\"data\":{\"items\":[" + string.Join(",", new[]
        {
            Item("doraemon", "hoathinh", "gia-dinh"), Item("chiikawa", "hoathinh", "tre-em"), Item("conan", "hoathinh", "bi-an"),
            Item("edgerunners", "hoathinh", "hinh-su"), Item("phim-ma", "hoathinh", "kinh-di"), Item("nguoi-dong", "series", "gia-dinh"),
            Item("Horror Collector", "hoathinh", "tre-em"),
        }) + "]}}"), Kk);
        var vs = Feed.FromFilms(new[] { new Film("https://vsmov.com/phim/na-tra", "Na Tra Truyền Kỳ", null, null) });
        var parts = new List<(HomeFeed.Part, Feed.Result)>
        {
            Part(HomeFeed.Role.Kids, "list|kk", "kkphim.com", list, "Hoạt hình thiếu nhi", "kids-young"),
            Part(HomeFeed.Role.Kids, "list|vs", "vsmov.com", vs, "Hoạt hình thiếu nhi", "kids-young"),
        };
        var rows = HomeFeed.BuildRows(Plan(parts, kids: true), parts, Ctx(new())).Rows;
        Assert.Single(rows);
        Assert.Equal(new HashSet<string> { "doraemon", "chiikawa", "conan", "Na Tra Truyền Kỳ" }, rows[0].Cards.Select(c => c.Film.Title).ToHashSet());
        Assert.Equal(8, HomeFeed.BuildRows(Plan(parts), parts, Ctx(new())).Rows[0].Cards.Count);
    }

    [Fact]
    public void PrefsForTheFeed()
    {
        Assert.True(FilmTags.NeedsLookup("Tiên Nghịch"));
        FilmTags.Remember(KkPhim.Norm("Tiên Nghịch"), new Tags(new() { "vo-thuat" }, new() { "trung-quoc" }, "hoathinh"));
        Assert.Equal("hoathinh", FilmTags.Of("Tiên nghịch")!.Type);
        FilmTags.Remember(KkPhim.Norm("Phim Lạ"), null);
        Assert.Null(FilmTags.Of("Phim Lạ"));
        Assert.False(FilmTags.NeedsLookup("Phim Lạ"));

        Prefs.SetSeenEp("muc than ky", 107);
        Prefs.SetSeenEp("muc than ky", 105);
        Assert.Equal(107, Prefs.SeenEp("muc than ky"));
        Prefs.SavePosition("https://yanhh3d.ee/muc-than-ky/tap-1", 0, "https://yanhh3d.ee/muc-than-ky");
        Prefs.SavePosition("https://yanhh3d.ee/muc-than-ky/tap-2", 30, "https://yanhh3d.ee/muc-than-ky");
        Assert.Equal(2, Prefs.WatchedCounts()["https://yanhh3d.ee/muc-than-ky"]);
        for (var i = 0; i < 45; i++) Prefs.RecordWatchLang(i % 3 == 0 ? "vs" : "tm");
        Assert.Equal(40, Prefs.WatchLangs.Count);
        Assert.True(Prefs.HomeMixOn);
        Prefs.HomeMix = false;
        Assert.False(Prefs.HomeMixOn);
        Prefs.Kids = true;
        Assert.True(Prefs.HomeMixOn);
        Prefs.HomeTopics = new() { "cn", "kh" };
        Assert.Equal(new HashSet<string> { "cn", "kh" }, Prefs.HomeTopics);
    }
}
