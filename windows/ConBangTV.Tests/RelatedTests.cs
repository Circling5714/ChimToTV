using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// Phim liên quan (11/2026): tên gốc, số phần, ghép cùng bộ, hàng cùng thể loại — cùng ca với RelatedTest.kt / RelatedTests.swift.
public class RelatedTests
{
    private static Film F(string title, string? url = null) => new(url ?? "https://x.test/" + KkPhim.Norm(title).Replace(' ', '-'), title, null, null);

    [Fact]
    public void SearchNameDropsPartYearAndEpisodes()
    {
        Assert.Equal("Tiên Nghịch", Related.SearchName("Tiên Nghịch (Phần 2) 2024 – 20 Tập"));
        Assert.Equal("Đấu Phá Thương Khung", Related.SearchName("Đấu Phá Thương Khung Phần 5"));
        Assert.Equal("Nhất Nguyệt Thần Kiếm", Related.SearchName("Nhất Nguyệt Thần Kiếm 1990 (Phần 01) – 20 Tập"));
        Assert.Equal("Avatar", Related.SearchName("Avatar: Dòng Chảy Của Nước"));
        Assert.Equal("1917", Related.SearchName("1917"));
    }

    [Fact]
    public void PartNumbers()
    {
        Assert.Equal(5, Related.PartOf("Đấu Phá Thương Khung Phần 5"));
        Assert.Equal(1, Related.PartOf("Nhất Nguyệt Thần Kiếm 1990 (Phần 01) – 20 Tập"));
        Assert.Equal(2, Related.PartOf("Squid Game Season 2"));
        Assert.Equal(3, Related.PartOf("Thế Giới Hoàn Mỹ 3"));
        Assert.Equal(4, Related.PartOf("Đấu Phá Thương Khung 4: Đào Vong"));
        Assert.Null(Related.PartOf("Tiên Nghịch"));
        Assert.Null(Related.PartOf("Trái Tim Quái Thú 2025"));
    }

    [Fact]
    public void SameSeriesMatchesOtherPartsOnly()
    {
        Assert.True(Related.SameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung Phần 1"));
        Assert.True(Related.SameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung"));
        Assert.True(Related.SameSeries("Avatar", "Avatar: Dòng Chảy Của Nước"));
        Assert.False(Related.SameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung Phần 5"));
        Assert.False(Related.SameSeries("Tiên Nghịch", "Tiên Nghịch Truyện Kỳ"));
        Assert.False(Related.SameSeries("Yêu", "Yêu 2"));
    }

    [Fact]
    public void PickSeriesOrdersByPart()
    {
        var found = new[] { F("Đấu Phá Thương Khung Phần 3"), F("Đấu Phá Thương Khung Phần 5", "https://x.test/me"), F("Đấu Phá Thương Khung"),
            F("Đấu Phá Thương Khung Phần 3", "https://kk.test/dpk3"), F("Kiếm Lai"), F("Đấu Phá Thương Khung Phần 2") };
        Assert.Equal(new[] { "Đấu Phá Thương Khung", "Đấu Phá Thương Khung Phần 2", "Đấu Phá Thương Khung Phần 3" },
            Related.PickSeries("Đấu Phá Thương Khung Phần 5", "https://x.test/me", found).Select(f => f.Title));
        var more = Related.PickSeries("Đấu Phá Thương Khung (Phần 5)", "https://x.test/me",
            new[] { F("Đấu Phá Thương Khung: Thức Tỉnh"), F("Đấu Phá Thương Khung 2: Ngừng Chiến"), F("Đấu Phá Thương Khung (Phần 2)") });
        Assert.Equal(new[] { "Đấu Phá Thương Khung: Thức Tỉnh", "Đấu Phá Thương Khung 2: Ngừng Chiến", "Đấu Phá Thương Khung (Phần 2)" },
            more.Select(f => f.Title));
    }

    [Fact]
    public void PickSimilarSkipsSelfSeriesAndPutsWatchedLast()
    {
        var found = new[] { F("Tiên Nghịch"), F("Phàm Nhân Tu Tiên"), F("Tiên Nghịch Phần 2"), F("Thế Giới Hoàn Mỹ"), F("Phàm Nhân Tu Tiên") };
        var got = Related.PickSimilar("Tiên Nghịch", "https://x.test/me", found, new HashSet<string> { KkPhim.Norm("Phàm Nhân Tu Tiên") });
        Assert.Equal(new[] { "Thế Giới Hoàn Mỹ", "Phàm Nhân Tu Tiên" }, got.Select(f => f.Title));
    }
}
