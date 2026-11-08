using System.Text;
using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

using JObj = Dictionary<string, object?>;

/// NguonC + VSMOV + CLB Phim Xưa (k-20) (11/2026) — cùng ca với NguonCTest.kt / StremioTest.kt / NguonCTests.swift.
public class NguonCTests
{
    private const string O = "https://phim.nguonc.com";
    private const string K = "https://sc.k-20.xyz";

    private static JObj Json(string s) => (JObj)JsonScan.Parse(s)!;

    private static readonly JObj Detail = Json("""
        {"status":"success","movie":{"name":"Mục Thần Ký","original_name":"Mu Shen Ji, Tales of Herding Gods",
        "slug":"muc-than-ky","description":"<p>Tần Mục lớn lên ở <b>Tàn Lão Thôn</b>.</p>","total_episodes":120,
        "current_episode":"Tập 108","time":"21 phút/tập","quality":"HD","language":"Vietsub","year":2023,
        "thumb_url":"https://img.nguonc.com/images/a.jpg","poster_url":"https://img.nguonc.com/images/b.jpg",
        "category":{"1":{"group":{"name":"Định dạng"},"list":[{"name":"Phim bộ"}]},
                    "2":{"group":{"name":"Thể loại"},"list":[{"name":"Hành động"},{"name":"Hoạt hình"}]},
                    "4":{"group":{"name":"Quốc gia"},"list":[{"name":"Trung Quốc"}]}},
        "episodes":[
          {"server_name":"Vietsub #1","items":[
            {"name":"1","slug":"tap-1","embed":"https://embed1.streamc.xyz/embed.php?hash=a1"},
            {"name":"107","slug":"tap-107","embed":"https://embed14.streamc.xyz/embed.php?hash=a107"},
            {"name":"108","slug":"tap-108","embed":"https://embed10.streamc.xyz/embed.php?hash=a108"},
            {"name":"109","slug":"tap-109","embed":""}]},
          {"server_name":"Thuyết Minh #1","items":[
            {"name":"108","slug":"tap-108","embed":"https://embed10.streamc.xyz/embed.php?hash=t108"}]}]}}
        """);

    private static EpisodeSource YanSite(string ep) => new()
    {
        EpUrl = ep, Servers = new() { new Server("VIETSUB 1080 V1", "tiktik") }, PostId = "1", Ep = "tap-108", Sv = "1",
        PlayerUrl = "https://yanhh3d.ee/player/player.php",
    };

    [Fact]
    public void ParsesListWithPortraitPostersAndPaging()
    {
        var root = Json("""
            {"status":"success","paginate":{"current_page":2,"total_page":3365},"items":[
             {"name":"Mục Thần Ký","slug":"muc-than-ky","current_episode":"Tập 108",
              "thumb_url":"https://img.nguonc.com/images/thumb.jpg","poster_url":"https://img.nguonc.com/images/wide.jpg"},
             {"name":"Doraemon","slug":"doraemon","current_episode":"Hoàn tất (26/26)","poster_url":"https://img.nguonc.com/images/d.jpg"},
             {"name":"","slug":"rong"}]}
            """);
        var p = NguonC.ParseList(root, $"{O}/api/films/danh-sach/hoat-hinh?page=2");
        Assert.Equal(new[] { "Mục Thần Ký", "Doraemon" }, p.Films.Select(f => f.Title));
        Assert.Equal($"{O}/phim/muc-than-ky", p.Films[0].Url);
        Assert.Equal("https://img.nguonc.com/images/thumb.jpg", p.Films[0].Poster);
        Assert.Equal(new[] { "Tập 108", "Tập 26" }, p.Films.Select(f => f.Badge));
        Assert.Equal($"{O}/api/films/danh-sach/hoat-hinh?page=3", p.NextUrl);
    }

    [Fact]
    public void ParsesDetailGroupsAndInfo()
    {
        var d = NguonC.ParseDetail(Detail, $"{O}/phim/muc-than-ky");
        Assert.Equal("Mu Shen Ji, Tales of Herding Gods", d.OtherTitle);
        Assert.Equal("Tần Mục lớn lên ở Tàn Lão Thôn.", d.Description);
        Assert.Equal("Tập 108  •  21 phút/tập  •  Vietsub  •  Trung Quốc  •  2023  •  Hành động, Hoạt hình", d.Info);
        Assert.Equal(new[] { "Vietsub #1", "Thuyết Minh #1" }, d.Groups.Select(g => g.Name));
        Assert.Equal(new[] { "Tập 1", "Tập 107", "Tập 108" }, d.Groups[0].Episodes.Select(e => e.Title));
        Assert.Equal($"{O}/phim/muc-than-ky/tap-108?sv=1", d.Groups[1].Episodes[0].Url);
        Assert.Equal($"{O}/phim/muc-than-ky", NguonC.FilmUrl($"{O}/phim/muc-than-ky/tap-108?sv=1"));
    }

