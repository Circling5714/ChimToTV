using System.Globalization;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace ConBangTV.Core;

using JObj = Dictionary<string, object?>;

// Trang chủ tổng hợp (từ Windows 1.5; từ 1.16 lập kế hoạch, tải, dựng hàng trong lõi Rust — home_feed.rs, app giữ phần sắp xếp hàng) — bản Android: HomeFeed.kt, iPad: HomeFeed.swift (giữ cùng tên hàm để sửa song song).

/// Thẻ phim của trang chủ tổng hợp. Links: các nguồn có phim này, nguồn mở trước đứng đầu.
public sealed record Card(Film Film, string? Chip, List<FeedItem> Links);

/// Một hàng. More: địa chỉ danh sách đầy đủ (null = «Xem thêm» hiện mọi phim đã có trong Cards); Slot: loại hàng trong cài
/// đặt của người xem (HomeFeed.Rows: "similar", "interest", "newest"…; "custom-<id>" = hàng tự thêm; "topics" = bản cũ).
public sealed record FeedRow(string Id, string Title, List<Card> Cards, string? More = null, string Slot = "");

/// Hàng người xem tự thêm (Cài đặt → Trang chủ → Các hàng): chủ đề có sẵn (Topic — HomeFeed.Topics) hoặc lọc theo loại phim
/// (hoathinh / series / single / tvshows) + nước + thể loại (slug KKPhim, rỗng = mọi) + ngôn ngữ (thuyet-minh / long-tieng).
public sealed record CustomRow(string Id, string? Topic = null, string Type = "", string Country = "", string Genre = "", string Lang = "")
{
    /// Mã trong cài đặt thứ tự các hàng.
    [System.Text.Json.Serialization.JsonIgnore]
    public string Key => $"custom-{Id}";

    /// Cùng nội dung (bỏ qua Id) — khỏi thêm trùng.
    public bool Same(CustomRow o) => this with { Id = "" } == o with { Id = "" };
}

/// Phim đang xem dở vừa có tập mới (Key: tên đã chuẩn hoá; Ep: số tập mới nhất).
public sealed record FreshCard(Card Card, string Key, double Ep);

public static class HomeFeed
{
    /// Không hợp trẻ em (hồ sơ trẻ em bỏ các phim này và mọi phim không phải hoạt hình).
    private static readonly HashSet<string> Adult = new() { "phim-18", "kinh-di", "hinh-su", "chien-tranh" };

    // ---- Các hàng người xem bật / tắt, sắp thứ tự, tự thêm (Cài đặt → Trang chủ → Các hàng) ----

    /// Theo thứ tự mặc định. «Có tập mới» / «Xem tiếp» (HeadRows) chỉ bật / tắt — luôn ở đầu trang; hàng trẻ em (KidsRows)
    /// chỉ có ở hồ sơ trẻ em.
    public static readonly (string Id, string Label)[] Rows =
    {
        ("new-eps", "Có tập mới"),
        ("continue", "Xem tiếp"),
        ("similar", "Giống «phim vừa xem»"),
        ("interest", "Vì … hay xem <thể loại> (sở thích 1)"),
        ("dominant", "Loại phim hay xem · mới cập nhật"),
        ("dub", "Thuyết minh / lồng tiếng mới"),
        ("kids-young", "Hoạt hình thiếu nhi"),
        ("kids-family", "Hoạt hình gia đình"),
        ("kids-dub", "Hoạt hình lồng tiếng"),
        ("interest2", "Vì … hay xem <thể loại> (sở thích 2)"),
        ("newest", "Mới cập nhật · mọi nguồn"),
        ("movies", "Phim lẻ mới"),
    };
    public static readonly HashSet<string> HeadRows = new() { "new-eps", "continue" };
    public static readonly HashSet<string> KidsRows = new() { "kids-young", "kids-family", "kids-dub" };
    /// Hàng hồ sơ trẻ em không có (chỉ hoạt hình hợp trẻ em).
    public static readonly HashSet<string> AdultRows = new() { "dominant", "dub", "newest", "movies" };
    /// Tối đa số hàng tự thêm.
    public const int CustomMax = 6;
    /// Chủ đề hợp hồ sơ trẻ em (chỉ hoạt hình).
    public static readonly HashSet<string> KidsTopics = new() { "cn", "jp", "kid" };
    /// Loại phim / ngôn ngữ cho hàng tự thêm (type, sort_lang của KKPhim).
    public static readonly (string Id, string Label)[] Types = { ("hoathinh", "Hoạt hình"), ("series", "Phim bộ"), ("single", "Phim lẻ"), ("tvshows", "TV show") };
    public static readonly (string Id, string Label)[] Langs = { ("", "Mọi bản"), ("thuyet-minh", "Thuyết minh"), ("long-tieng", "Lồng tiếng") };
    /// Thể loại không hợp trẻ em (hàng tự thêm của hồ sơ trẻ em không chọn được).
    public static IReadOnlySet<string> AdultGenres => Adult;

