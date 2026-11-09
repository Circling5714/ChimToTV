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
}
