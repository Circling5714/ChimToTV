using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// Bộ đọc trang HTML (yanhh3d / HaLim) trên HTML dựng tay — cùng ca với SiteTests.swift / SiteTest.kt.
public class SiteTests
{
    private const string Base = "https://hh.test";

    [Fact]
    public void HomeCardsUnderHeadings()
    {
        var html = """
        <html><body>
        <div class="block_area"><h2 class="cat-heading">Mới cập nhật</h2>
          <div class="flw-item"><div class="film-poster"><img data-src="/img/a.jpg" alt="Ảnh A">
            <a class="film-poster-ahref" href="/phim/a"></a></div><div class="tick-rate">Tập 5</div>
            <h3 class="film-name"><a href="/phim/a">Phim A</a></h3></div>
          <div class="flw-item"><a href="/phim/b" title="Phim B"><img src="https://cdn.test/b.jpg"></a></div>
        </div>
        <div class="block_area-realtime"><h2 class="cat-heading"></h2>
          <li class="item-top"><a href="/phim/c"><h4>Phim C</h4></a></li>
        </div>
        <nav><ul><li class="menu-item"><a href="/the-loai/tien-hiep">Tiên Hiệp</a></li>
          <li class="menu-item"><a href="#">Thể loại</a></li><li class="menu-item"><a href="/dang-nhap">Đăng nhập</a></li></ul></nav>
        </body></html>
        """;
        var h = Site.ParseHome(Html.Parse(html, Base + "/"));
        Assert.Equal(new[] { "Mới cập nhật", "Xem nhiều" }, h.Sections.Select(s => s.Title));
        var a = h.Sections[0].Films[0];
        Assert.Equal("Phim A", a.Title);
        Assert.Equal($"{Base}/phim/a", a.Url);
        Assert.Equal($"{Base}/img/a.jpg", a.Poster);
        Assert.Equal("Tập 5", a.Badge);
        Assert.Equal("Phim B", h.Sections[0].Films[1].Title);
        Assert.Equal(new[] { "Phim C" }, h.Sections[1].Films.Select(f => f.Title));
        Assert.Equal(new[] { "Tiên Hiệp" }, h.Categories.Select(c => c.Title));
    }

    [Fact]
    public void DetailEpisodesSortedAndGrouped()
    {
        var html = """
        <html><head><meta property="og:image" content="https://cdn.test/p.jpg"></head><body>
        <h1 class="film-name">Tiên Nghịch</h1>
        <div class="film-description"><div class="text">Cuộc hành trình của Vương Lâm, một thiếu niên bình thường.</div></div>
        <div class="halim-server"><span class="halim-server-name">#Thuyết Minh:</span><ul class="halim-list-eps">
          <li><a href="/xem/tn/tap-2-tm">2</a></li><li><a href="/xem/tn/tap-1-tm">1</a></li><li><a href="/xem/tn/tap-10-tm">10</a></li></ul></div>
        <div class="halim-server"><span class="halim-server-name">Vietsub</span><ul class="halim-list-eps">
          <li><a href="/xem/tn/tap-1">Tập 1</a></li></ul></div>
        <div class="category"><a rel="category tag" href="/the-loai/a">Tiên Hiệp</a></div>
        </body></html>
        """;
        var d = Site.ParseDetail(Html.Parse(html, $"{Base}/phim/tn"), $"{Base}/phim/tn");
        Assert.Equal("Tiên Nghịch", d.Title);
        Assert.Equal("https://cdn.test/p.jpg", d.Poster);
        Assert.Equal(new[] { "Thuyết Minh", "Vietsub" }, d.Groups.Select(g => g.Name));
        Assert.Equal(new[] { "Tập 1", "Tập 2", "Tập 10" }, d.Groups[0].Episodes.Select(e => e.Title));
        Assert.Equal("Tiên Hiệp", d.Info);
        Assert.Equal("Tập 10", d.LatestEpisode?.Title);
    }

    [Fact]
    public void SourceWithPlayerPhp()
    {
        var html = """
        <html><body>
        <div id="list_sv"><a data-type="tiktik">VIETSUB 1080 V1</a><a data-type="vip2">VIETSUB 1080 V2</a><a data-type="">x</a></div>
        <ul><li class="active"><a class="ep-item active" data-post-id="123" data-ep="165" data-sv="1" href="/xem/tn/tap-165">165</a></li></ul>
        <script>jQuery.ajax({ url: '/player/player.php', data: { action: 'dox_ajax_player' } });</script>
        </body></html>
        """;
        var epUrl = $"{Base}/xem/tn/tap-165";
        var s = Site.ParseSource(Html.Parse(html, epUrl), epUrl);
        Assert.False(s.PageMode);
        Assert.Equal(new[] { "tiktik", "vip2" }, s.Servers.Select(x => x.Type));
        Assert.Equal("123", s.PostId);
        Assert.Equal("165", s.Ep);
        Assert.Equal($"{Base}/player/player.php", s.PlayerUrl);
    }

