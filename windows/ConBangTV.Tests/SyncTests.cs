using System.Text;
using System.Text.Json;
using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// Đồng bộ trong nhà: gộp lịch sử / chỗ đang xem / theo dõi / danh sách trang — cùng ca với SyncMergeTests.swift / SyncTest.kt.
public class SyncTests
{
    private static SyncHistory H(string film, long time, string ep = "e1") => new(film, film.ToUpperInvariant(), "", ep, $"{film}/{ep}", time);

    private static SyncFollow F(string film, int count, int seen, long t) => new(film, film, "", count, $"Tập {count}", seen, t);

    [Fact]
    public void HistoryNewestWinsAndUnion()
    {
        var a = new SyncProfile { Name = "Cả nhà", History = new() { H("x", 100, "e1"), H("y", 50) } };
        var b = new SyncProfile { Name = "Cả nhà", History = new() { H("x", 200, "e2"), H("z", 150) } };
        var m = SyncMerge.Profile(a, b);
        Assert.Equal(new[] { "x", "z", "y" }, m.History.Select(h => h.Film));
        Assert.Equal("e2", m.History[0].Ep);
        Assert.Equal(m.History, SyncMerge.Profile(b, a).History);
    }

    [Fact]
    public void DeletedHistoryStaysDeleted()
    {
        var a = new SyncProfile { Name = "n", HistDel = new() { ["x"] = 300 } };
        var b = new SyncProfile
        {
            Name = "n", History = new() { H("x", 200) },
            Pos = new() { new SyncPos("x/e1", 30, "x", 200), new SyncPos("y/e1", 5, "y", 10) },
        };
        var m = SyncMerge.Profile(a, b);
        Assert.Empty(m.History);
        Assert.Equal(new[] { "y/e1" }, m.Pos.Select(p => p.Ep));
        Assert.Equal(300, m.HistDel["x"]);
        var c = new SyncProfile { Name = "n", History = new() { H("x", 400) } };
        Assert.Equal(new[] { "x" }, SyncMerge.Profile(m, c).History.Select(h => h.Film));
    }

    [Fact]
    public void PositionsNewestWinsOldestFirst()
    {
        var a = new SyncProfile { Name = "n", Pos = new() { new SyncPos("old", 1, "f", 0), new SyncPos("e", 100, "f", 500) } };
        var b = new SyncProfile { Name = "n", Pos = new() { new SyncPos("e", 250, "f", 600), new SyncPos("n", 7, "f", 50) } };
        var m = SyncMerge.Profile(a, b);
        Assert.Equal(new[] { "old", "n", "e" }, m.Pos.Select(p => p.Ep));
        Assert.Equal(250, m.Pos[^1].Sec);
    }

    [Fact]
    public void FollowsMergeAndUnfollow()
    {
        var a = new SyncProfile { Name = "n", Follows = new() { F("x", 10, 10, 100), F("y", 3, 1, 50) } };
        var b = new SyncProfile { Name = "n", Follows = new() { F("x", 12, 9, 100) }, FollowDel = new() { ["y"] = 80 } };
        var m = SyncMerge.Profile(a, b);
        Assert.Equal(new[] { "x" }, m.Follows.Select(f => f.Film));
        Assert.Equal(12, m.Follows[0].Count);
        Assert.Equal(10, m.Follows[0].Seen);
        Assert.Equal("Tập 12", m.Follows[0].Latest);
        var c = new SyncProfile { Name = "n", Follows = new() { F("y", 3, 3, 90) } };
        Assert.Equal(new[] { "x", "y" }, SyncMerge.Profile(m, c).Follows.Select(f => f.Film).OrderBy(x => x));
    }

    [Fact]
    public void MatchProfiles()
    {
        var local = new List<SyncProfile> { new() { Id = "", Name = "Cả nhà" }, new() { Id = "p1", Name = "Bố" }, new() { Id = "p2", Name = "Bé Na" } };
        var remote = new List<SyncProfile> { new() { Id = "p3", Name = " bé na " }, new() { Id = "", Name = "Nhà mình" }, new() { Id = "p1", Name = "Mẹ" } };
        Assert.Equal(new[] { "0-1", "2-0" }, SyncMerge.Match(local, remote).Select(m => $"{m.Local}-{m.Remote}"));
    }

    [Fact]
    public void Sites()
    {
        var a = new SyncSites { List = new() { "a", "b", "c" }, Add = new() { ["c"] = 100 }, Del = new() { ["d"] = 100 } };
        var b = new SyncSites { List = new() { "a", "d", "e" }, Add = new() { ["d"] = 50, ["e"] = 200 }, Del = new() { ["b"] = 300 } };
        var m = SyncMerge.Sites(a, b);
        Assert.Equal(new[] { "a", "c", "e" }, m.List);
        Assert.Equal(m.List.OrderBy(x => x), SyncMerge.Sites(b, a).List.OrderBy(x => x));
    }

    [Fact]
    public void TombstoneCap()
    {
        var big = Enumerable.Range(0, SyncMerge.MaxTombstones + 20).ToDictionary(i => $"k{i}", i => (long)i);
        var m = SyncMerge.Tombstones(big, new());
        Assert.Equal(SyncMerge.MaxTombstones, m.Count);
        Assert.False(m.ContainsKey("k0"));
        Assert.True(m.ContainsKey($"k{SyncMerge.MaxTombstones + 19}"));
    }