    /// Tên hàng tự thêm: «Hoạt hình Trung Quốc · phiêu lưu · thuyết minh», chủ đề thì tên chủ đề.
    public static string CustomTitle(CustomRow r)
    {
        if (r.Topic is { } t) return Topics.FirstOrDefault(x => x.Id == t).Label ?? t;
        var head = Types.FirstOrDefault(x => x.Id == r.Type).Label ?? "Phim";
        if (r.Country.Length > 0) head += " " + ApiSite.CountryName(r.Country);
        var parts = new List<string> { head };
        if (r.Genre.Length > 0) parts.Add(ApiSite.GenreName(r.Genre));
        if (r.Lang.Length > 0 && Langs.FirstOrDefault(x => x.Id == r.Lang).Label is { } l) parts.Add(l.ToLowerInvariant());
        return string.Join(" · ", parts);
    }

    /// Thứ tự mặc định: «Có tập mới», «Xem tiếp», các hàng tự thêm (theo lúc thêm), rồi các hàng app tự chọn.
    public static List<string> RowDefaults(List<CustomRow>? custom = null) =>
        Rows.Take(2).Select(x => x.Id).Concat((custom ?? Prefs.HomeCustom).Select(c => c.Key)).Concat(Rows.Skip(2).Select(x => x.Id)).ToList();

    /// Ghép thứ tự đã lưu ("id" bật, "-id" tắt) với các hàng mặc định: bỏ id lạ / trùng, hàng mặc định chưa có (hàng mới ở bản
    /// sau, hàng vừa tự thêm) chèn ngay sau hàng đứng trước nó trong thứ tự mặc định.
    public static List<(string Id, bool On)> MergeRows(List<string> saved, List<string>? defaults = null)
    {
        defaults ??= Rows.Select(x => x.Id).ToList();
        var o = new List<(string Id, bool On)>();
        foreach (var s in saved)
        {
            var on = !s.StartsWith('-');
            var id = on ? s : s[1..];
            if (defaults.Contains(id) && o.All(x => x.Id != id)) o.Add((id, on));
        }
        for (var i = 0; i < defaults.Count; i++)
        {
            var id = defaults[i];
            if (o.Any(x => x.Id == id)) continue;
            var prev = defaults.Take(i).LastOrDefault(p => o.Any(x => x.Id == p));
            var at = prev == null ? 0 : o.FindIndex(x => x.Id == prev) + 1;
            o.Insert(at, (id, true));
        }
        return o;
    }

    /// Các hàng của người xem đang dùng (thứ tự, bật / tắt), kể cả hàng tự thêm.
    public static List<(string Id, bool On)> RowPrefs() => MergeRows(Prefs.HomeRows, RowDefaults());

    public static bool RowOn(string id)
    {
        foreach (var p in RowPrefs()) if (p.Id == id) return p.On;
        return true;
    }

    /// Loại hàng của một hàng lưu từ bản cũ (chưa ghi Slot): theo id; hai hàng «Vì … hay xem» theo thứ tự xuất hiện.
    private static string LegacySlot(string id, int interestBefore) =>
        id.StartsWith("interest-") ? (interestBefore == 0 ? "interest" : "interest2") : id.StartsWith("topic-") ? "topics" : id;

    /// Xếp các hàng theo cài đặt của người xem: bỏ hàng đã tắt, theo thứ tự đã chọn; hàng chủ đề bản cũ ("topics") đầu, hàng
    /// lạ cuối.
    public static List<FeedRow> Arrange(List<FeedRow> rows, List<(string Id, bool On)>? prefs = null) =>
        ArrangeSlots(rows, prefs).Select(x => x.Row).ToList();

