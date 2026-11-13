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
        Assert.Equal((0, 1, false, false), c[0]);
        Assert.Equal((1, 7, false, false), c[1]);
        Assert.Equal(new[] { 2, 2, 2, 2, 2, 2 }, c.Skip(2).Take(6).Select(x => x.Length));
        Assert.Equal((20, 2, false, false), c[^1]);
        Assert.Equal(hello.Length, c.Sum(x => x.Length));
        Assert.DoesNotContain(c, x => x.Ttl1 || x.Hold);
        Assert.Single(Unblock.HelloChunks(hello, "phimmois.fun", 0));
        Assert.Single(Unblock.HelloChunks(new byte[] { 0x17, 3, 3, 0, 1, 5, 6 }, "x.y", 1));
    }

    /// Mức 2 trên Windows: Windows gửi lại MỌI byte chưa được xác nhận trong một gói, nên tên trang phải nằm sau điểm chờ
    /// (Hold) — lúc byte đầu (TTL 1) được gửi lại thì gói gửi lại chỉ có 4 byte đầu, chưa có tên trang.
    [Fact]
    public void UnblockLevel2HoldsNameUntilFirstBytesAcked()
    {
        var hello = new byte[] { 0x16, 0x03, 0x01, 0x00, 0x20, 1, 2, 3 }.Concat(System.Text.Encoding.ASCII.GetBytes("phimmois.fun")).Concat(new byte[] { 9, 9 }).ToArray();
        var c = Unblock.HelloChunks(hello, "phimmois.fun", 2);
        Assert.Equal((0, 1, true, false), c[0]);
        Assert.Equal((1, 1, false, false), c[1]);
        Assert.Equal((2, 1, false, false), c[2]);
        Assert.Equal((3, 1, false, true), c[3]);
        Assert.Equal((4, 4, false, false), c[4]);
        Assert.Equal(new[] { 2, 2, 2, 2, 2, 2 }, c.Skip(5).Take(6).Select(x => x.Length));
        Assert.Equal((20, 2, false, false), c[^1]);
        // Các mẩu nối tiếp nhau và phủ hết gói
        var at = 0;
        foreach (var x in c)
        {
            Assert.Equal(at, x.Start);
            at += x.Length;
        }
        Assert.Equal(hello.Length, at);
        // Một byte TTL 1, một điểm chờ; trước điểm chờ chỉ có 4 byte đầu
        Assert.Single(c, x => x.Ttl1);
        Assert.Single(c, x => x.Hold);
        Assert.All(c.Take(c.FindIndex(x => x.Hold) + 1), x => Assert.True(x.Start + x.Length <= 4));
        // Gói bắt tay không có tên trang (vd bên gọi gửi IP): vẫn 4 byte đầu rồi phần còn lại
        var other = Unblock.HelloChunks(hello, "khac.vn", 2);
        Assert.Equal(5, other.Count);
        Assert.Equal((4, hello.Length - 4, false, false), other[^1]);
    }

    /// Gửi thật qua một kết nối trong máy: mọi mức đều phải tới đủ, đúng thứ tự, và không treo ở điểm chờ của mức 2.
    [Fact]
    public void UnblockWriteHelloDeliversEveryByteInOrder()
    {
        var hello = new byte[] { 0x16, 0x03, 0x01, 0x00, 0x20, 1, 2, 3 }.Concat(System.Text.Encoding.ASCII.GetBytes("phimmois.fun")).Concat(new byte[] { 9, 9 }).ToArray();
        foreach (var (level, viaProxy) in new[] { (1, false), (2, false), (2, true) })
        {
            var listener = new System.Net.Sockets.TcpListener(System.Net.IPAddress.Loopback, 0);
            listener.Start();
            try
            {
                using var client = new System.Net.Sockets.Socket(System.Net.Sockets.SocketType.Stream, System.Net.Sockets.ProtocolType.Tcp) { NoDelay = true };
                client.Connect(listener.LocalEndpoint);
                using var server = listener.AcceptSocket();
                Assert.Equal(0, Unblock.InFlight(client)); // hỏi được trạng thái TCP trên Windows 10 / 11
                var sw = System.Diagnostics.Stopwatch.StartNew();
                Unblock.WriteHello(client, hello, "phimmois.fun", level, viaProxy);
                Assert.True(sw.ElapsedMilliseconds < 1000, $"mức {level} mất {sw.ElapsedMilliseconds} ms");
                client.Shutdown(System.Net.Sockets.SocketShutdown.Send);
                var got = new System.IO.MemoryStream();
                var b = new byte[64];
                int n;
                while ((n = server.Receive(b)) > 0) got.Write(b, 0, n);
                Assert.Equal(hello, got.ToArray());
            }
            finally
            {
                listener.Stop();
            }
        }
    }

    /// Yêu cầu HTTP thường: tách quanh tên trang trong dòng Host (như bản TV); mức 2 vẫn 4 byte đầu rồi điểm chờ.
    [Fact]
    public void UnblockSplitsHttpRequestAroundHost()
    {
        var req = System.Text.Encoding.ASCII.GetBytes("GET /phim HTTP/1.1\r\nHost: phim.test\r\nAccept: */*\r\n\r\n");
        string Parts(int level) => string.Join("|", Unblock.HelloChunks(req, "phim.test", level)
            .Select(x => System.Text.Encoding.ASCII.GetString(req, x.Start, x.Length)));
        Assert.Equal("G|ET /phim HTTP/1.1\r\nHo|st: |ph|im|.t|es|t|\r\nAccept: */*\r\n\r\n", Parts(1));
        Assert.Equal("G|E|T| |/phim HTTP/1.1\r\nHo|st: |ph|im|.t|es|t|\r\nAccept: */*\r\n\r\n", Parts(2));
        Assert.Single(Unblock.HelloChunks(System.Text.Encoding.ASCII.GetBytes("SSH-2.0-x\r\n"), "phim.test", 1));
    }

    [Fact]
    public void UnblockBaseDomainsAndLevel2Memo()
    {
        Assert.Equal("phimmoi.net", Unblock.BaseDomain("img.phimmoi.net"));
        Assert.Equal("abc.com.vn", Unblock.BaseDomain("www.abc.com.vn"));
        Assert.Equal("1.2.3.4", Unblock.BaseDomain("1.2.3.4"));
        Unblock.Level2Memo.Persist = false;
        const long now = 1_000_000_000_000;
        Unblock.Level2Memo.Load(new() { ["phimmoi.test"] = now - Unblock.Level2Memo.KeepMs - 1, ["kkphim.test"] = now - 1000 }, now);
        Assert.False(Unblock.Level2Memo.Needs("phimmoi.test", now)); // quá 7 ngày: về Mức 1
        Assert.True(Unblock.Level2Memo.Needs("www.kkphim.test", now)); // theo tên miền gốc
        Unblock.Level2Memo.Clear();
    }

    /// Máy đích giả nhà mạng chặn ở Mức 1: kết nối đầu nhận gói bắt tay nhưng không trả lời; kết nối sau trả lời ngay.
    /// HelloSplit (luồng của HttpClient) phải tự kết nối lại, gửi lại gói bắt tay theo Mức 2 — bên đọc nhận trả lời như thường.
    [Fact]
    public async Task UnblockHangAtLevel1RetriesAtLevel2()
    {
        var hello = new byte[] { 0x16, 0x03, 0x01, 0x00, 0x20, 1, 2, 3 }.Concat(System.Text.Encoding.ASCII.GetBytes("phim.test")).Concat(new byte[] { 9, 9 }).ToArray();
        var listener = new System.Net.Sockets.TcpListener(System.Net.IPAddress.Loopback, 0);
        listener.Start();
        var seen = new System.Collections.Concurrent.ConcurrentQueue<int>();
        var accepts = 0;
        _ = Task.Run(async () =>
        {
            while (true)
            {
                System.Net.Sockets.Socket s;
                try { s = await listener.AcceptSocketAsync(); } catch { return; }
                var first = Interlocked.Increment(ref accepts) == 1;
                _ = Task.Run(async () =>
                {
                    try
                    {
                        var b = new byte[256];
                        var n = 0;
                        while (n < hello.Length) n += s.Receive(b, n, b.Length - n, System.Net.Sockets.SocketFlags.None);
                        seen.Enqueue(n);
                        if (first) await Task.Delay(3000);
                        else s.Send(System.Text.Encoding.ASCII.GetBytes("hello-back"));
                        await Task.Delay(200);
                    }
                    catch { }
                    s.Dispose();
                });
            }
        });
        var oldMs = Unblock.FirstReplyMs;
        Unblock.Level2Memo.Persist = false;
        Unblock.Level2Memo.Clear();
        Unblock.FirstReplyMs = 500;
        try
        {
            var sock = new System.Net.Sockets.Socket(System.Net.Sockets.SocketType.Stream, System.Net.Sockets.ProtocolType.Tcp) { NoDelay = true };
            await sock.ConnectAsync(listener.LocalEndpoint);
            using var st = new Unblock.HelloSplit(new System.Net.Sockets.NetworkStream(sock, true), sock, "phim.test", 1, false, async ct =>
            {
                var s2 = new System.Net.Sockets.Socket(System.Net.Sockets.SocketType.Stream, System.Net.Sockets.ProtocolType.Tcp) { NoDelay = true };
                await s2.ConnectAsync(listener.LocalEndpoint, ct);
                return s2;
            });
            st.Write(hello);
            // Như SslStream (.NET 8): đọc 0 byte để chờ dữ liệu trước — không được coi là «trang im»
            Assert.Equal(0, await st.ReadAsync(Memory<byte>.Empty));
            var got = new byte[10];
            var m = await st.ReadAsync(got);
            Assert.Equal("hello-back", System.Text.Encoding.ASCII.GetString(got, 0, m));
            Assert.Equal(new[] { hello.Length, hello.Length }, seen.ToArray()); // gửi lại đủ gói bắt tay ở kết nối thứ hai
            Assert.True(Unblock.Level2Memo.Needs("phim.test")); // Windows hỏi được trạng thái TCP → Mức 2 thật → nhớ trang
        }
        finally
        {
            Unblock.FirstReplyMs = oldMs;
            Unblock.Level2Memo.Clear();
            listener.Stop();
        }
    }

    [Fact]
    public void FormEncodedLikeJava()
    {
        Assert.Equal("ti%C3%AAn+ngh%E1%BB%8Bch", "tiên nghịch".FormEncoded());
        Assert.Equal("a-b_c.d*e", "a-b_c.d*e".FormEncoded());
    }
}
