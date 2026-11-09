using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// Các hàng trang chủ người xem bật / tắt, sắp thứ tự, tự thêm (bản Android: HomeRowsTest.kt, iPad: HomeRowsTests.swift).
public class HomeRowsTests
{
    private static readonly List<string> Defaults = new() { "new-eps", "continue", "similar", "interest", "dominant", "newest", "movies" };

    private static List<string> Ids(List<(string Id, bool On)> l) => l.Select(x => x.Id).ToList();

    [Fact]
    public void EmptyMeansDefaultAllOn()
    {
        var m = HomeFeed.MergeRows(new(), Defaults);
        Assert.Equal(Defaults, Ids(m));
        Assert.All(m, x => Assert.True(x.On));
    }

    [Fact]
    public void KeepsSavedOrderAndOff()
    {
        var m = HomeFeed.MergeRows(new() { "new-eps", "continue", "newest", "-similar", "interest", "dominant", "movies" }, Defaults);
        Assert.Equal(new[] { "new-eps", "continue", "newest", "similar", "interest", "dominant", "movies" }, Ids(m));
        Assert.False(m.First(x => x.Id == "similar").On);
    }

    [Fact]
    public void DropsUnknownAndDuplicates()
    {
        var m = HomeFeed.MergeRows(new() { "bogus", "movies", "-movies", "similar" }, Defaults);
        Assert.Single(m, x => x.Id == "movies");
        Assert.True(m.First(x => x.Id == "movies").On);
        Assert.Equal(Defaults.Count, m.Count);
    }

    /// Hàng mới ở bản sau (chưa có trong thứ tự đã lưu) chèn ngay sau hàng đứng trước nó trong thứ tự mặc định.
    [Fact]
    public void NewRowsInsertedAfterTheirDefaultPredecessor()
    {
        var m = HomeFeed.MergeRows(new() { "movies", "newest", "similar", "new-eps", "continue" }, Defaults);
        Assert.Equal(new[] { "movies", "newest", "similar", "interest", "dominant", "new-eps", "continue" }, Ids(m));
    }

    [Fact]
    public void CustomRowsDefaultAfterHead()
    {
        var custom = new List<CustomRow> { new("a", null, "hoathinh"), new("t-kr", "kr") };
        var d = HomeFeed.RowDefaults(custom);
        Assert.Equal(new[] { "new-eps", "continue", "custom-a", "custom-t-kr", "similar" }, d.Take(5));
        var m = HomeFeed.MergeRows(new() { "new-eps", "continue", "custom-a", "movies", "similar" }, d);
        Assert.Equal(new[] { "new-eps", "continue", "custom-a", "custom-t-kr", "movies", "similar" }, Ids(m).Take(6));
    }

    [Fact]
    public void CustomTitles()
    {
        Assert.Equal("Hoạt hình Trung Quốc · phiêu lưu · thuyết minh",
            HomeFeed.CustomTitle(new CustomRow("x", null, "hoathinh", "trung-quoc", "phieu-luu", "thuyet-minh")));
        Assert.Equal("Phim bộ Hàn Quốc", HomeFeed.CustomTitle(new CustomRow("y", null, "series", "han-quoc")));
        Assert.Equal("Phim lẻ · hành động", HomeFeed.CustomTitle(new CustomRow("z", null, "single", "", "hanh-dong")));
        Assert.Equal("Phim Hàn", HomeFeed.CustomTitle(new CustomRow("t-kr", "kr")));
        Assert.True(new CustomRow("c1", null, "series", "han-quoc").Same(new CustomRow("khac", null, "series", "han-quoc")));
    }

    [Fact]
    public void ArrangeFiltersAndOrders()
    {
        static FeedRow Row(string id, string slot) => new(id, id, new(), null, slot);
        var rows = new List<FeedRow> { Row("topic-cn", "topics"), Row("similar", "similar"), Row("interest-a", "interest"), Row("newest", "newest"), Row("movies", "movies") };
        var prefs = new List<(string Id, bool On)> { ("new-eps", true), ("continue", true), ("movies", true), ("similar", false), ("newest", true), ("interest", true) };
        Assert.Equal(new[] { "topic-cn", "movies", "newest", "interest-a" }, HomeFeed.Arrange(rows, prefs).Select(r => r.Id));
    }

