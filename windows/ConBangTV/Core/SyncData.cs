using System.Text.Json;
using System.Text.Json.Serialization;

namespace ConBangTV.Core;

// Đồng bộ trong nhà (TV ↔ iPad / iPhone ↔ Windows): dữ liệu trao đổi và cách gộp — giống hệt bản Android (Sync.kt) và iPad
// (SyncData.swift), cùng định dạng JSON. Mỗi máy gửi toàn bộ dữ liệu của mình, máy kia gộp rồi trả lại bản đã gộp; gộp xong
// hai máy như nhau. Xoá cũng được đồng bộ nhờ "bia mộ" (thời điểm xoá).

public sealed record SyncHistory(string Film, string Title, string Poster, string Ep, string EpUrl, long Time);

/// Chỗ đang xem một tập (giây; 0 = đã xem hết), T = lúc lưu (0 = không rõ).
public sealed record SyncPos(string Ep, int Sec, string Film, long T);

public sealed record SyncFollow(string Film, string Title, string Poster, int Count, string Latest, int Seen, long T);

/// Dữ liệu riêng một người xem (bản Windows chỉ có người xem đầu tiên, id "").
public sealed class SyncProfile
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public List<SyncHistory> History { get; set; } = new();
    public Dictionary<string, long> HistDel { get; set; } = new();
    public List<SyncPos> Pos { get; set; } = new();
    public List<SyncFollow> Follows { get; set; } = new();
    public Dictionary<string, long> FollowDel { get; set; } = new();

    public bool SameAs(SyncProfile o) =>
        Id == o.Id && History.SequenceEqual(o.History) && Pos.SequenceEqual(o.Pos) && Follows.SequenceEqual(o.Follows) &&
        DictEq(HistDel, o.HistDel) && DictEq(FollowDel, o.FollowDel);

    internal static bool DictEq(Dictionary<string, long> a, Dictionary<string, long> b) =>
        a.Count == b.Count && a.All(kv => b.TryGetValue(kv.Key, out var v) && v == kv.Value);
}

/// Danh sách trang phim (dùng chung cả máy): thứ tự, lúc thêm, lúc bỏ.
public sealed class SyncSites
{
    public List<string> List { get; set; } = new();
    public Dictionary<string, long> Add { get; set; } = new();
    public Dictionary<string, long> Del { get; set; } = new();

    public bool SameAs(SyncSites o) => List.SequenceEqual(o.List) && SyncProfile.DictEq(Add, o.Add) && SyncProfile.DictEq(Del, o.Del);
}

public sealed class SyncPayload
{
    public int V { get; set; } = 1;
    public string Device { get; set; } = "";
    public string Name { get; set; } = "";
    public string Platform { get; set; } = "";
    public List<SyncProfile> Profiles { get; set; } = new();
    public SyncSites Sites { get; set; } = new();

    public static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.Never,
        NumberHandling = JsonNumberHandling.AllowReadingFromString,
    };

    public byte[] ToJson() => JsonSerializer.SerializeToUtf8Bytes(this, Json);

    /// Đọc dữ liệu máy kia (trường thiếu / null → mặc định).
    public static SyncPayload? Parse(byte[] data)
    {
        try
        {
            var p = JsonSerializer.Deserialize<SyncPayload>(data, Json);
            if (p == null) return null;
            p.Profiles ??= new();
            p.Sites ??= new();
            p.Sites.List ??= new();
            p.Sites.Add ??= new();
            p.Sites.Del ??= new();
            foreach (var pr in p.Profiles)
            {
                pr.Id ??= "";
                pr.Name ??= "";
                pr.History = (pr.History ?? new()).Select(h => h with
                {
                    Film = h.Film ?? "", Title = h.Title ?? "", Poster = h.Poster ?? "", Ep = h.Ep ?? "", EpUrl = h.EpUrl ?? "",
                }).Where(h => h.Film.Length > 0).ToList();
                pr.Pos = (pr.Pos ?? new()).Select(x => x with { Ep = x.Ep ?? "", Film = x.Film ?? "" }).Where(x => x.Ep.Length > 0).ToList();
                pr.Follows = (pr.Follows ?? new()).Select(f => f with
                {
                    Film = f.Film ?? "", Title = f.Title ?? "", Poster = f.Poster ?? "", Latest = f.Latest ?? "",
                }).Where(f => f.Film.Length > 0).ToList();
                pr.HistDel ??= new();
                pr.FollowDel ??= new();
            }
            return p;
        }
        catch (JsonException)
        {
            return null;
        }
    }
}

public static class SyncMerge
{
    public const int MaxHistory = 60;
    public const int MaxPositions = 400;
    /// Giữ tối đa bấy nhiêu "bia mộ" mỗi loại (mới nhất).
    public const int MaxTombstones = 300;

    /// Ghép người xem hai máy: người đầu tiên (id "") với nhau, còn lại theo tên (không phân biệt hoa / thường, khoảng trắng).
    public static List<(int Local, int Remote)> Match(List<SyncProfile> local, List<SyncProfile> remote)
    {
        static string Key(string s) => s.Trim().ToLowerInvariant();
        var outList = new List<(int, int)>();
        var used = new HashSet<int>();
        var l0 = local.FindIndex(p => p.Id.Length == 0);
        var r0 = remote.FindIndex(p => p.Id.Length == 0);
        if (l0 >= 0 && r0 >= 0)
        {
            outList.Add((l0, r0));
            used.Add(r0);
        }
        for (var i = 0; i < local.Count; i++)
        {
            var p = local[i];
            if (p.Id.Length == 0) continue;
            var j = Enumerable.Range(0, remote.Count)
                .FirstOrDefault(k => !used.Contains(k) && remote[k].Id.Length > 0 && Key(remote[k].Name) == Key(p.Name), -1);
            if (j < 0) continue;
            outList.Add((i, j));
            used.Add(j);
        }
        return outList;
    }

