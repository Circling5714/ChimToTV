using System.Text;
using System.Text.Json;
using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

using JObj = Dictionary<string, object?>;

/// mephim.ddns.net — cùng ca với ApiSiteTests.swift / ApiSiteTest.kt.
public class ApiSiteTests
{
    private const string O = "https://mephim.ddns.net";

    private static JObj Json(string s) => (JObj)JsonScan.Parse(s)!;

    [Fact]
    public void ParsesListWithRelativePostersAndPaging()
    {
        var body = """
        {"status":true,"msg":"done","data":{"items":[
        {"name":"Tiên Nghịch","slug":"tien-nghich","poster_url":"uploads/movies/20260720/tien-nghich-poster.webp",
         "thumb_url":"uploads/movies/20260720/tien-nghich-thumb.webp","episode_current":"Tập 165"},
        {"name":"Avatar","slug":"avatar","poster_url":"https://phimimg.com/upload/vod/a.jpg","episode_current":"Full"},
        {"name":"Hết","slug":"het","thumb_url":"uploads/movies/x-thumb.webp","episode_current":"Hoàn Tất (12/12)"}],
        "params":{"pagination":{"totalItems":3629,"totalItemsPerPage":24,"currentPage":2,"totalPages":152}},
        "APP_DOMAIN_CDN_IMAGE":"https://phimimg.com"}}
        """;
        var url = $"{O}/api/movies/category/hoat-hinh?country=trung-quoc&page=2&limit=24";
        var p = ApiSite.ParseList(Json(body), url);
        Assert.Equal(new[] { "Tiên Nghịch", "Avatar", "Hết" }, p.Films.Select(f => f.Title));
        Assert.Equal($"{O}/phim/tien-nghich", p.Films[0].Url);
        Assert.Equal("https://phimimg.com/uploads/movies/20260720/tien-nghich-poster.webp", p.Films[0].Poster);
        Assert.Equal("https://phimimg.com/upload/vod/a.jpg", p.Films[1].Poster);
        Assert.Equal("https://phimimg.com/uploads/movies/x-thumb.webp", p.Films[2].Poster);
        Assert.Equal(new[] { "Tập 165", "Full", "Tập 12" }, p.Films.Select(f => f.Badge));
        Assert.Equal($"{O}/api/movies/category/hoat-hinh?country=trung-quoc&page=3&limit=24", p.NextUrl);
        var last = body.Replace("\"currentPage\":2", "\"currentPage\":152");
        Assert.Null(ApiSite.ParseList(Json(last), url).NextUrl);
    }

    [Fact]
    public void ParsesTrendingItemsAtRoot()
    {
        var body = """
        {"status":true,"cdnDomain":"https://phimimg.com/uploads/movies","items":[
        {"name":"VisionQuest","slug":"vision","poster_url":"https://image.tmdb.org/t/p/w500/x.jpg","episode_current":"Tập 6"}]}
        """;
        var films = ApiSite.ParseItems(Json(body), O);
        Assert.Equal(new[] { $"{O}/phim/vision" }, films.Select(f => f.Url));
        Assert.Equal("https://image.tmdb.org/t/p/w500/x.jpg", films[0].Poster);
    }