    /// Trang chủ lưu từ bản cũ (chưa ghi Slot): đoán theo id — hai hàng «Vì … hay xem» theo thứ tự xuất hiện.
    [Fact]
    public void ArrangeLegacyRows()
    {
        static FeedRow Row(string id) => new(id, id, new());
        var rows = new List<FeedRow> { Row("similar"), Row("interest-hanh-dong"), Row("dominant"), Row("interest-tien-hiep"), Row("newest") };
        var prefs = new List<(string Id, bool On)> { ("interest2", true), ("similar", true), ("interest", false), ("dominant", true), ("newest", true) };
        Assert.Equal(new[] { "interest-tien-hiep", "similar", "dominant", "newest" }, HomeFeed.Arrange(rows, prefs).Select(r => r.Id));
    }

    /// Sửa nhanh trên trang chủ: chuyển qua hàng đang hiện kề nó (hàng tắt / không hiện nằm giữa bị vượt qua).
    [Fact]
    public void MoveShownSkipsHiddenRows()
    {
        var prefs = new List<(string Id, bool On)> { ("new-eps", true), ("continue", true), ("similar", true), ("interest", false), ("dominant", true),
            ("newest", true), ("movies", true) };
        var shown = new List<string> { "similar", "dominant", "movies" }; // «Mới cập nhật» không đủ phim nên không hiện
        Assert.Equal(new[] { "new-eps", "continue", "dominant", "similar", "interest", "newest", "movies" },
            Ids(HomeFeed.MoveShown(prefs, shown, "dominant", true)!));
        var down = HomeFeed.MoveShown(prefs, shown, "dominant", false)!;
        Assert.Equal(new[] { "new-eps", "continue", "similar", "interest", "newest", "movies", "dominant" }, Ids(down));
        Assert.False(down.First(x => x.Id == "interest").On);
        Assert.Null(HomeFeed.MoveShown(prefs, shown, "similar", true));
        Assert.Null(HomeFeed.MoveShown(prefs, shown, "movies", false));
        Assert.Null(HomeFeed.MoveShown(prefs, new List<string> { "continue", "similar" }, "continue", false));
        Assert.Null(HomeFeed.MoveShown(prefs, shown, "newest", true));
    }

    [Fact]
    public void HideTurnsOffOnlyThatRow()
    {
        var h = HomeFeed.Hide(new List<(string Id, bool On)> { ("similar", true), ("custom-a", true), ("movies", false) }, "custom-a");
        Assert.Equal(new[] { "similar=True", "custom-a=False", "movies=False" }, h.Select(x => $"{x.Id}={x.On}"));
    }

    [Fact]
    public void ArrangeSlotsKeepsSlotOfLegacyRows()
    {
        static FeedRow Row(string id, string slot = "") => new(id, id, new(), null, slot);
        var rows = new List<FeedRow> { Row("interest-a"), Row("custom-x", "custom-x"), Row("interest-b") };
        var prefs = new List<(string Id, bool On)> { ("custom-x", true), ("interest2", true), ("interest", true) };
        Assert.Equal(new[] { "custom-x=custom-x", "interest-b=interest2", "interest-a=interest" },
            HomeFeed.ArrangeSlots(rows, prefs).Select(x => $"{x.Row.Id}={x.Slot}"));
    }

    /// Nguồn tắt thì bỏ; tắt hết nguồn đang có thì coi như bật hết (trang chủ không trống).
    [Fact]
    public void SourcesOff()
    {
        var all = new HomeFeed.Sources("https://kk", "https://nc", null, "https://k20", new() { "https://web1", "https://web2" });
        Assert.Equal(new[] { "kk", "nc", "k20", "web" }, all.Ids);
        var s = HomeFeed.WithoutOff(all, new HashSet<string> { "kk", "web", "vs" });
        Assert.Equal(new[] { "nc", "k20" }, s.Ids);
        Assert.Null(s.Kk);
        Assert.Empty(s.Web);
        Assert.Equal(all.Ids, HomeFeed.WithoutOff(all, new HashSet<string> { "kk", "nc", "k20", "web" }).Ids);
        Assert.Equal(new[] { "nc" }, HomeFeed.WithoutOff(new HomeFeed.Sources(null, "https://nc", null, null, new()), new HashSet<string> { "kk" }).Ids);
    }
}