    /// Như Arrange, kèm loại hàng (id trong cài đặt) của từng hàng — kể cả hàng lưu từ bản cũ (chưa ghi Slot).
    public static List<(FeedRow Row, string Slot)> ArrangeSlots(List<FeedRow> rows, List<(string Id, bool On)>? prefs = null)
    {
        prefs ??= RowPrefs();
        var pos = new Dictionary<string, int>();
        for (var i = 0; i < prefs.Count; i++) pos[prefs[i].Id] = prefs[i].On ? i : -1;
        var interest = 0;
        var keyed = new List<(FeedRow Row, string Slot, int Index)>();
        foreach (var r in rows)
        {
            var slot = r.Slot.Length == 0 ? LegacySlot(r.Id, interest) : r.Slot;
            if (slot.StartsWith("interest")) interest++;
            keyed.Add((r, slot, keyed.Count));
        }
        int Rank(string slot) => slot == "topics" ? -1 : pos.TryGetValue(slot, out var p) ? p : int.MaxValue;
        return keyed.Where(x => !pos.TryGetValue(x.Slot, out var p) || p != -1)
            .OrderBy(x => Rank(x.Slot)).ThenBy(x => x.Index).Select(x => (x.Row, x.Slot)).ToList();
    }

    /// Lưu cài đặt các hàng ("id" bật, "-id" tắt).
    public static void SaveRows(List<(string Id, bool On)> prefs) =>
        Prefs.HomeRows = prefs.Select(p => p.On ? p.Id : "-" + p.Id).ToList();

    /// Tắt một hàng (sửa nhanh trên trang chủ: «Ẩn hàng này»).
    public static List<(string Id, bool On)> Hide(List<(string Id, bool On)> prefs, string slot) =>
        prefs.Select(p => p.Id == slot ? (slot, false) : p).ToList();

    /// Sửa nhanh trên trang chủ: chuyển hàng slot lên / xuống qua hàng đang hiện kề nó (shown = loại hàng đang hiện, theo thứ
    /// tự trên trang — hàng tắt / không đủ phim nằm giữa bị vượt qua). null: không chuyển được (hàng đầu / cuối, «Có tập
    /// mới» / «Xem tiếp», hàng chủ đề bản cũ).
    public static List<(string Id, bool On)>? MoveShown(List<(string Id, bool On)> prefs, List<string> shown, string slot, bool up)
    {
        bool Fixed(string s) => HeadRows.Contains(s) || s == "topics";
        var i = shown.IndexOf(slot);
        if (i < 0 || Fixed(slot)) return null;
        var j = up ? i - 1 : i + 1;
        if (j < 0 || j >= shown.Count || Fixed(shown[j])) return null;
        var other = shown[j];
        var output = prefs.ToList();
        var at = output.FindIndex(p => p.Id == slot);
        if (at < 0) return null;
        var me = output[at];
        output.RemoveAt(at);
        var k = output.FindIndex(p => p.Id == other);
        if (k < 0) return null;
        output.Insert(up ? k : k + 1, me);
        return output;
    }

    /// Chủ đề cho người xem chưa có lịch sử (id lưu trong Prefs.HomeTopics).
    public static readonly (string Id, string Label)[] Topics =
    {
        ("cn", "Hoạt hình Trung Quốc"), ("kh", "Kiếm hiệp"), ("kr", "Phim Hàn"), ("mv", "Phim lẻ Âu Mỹ"),
        ("hk", "Phim xưa Hồng Kông"), ("kid", "Hoạt hình cho bé"), ("jp", "Anime Nhật"), ("tm", "Thuyết minh / lồng tiếng"),
    };

    /// Nguồn trong danh sách trang (gốc địa chỉ); Web: trang web đang dùng + trang hay xem gần đây.
    public sealed record Sources(string? Kk, string? Nc, string? Vs, string? K20, List<string> Web)
    {
        public int Count => new[] { Kk, Nc, Vs, K20 }.Count(x => x != null) + Web.Count;

        /// id (trong HomeFeed.SourceNames) của các nguồn có mặt.
        public List<string> Ids =>
            new[] { Kk != null ? "kk" : null, Nc != null ? "nc" : null, Vs != null ? "vs" : null, K20 != null ? "k20" : null, Web.Count > 0 ? "web" : null }
                .OfType<string>().ToList();
    }