    [Fact]
    public void ParsesDetailGroupsPerLanguage()
    {
        var body = """
        {"status":true,"movie":{"name":"Tiên Nghịch","origin_name":"Renegade Immortal",
        "content":"<p>Phim kể về <b>Vương Lâm</b>, một thiếu niên bình thường.</p>",
        "poster_url":"https://phimimg.com/uploads/movies/20260720/tien-nghich-poster.webp","episode_current":"Tập 165",
        "time":"23 phút/tập","lang":"Vietsub + Thuyết Minh","year":2023,
        "country":[{"name":"Trung Quốc"}],"category":[{"name":"Cổ Trang"},{"name":"Hoạt Hình"}]},
        "episodes":[
         {"server_name":"Vietsub","server_data":[
           {"name":"Tập 01","slug":"tap-01","link_embed":"https://player.phimapi.com/player/?url=a","link_m3u8":"https://s4.phim1280.tv/a/index.m3u8"},
           {"name":"Tập 02","slug":"tap-02","link_embed":"","link_m3u8":"https://s4.phim1280.tv/b/index.m3u8"},
           {"name":"Tập 03","slug":"tap-03","link_embed":"","link_m3u8":""}]},
         {"server_name":"Thuyết Minh","server_data":[
           {"name":"Tập 01","slug":"tap-01","link_embed":"","link_m3u8":"https://s5.phim1280.tv/c/index.m3u8"}]}]}
        """;
        var d = ApiSite.ParseDetail(Json(body), $"{O}/phim/tien-nghich");
        Assert.Equal("Tiên Nghịch", d.Title);
        Assert.Equal("Renegade Immortal", d.OtherTitle);
        Assert.Equal("Phim kể về Vương Lâm, một thiếu niên bình thường.", d.Description);
        Assert.Equal("Tập 165  •  23 phút/tập  •  Vietsub + Thuyết Minh  •  Trung Quốc  •  2023  •  Cổ Trang, Hoạt Hình", d.Info);
        Assert.Equal(new[] { "Vietsub", "Thuyết Minh" }, d.Groups.Select(g => g.Name));
        Assert.Equal(new[] { "Tập 1", "Tập 2" }, d.Groups[0].Episodes.Select(e => e.Title));
        Assert.Equal($"{O}/phim/tien-nghich/tap-01?sv=0", d.Groups[0].Episodes[0].Url);
        Assert.Equal($"{O}/phim/tien-nghich/tap-01?sv=1", d.Groups[1].Episodes[0].Url);
    }

    [Fact]
    public void FallsBackToKkPhimApi()
    {
        Assert.Equal("https://phimapi.com/v1/api/danh-sach/phim-moi-cap-nhat?page=2&limit=24", ApiSite.Fallback($"{O}/api/movies/newest?page=2&limit=24"));
        Assert.Equal("https://phimapi.com/v1/api/danh-sach/hoat-hinh?country=trung-quoc&page=1&limit=24",
            ApiSite.Fallback($"{O}/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24"));
        Assert.Equal("https://phimapi.com/v1/api/the-loai/co-trang?page=1&limit=24", ApiSite.Fallback($"{O}/api/movies/the-loai/co-trang?page=1&limit=24"));
        Assert.Equal("https://phimapi.com/v1/api/tim-kiem?keyword=ti%C3%AAn&page=1&limit=24",
            ApiSite.Fallback($"{O}/api/movies/search?keyword=ti%C3%AAn&page=1&limit=24"));
        Assert.Equal("https://phimapi.com/phim/tien-nghich", ApiSite.Fallback($"{O}/api/movies/detail/tien-nghich"));
        Assert.Null(ApiSite.Fallback($"{O}/api/movies/trending"));
    }

    [Fact]
    public void FilmUrlFromEpisode()
    {
        Assert.Equal($"{O}/phim/tien-nghich", ApiSite.FilmUrl($"{O}/phim/tien-nghich/tap-01?sv=1"));
        Assert.Equal($"{O}/phim/tien-nghich", ApiSite.FilmUrl($"{O}/phim/tien-nghich"));
        Assert.True(ApiSite.IsKnown($"{O}/"));
        Assert.Equal($"{O}/api/movies/search?keyword=ti%C3%AAn+ngh%E1%BB%8Bch&page=1&limit=24", ApiSite.SearchUrl(O, " tiên nghịch "));
    }
}

/// hoathinh3d.pw: danh sách nạp bằng API JSON — cùng ca với HalimApiTests.swift.
public class HalimApiTests
{
    private const string PageHtml = """
    <main id="main-contents"><div class="mv-grid" id="mvGrid"></div></main>
    <script>(function(){ const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/halim\/v1\/movies"; const PER_PAGE = 20;
    const SORTBY = "default"; })();</script>
    """;