    public static Dictionary<string, long> Tombstones(Dictionary<string, long> a, Dictionary<string, long> b)
    {
        var m = new Dictionary<string, long>(a);
        foreach (var (k, v) in b)
            if (!m.TryGetValue(k, out var old) || v > old) m[k] = v;
        if (m.Count > MaxTombstones)
            m = m.OrderByDescending(kv => kv.Value).ThenBy(kv => kv.Key, StringComparer.Ordinal).Take(MaxTombstones)
                .ToDictionary(kv => kv.Key, kv => kv.Value);
        return m;
    }

    /// Gộp dữ liệu một người xem (local: máy này, remote: máy kia).
    public static SyncProfile Profile(SyncProfile local, SyncProfile remote)
    {
        var histDel = Tombstones(local.HistDel, remote.HistDel);

        // Lịch sử: mỗi phim một mục, lấy lần xem mới hơn; bỏ phim đã xoá sau lần xem đó
        var hist = new Dictionary<string, SyncHistory>();
        foreach (var h in local.History.Concat(remote.History))
        {
            if (hist.TryGetValue(h.Film, out var old) && old.Time >= h.Time) continue;
            hist[h.Film] = h;
        }
        var history = hist.Values.Where(h => !histDel.TryGetValue(h.Film, out var d) || d < h.Time)
            .OrderByDescending(h => h.Time).ThenBy(h => h.Film, StringComparer.Ordinal).Take(MaxHistory).ToList();

        // Chỗ đang xem: mỗi tập lấy lần lưu mới hơn; phim đã xoá khỏi lịch sử sau lần lưu đó thì bỏ
        var pos = new Dictionary<string, SyncPos>();
        var order = new List<string>();
        foreach (var p in local.Pos.Concat(remote.Pos))
        {
            if (pos.TryGetValue(p.Ep, out var old))
            {
                if (p.T > old.T) pos[p.Ep] = p;
            }
            else
            {
                pos[p.Ep] = p;
                order.Add(p.Ep);
            }
        }
        var kept = order.Select(e => pos[e]).Where(p => !histDel.TryGetValue(p.Film, out var d) || d < p.T).ToList();
        // Thứ tự lưu: cũ nhất trước (không rõ lúc lưu — T = 0 — đứng đầu, giữ thứ tự cũ)
        var sorted = kept.Select((p, i) => (p, i)).OrderBy(x => x.p.T).ThenBy(x => x.i).Select(x => x.p).ToList();
        var positions = sorted.Skip(Math.Max(0, sorted.Count - MaxPositions)).ToList();

        // Phim theo dõi: hợp hai bên; số tập / số tập đã thấy lấy số lớn hơn; bỏ phim đã bỏ theo dõi sau lần theo dõi đó
        var followDel = Tombstones(local.FollowDel, remote.FollowDel);
        var fol = new Dictionary<string, SyncFollow>();
        var forder = new List<string>();
        foreach (var f in local.Follows.Concat(remote.Follows))
        {
            if (!fol.TryGetValue(f.Film, out var old))
            {
                fol[f.Film] = f;
                forder.Add(f.Film);
                continue;
            }
            var latest = f.Count > old.Count ? f.Latest : old.Latest;
            var m = old with { Latest = latest, Count = Math.Max(old.Count, f.Count), Seen = Math.Max(old.Seen, f.Seen) };
            if (f.T > old.T) m = m with { T = f.T, Title = f.Title, Poster = f.Poster.Length > 0 ? f.Poster : m.Poster };
            fol[f.Film] = m;
        }
        var follows = forder.Select(k => fol[k]).Where(f => !followDel.TryGetValue(f.Film, out var d) || d < f.T)
            .Select((f, i) => (f, i)).OrderByDescending(x => x.f.T).ThenBy(x => x.i).Select(x => x.f).ToList();

        return new SyncProfile
        {
            Id = local.Id, Name = local.Name, History = history, HistDel = histDel, Pos = positions, Follows = follows, FollowDel = followDel,
        };
    }

    /// Gộp danh sách trang: trang có mặt nếu lần thêm mới hơn lần bỏ (trang có sẵn: lúc thêm = 0). Thứ tự: trang của máy này
    /// trước, trang mới của máy kia sau.
    public static SyncSites Sites(SyncSites local, SyncSites remote)
    {
        var add = Tombstones(local.Add, remote.Add);
        var del = Tombstones(local.Del, remote.Del);
        var list = new List<string>();
        foreach (var s in local.List.Concat(remote.List))
        {
            if (list.Contains(s)) continue;
            if (del.TryGetValue(s, out var d) && d >= add.GetValueOrDefault(s, 0)) continue;
            list.Add(s);
        }
        if (list.Count == 0) list = local.List.ToList(); // không để trống danh sách
        return new SyncSites { List = list, Add = add, Del = del };
    }
}
