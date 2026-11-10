using System.Globalization;
using System.IO;
using System.Text.Json;
using System.Text.RegularExpressions;
using JObj = System.Collections.Generic.Dictionary<string, object?>;

namespace ConBangTV.Core;

/// Điểm IMDb của một phim — lấy thẳng từ dữ liệu KKPhim (phimapi.com có sẵn imdb.vote_average / vote_count), như bản TV
/// (ImdbTop.kt). Không gọi IMDb. Ít hơn MinVotes lượt chấm thì coi như chưa có điểm (10 điểm từ 1 lượt chấm).
public sealed record Imdb(double Score, int Votes)
{
    public const int MinVotes = 100;

    /// Nhãn trên thẻ phim, vd "IMDb 8.5".
    public string Label => "IMDb " + Score.ToString("0.0", CultureInfo.InvariantCulture);

    /// Dòng thông tin ở trang phim, vd "IMDb 8.5 (378 nghìn lượt chấm)".
    public string Detail => $"{Label} ({VotesText(Votes)} lượt chấm)";

    /// Đối tượng "imdb" của KKPhim → Imdb (null = chưa có điểm / quá ít lượt).
    public static Imdb? Of(JObj? o)
    {
        if (o == null) return null;
        var s = Feed.Dbl(o.GetValueOrDefault("vote_average")) ?? 0;
        var v = (int)Feed.Num64(o.GetValueOrDefault("vote_count"));
        return s > 0 && s <= 10 && v >= MinVotes ? new Imdb(s, v) : null;
    }

    public static string VotesText(int v)
    {
        if (v >= 1_000_000)
        {
            var s = (v / 1_000_000.0).ToString("0.0", CultureInfo.InvariantCulture).Replace('.', ',');
            return (s.EndsWith(",0") ? s[..^2] : s) + " triệu";
        }
        return v >= 1000 ? $"{v / 1000} nghìn" : v.ToString(CultureInfo.InvariantCulture);
    }
}

/// Danh mục «Điểm IMDb cao» (như bản TV): vài trang đầu (phim mới cập nhật) của các danh sách KKPhim, xếp theo điểm IMDb có
/// trọng số (như Top 250 của IMDb: WR = v/(v+m)·R + m/(v+m)·C; dưới số lượt tối thiểu thì bỏ, m = 5 lần số đó). API KKPhim không
/// sắp theo điểm, kho quá lớn để máy tự quét hết → chỉ xếp trong vài trăm phim mới nhất mỗi nhóm. Lưu kết quả 12 giờ.
public static class ImdbTop
{
    public sealed record Group(string Id, string Title, string Path, int Pages, int MinVotes, bool Kids = false);

    /// Đường dẫn API kiểu KKPhim (www.kkphim.com/api/movies/… — ApiSite gọi thẳng phimapi.com).
    public static readonly Group[] Groups =
    {
        new("le", "Phim lẻ", "category/phim-le", 8, 10_000),
        new("bo", "Phim bộ", "category/phim-bo", 8, 2_000),
        new("han", "Phim Hàn Quốc", "quoc-gia/han-quoc", 5, 1_000),
        new("aumy", "Phim Âu Mỹ", "quoc-gia/au-my", 5, 5_000),
        new("tq", "Phim Trung Quốc", "quoc-gia/trung-quoc", 5, 500),
        new("hh", "Hoạt hình", "category/hoat-hinh", 6, 1_000, Kids: true),
    };

    private const string Origin = "https://www.kkphim.com";
    private const int PageSize = 64;
    private const int Top = 60;
    private const long Ttl = 12 * 3600_000L;

    /// Danh mục (của trang) xếp được theo IMDb: danh sách KKPhim (dữ liệu có điểm IMDb).
    public static bool CanRank(string url) =>
        Site.HostOf(url) is { } h && (h.EndsWith(ApiSite.KkHost) || h.EndsWith("phimapi.com")) && url.Contains("/api/");

    /// Trang «Điểm IMDb cao»: mỗi nhóm một hàng (hồ sơ trẻ em: chỉ hoạt hình).
    public static async Task<FilmPage> Page(bool kids)
    {
        var gs = Groups.Where(g => !kids || g.Kids).ToList();
        var lists = await Task.WhenAll(gs.Select(async g =>
        {
            try
            {
                return await GroupFilms(g);
            }
            catch (Exception)
            {
                return null;
            }
        }));
        var sections = gs.Zip(lists).Where(x => x.Second is { Count: > 0 }).Select(x => new FilmSection(x.First.Title, x.Second!)).ToList();
        if (sections.Count == 0) throw new IOException("Không tải được điểm IMDb (danh sách KKPhim)");
        return new FilmPage { Sections = sections };
    }

    /// Xếp một danh mục KKPhim theo IMDb: 5 trang đầu (≈ 320 phim mới cập nhật), phim từ 1000 lượt chấm.
    public static async Task<FilmPage> ForList(string url)
    {
        var key = "list|" + Regex.Replace(url, @"[?&]page=\d+", "");
        if (Cached(key) is { } c) return new FilmPage { Films = c };
        var films = await Fetch(Enumerable.Range(1, 5).Select(p => WithPage(url, p)).ToList());
        var ranked = Rank(films, 1000, 120);
        if (ranked.Count > 0) Store(key, ranked);
        return new FilmPage { Films = ranked };
    }