    [Fact]
    public void FindsApiInPage()
    {
        Assert.Equal("https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=1&per_page=20", Site.HalimApi.FirstPage(PageHtml));
        Assert.Equal("https://hoathinh3d.pw/wp-json/halim/v1/ongoing?page=1&per_page=20",
            Site.HalimApi.FirstPage("""const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/halim\/v1\/ongoing";"""));
        Assert.Null(Site.HalimApi.FirstPage("""<div class="halim-item"></div>"""));
        Assert.Equal("https://hoathinh3d.pw/wp-json/movie-schedule/v1/daily",
            Site.HalimApi.ScheduleApi("""const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/movie-schedule\/v1\/daily";"""));
    }

    [Fact]
    public void ParsesItemsAndNextPage()
    {
        var json = """
        {"success":true,"page":1,"per_page":20,"total":33,"has_more":true,"items":[
        {"id":1,"title":"Tiên Nghịch","permalink":"https:\/\/hoathinh3d.pw\/tien-nghich","poster_url":"https:\/\/x\/a.jpg","average_rating":4.6,"latest_episode":"Tập 165"},
        {"id":2,"rank":2,"title":"Mục Thần Ký","permalink":"https:\/\/hoathinh3d.pw\/muc-than-ky","poster_url":"https:\/\/x\/b.jpg"},
        {"id":3,"title":"Phim","permalink":"https:\/\/hoathinh3d.pw\/phim","poster_url":"","latest_episode":"12"}]}
        """;
        var url = "https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=1&per_page=20";
        var p = Site.HalimApi.Parse(Encoding.UTF8.GetBytes(json), url);
        Assert.Equal(new[] { "Tiên Nghịch", "Mục Thần Ký", "Phim" }, p.Films.Select(f => f.Title));
        Assert.Equal(new[] { "Tập 165", "Top 2", "Tập 12" }, p.Films.Select(f => f.Badge));
        Assert.Null(p.Films[2].Poster);
        Assert.Equal("https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=2&per_page=20", p.NextUrl);
        var last = json.Replace("\"has_more\":true", "\"has_more\":false");
        Assert.Null(Site.HalimApi.Parse(Encoding.UTF8.GetBytes(last), url).NextUrl);
    }

    [Fact]
    public void ParsesScheduleDay()
    {
        var json = """
        {"success":true,"day":"thu-nam","early_movies":[{"title":"Tru Tiên","permalink":"https:\/\/h\/tru-tien",
        "thumbnail":"https:\/\/x\/t.jpg","latest_episode":"Tập 91","is_early_screening":true,"early_screening_time":"18:00"}],
        "regular_movies":[{"title":"A","permalink":"https:\/\/h\/a","thumbnail":"","latest_episode":"Tập 17"}]}
        """;
        var films = Site.HalimApi.ParseDay(Encoding.UTF8.GetBytes(json));
        Assert.Equal(new[] { "Tru Tiên", "A" }, films.Select(f => f.Title));
        Assert.Equal("18:00 · Tập 91", films[0].Badge);
        Assert.Equal("https://x/t.jpg", films[0].Poster);
    }
}

/// Trang Next.js kiểu PhimMoi — cùng ca với NextSiteTests.swift.
public class NextSiteTests
{
    private const string Base = "https://phim.test";

    private static string Quote(string s)
    {
        var j = JsonSerializer.Serialize(s, new JsonSerializerOptions { Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping });
        return j;
    }

    /// Trang có dữ liệu cắt thành nhiều đoạn push (trang thật cắt ngay giữa đối tượng JSON).
    private static AngleSharp.Dom.IDocument Page(string url, string data, int parts = 3, string extra = "")
    {
        var size = (data.Length + parts - 1) / parts;
        var scripts = new StringBuilder();
        for (var i = 0; i < data.Length; i += size)
            scripts.Append($"<script>self.__next_f.push([1,{Quote(data.Substring(i, Math.Min(size, data.Length - i)))}])</script>");
        return Html.Parse($"<html><head><script>self.__next_f=self.__next_f||[]</script></head><body>{scripts}{extra}</body></html>", url);
    }