    [Fact]
    public async Task BackupPicksSameEpisodeAndPlaysFromLinks()
    {
        var m = J.Obj(Detail, "movie")!;
        var all = NguonC.Pick(m, 108, null);
        Assert.Equal(new[] { "nc-vs", "nc-tm" }, all.Select(x => x.Item1.Type));
        Assert.Equal(new[] { "NguonC Vietsub", "NguonC Thuyết Minh" }, all.Select(x => x.Item1.Label));
        Assert.Equal(new[] { "nc-tm" }, NguonC.Pick(m, 108, "tm").Select(x => x.Item1.Type));
        Assert.Empty(NguonC.Pick(m, 109, null));
        Assert.True(KkPhim.Matches(m, new[] { "Tales of Herding Gods" }.Select(KkPhim.Norm).ToHashSet()));
        var s = KkPhim.Attach(YanSite("https://yanhh3d.ee/tu-tien/muc-than-ky/tap-108.html"), all);
        Assert.Equal(new[] { "tiktik", "nc-vs", "nc-tm" }, s.Servers.Select(x => x.Type));
        Assert.Equal("https://embed10.streamc.xyz/embed.php?hash=t108", (await Site.VideoOf(s, s.Servers[2])).Url);
    }

    [Fact]
    public async Task SlowBackupDoesNotHideTheFastOne()
    {
        var ep = "https://yanhh3d.ee/tu-tien/muc-than-ky/tap-108.html";
        var nc = new List<(Server, Video)> { (new Server("NguonC Vietsub", "nc-vs"), new Video("https://embed1.streamc.xyz/embed.php?hash=x")) };
        var kk = new List<(Server, Video)> { (new Server("KKPhim Vietsub", "kk-vs"), new Video("https://a.kvp726.com/108/index.m3u8")) };
        var t0 = Util.NowMs();
        var s = await KkPhim.CombineAll(ep, () => Task.FromResult(YanSite(ep)), new()
        {
            async () => { await Task.Delay(3000); return kk; }, () => Task.FromResult(nc),
        }, waitMs: 400);
        Assert.Equal(new[] { "tiktik", "nc-vs" }, s.Servers.Select(x => x.Type));
        Assert.True(Util.NowMs() - t0 < 2000);
        var both = await KkPhim.CombineAll(ep, () => Task.FromResult(YanSite(ep)), new() { () => Task.FromResult(kk), () => Task.FromResult(nc) });
        Assert.Equal(new[] { "tiktik", "kk-vs", "nc-vs" }, both.Servers.Select(x => x.Type));
        var down = await KkPhim.CombineAll(ep, () => throw new AppError("HTTP 522"),
            new() { () => throw new AppError("KK lỗi"), () => Task.FromResult(nc) });
        Assert.Equal(new[] { "nc-vs" }, down.Servers.Select(x => x.Type));
    }

    [Fact]
    public void VsmovListsAndUrls()
    {
        const string vs = "https://vsmov.com";
        Assert.True(ApiSite.IsKnown($"{vs}/") && ApiSite.IsVs($"{vs}/phim/tien-nghich"));
        Assert.Equal($"{vs}/api/danh-sach/phim-moi-cap-nhat?page=1", ApiSite.NewestUrl(vs));
        Assert.Equal($"{vs}/api/the-loai/kiem-hiep?page=1", ApiSite.Filters(vs).Genres.First(g => g.Title == "Kiếm Hiệp").Url);
        var p = ApiSite.ParseList(Json("""
            {"status":true,"items":[{"name":"Túy Linh Lung","slug":"tuy-linh-lung","poster_url":"https://vsmov.com/storage/images/e6.jpg"}],
             "pagination":{"totalItems":2804,"totalItemsPerPage":20,"currentPage":1,"totalPages":141}}
            """), $"{vs}/api/the-loai/hoat-hinh?country=trung-quoc&page=1");
        Assert.Equal($"{vs}/phim/tuy-linh-lung", p.Films.Single().Url);
        Assert.Equal($"{vs}/api/the-loai/hoat-hinh?country=trung-quoc&page=2", p.NextUrl);
        var d = ApiSite.ParseDetail(Json("""
            {"status":true,"movie":{"name":"Túy Linh Lung"},"episodes":[{"server_name":"Lồng tiếng\r\n      #1","server_data":[
             {"name":"1","slug":"tap-1","link_embed":"https://v15.streamvsmov.com/video/fc1d03d9"}]}]}
            """), $"{vs}/phim/tuy-linh-lung");
        Assert.Equal("Lồng tiếng #1", d.Groups.Single().Name);
        Assert.Equal("Tập 1", d.Groups.Single().Episodes.Single().Title);
        Assert.Equal($"{vs}/phim/tuy-linh-lung/tap-1?sv=0", d.Groups.Single().Episodes.Single().Url);
    }