    /// Nguồn của trang chủ tổng hợp người xem bật / tắt được (Cài đặt → «Nguồn cho trang chủ…», riêng từng người; khoá
    /// Prefs.HomeSrcOff), theo thứ tự hiện. «web» = trang đang dùng + trang xem gần đây.
    public static readonly (string Id, string Label)[] SourceNames =
    {
        ("kk", "KKPhim"), ("nc", "NguonC"), ("vs", "VSMOV"), ("k20", "CLB Phim Xưa"), ("web", "Trang đang dùng và trang xem gần đây"),
    };

    /// Bỏ các nguồn người xem đã tắt. Tắt hết các nguồn đang có (vd nguồn còn bật vừa bị xoá khỏi danh sách trang) thì coi như
    /// bật hết — trang chủ không bao giờ trống vì cài đặt nguồn.
    public static Sources WithoutOff(Sources all, ICollection<string> off)
    {
        if (all.Ids.All(off.Contains)) return all;
        return new Sources(off.Contains("kk") ? null : all.Kk, off.Contains("nc") ? null : all.Nc, off.Contains("vs") ? null : all.Vs,
            off.Contains("k20") ? null : all.K20, off.Contains("web") ? new() : all.Web);
    }

    /// Nguồn trang chủ của người xem đang dùng (đã bỏ nguồn tắt).
    public static Sources GetSources() => WithoutOff(AllSources(), Prefs.HomeSrcOff);


    /// Mọi nguồn có trong danh sách trang (cả nguồn người xem đã tắt) — lõi tính theo danh sách trang + lịch sử xem.
    public static Sources AllSources() => Native.Sync<Sources>("home.allSources");

    // ---- Lập kế hoạch, tải, dựng hàng: trong lõi Rust (kế hoạch / kết quả ở lại lõi, app giữ mã số) ----

    /// Kế hoạch tải của lần mở trang chủ (SrcCount: số nguồn).
    public sealed class Plan
    {
        public long Handle;
        public int SrcCount;
    }

    /// Kết quả dựng trang chủ. Pending: còn nguồn chậm có thể về muộn (hỏi lại bằng Late).
    public sealed class Snapshot
    {
        public long Handle;
        public List<FeedRow> Rows = new();
        public List<FreshCard> Fresh = new();
        public List<Category>? Categories;
        public Moved? MovedToJson;
        public string Note = "", NoteGaveUp = "";
        public bool Empty, FromCache, Pending;
        public int SrcCount;

        /// Trang đang dùng đã chuyển tên miền: (trang cũ, gốc mới).
        public (string Old, string New)? MovedTo => MovedToJson is { } m ? (m.Old, m.New) : null;
    }

    public sealed class Moved
    {
        public string Old = "", New = "";
    }

    private static Snapshot? Snap(System.Text.Json.Nodes.JsonNode? n)
    {
        if (n == null) return null;
        if (n["movedTo"] is { } m) n["movedToJson"] = m.DeepClone();
        return Native.To<Snapshot>(n);
    }

    public static Plan MakePlan(bool tagsKnownOnly = false) => Native.Sync<Plan>("home.makePlan", new { tagsKnownOnly });

    /// Bản đã lưu của mọi nguồn trong kế hoạch (mở trang chủ là có ngay, trong lúc tải mới).
    public static Snapshot? Cached(Plan plan) => Snap(Native.SyncNode("home.cached", new { plan = plan.Handle }));

    public static async Task<Snapshot> Load(Plan plan) =>
        Snap(await Native.CallNode("home.load", new { plan = plan.Handle })) ?? new Snapshot { Empty = true };

    /// Nguồn chậm của lần dựng trước đã về: dựng lại; chưa có gì mới thì null.
    public static async Task<Snapshot?> Late(Snapshot s) => Snap(await Native.CallNode("home.late", new { snap = s.Handle }));

    /// Trang chủ lần trước của người xem pid (mở app là hiện ngay).
    public static Snapshot? Last(string pid) => Snap(Native.SyncNode("home.last", new { pid }));

    /// Đã xem phim (mở trang phim / phát): hết báo «tập mới» tới khi có tập sau.
    public static void MarkSeen(FilmDetail d) => Native.SyncVoid("home.markSeen", new { detail = d });
}