    [Fact]
    public void JsonShapeMatchesOtherApps()
    {
        var p = new SyncPayload
        {
            Device = "d", Name = "PC", Platform = "windows",
            Profiles = new() { new SyncProfile { Name = "Cả nhà", History = new() { H("x", 1) } } },
            Sites = new SyncSites { List = new() { "a" } },
        };
        using var doc = JsonDocument.Parse(p.ToJson());
        var root = doc.RootElement;
        Assert.Equal(1, root.GetProperty("v").GetInt32());
        var prof = root.GetProperty("profiles")[0];
        Assert.Equal(new[] { "id", "name", "history", "histDel", "pos", "follows", "followDel" }.OrderBy(x => x),
            prof.EnumerateObject().Select(o => o.Name).OrderBy(x => x));
        Assert.Equal(new[] { "film", "title", "poster", "ep", "epUrl", "time" }.OrderBy(x => x),
            prof.GetProperty("history")[0].EnumerateObject().Select(o => o.Name).OrderBy(x => x));
        Assert.Equal(new[] { "list", "add", "del" }.OrderBy(x => x), root.GetProperty("sites").EnumerateObject().Select(o => o.Name).OrderBy(x => x));
    }

    [Fact]
    public void ParsesAndroidPayloadWithMissingFields()
    {
        var json = """
        {"v":1,"device":"tv1","name":"TX3","platform":"android","profiles":[{"id":"","name":"Cả nhà",
         "history":[{"film":"f","title":"T","poster":"","ep":"Tập 1","epUrl":"f/1","time":5}],
         "histDel":{},"pos":[{"ep":"f/1","sec":30,"film":"f","t":5}],"follows":[],"followDel":{"g":7}}],
         "sites":{"list":["https://a"],"add":{},"del":{}}}
        """;
        var p = SyncPayload.Parse(Encoding.UTF8.GetBytes(json))!;
        Assert.Equal("android", p.Platform);
        Assert.Equal(30, p.Profiles[0].Pos[0].Sec);
        Assert.Equal(7, p.Profiles[0].FollowDel["g"]);
        var bare = SyncPayload.Parse(Encoding.UTF8.GetBytes("""{"device":"x","profiles":[{"id":"","history":[{"film":"f","time":1}]}]}"""))!;
        Assert.Equal("", bare.Profiles[0].History[0].Title);
        Assert.Empty(bare.Sites.List);
    }

    [Fact]
    public void SignatureAndHttpParsing()
    {
        // HMAC-SHA256("k", "123\n{}") — cùng cách ký bản TV / iPad
        Assert.Equal("3317e7dfdb031bbbe0279631aadd48916bbed405865b3a698b8c6884f0f90685", Http1.Sign("k", "123", "{}"u8.ToArray()));
        var body = "{\"a\":1}"u8.ToArray();
        var resp = Http1.Response(200, body, "tok");
        var m = Http1.Parse(resp, false)!;
        Assert.Equal(200, m.Status);
        Assert.Equal(body, m.Body);
        Assert.True(Http1.Verify(m, "tok"));
        Assert.False(Http1.Verify(m, "khac"));
        Assert.Null(Http1.Parse(resp.AsSpan(0, resp.Length - 1), false)); // chưa đủ thân
        Assert.Equal(("10.0.0.5", 8791), SyncService.ParseAddress("10.0.0.5"));
        Assert.Equal(("10.0.0.5", 8792), SyncService.ParseAddress(" 10.0.0.5:8792 "));
        Assert.Equal(("fe80::1%5", 8791), SyncService.ParseAddress("[fe80::1%5]:8791"));
    }

    [Fact]
    public void ParsesMdnsAnswer()
    {
        // Trả lời mDNS dựng tay: PTR → "TX3._conbangtv._tcp.local", SRV cổng 8791 → "tx3.local", TXT id/p, A 10.1.2.3
        var b = new List<byte> { 0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 3 };
        void Name(string n) { foreach (var l in n.Split('.')) { var x = Encoding.UTF8.GetBytes(l); b.Add((byte)x.Length); b.AddRange(x); } b.Add(0); }
        void Rr(string n, int type, byte[] data) { Name(n); b.AddRange(new byte[] { 0, (byte)type, 0, 1, 0, 0, 0, 120, (byte)(data.Length >> 8), (byte)data.Length }); b.AddRange(data); }
        byte[] Enc(string n) { var t = new List<byte>(); foreach (var l in n.Split('.')) { var x = Encoding.UTF8.GetBytes(l); t.Add((byte)x.Length); t.AddRange(x); } t.Add(0); return t.ToArray(); }
        Rr("_conbangtv._tcp.local", 12, Enc("TX3._conbangtv._tcp.local"));
        Rr("TX3._conbangtv._tcp.local", 33, new byte[] { 0, 0, 0, 0, 0x22, 0x57 }.Concat(Enc("tx3.local")).ToArray());
        var txt = new List<byte>();
        foreach (var kv in new[] { "id=abc", "p=android" }) { txt.Add((byte)kv.Length); txt.AddRange(Encoding.UTF8.GetBytes(kv)); }
        Rr("TX3._conbangtv._tcp.local", 16, txt.ToArray());
        Rr("tx3.local", 1, new byte[] { 10, 1, 2, 3 });
        var recs = Mdns.ParseRecords(b.ToArray());
        Assert.Equal(4, recs.Count);
        Assert.Equal("TX3._conbangtv._tcp.local", recs[0].Target);
        Assert.Equal(8791, recs[1].Port);
        Assert.Equal("abc", recs[2].Txt!["id"]);
        Assert.Equal("10.1.2.3", recs[3].Address!.ToString());
    }
}
