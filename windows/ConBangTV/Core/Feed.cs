using System.Collections.Concurrent;
using System.Globalization;
using System.IO;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace ConBangTV.Core;

using JObj = Dictionary<string, object?>;

// Trang chủ tổng hợp (từ Windows 1.5) — bản Android: Feed.kt, iPad: Feed.swift (giữ cùng tên hàm để sửa song song).

/// Thể loại / quốc gia / loại phim theo slug của KKPhim (hanh-dong, trung-quoc; loại: hoathinh, series, single, tvshows).
public sealed record Tags(List<string> Genres, List<string> Countries, string? Type)
{
    public JObj Json() => new() { ["g"] = Genres.Cast<object?>().ToList(), ["c"] = Countries.Cast<object?>().ToList(), ["t"] = Type ?? "" };

    public static Tags? Of(JObj? o) => o == null ? null
        : new Tags(Strings(J.Arr(o, "g")), Strings(J.Arr(o, "c")), J.Str(o, "t").NonEmpty());

    /// category / country [{name, slug}] + type của KKPhim / VSMOV.
    public static Tags? FromKk(JObj m)
    {
        var g = Slugs(J.Arr(m, "category"));
        var c = Slugs(J.Arr(m, "country"));
        var t = J.Str(m, "type").NonEmpty();
        return g.Count == 0 && c.Count == 0 && t == null ? null : new Tags(g, c, t);
    }

    private static List<string> Slugs(List<object?>? a) =>
        J.Objects(a).Select(o => J.Str(o, "slug").NonEmpty() ?? Feed.Slug(J.Str(o, "name")).NonEmpty()).OfType<string>().ToList();

    private static List<string> Strings(List<object?>? a) => (a ?? new()).OfType<string>().Where(s => s.Length > 0).ToList();

    public bool Equals(Tags? o) => o != null && Genres.SequenceEqual(o.Genres) && Countries.SequenceEqual(o.Countries) && Type == o.Type;

    public override int GetHashCode() => HashCode.Combine(string.Join(",", Genres), string.Join(",", Countries), Type);
}

/// Một phim trong danh sách của một nguồn. Names: tên đã chuẩn hoá (KkPhim.Norm) để gộp phim trùng; Updated: lúc nguồn cập
/// nhật (ms; 0 = không ghi); Ep: số tập mới nhất.
public sealed record FeedItem(string Url, string Title, string? Poster, string? Badge, HashSet<string> Names, int? Year, long Updated,
    double? Ep, Tags? Tags, Imdb? Imdb = null)
{
    /// Tên miền nguồn (tính một lần).
    public string Host { get; } = Site.HostOf(Url) ?? "";

    public JObj Json()
    {
        var o = new JObj
        {
            ["u"] = Url, ["n"] = Title, ["p"] = Poster ?? "", ["b"] = Badge ?? "", ["k"] = Names.Cast<object?>().ToList(), ["y"] = (long)(Year ?? 0),
            ["t"] = Updated, ["e"] = Ep ?? -1.0,
        };
        if (Tags != null) o["g"] = Tags.Json();
        if (Imdb != null)
        {
            o["is"] = Imdb.Score;
            o["iv"] = (long)Imdb.Votes;
        }
        return o;
    }

    public static FeedItem? Of(JObj o)
    {
        var url = J.Str(o, "u").NonEmpty();
        var title = J.Str(o, "n").NonEmpty();
        if (url == null || title == null) return null;
        var e = Feed.Dbl(o.GetValueOrDefault("e")) ?? -1;
        var y = (int)Feed.Num64(o.GetValueOrDefault("y"));
        return new FeedItem(url, title, J.Str(o, "p").NonEmpty(), J.Str(o, "b").NonEmpty(),
            (J.Arr(o, "k") ?? new()).OfType<string>().Where(s => s.Length > 0).ToHashSet(), y > 0 ? y : null,
            Feed.Num64(o.GetValueOrDefault("t")), e >= 0 ? e : null, Tags.Of(J.Obj(o, "g")),
            Feed.Dbl(o.GetValueOrDefault("is")) is > 0 and var s ? new Imdb(s, (int)Feed.Num64(o.GetValueOrDefault("iv"))) : null);
    }

    public bool Equals(FeedItem? o) => o != null && Url == o.Url && Title == o.Title && Poster == o.Poster && Badge == o.Badge &&
        Names.SetEquals(o.Names) && Year == o.Year && Updated == o.Updated && Ep == o.Ep && Equals(Tags, o.Tags);

    public override int GetHashCode() => HashCode.Combine(Url, Title, Updated);
}

