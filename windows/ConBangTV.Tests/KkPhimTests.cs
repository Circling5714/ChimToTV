using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

using JObj = Dictionary<string, object?>;

/// Server dự phòng KKPhim + trang KKPhim (11/2026) — cùng ca với KkPhimTest.kt / KkPhimTests.swift.
public class KkPhimTests
{
    private static JObj Json(string s) => (JObj)JsonScan.Parse(s)!;

    private static JObj Movie(string name, string origin = "", string type = "hoathinh", string slug = "x") =>
        new() { ["name"] = name, ["origin_name"] = origin, ["type"] = type, ["slug"] = slug };

    private static HashSet<string> Mine(params string[] names) => names.Select(KkPhim.Norm).ToHashSet();

    [Fact]
    public void NormalizesSeasonsAndAccents()
    {
        Assert.Equal(KkPhim.Norm("Đấu Phá Thương Khung Phần 5"), KkPhim.Norm("Đấu Phá Thương Khung (Phần 5)"));
        Assert.Equal("dau pha thuong khung 5", KkPhim.Norm("Đấu Phá Thương Khung (Phần 5)"));
        Assert.Equal(KkPhim.Norm("Sword of Coming SS2"), KkPhim.Norm("Sword of Coming (Season 2)"));
        Assert.Equal(KkPhim.Norm("Fights Break Sphere 5"), KkPhim.Norm("Fights Break Sphere (Season 5)"));
        Assert.Equal(KkPhim.Norm("Đấu La Đại Lục 2: Tuyệt Thế Đường Môn"), KkPhim.Norm("Đấu La Đại Lục 2 (Tuyệt Thế Đường Môn)"));
        Assert.Equal(KkPhim.Norm("Spare Me, Great Lord! Season 3"), KkPhim.Norm("Spare Me, Great Lord! (Season 3)"));
    }

    [Fact]
    public void MatchesByNameOrOriginalName()
    {
        var mine = Mine("Già Thiên", "Zhe Tian");
        Assert.True(KkPhim.Matches(Movie("Già Thiên", "Shrouding The Heavens"), mine));
        Assert.True(KkPhim.Matches(Movie("Che Trời", "Zhe Tian"), mine));
        Assert.False(KkPhim.Matches(Movie("Già Thiên Movie", "Shrouding The Heavens Movie"), mine));
        var doctor = Movie("Thần Y Cổ Đại Ở Đô Thị (Đô Thị Cổ Tiên Y)", "Urban Miracle Doctor");
        Assert.True(KkPhim.Matches(doctor, Mine("Thần Y Cổ Đại Ở Đô Thị")));
        Assert.True(KkPhim.Matches(doctor, Mine("Đô Thị Cổ Tiên Y")));
        // Trong ngoặc chỉ là số phần: phần 5 không được ghép với phim "Đấu Phá Thương Khung" (phần 1)
        var s5 = Movie("Đấu Phá Thương Khung (Phần 5)", "Fights Break Sphere (Season 5)");
        Assert.False(KkPhim.Matches(s5, Mine("Đấu Phá Thương Khung")));
        Assert.True(KkPhim.Matches(s5, Mine("Đấu Phá Thương Khung Phần 5")));
    }

    [Fact]
    public void PrefersAnimationAmongSameNameResults()
    {
        var mine = Mine("Già Thiên");
        var live = Movie("Già Thiên", type: "series", slug: "gia-thien-live");
        var anim = Movie("Già Thiên", type: "hoathinh", slug: "gia-thien");
        Assert.Equal("gia-thien", J.Str(KkPhim.Choose(new() { live, anim }, mine), "slug"));
        Assert.Equal("gia-thien-live", J.Str(KkPhim.Choose(new() { live }, mine), "slug"));
        Assert.Null(KkPhim.Choose(new() { Movie("Tiên Nghịch") }, mine));
    }

    [Fact]
    public void ReadsSlugFromFilmUrl()
    {
        Assert.Equal("tien-nghich", KkPhim.SlugOf("https://yanhh3d.ee/tu-tien/tien-nghich"));
        Assert.Equal("dau-pha-thuong-khung-phan-5", KkPhim.SlugOf("https://hoathinh3d.ee/dau-pha-thuong-khung-phan-5/"));
        Assert.Null(KkPhim.SlugOf("https://example.com/"));
    }

