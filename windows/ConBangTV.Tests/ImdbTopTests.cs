using System.Text.Json;
using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// Điểm IMDb từ dữ liệu KKPhim + xếp hạng «Điểm IMDb cao» (ImdbTop.cs) — cùng ca với ImdbTopTest.kt / ImdbTopTests.swift.
public class ImdbTopTests
{
    private static Dictionary<string, object?> ImdbObj(double s, long v) => new() { ["id"] = "tt1", ["vote_average"] = s, ["vote_count"] = v };

    [Fact]
    public void ParsesScoreAndIgnoresTinyVoteCounts()
    {
        Assert.Equal(new Imdb(8.5, 377702), Imdb.Of(ImdbObj(8.5, 377702)));
        Assert.Null(Imdb.Of(ImdbObj(10, 1)));
        Assert.Null(Imdb.Of(ImdbObj(0, 0)));
        Assert.Null(Imdb.Of(null));
        Assert.Equal("IMDb 8.5", new Imdb(8.5, 1000).Label);
        Assert.Equal("IMDb 7.0", new Imdb(7, 1000).Label);
        Assert.Equal("IMDb 8.5 (377 nghìn lượt chấm)", new Imdb(8.5, 377702).Detail);
        Assert.Equal("2,4 triệu", Imdb.VotesText(2_400_000));
        Assert.Equal("1 triệu", Imdb.VotesText(1_000_000));
        Assert.Equal("850", Imdb.VotesText(850));
    }

    [Fact]
    public void FeedItemKeepsImdbInCache()
    {
        var f = new FeedItem("https://www.kkphim.com/phim/a", "A", null, null, new HashSet<string> { "a" }, 2026, 0, null, null, new Imdb(7.9, 5000));
        Assert.Equal(new Imdb(7.9, 5000), FeedItem.Of(f.Json())!.Imdb);
        var g = f with { Imdb = null };
        Assert.Null(FeedItem.Of(g.Json())!.Imdb);
    }

    [Fact]
    public void WeightedRankPullsLowVoteFilmsTowardTheMean()
    {
        static Film F(string n, double s, int v) => new($"https://www.kkphim.com/phim/{n}", n, null, null, new Imdb(s, v));
        var films = new List<Film>
        {
            F("nhieu-luot", 8.6, 2_000_000), F("it-luot", 9.4, 1_200), F("trung-binh", 7.0, 300_000), F("qua-it", 9.9, 300),
            new("https://www.kkphim.com/phim/khong", "không điểm", null, null),
        };
        Assert.Equal(new[] { "nhieu-luot", "it-luot", "trung-binh" }, ImdbTop.Rank(films, 1000, 10).Select(f => f.Title));
        Assert.Empty(ImdbTop.Rank(films, 5_000_000, 10));
        Assert.Single(ImdbTop.Rank(films, 1000, 1));
    }

    [Fact]
    public void PagingUrls()
    {
        Assert.Equal("https://www.kkphim.com/api/movies/the-loai/hanh-dong?page=3&limit=64",
            ImdbTop.WithPage("https://www.kkphim.com/api/movies/the-loai/hanh-dong?page=1&limit=24", 3));
        Assert.Equal("https://www.kkphim.com/api/movies/category/phim-le?page=2&limit=64",
            ImdbTop.WithPage("https://www.kkphim.com/api/movies/category/phim-le", 2));
        Assert.Equal("https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=4&limit=64",
            ImdbTop.WithPage("https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24", 4));
        Assert.True(ImdbTop.CanRank("https://www.kkphim.com/api/movies/category/phim-le?page=1&limit=24"));
        Assert.False(ImdbTop.CanRank("https://vsmov.com/api/the-loai/hoat-hinh?page=1"));
    }
}