    [Fact]
    public void VsmovHlsFromPlayerPage()
    {
        var signed = """enableSignedUrl: true, signedMasterUrl: "https://v2.streamvsmov.com/stream/55010e78/master.m3u8?expires=1794131570&signature=ca8c", volume: 100 }; const baseUrl = "https://v2.streamvsmov.com"; const videoHash = "55010e78";""";
        Assert.Equal("https://v2.streamvsmov.com/stream/55010e78/master.m3u8?expires=1794131570&signature=ca8c", ApiSite.VsHlsFrom(signed));
        var plain = """enableSignedUrl: false, signedMasterUrl: "", const baseUrl = "https://v15.streamvsmov.com"; const videoHash = "fc1d03d9-0667";""";
        Assert.Equal("https://v15.streamvsmov.com/stream/fc1d03d9-0667/master.m3u8", ApiSite.VsHlsFrom(plain));
        Assert.Null(ApiSite.VsHlsFrom("<html>lỗi</html>"));
    }

    [Fact]
    public void StremioCatalogUrlsAndPaging()
    {
        Assert.Equal($"{K}/catalog/movie/clbpx-movie.json", Stremio.Catalog(K));
        Assert.Equal($"{K}/catalog/movie/clbpx-movie/genre=Th%E1%BB%83%20lo%E1%BA%A1i%3A%20Ki%E1%BA%BFm%20Hi%E1%BB%87p&skip=30.json",
            Stremio.Catalog(K, genre: "Kiếm Hiệp", skip: 30));
        Assert.Equal($"{K}/catalog/movie/clbpx-movie/search=thi%C3%AAn%20long.json", Stremio.SearchUrl(K, " thiên long "));
        var data = Encoding.UTF8.GetBytes("""
            {"metas":[
              {"id":"clbpx:nch1993","name":"Nguyên Chấn Hiệp 1993 – 20 Tập","poster":"https://clbphimxua.com/a.jpg"},
              {"id":"clbpx:harry-5","name":"Harry Bẩn Thỉu 5: Mồ Chôn Tội Ác 1988","poster":"https://clbphimxua.com/b.jpg"},
              {"id":"nguonc:khac","name":"Không phải CLBPX"}]}
            """);
        var p = Stremio.ParseList(data, Stremio.Catalog(K, genre: "Kiếm Hiệp", skip: 30));
        Assert.Equal(new[] { $"{K}/clbpx/nch1993", $"{K}/clbpx/harry-5" }, p.Films.Select(f => f.Url));
        Assert.Equal(new[] { "20 tập", null }, p.Films.Select(f => f.Badge));
        Assert.Equal(Stremio.Catalog(K, genre: "Kiếm Hiệp", skip: 32), p.NextUrl);
        Assert.Equal(Stremio.Catalog(K, skip: 2), Stremio.ParseList(data, Stremio.Catalog(K)).NextUrl);
        Assert.Null(Stremio.ParseList(data, Stremio.SearchUrl(K, "x")).NextUrl);
    }

    [Fact]
    public async Task StremioDetailAndMp4Servers()
    {
        var d = Stremio.ParseDetail(Encoding.UTF8.GetBytes("""
            {"meta":{"id":"clbpx:nch1993","name":"Nguyên Chấn Hiệp 1993 – 20 Tập","poster":"https://clbphimxua.com/p.jpg","releaseInfo":"1993",
             "videos":[{"id":"clbpx:nch1993:HFvsf0tTx","title":"Tập 02","season":1,"episode":2},
                       {"id":"clbpx:nch1993:LM4bPc-3a","title":"Tập 01","season":1,"episode":1}]}}
            """), $"{K}/clbpx/nch1993");
        Assert.Equal("1993", d.Info);
        Assert.Equal(new[] { "Tập 1", "Tập 2" }, d.Groups.Single().Episodes.Select(e => e.Title));
        Assert.Equal($"{K}/clbpx/nch1993/LM4bPc-3a", d.Groups.Single().Episodes[0].Url);
        Assert.Equal($"{K}/clbpx/nch1993", Stremio.FilmUrl($"{K}/clbpx/nch1993/LM4bPc-3a"));
        var movie = Stremio.ParseDetail(Encoding.UTF8.GetBytes("""{"meta":{"id":"clbpx:harry-5","name":"Harry 5"}}"""), $"{K}/clbpx/harry-5");
        Assert.Equal(new[] { $"{K}/clbpx/harry-5/full" }, movie.Groups.Single().Episodes.Select(e => e.Url));
        var s = Stremio.ParseSource(Encoding.UTF8.GetBytes("""
            {"streams":[
              {"title":"1080p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=5&size=3069863340"},
              {"title":"1080p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=5&size=1938835536"},
              {"title":"360p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=2&size=395465856"},
              {"title":"720p","url":""}]}
            """), $"{K}/clbpx/harry-5/full");
        Assert.Equal(new[] { "1080p · 3,1 GB", "1080p · 1,9 GB", "360p" }, s.Servers.Select(x => x.Label));
        var v = await Site.VideoOf(s, s.Servers[2]);
        Assert.Equal("https://sc.k-20.xyz/hx-mp4?embed=x&res=2&size=395465856", v.Url);
        Assert.True(v.File);
        Assert.Equal("CLB Phim Xưa (sc.k-20.xyz)", Site.DisplayName("sc.k-20.xyz"));
    }
}