/// Phim đã gộp từ các nguồn (Items: mỗi nguồn một mục). So sánh theo chính đối tượng (một phim chỉ thuộc một nhóm).
public sealed class FeedFilm
{
    public readonly List<FeedItem> Items;
    public readonly HashSet<string> Names;
    public readonly long Updated;

    public FeedFilm(List<FeedItem> items)
    {
        Items = items;
        Names = items.SelectMany(i => i.Names).ToHashSet();
        Updated = items.Count == 0 ? 0 : items.Max(i => i.Updated);
    }

    public Tags? Tags => Items.Select(i => i.Tags).FirstOrDefault(t => t != null);
    public int? Year => Items.Select(i => i.Year).FirstOrDefault(y => y != null);

    /// Nguồn có tập mới nhất (null = không nguồn nào ghi số tập).
    public FeedItem? Latest => Items.Where(i => i.Ep != null).Aggregate((FeedItem?)null, (best, i) => best == null || i.Ep > best.Ep ? i : best);

    /// Xếp lại các nguồn theo thứ tự ưu tiên mở (giữ thứ tự cũ khi bằng nhau).
    public FeedFilm Ordered(IList<string> prefer)
    {
        int Rank(FeedItem i) => prefer.IndexOf(i.Host) is var r && r >= 0 ? r : prefer.Count;
        return new FeedFilm(Items.Select((i, n) => (i, n)).OrderBy(x => Rank(x.i)).ThenBy(x => x.n).Select(x => x.i).ToList());
    }

    public bool Matches(HashSet<string> other) => other.Overlaps(Names);
}

/// Đọc danh sách phim mới của các nguồn, gộp phim trùng, giữ bản lưu từng nguồn. Nguồn lỗi / quá hạn: dùng bản lưu (dưới
/// 24 giờ), lỗi 3 lượt liên tiếp thì tạm nghỉ 30 phút (rồi 2 giờ, 6 giờ).
public static class Feed
{
    /// Mỗi nguồn chờ tối đa 6 giây; quá hạn thì dùng bản lưu, về muộn vẫn lưu cho lần sau.
    public const long Timeout = 6_000;
    private const long Total = 15_000;
    public const long CacheMax = 24 * 3_600_000L;
    private const long PseudoGap = 15 * 60_000L;
    private const int FailsToRest = 3;
    private static readonly long[] Rest = { 30 * 60_000L, 2 * 3_600_000L, 6 * 3_600_000L };

    /// Thư mục bản lưu (cạnh prefs.json).
    private static string Dir => Path.Combine(Prefs.DataDir, "feed");

    // ---- Đọc danh sách của từng nguồn ----

    /// KKPhim (phimapi.com v1) / VSMOV: có thể loại, quốc gia, loại phim, giờ cập nhật (VSMOV không có thể loại).
    public static List<FeedItem> ParseKk(JObj root, string origin)
    {
        var data = J.Obj(root, "data") ?? root;
        var cdn = J.Str(data, "APP_DOMAIN_CDN_IMAGE").NonEmpty() ?? J.Str(root, "pathImage").NonEmpty() ?? "https://phimimg.com";
        return J.Objects(J.Arr(data, "items")).Select(o =>
        {
            var slug = J.Str(o, "slug");
            var name = J.Str(o, "name");
            if (slug.Length == 0 || name.Length == 0) return null;
            var ep = J.Str(o, "episode_current");
            return new FeedItem($"{origin}/phim/{slug}", name, ApiSite.Image(J.Str(o, "poster_url").NonEmpty() ?? J.Str(o, "thumb_url"), cdn),
                NextSite.Badge(ep), KkPhim.Names(o), Year(o), Time(J.Str(J.Obj(o, "modified"), "time")), EpOf(ep), Tags.FromKk(o),
                Imdb.Of(J.Obj(o, "imdb")));
        }).OfType<FeedItem>().DistinctBy(i => i.Url).ToList();
    }