    private static async Task<List<Film>> GroupFilms(Group g)
    {
        var key = "group|" + g.Id;
        if (Cached(key) is { } c) return c;
        var films = await Fetch(Enumerable.Range(1, g.Pages).Select(p => WithPage($"{Origin}/api/movies/{g.Path}", p)).ToList());
        var ranked = Rank(films, g.MinVotes, Top);
        if (ranked.Count > 0) Store(key, ranked);
        return ranked;
    }

    /// Tải các trang song song (tối đa 4 cùng lúc); trang lỗi thì bỏ, lỗi hết thì báo.
    private static async Task<List<Film>> Fetch(List<string> urls)
    {
        using var gate = new SemaphoreSlim(4);
        var pages = await Task.WhenAll(urls.Select(async u =>
        {
            await gate.WaitAsync();
            try
            {
                return (await Site.List(u)).Films;
            }
            catch (Exception)
            {
                return null;
            }
            finally
            {
                gate.Release();
            }
        }));
        if (pages.All(p => p == null)) throw new IOException("Không tải được danh sách KKPhim");
        return pages.Where(p => p != null).SelectMany(p => p!).DistinctBy(f => f.Url).ToList();
    }

    /// Trang thứ n, mỗi trang PageSize phim (thay page= / limit= có sẵn).
    public static string WithPage(string url, int page)
    {
        var u = Regex.Replace(url, @"([?&])page=\d+", "${1}page=" + page);
        u = Regex.Replace(u, @"([?&])limit=\d+", "${1}limit=" + PageSize);
        if (!Regex.IsMatch(u, "[?&]page=")) u += (u.Contains('?') ? "&" : "?") + "page=" + page;
        if (!Regex.IsMatch(u, "[?&]limit=")) u += "&limit=" + PageSize;
        return u;
    }

    /// Xếp theo điểm có trọng số: chỉ phim từ minVotes lượt chấm; C = điểm trung bình của các phim đó; m = 5 lần minVotes (phim
    /// 9,4 điểm từ 1.200 lượt không vượt phim 8,6 điểm từ 2 triệu lượt). Bằng điểm thì phim nhiều lượt hơn trước.
    public static List<Film> Rank(List<Film> films, int minVotes, int top)
    {
        var ok = films.Where(f => (f.Imdb?.Votes ?? 0) >= minVotes).ToList();
        if (ok.Count == 0) return new List<Film>();
        var c = ok.Average(f => f.Imdb!.Score);
        var m = minVotes * 5.0;
        double Wr(Film f)
        {
            double v = f.Imdb!.Votes;
            return v / (v + m) * f.Imdb.Score + m / (v + m) * c;
        }
        return ok.OrderByDescending(Wr).ThenByDescending(f => f.Imdb!.Votes).Take(top).ToList();
    }

    // ---- Lưu kết quả 12 giờ ----

    private static readonly object Lock = new();
    private static readonly Dictionary<string, (long T, List<Film> Films)> Memory = new();

    /// Tệp lưu (đặt lúc mở app: thư mục dữ liệu của app). null = chỉ lưu trong bộ nhớ.
    public static string? CacheFile;

    private static List<Film>? Cached(string key)
    {
        lock (Lock)
        {
            if (Memory.TryGetValue(key, out var m) && Util.NowMs() - m.T < Ttl) return m.Films;
            if (CacheFile == null || !File.Exists(CacheFile)) return null;
            try
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(CacheFile));
                if (!doc.RootElement.TryGetProperty(key, out var o)) return null;
                var t = o.GetProperty("t").GetInt64();
                if (Util.NowMs() - t >= Ttl) return null;
                var films = o.GetProperty("f").EnumerateArray().Select(x =>
                {
                    var s = x.GetProperty("s").GetDouble();
                    return new Film(x.GetProperty("u").GetString()!, x.GetProperty("n").GetString() ?? "",
                        x.GetProperty("p").GetString().NonEmpty(), x.GetProperty("b").GetString().NonEmpty(),
                        s > 0 ? new Imdb(s, x.GetProperty("v").GetInt32()) : null);
                }).ToList();
                Memory[key] = (t, films);
                return films;
            }
            catch (Exception)
            {
                return null;
            }
        }
    }

    private static void Store(string key, List<Film> films)
    {
        lock (Lock)
        {
            var t = Util.NowMs();
            Memory[key] = (t, films);
            if (CacheFile == null) return;
            try
            {
                var root = new Dictionary<string, object>();
                if (File.Exists(CacheFile))
                {
                    using var doc = JsonDocument.Parse(File.ReadAllText(CacheFile));
                    foreach (var p in doc.RootElement.EnumerateObject()) root[p.Name] = p.Value.Clone();
                }
                root[key] = new
                {
                    t,
                    f = films.Select(f => new { u = f.Url, n = f.Title, p = f.Poster ?? "", b = f.Badge ?? "", s = f.Imdb?.Score ?? 0, v = f.Imdb?.Votes ?? 0 }),
                };
                File.WriteAllText(CacheFile, JsonSerializer.Serialize(root));
            }
            catch (Exception)
            {
            }
        }
    }
}