    private static string Movie(int id, string slug, string name, string status) =>
        $$"""{"id":{{id}},"name":"{{name}}","origin_name":"","slug":"{{slug}}","description":"$27","poster_url":"/public/images/posters/{{slug}}.webp",""" +
        "\"" + $$"""display_status":"{{status}}","country":{"name":"Trung Quốc","slug":"trung-quoc"},"genres":[{"id":11,"name":"Hoạt Hình","slug":"hoat-hinh","is_active":true}]}""";

    private static string Eps(int n, string lang) =>
        "[" + string.Join(",", Enumerable.Range(1, n).Select(i =>
            $$"""{"id":{{i}},"episode_label":"Tập {{i}}","slug":"tap-{{i}}","is_embed":false,"language":"{{lang}}","name":"Tập {{i}}"}""")) + "]";

    private const string Sources =
        """[{"server_name":"KK Phim","server_slug":"kk","language_name":"Vietsub","language_slug":"vietsub","total_episodes":3,"priority_order":2},""" +
        """{"server_name":"NguonC","server_slug":"nc","language_name":"Vietsub","language_slug":"vietsub","total_episodes":3,"priority_order":3},""" +
        """{"server_name":"KK Phim","server_slug":"kk","language_name":"Thuyết Minh","language_slug":"thuyetminh","total_episodes":2,"priority_order":2}]""";

    [Fact]
    public void Filters()
    {
        static string Item(int id, string name, string slug, bool active = true) =>
            $$"""{"id":{{id}},"name":"{{name}}","slug":"{{slug}}","is_active":{{(active ? "true" : "false")}}}""";
        var data = """1:{"results":[""" + Movie(1, "a", "A", "Tập 1") + """],"genres":[""" +
                   string.Join(",", Item(1, "Âm Nhạc", "am-nhac"), Item(2, "Cổ Trang", "co-trang"), Item(3, "Ẩn", "an", false)) +
                   """],"countries":[""" +
                   string.Join(",", Item(54, "1983", "1983"), Item(10, "Anh", "anh"), Item(60, "KW", "kw"), Item(61, "Hàn Quốc", "han-quoc"),
                       Item(62, "hàn quốc", "han-quoc-2")) + "]}\n";
        var doc = Page($"{Base}/loc-phim", data);
        var f = NextSite.ParseFilters(doc, NextSite.Data(doc)!);
        Assert.Equal(new[] { "Âm Nhạc", "Cổ Trang" }, f.Genres.Select(c => c.Title));
        Assert.Equal($"{Base}/loc-phim?genre=co-trang&sort=updated_at&order=desc", f.Genres[1].Url);
        Assert.Equal(new[] { "Anh", "Hàn Quốc" }, f.Countries.Select(c => c.Title));
        Assert.Equal($"{Base}/loc-phim?country=han-quoc&sort=updated_at&order=desc", f.Countries[1].Url);
    }

    [Fact]
    public void NotNextJs()
    {
        Assert.Null(NextSite.Data(Html.Parse("<html><body><script>var a=1</script><div class=flw-item></div></body></html>", $"{Base}/")));
    }