    private static readonly JObj Detail = Json("""
    {"status":true,"movie":{"name":"Tiên Nghịch","slug":"tien-nghich","type":"hoathinh"},"episodes":[
    {"server_name":"Vietsub #Hà Nội","is_ai":false,"server_data":[
      {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://a.kvp726.com/164/index.m3u8"},
      {"name":"Tập 165","slug":"tap-165","link_m3u8":"https://a.kvp726.com/165/index.m3u8"}]},
    {"server_name":"Thuyết Minh","is_ai":false,"server_data":[
      {"name":"Tập 08","slug":"tap-08","link_m3u8":"https://s6.kkphimplayer6.com/8/index.m3u8"},
      {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://s6.kkphimplayer6.com/164/index.m3u8"}]},
    {"server_name":"Thuyết Minh","is_ai":true,"server_data":[
      {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://ai.example/164/index.m3u8"}]},
    {"server_name":"Lồng Tiếng","is_ai":false,"server_data":[
      {"name":"Tập 164","slug":"tap-164","link_m3u8":"","link_embed":"https://player.phimapi.com/player/?url=x"}]}
    ]}
    """);

    [Fact]
    public void PicksSameEpisodeNumberInEveryLanguage()
    {
        var all = KkPhim.Pick(Detail, 164, null);
        Assert.Equal(new[] { "kk-vs", "kk-tm", "kk-tm-ai" }, all.Select(x => x.Item1.Type));
        Assert.Equal(new[] { "KKPhim Vietsub", "KKPhim Thuyết Minh", "KKPhim Thuyết Minh (AI)" }, all.Select(x => x.Item1.Label));
        Assert.Equal("https://a.kvp726.com/164/index.m3u8", all[0].Item2.Url);
        Assert.Equal(new[] { "kk-tm" }, KkPhim.Pick(Detail, 8, null).Select(x => x.Item1.Type));
        Assert.Equal(new[] { "kk-vs" }, KkPhim.Pick(Detail, 165, null).Select(x => x.Item1.Type));
        Assert.Empty(KkPhim.Pick(Detail, 166, null));
    }

    [Fact]
    public void OnlyLanguageOfTheVersionBeingWatched()
    {
        Assert.Equal(new[] { "kk-vs" }, KkPhim.Pick(Detail, 164, KkPhim.Lang("Việt Sub")).Select(x => x.Item1.Type));
        Assert.Equal(new[] { "kk-tm", "kk-tm-ai" }, KkPhim.Pick(Detail, 164, KkPhim.Lang("Thuyết Minh")).Select(x => x.Item1.Type));
        Assert.Equal("vs", KkPhim.Lang("4K Việt Sub"));
        Assert.Equal("tm", KkPhim.Lang("Thuyết minh 1080 V1"));
        Assert.Null(KkPhim.Lang("Tập phim"));
    }

    [Fact]
    public async Task AttachesAfterSiteServersAndPlaysFromLinks()
    {
        var kk = KkPhim.Pick(Detail, 164, null);
        var yan = new EpisodeSource
        {
            EpUrl = "https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html", Servers = new() { new("VIETSUB 1080 V1", "tiktik") },
            PostId = "1010", Ep = "tap-164", Sv = "1", PlayerUrl = "https://yanhh3d.ee/player/player.php",
        };
        var s = KkPhim.Attach(yan, kk);
        Assert.Equal(new[] { "tiktik", "kk-vs", "kk-tm", "kk-tm-ai" }, s.Servers.Select(x => x.Type));
        Assert.False(s.PageOnly);
        Assert.Equal("https://s6.kkphimplayer6.com/164/index.m3u8", (await Site.VideoOf(s, s.Servers[2])).Url);
        Assert.Same(yan, KkPhim.Attach(yan, new()));
        // Trang mở thẳng trang tập (hoathinh3d.pw): trình phát của trang thành server đầu tiên
        var pw = new EpisodeSource { EpUrl = "https://hoathinh3d.pw/xem-phim-gia-thien/tap-2-sv1.html", PageMode = true };
        Assert.True(pw.PageOnly);
        var p = KkPhim.Attach(pw, kk);
        Assert.Equal(EpisodeSource.PageServer, p.Servers[0].Type);
        Assert.False(p.PageOnly);
        Assert.True(p.PageMode);
    }