    /// NguonC /api/films/…: tên gốc (có thể nhiều tên), năm, tập hiện tại, giờ cập nhật; không có thể loại.
    public static List<FeedItem> ParseNc(JObj root, string origin) => J.Objects(J.Arr(root, "items")).Select(o =>
    {
        var slug = J.Str(o, "slug");
        var name = J.Str(o, "name");
        if (slug.Length == 0 || name.Length == 0) return null;
        var ep = J.Str(o, "current_episode");
        var poster = J.Str(o, "thumb_url").NonEmpty() ?? J.Str(o, "poster_url");
        return new FeedItem($"{origin}/phim/{slug}", name, poster.StartsWith("http") ? poster : null, NextSite.Badge(ep), KkPhim.Names(o),
            Year(o), Time(J.Str(o, "modified")), EpOf(ep), null);
    }).OfType<FeedItem>().DistinctBy(i => i.Url).ToList();

    /// Phim từ trang web / CLB Phim Xưa (chỉ có tên, ảnh, nhãn tập).
    public static List<FeedItem> FromFilms(IEnumerable<Film> films) => films.Select(f =>
        new FeedItem(f.Url, f.Title, f.Poster, f.Badge, KkPhim.Names(new JObj { ["name"] = f.Title }), null, 0, EpOf(f.Badge), null))
        .DistinctBy(i => i.Url).ToList();

    private static readonly Regex TapRx = new(@"(?i)t[aậ]p\s*(\d+(?:\.\d+)?)");
    private static readonly Regex OfRx = new(@"(\d+)\s*/\s*(\d+|\?+)");
    private static readonly Regex NumRx = new(@"^(\d+(?:\.\d+)?)$");

    /// Số tập trong nhãn: "Tập 108", "Hoàn Tất (16/16)", "108/120", "108"; "Full", "Top 1", "HD" → null.
    public static double? EpOf(string? label)
    {
        var s = label?.Trim() ?? "";
        if (s.Length == 0) return null;
        foreach (var rx in new[] { TapRx, OfRx, NumRx })
        {
            var m = rx.Match(s);
            if (m.Success) return double.Parse(m.Groups[1].Value, CultureInfo.InvariantCulture);
        }
        return null;
    }

    private static readonly Regex IsoRx = new(@"(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2}))?(?:\.\d+)?\s*(Z|[+-]\d{2}:?\d{2})?");

    /// "2026-11-08T18:55:29.000Z", "2026-11-08T21:12:36+07:00" → ms; không ghi múi giờ thì coi là giờ Việt Nam.
    public static long Time(string? s)
    {
        if (s == null) return 0;
        var m = IsoRx.Match(s);
        if (!m.Success) return 0;
        int G(int i) => int.TryParse(m.Groups[i].Value, out var v) ? v : 0;
        long t;
        try
        {
            t = new DateTimeOffset(G(1), G(2), G(3), G(4), G(5), G(6), TimeSpan.Zero).ToUnixTimeMilliseconds();
        }
        catch (ArgumentOutOfRangeException)
        {
            return 0;
        }
        var zone = m.Groups[7].Value;
        int offset;
        if (zone.Length == 0) offset = 7 * 60;
        else if (zone == "Z") offset = 0;
        else
        {
            var d = zone[1..].Replace(":", "");
            var min = int.Parse(d[..2]) * 60 + int.Parse(d[2..]);
            offset = zone[0] == '-' ? -min : min;
        }
        return t - offset * 60_000L;
    }