    [Fact]
    public void HomeSections()
    {
        var data = """0:["$","div",null,{"children":[""" + Movie(1, "slide-a", "Phim Nổi Bật", "Vietsub 4") + "]}]\n" +
                   """5:["$","$L26","Trung Quốc Hot Nhất",{"movies":[""" + Movie(2, "tien-nghich", "Tiên Nghịch", "Thuyết Minh 107, Vietsub 165") + "," +
                   Movie(3, "phim-le", "Phim Lẻ", "Vietsub Full") + "]}]";
        var doc = Page($"{Base}/", data, extra: """<a href="/the-loai/hanh-dong">Hành Động</a>""");
        var h = Site.ParseHome(doc);
        Assert.Equal(new[] { "Nổi bật", "Trung Quốc Hot Nhất" }, h.Sections.Select(s => s.Title));
        var f = h.Sections[1].Films;
        Assert.Equal(new[] { "Tiên Nghịch", "Phim Lẻ" }, f.Select(x => x.Title));
        Assert.Equal($"{Base}/phim/tien-nghich", f[0].Url);
        Assert.Equal($"{Base}/public/images/posters/tien-nghich.webp", f[0].Poster);
        Assert.Equal(new[] { "Tập 165", "Full" }, f.Select(x => x.Badge));
        Assert.Equal("Hoạt hình Trung Quốc", h.Categories.First().Title);
        Assert.Equal($"{Base}/the-loai/hanh-dong", h.Categories.Last().Url);
    }

    [Fact]
    public void ListPaging()
    {
        var movies = string.Join(",", Enumerable.Range(1, 21).Select(i => Movie(i, $"phim-{i}", $"Phim {i}", $"Vietsub {i}")));
        var url = $"{Base}/loc-phim?genre=hoat-hinh&sort=updated_at&order=desc";
        var p = Site.ParseList(Page(url, """{"movies":[""" + movies + """],"total":50,"page":1,"limit":21,"filters":{}}"""), url);
        Assert.Equal(21, p.Films.Count);
        Assert.Equal($"{url}&page=2", p.NextUrl);
        var last = Site.ParseList(Page($"{url}&page=3", """{"movies":[""" + Movie(1, "a", "A", "") + """],"total":43,"page":3,"limit":21}"""), $"{url}&page=3");
        Assert.Null(last.NextUrl);
    }

    [Fact]
    public void DetailOfNext()
    {
        var data = """{"movie":{"id":283,"name":"Tiên Nghịch","origin_name":"Renegade Immortal","slug":"tien-nghich",""" +
                   "\"" + """description":"Cuộc hành trình của Vương Lâm, một thiếu niên bình thường.","poster_url":"/public/images/posters/p.webp",""" +
                   "\"" + """status_text":"Thuyết Minh 2, Vietsub 3","year":2023,"country":{"name":"Trung Quốc"},"genres":[{"name":"Hoạt Hình"}]},""" +
                   "\"" + """episode":{"episodes":""" + Eps(3, "vietsub") + ""","available_languages":[],"available_sources":""" + Sources +
                   ""","current_language":"vietsub","current_server":"kk"}}""";
        var d = Site.ParseDetail(Page($"{Base}/phim/tien-nghich", data), $"{Base}/phim/tien-nghich");
        Assert.Equal("Tiên Nghịch", d.Title);
        Assert.Equal("Renegade Immortal", d.OtherTitle);
        Assert.StartsWith("Thuyết Minh 2, Vietsub 3  •  Trung Quốc  •  2023", d.Info);
        Assert.Single(d.Groups);
        Assert.Equal("Vietsub", d.Groups[0].Name);
        Assert.Equal(new[] { "Tập 1", "Tập 2", "Tập 3" }, d.Groups[0].Episodes.Select(e => e.Title));
        Assert.Equal($"{Base}/xem-phim/tien-nghich/tap-3/vietsub", d.Groups[0].Episodes.Last().Url);
        Assert.Equal($"{Base}/public/images/posters/p.webp", d.Poster);
    }