    [Fact]
    public async Task SiteDownStillPlaysFromKkAndSlowKkIsSkipped()
    {
        const string ep = "https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html";
        var kk = KkPhim.Pick(Detail, 164, null);
        var site = new EpisodeSource
        {
            EpUrl = ep, Servers = new() { new("VIETSUB 1080 V1", "tiktik") }, PostId = "1", Ep = "tap-164", Sv = "1",
            PlayerUrl = "https://yanhh3d.ee/player/player.php",
        };
        // Trang lỗi → chỉ còn server KKPhim, phát được
        var only = await KkPhim.Combine(ep, () => throw new AppError("Trang trả về lỗi HTTP 522"), () => Task.FromResult(kk));
        Assert.Equal(new[] { "kk-vs", "kk-tm", "kk-tm-ai" }, only.Servers.Select(x => x.Type));
        Assert.Equal("https://a.kvp726.com/164/index.m3u8", (await Site.VideoOf(only, only.Servers[0])).Url);
        // Trang lỗi và KKPhim cũng không có → báo lỗi của trang
        var err = await Assert.ThrowsAsync<AppError>(() =>
            KkPhim.Combine(ep, () => throw new AppError("HTTP 522"), () => Task.FromResult(new List<(Server, Video)>())));
        Assert.Equal("HTTP 522", err.Message);
        // KKPhim lỗi / quá chậm → phát bằng server của trang như cũ, không chờ mãi
        Assert.Same(site, await KkPhim.Combine(ep, () => Task.FromResult(site), () => throw new AppError("KK lỗi")));
        var t0 = Util.NowMs();
        var slow = await KkPhim.Combine(ep, () => Task.FromResult(site), async () =>
        {
            await Task.Delay(3_000);
            return kk;
        }, waitMs: 300);
        Assert.Same(site, slow);
        Assert.True(Util.NowMs() - t0 < 2_000);
        Assert.Equal(4, (await KkPhim.Combine(ep, () => Task.FromResult(site), () => Task.FromResult(kk))).Servers.Count);
    }

    /// Trang KKPhim (1.3): địa chỉ trong app theo www.kkphim.com, dữ liệu gọi thẳng phimapi.com.
    [Fact]
    public void KkPhimSiteUsesPublicApi()
    {
        const string kk = Prefs.KkSite;
        Assert.True(ApiSite.IsKnown($"{kk}/"));
        Assert.True(ApiSite.IsKk($"{kk}/phim/tien-nghich/tap-165?sv=0"));
        Assert.True(ApiSite.IsKk("https://kkphim.com"));
        Assert.False(ApiSite.IsKk("https://mephim.ddns.net/phim/tien-nghich"));
        Assert.Equal("https://phimapi.com/v1/api/danh-sach/phim-moi-cap-nhat?page=1&limit=24", ApiSite.Fallback(ApiSite.NewestUrl(kk)));
        Assert.Equal("https://phimapi.com/v1/api/tim-kiem?keyword=gi%C3%A0+thi%C3%AAn&page=1&limit=24",
            ApiSite.Fallback(ApiSite.SearchUrl(kk, "già thiên")));
        Assert.Equal("https://phimapi.com/v1/api/danh-sach/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh&page=1&limit=24",
            ApiSite.Fallback($"{kk}/api/movies/category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh&page=1&limit=24"));
        Assert.Equal($"{kk}/phim/tien-nghich", ApiSite.FilmUrl($"{kk}/phim/tien-nghich/tap-165?sv=0"));
        var body = """
        {"status":true,"data":{"items":[{"name":"Đấu Phá Thương Khung (Phần 5)","slug":"dau-pha-thuong-khung-phan-5",
        "poster_url":"upload/vod/20240822-1/58a4.jpg","episode_current":"Tập 218"}],
        "params":{"pagination":{"totalItems":462,"totalItemsPerPage":24,"currentPage":1,"totalPages":20}},
        "APP_DOMAIN_CDN_IMAGE":"https://phimimg.com"}}
        """;
        var p = ApiSite.ParseList(Json(body), $"{kk}/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24");
        Assert.Equal("https://www.kkphim.com/phim/dau-pha-thuong-khung-phan-5", Assert.Single(p.Films).Url);
        Assert.Equal("https://phimimg.com/upload/vod/20240822-1/58a4.jpg", p.Films[0].Poster);
        Assert.Equal($"{kk}/api/movies/category/hoat-hinh?country=trung-quoc&page=2&limit=24", p.NextUrl);
    }

    [Fact]
    public void BareKkDomainBecomesTheWwwSite()
    {
        // kkphim.com trần bị nhà mạng chặn, và trùng với trang mẫu www.kkphim.com trong danh sách
        Assert.Equal(Prefs.KkSite, Prefs.NormalizeUrl("kkphim.com"));
        Assert.Equal(Prefs.KkSite, Prefs.NormalizeUrl("https://KKPhim.com/"));
        Assert.Equal(Prefs.KkSite, Prefs.NormalizeUrl("www.kkphim.com"));
        Assert.Equal("https://yanhh3d.ee", Prefs.NormalizeUrl("yanhh3d.ee/"));
    }
}