    private static int? Year(JObj o)
    {
        var y = J.Int(o, "year");
        return y is >= 1900 and <= 2100 ? y : null;
    }

    /// "Hành Động" → "hanh-dong" (NguonC ghi tên thể loại, không có slug).
    public static string Slug(string name) => KkPhim.Norm(name).Replace(' ', '-');

    /// Số trong dữ liệu JSON (long, double, int, chuỗi số).
    public static long Num64(object? v) => v switch
    {
        long l => l,
        int i => i,
        double d => (long)d,
        string s => long.TryParse(s, out var x) ? x : 0,
        _ => 0,
    };

    public static double? Dbl(object? v) => v switch
    {
        long l => l,
        int i => i,
        double d => d,
        string s => double.TryParse(s, NumberStyles.Float, CultureInfo.InvariantCulture, out var x) ? x : null,
        _ => null,
    };

    // ---- Gộp phim trùng ----

    /// Gộp các danh sách (thứ tự = ưu tiên): có tên chung, năm lệch ≤ 1, khác nguồn. Cùng địa chỉ = cùng phim. Mục không ghi
    /// giờ cập nhật tính giờ giả theo thứ tự trên trang. Kết quả xếp mới cập nhật trước.
    public static List<FeedFilm> Merge(IEnumerable<List<FeedItem>> lists, long? now = null)
    {
        var t0 = now ?? Util.NowMs();
        var groups = new List<List<FeedItem>>();
        var byName = new Dictionary<string, List<int>>();
        var byUrl = new HashSet<string>();
        foreach (var list in lists)
        {
            for (var rank = 0; rank < list.Count; rank++)
            {
                var raw = list[rank];
                if (!byUrl.Add(raw.Url)) continue;
                var item = raw.Updated > 0 ? raw : raw with { Updated = t0 - (rank + 1) * PseudoGap };
                int? hit = null;
                foreach (var n in item.Names)
                {
                    foreach (var gi in byName.GetValueOrDefault(n) ?? new())
                    {
                        var g = groups[gi];
                        var y = g.Select(i => i.Year).FirstOrDefault(v => v != null);
                        if (g.All(i => i.Host != item.Host) && (y == null || item.Year == null || Math.Abs(y.Value - item.Year.Value) <= 1))
                        {
                            hit = gi;
                            break;
                        }
                    }
                    if (hit != null) break;
                }
                var idx = hit ?? groups.Count;
                if (hit == null) groups.Add(new());
                groups[idx].Add(item);
                foreach (var n in item.Names)
                {
                    if (!byName.TryGetValue(n, out var l)) byName[n] = l = new();
                    if (!l.Contains(idx)) l.Add(idx);
                }
            }
        }
        return groups.Select((g, i) => (f: new FeedFilm(g), i)).OrderByDescending(x => x.f.Updated).ThenBy(x => x.i).Select(x => x.f).ToList();
    }

    // ---- Tải các nguồn: song song, hạn 6 giây, bản lưu, tạm nghỉ ----

    /// Một lần đọc: Key = tên bản lưu, Host = nguồn (tính lỗi / tạm nghỉ); Keep = false: không lưu bản.
    public sealed record Req(string Key, string Host, Func<Task<Loaded>> Load, bool Keep = true);

    public sealed record Loaded(List<FeedItem> Items, List<Category>? Categories = null, string? MovedTo = null);

    public enum State { Ok, Cached, NoData }

    public sealed record Result(Req Req, State State, List<FeedItem> Items, List<Category>? Categories, string? Error, bool Resting,
        long Ms, long SavedAt, string? MovedTo = null);

    private sealed class Job
    {
        public long StartedAt, FinishedAt;
        public Loaded? Done;
        public Exception? Failed;
        public bool Counted;
    }

