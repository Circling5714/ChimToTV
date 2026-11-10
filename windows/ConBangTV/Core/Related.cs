using System.Text.RegularExpressions;

namespace ConBangTV.Core;

/// Phim liên quan ở trang phim (từ 1.9; bản TV: Related.kt, iPad: Related.swift):
/// - «Cùng bộ»: các phần / mùa khác của cùng bộ — tìm theo tên gốc (bỏ «Phần N / Season N / Mùa N», năm, «– 20 Tập», phần sau
///   dấu «:») trên trang của phim + KKPhim, xếp theo số phần.
/// - «Cùng thể loại»: KKPhim / VSMOV lọc theo loại + nước + thể loại đặc trưng nhất (như hàng «Giống …» của trang chủ); bỏ phim
///   đang xem và các phần cùng bộ, phim chưa xem lên trước.
/// Không nguồn nào (KKPhim, VSMOV, NguonC, ophim) tìm được theo diễn viên / đạo diễn nên chưa có hàng đó.
public static class Related
{
    public const int Max = 12;

    public sealed record Row(string Id, string Title, List<Film> Films);

    private static readonly Regex Part = new(@"(?:phần|phan|season|mùa|mua|part|ss)\s*0*(\d{1,3})", RegexOptions.IgnoreCase);
    private static readonly Regex Episodes = new(@"\s[–\-]\s*\d+\s*tập.*$", RegexOptions.IgnoreCase);
    private static readonly Regex Year = new(@"\(\s*(?:19|20)\d{2}\s*\)|\b(?:19|20)\d{2}\b");
    private static readonly Regex Brackets = new(@"\([^)]*\)|\[[^\]]*\]");
    private static readonly Regex TrailingNumber = new(@"\s(\d{1,2})$");

    /// Tên để tìm (còn dấu): «Tiên Nghịch (Phần 2) 2024 – 20 Tập» → «Tiên Nghịch».
    public static string SearchName(string title)
    {
        var t = Episodes.Replace(title, " ");
        t = Part.Replace(t, " ");
        t = Year.Replace(t, " ");
        t = Brackets.Replace(t, " ");
        t = t.Split(':')[0];
        var dash = t.IndexOf(" - ", StringComparison.Ordinal);
        if (dash >= 0) t = t[..dash];
        var o = Regex.Replace(t, @"\s+", " ").Trim().TrimEnd('-', '–', ' ');
        return o.Length > 0 ? o : title.Trim();
    }

    /// Tên gốc để so cùng bộ (chuẩn hoá, bỏ số phần đứng cuối): «Đấu Phá Thương Khung 5» → «dau pha thuong khung».
    public static string Base(string title)
    {
        var n = TrailingNumber.Replace(KkPhim.Norm(SearchName(title)), "").Trim();
        return n.Length > 0 ? n : KkPhim.Norm(title);
    }

    /// Số phần: «Phần 5», «Season 2», «(Phần 01)», số cuối phần tên trước «:» («Đấu Phá Thương Khung 4: Đào Vong»); không có → null.
    public static int? PartOf(string title)
    {
        var m = Part.Match(title);
        if (m.Success && int.TryParse(m.Groups[1].Value, out var p)) return p;
        var n = KkPhim.Norm(Episodes.Replace(Year.Replace(title.Split(':')[0], " "), " "));
        var t = TrailingNumber.Match(n);
        return t.Success && int.TryParse(t.Groups[1].Value, out var q) ? q : null;
    }

    /// Cùng bộ với phim đang xem: tên gốc trùng (đủ dài để khỏi trùng nhầm), không phải chính phim đó (cùng tên đầy đủ).
    public static bool SameSeries(string mine, string other)
    {
        var b = Base(mine);
        return b.Length >= 4 && b == Base(other) && KkPhim.Norm(mine) != KkPhim.Norm(other);
    }

    /// Các phần khác của cùng bộ (đã gộp các nguồn, nguồn trước được ưu tiên): bỏ phim trùng tên, xếp theo số phần (không ghi
    /// số = phần 1; cùng số giữ thứ tự tìm được).
    public static List<Film> PickSeries(string title, string url, IEnumerable<Film> found)
    {
        var seen = new HashSet<string>();
        return found.Where(f => f.Url != url && SameSeries(title, f.Title) && seen.Add(KkPhim.Norm(f.Title)))
            .OrderBy(f => PartOf(f.Title) ?? 1).Take(Max).ToList();
    }

    /// Hàng «Cùng thể loại»: bỏ phim đang xem và các phần cùng bộ; phim chưa xem lên trước (giữ thứ tự của nguồn).
    public static List<Film> PickSimilar(string title, string url, IEnumerable<Film> found, ISet<string> watched)
    {
        var seen = new HashSet<string>();
        var mine = KkPhim.Norm(title);
        var films = found.Where(f => f.Url != url && KkPhim.Norm(f.Title) != mine && !SameSeries(title, f.Title) && seen.Add(KkPhim.Norm(f.Title))).ToList();
        return films.Where(f => !watched.Contains(KkPhim.Norm(f.Title))).Concat(films.Where(f => watched.Contains(KkPhim.Norm(f.Title))))
            .Take(Max).ToList();
    }

    /// Tải các hàng (lỗi một nguồn thì bỏ nguồn đó).
    public static async Task<List<Row>> Load(string title, string url)
    {
        var rows = new List<Row>();
        var name = SearchName(title);
        // Cùng bộ: trang của phim trước (cùng nguồn), rồi KKPhim
        var found = new List<Film>();
        foreach (var o in new[] { Site.OriginOf(url), HomeFeed.GetSources().Kk }.OfType<string>().Distinct())
        {
            try { found.AddRange((await Site.Search(name, o)).Films); } catch (Exception) { }
        }
        var series = PickSeries(title, url, found);
        if (series.Count > 0) rows.Add(new Row("series", $"Cùng bộ «{name}»", series));
        // Cùng thể loại (tra thể loại qua KKPhim theo tên, nhớ chung cả nhà)
        if (FilmTags.NeedsLookup(title))
        {
            try { await FilmTags.Lookup(title); } catch (Exception) { }
        }
        if (FilmTags.Of(title) is { } tags && HomeFeed.RelatedList(tags, Prefs.Kids) is { } sim)
        {
            List<Film> list;
            try { list = (await Site.List(sim.Url)).Films; } catch (Exception) { list = new(); }
            var watched = Prefs.History.Select(h => KkPhim.Norm(h.Title)).ToHashSet();
            var films = PickSimilar(title, url, list, watched).Where(f => series.All(x => x.Url != f.Url)).ToList();
            if (films.Count > 0) rows.Add(new Row("similar", $"Cùng thể loại · {sim.Title}", films));
        }
        return rows;
    }
}