    [Fact]
    public void PageModeWithoutPlayer()
    {
        var epUrl = $"{Base}/xem/tn/tap-1.html";
        var s = Site.ParseSource(Html.Parse("<html><body><div id='player'></div></body></html>", epUrl), epUrl);
        Assert.True(s.PageMode);
        Assert.Equal("tap-1", s.Ep);
    }

    [Fact]
    public void DirectIframeIgnoresEmptyPlaceholder()
    {
        var epUrl = $"{Base}/xem/tn/tap-2";
        var html = """<div id="halim-player-wrapper"><iframe src=""></iframe><iframe src="//player.test/e/1"></iframe></div>""";
        var s = Site.ParseSource(Html.Parse(html, epUrl), epUrl);
        Assert.Equal("https://player.test/e/1", s.DirectEmbed);
        Assert.False(s.PageMode);
    }

    [Fact]
    public void JsonScanReadsInPlace()
    {
        var s = """xx{"a":"b\"cạ😀","n":[1,2.5,-3e2,true,null],"o":{"k":"v"}} trailing""";
        var v = JsonScan.Value(s, 2) as Dictionary<string, object?>;
        Assert.Equal("b\"cạ😀", v?["a"]);
        var n = v?["n"] as List<object?>;
        Assert.Equal(1L, n?[0]);
        Assert.Equal(2.5, n?[1]);
        Assert.Equal(-300.0, n?[2]);
        Assert.Equal(true, n?[3]);
        Assert.Null(n?[4]);
        Assert.Equal("v", (v?["o"] as Dictionary<string, object?>)?["k"]);
        Assert.Same(JsonScan.Fail, JsonScan.Value("""{"a":""", 0));
    }

    [Fact]
    public void UpdateSummaryAndVersions()
    {
        Assert.True(UpdateCheck.IsNewer("1.0.1", "1.0"));
        Assert.False(UpdateCheck.IsNewer("1.0", "1.0"));
        Assert.True(UpdateCheck.IsNewer("1.10", "1.9"));
        Assert.False(UpdateCheck.IsNewer("1.0.0+4265f12", "1.0"));
        Assert.True(UpdateCheck.IsNewer("1.0.1", "1.0.0+4265f12"));
        var notes = "Mở đầu\n## Có gì mới\n- **Theo dõi** phim\n- `x` y\n## Kỹ thuật\n- SHA-256 abc";
        Assert.Equal("Theo dõi phim\nx y", UpdateCheck.Summary(notes));
    }

    [Fact]
    public void AdBlockHosts()
    {
        Assert.True(AdBlock.IsAd("static.cloudflareinsights.com"));
        Assert.False(AdBlock.IsAd("ssl.p.jwpcdn.com"));
        Assert.Contains("*://*.doubleclick.net/*", AdBlock.Filters());
    }

    [Fact]
    public void ApiNumbersAreIntegers()
    {
        var a = JsonScan.Parse("[2,2.5,{\"n\":3}]") as List<object?>;
        Assert.IsType<long>(a![0]);
        Assert.IsType<double>(a[1]);
        Assert.Equal(3, J.Int(a[2] as Dictionary<string, object?>, "n"));
    }

    [Fact]
    public void UnblockSplitsClientHello()
    {
        // Gói bắt tay giả: 16 03 01 … rồi tên trang ở giữa
        var hello = new byte[] { 0x16, 0x03, 0x01, 0x00, 0x20, 1, 2, 3 }.Concat(System.Text.Encoding.ASCII.GetBytes("phimmois.fun")).Concat(new byte[] { 9, 9 }).ToArray();
        var c = Unblock.HelloChunks(hello, "PhimMois.fun", 1);
        Assert.Equal((0, 1, false), c[0]);
        Assert.Equal((1, 7, false), c[1]);
        Assert.Equal(new[] { 2, 2, 2, 2, 2, 2 }, c.Skip(2).Take(6).Select(x => x.Length));
        Assert.Equal((20, 2, false), c[^1]);
        Assert.Equal(hello.Length, c.Sum(x => x.Length));
        Assert.True(Unblock.HelloChunks(hello, "phimmois.fun", 2)[0].Ttl1);
        Assert.Single(Unblock.HelloChunks(hello, "phimmois.fun", 0));
        Assert.Single(Unblock.HelloChunks(new byte[] { 0x17, 3, 3, 0, 1, 5, 6 }, "x.y", 1));
    }

    [Fact]
    public void FormEncodedLikeJava()
    {
        Assert.Equal("ti%C3%AAn+ngh%E1%BB%8Bch", "tiên nghịch".FormEncoded());
        Assert.Equal("a-b_c.d*e", "a-b_c.d*e".FormEncoded());
    }
}