    /// Tải các nguồn. Trả về khi mọi nguồn xong / quá hạn — nguồn quá hạn vẫn chạy tiếp để lưu bản mới.
    public static async Task<List<Result>> Load(List<Req> reqs)
    {
        var t0 = Util.NowMs();
        var results = new Result?[reqs.Count];
        var jobs = new Job?[reqs.Count];
        // Một lượt hỏi một nguồn nhiều lần (KKPhim: mới cập nhật + các hàng gợi ý) — chỉ tính một lần lỗi mỗi lượt
        var failedHosts = new ConcurrentDictionary<string, bool>();
        void FailedOnce(string host)
        {
            if (failedHosts.TryAdd(host, true)) Failed(host);
        }
        for (var i = 0; i < reqs.Count; i++)
        {
            var r = reqs[i];
            if (Resting(r.Host))
            {
                results[i] = Cached(r, null, true, 0);
                continue;
            }
            var job = jobs[i] = new Job();
            _ = Task.Run(async () =>
            {
                lock (job) job.StartedAt = Util.NowMs();
                try
                {
#if DEBUG
                    // Bản debug: tệp <thư mục dữ liệu>/feed_fail_<tên miền> (hoặc feed_fail_all) → nguồn đó giả lỗi
                    if (File.Exists(Path.Combine(Prefs.DataDir, "feed_fail_" + r.Host)) || File.Exists(Path.Combine(Prefs.DataDir, "feed_fail_all")))
                    {
                        await Task.Delay(400);
                        throw new IOException("máy chủ trả lỗi 502 (giả lỗi — bản debug)");
                    }
#endif
                    var got = await r.Load();
                    lock (job) job.Done = got;
                    if (r.Keep) Save(r.Key, got);
                    Healthy(r.Host);
                }
                catch (Exception e)
                {
                    bool counted;
                    lock (job)
                    {
                        job.Failed = e;
                        counted = job.Counted;
                    }
                    Log.D("feed", $"{r.Key} lỗi: {e.Message}");
                    if (!counted) FailedOnce(r.Host);
                }
                lock (job) job.FinishedAt = Util.NowMs();
            });
        }
        while (true)
        {
            var now = Util.NowMs();
            var waiting = false;
            for (var i = 0; i < reqs.Count; i++)
            {
                var j = jobs[i];
                if (j == null || results[i] != null) continue;
                long started, finished;
                Loaded? done;
                Exception? err;
                lock (j)
                {
                    started = j.StartedAt;
                    finished = j.FinishedAt;
                    done = j.Done;
                    err = j.Failed;
                }
                var ms = (finished > 0 ? finished : now) - (started > 0 ? started : now);
                if (done != null)
                {
                    results[i] = new Result(reqs[i], State.Ok, done.Items, done.Categories, null, false, ms, now, done.MovedTo);
                }
                else if (err != null)
                {
                    results[i] = Cached(reqs[i], Short(err), false, ms);
                }
                else if (started > 0 && now - started > Timeout)
                {
                    lock (j) j.Counted = true;
                    FailedOnce(reqs[i].Host);
                    results[i] = Cached(reqs[i], $"quá {Timeout / 1000} giây", false, ms);
                }
                else if (now - t0 > Total)
                {
                    results[i] = Cached(reqs[i], null, false, ms);
                }
                else
                {
                    waiting = true;
                }
            }
            if (!waiting) break;
            await Task.Delay(80);
        }
        foreach (var r in results) Log.D("feed", $"{r!.Req.Key} {r.State} {r.Ms} ms {r.Items.Count} phim {r.Error}");
        return results.Select(r => r!).ToList();
    }

    private static string Short(Exception e) => e switch
    {
        Http.HttpError h => $"lỗi {h.Code}",
        TaskCanceledException => "không trả lời",
        HttpRequestException => "không kết nối được",
        _ => e.Message.Length > 60 ? e.Message[..60] : e.Message,
    };

    private static Result Cached(Req r, string? error, bool resting, long ms)
    {
        var c = r.Keep ? ReadCache(r.Key) : null;
        return c != null
            ? new Result(r, State.Cached, c.Value.Loaded.Items, c.Value.Loaded.Categories, error, resting, ms, c.Value.At)
            : new Result(r, State.NoData, new(), null, error, resting, ms, 0);
    }