    [Fact]
    public void SourceOfNext()
    {
        var epUrl = $"{Base}/xem-phim/tien-nghich/tap-3/vietsub";
        var data = """{"episode":{"episodes":""" + Eps(3, "vietsub") + ""","current_language":"vietsub","available_sources":""" + Sources + "," +
                   "\"" + """episode_sources":[{"server":{"id":3,"name":"KK Phim","slug":"kk"},"language":{"name":"Vietsub","slug":"vietsub"},"is_lock":false},""" +
                   """{"server":{"id":2,"name":"NguonC","slug":"nc"},"language":{"name":"Vietsub","slug":"vietsub"},"is_lock":false},""" +
                   """{"server":{"id":3,"name":"KK Phim","slug":"kk"},"language":{"name":"Thuyết Minh","slug":"thuyetminh"},"is_lock":false}],""" +
                   "\"" + """current_server":{"id":3,"name":"KK Phim","slug":"kk"},"current_source":{"server":"$1f:props:data:episode:current_server"},""" +
                   "\"" + """video":{"id":1,"link_embed":"https://player.test/?url=x","link_m3u8":"https://cdn.test/a/index.m3u8","is_embed":false}}}""";
        var s = Site.ParseSource(Page(epUrl, data), epUrl);
        Assert.True(s.Next);
        Assert.False(s.PageMode);
        Assert.Equal(new[] { "kk=KK Phim", "nc=NguonC" }, s.Servers.Select(x => $"{x.Type}={x.Label}"));
        Assert.Equal(new[] { "kk" }, s.Links.Keys);
        Assert.Equal("https://cdn.test/a/index.m3u8", s.Links["kk"].Url);
        Assert.Null(s.Links["kk"].Subtitle);
        Assert.Equal("tap-3", s.Ep);
        var embed = Site.ParseSource(Page(epUrl, data.Replace("https://cdn.test/a/index.m3u8", "")), epUrl);
        Assert.Equal("https://player.test/?url=x", embed.Links["kk"].Url);
    }

    [Fact]
    public void OwnServerWithSubtitles()
    {
        var epUrl = $"{Base}/xem-phim/bac-si-x/tap-1/song-ngu";
        var data = """{"episode":{"episodes":""" + Eps(1, "song-ngu") + ""","current_language":"song-ngu",""" +
                   "\"" + """episode_sources":[{"server":{"id":9,"name":"HD","slug":"hd"},"language":{"name":"Song Ngữ","slug":"song-ngu"},"is_lock":false}],""" +
                   "\"" + """current_server":{"id":9,"name":"HD","slug":"hd"},"video":{"id":1,"link_embed":"/embed/v2/abc","link_m3u8":"/public/m3u8/job_1.m3u8",""" +
                   "\"" + """is_embed":false,"subtitles":[{"url":"https://sub.test/zh","lang":"chi","label":"Chinese","isDefault":false},""" +
                   """{"url":"https://sub.test/vi","lang":"vie","label":"Vietnamese","isDefault":true}]}}}""";
        var s = Site.ParseSource(Page(epUrl, data), epUrl);
        Assert.Equal($"{Base}/public/m3u8/job_1.m3u8", s.Links["hd"].Url);
        Assert.Equal("https://sub.test/vi", s.Links["hd"].Subtitle);
    }

    [Fact]
    public void OddPosterDoesNotBreakList()
    {
        var odd = Movie(1, "phim-a", "Phim A", "Vietsub 3").Replace("/public/images/posters/phim-a.webp", "data:image/png;base64,AAAA");
        var url = $"{Base}/loc-phim?type=phim-bo";
        var p = Site.ParseList(Page(url, """{"movies":[""" + odd + "," + Movie(2, "phim-b", "Phim B", "Full") + """],"total":2,"page":1,"limit":21}"""), url);
        Assert.Equal(new[] { "Phim A", "Phim B" }, p.Films.Select(f => f.Title));
        Assert.Null(p.Films[0].Poster);
        Assert.Equal($"{Base}/public/images/posters/phim-b.webp", p.Films[1].Poster);
    }

    [Fact]
    public void BadgeOf()
    {
        Assert.Equal("Tập 8", NextSite.Badge("Song Ngữ 4, Thuyết Minh 8, Vietsub 8"));
        Assert.Equal("Full", NextSite.Badge("Thuyết Minh Full, Vietsub Full"));
        Assert.Null(NextSite.Badge(""));
    }
}