    /// Bản lưu của một nguồn (dưới 24 giờ).
    public static (Loaded Loaded, long At)? ReadCache(string key)
    {
        var text = ReadText(key);
        if (text == null) return null;
        try
        {
            if (JsonScan.Parse(text) is not JObj o) return null;
            var at = Num64(o.GetValueOrDefault("at"));
            if (Util.NowMs() - at > CacheMax) return null;
            var items = J.Objects(J.Arr(o, "items")).Select(FeedItem.Of).OfType<FeedItem>().ToList();
            var cats = J.Arr(o, "cats") is { } c ? J.Objects(c).Select(x => new Category(J.Str(x, "t"), J.Str(x, "u"))).ToList() : null;
            return (new Loaded(items, cats), at);
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static void Save(string key, Loaded l)
    {
        var o = new JObj { ["at"] = Util.NowMs(), ["items"] = l.Items.Select(i => (object?)i.Json()).ToList() };
        if (l.Categories != null) o["cats"] = l.Categories.Select(c => (object?)new JObj { ["t"] = c.Title, ["u"] = c.Url }).ToList();
        WriteText(key, JsonSerializer.Serialize(o));
    }

    /// Tệp lưu tự do theo khoá (vd trang chủ lần trước của từng người xem).
    public static void WriteText(string key, string text)
    {
        try
        {
            Directory.CreateDirectory(Dir);
            var f = FileOf(key);
            File.WriteAllText(f + ".tmp", text);
            File.Move(f + ".tmp", f, true);
        }
        catch (Exception)
        {
        }
    }

    public static string? ReadText(string key)
    {
        try
        {
            var f = FileOf(key);
            return File.Exists(f) ? File.ReadAllText(f) : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static string FileOf(string key) =>
        Path.Combine(Dir, Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(key)))[..32].ToLowerInvariant() + ".json");

    // ---- Tình trạng nguồn (host → {f: lỗi liên tiếp, r: tạm nghỉ tới, l: số lần đã tạm nghỉ}) ----

    private static readonly object HealthLock = new();

    private static Dictionary<string, JObj> Health() =>
        (JsonScan.Parse(Prefs.FeedHealth) as JObj ?? new()).Where(kv => kv.Value is JObj).ToDictionary(kv => kv.Key, kv => (JObj)kv.Value!);

    private static void SaveHealth(Dictionary<string, JObj> h) => Prefs.FeedHealth = JsonSerializer.Serialize(h);

    public static bool Resting(string host)
    {
        lock (HealthLock) return Health().TryGetValue(host, out var o) && Num64(o.GetValueOrDefault("r")) > Util.NowMs();
    }

    /// Nguồn đang tạm nghỉ: host → tạm nghỉ tới (ms).
    public static Dictionary<string, long> RestingHosts()
    {
        lock (HealthLock)
        {
            var now = Util.NowMs();
            return Health().Select(kv => (kv.Key, r: Num64(kv.Value.GetValueOrDefault("r")))).Where(x => x.r > now).ToDictionary(x => x.Key, x => x.r);
        }
    }

    private static void Healthy(string host)
    {
        lock (HealthLock)
        {
            var h = Health();
            if (h.Remove(host)) SaveHealth(h);
        }
    }

    private static void Failed(string host)
    {
        lock (HealthLock)
        {
            var h = Health();
            var o = h.GetValueOrDefault(host) ?? new JObj();
            var f = Num64(o.GetValueOrDefault("f")) + 1;
            o["f"] = f;
            if (f >= FailsToRest)
            {
                var level = (int)Num64(o.GetValueOrDefault("l"));
                o["r"] = Util.NowMs() + Rest[Math.Min(level, Rest.Length - 1)];
                o["l"] = (long)(level + 1);
                o["f"] = 0L;
            }
            h[host] = o;
            SaveHealth(h);
        }
    }

    /// «Thử lại ngay» trong Cài đặt: bỏ tạm nghỉ của mọi nguồn.
    public static void ClearRest()
    {
        lock (HealthLock) Prefs.FeedHealth = "{}";
    }
}
