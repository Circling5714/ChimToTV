import XCTest

/// Điểm IMDb từ dữ liệu KKPhim + xếp hạng «Điểm IMDb cao» (ImdbTop.swift) — cùng ca với ImdbTopTest.kt của bản TV.
final class ImdbTopTests: XCTestCase {
    private func imdb(_ s: Double, _ v: Int) -> [String: Any] { ["id": "tt1", "vote_average": s, "vote_count": v] }

    func testParsesScoreAndIgnoresTinyVoteCounts() {
        XCTAssertEqual(Imdb.of(imdb(8.5, 377702)), Imdb(score: 8.5, votes: 377702))
        XCTAssertNil(Imdb.of(imdb(10, 1)))
        XCTAssertNil(Imdb.of(imdb(0, 0)))
        XCTAssertNil(Imdb.of(nil))
        XCTAssertEqual(Imdb(score: 8.5, votes: 1000).label, "IMDb 8.5")
        XCTAssertEqual(Imdb(score: 7, votes: 1000).label, "IMDb 7.0")
        XCTAssertEqual(Imdb(score: 8.5, votes: 377702).detail, "IMDb 8.5 (377 nghìn lượt chấm)")
        XCTAssertEqual(Imdb.votesText(2_400_000), "2,4 triệu")
        XCTAssertEqual(Imdb.votesText(1_000_000), "1 triệu")
        XCTAssertEqual(Imdb.votesText(850), "850")
    }

    func testListAndDetailCarryImdb() throws {
        let body = """
        {"status":true,"data":{"items":[
            {"name":"Sử Thi Odyssey","slug":"su-thi-odyssey","poster_url":"a.jpg","episode_current":"Full",
             "imdb":{"id":"tt33764258","vote_average":8.5,"vote_count":377702}},
            {"name":"Không điểm","slug":"khong-diem","poster_url":"b.jpg","imdb":{"id":"tt1","vote_average":0,"vote_count":0}}
        ]}}
        """
        let root = try JSONSerialization.jsonObject(with: Data(body.utf8)) as! [String: Any]
        let films = ApiSite.parseItems(root, "https://www.kkphim.com")
        XCTAssertEqual(films[0].imdb, Imdb(score: 8.5, votes: 377702))
        XCTAssertNil(films[1].imdb)
    }

    func testFeedItemKeepsImdbInCache() {
        let f = FeedItem(url: "https://www.kkphim.com/phim/a", title: "A", poster: nil, badge: nil, names: ["a"], year: 2026,
                         updated: 0, ep: nil, tags: nil, imdb: Imdb(score: 7.9, votes: 5000))
        XCTAssertEqual(FeedItem.of(f.json())?.imdb, Imdb(score: 7.9, votes: 5000))
        let g = FeedItem(url: "https://www.kkphim.com/phim/a", title: "A", poster: nil, badge: nil, names: ["a"], year: 2026,
                         updated: 0, ep: nil, tags: nil)
        XCTAssertNil(FeedItem.of(g.json())?.imdb)
    }

    func testWeightedRankPullsLowVoteFilmsTowardTheMean() {
        func film(_ n: String, _ s: Double, _ v: Int) -> Film {
            Film(url: "https://www.kkphim.com/phim/\(n)", title: n, poster: nil, badge: nil, imdb: Imdb(score: s, votes: v))
        }
        let films = [film("nhieu-luot", 8.6, 2_000_000), film("it-luot", 9.4, 1_200), film("trung-binh", 7.0, 300_000),
                     film("qua-it", 9.9, 300), Film(url: "https://www.kkphim.com/phim/khong", title: "không điểm", poster: nil, badge: nil)]
        XCTAssertEqual(ImdbTop.rank(films, minVotes: 1000, top: 10).map(\.title), ["nhieu-luot", "it-luot", "trung-binh"])
        XCTAssertTrue(ImdbTop.rank(films, minVotes: 5_000_000, top: 10).isEmpty)
        XCTAssertEqual(ImdbTop.rank(films, minVotes: 1000, top: 1).count, 1)
    }

    func testPagingUrls() {
        XCTAssertEqual(ImdbTop.withPage("https://www.kkphim.com/api/movies/the-loai/hanh-dong?page=1&limit=24", 3),
                       "https://www.kkphim.com/api/movies/the-loai/hanh-dong?page=3&limit=64")
        XCTAssertEqual(ImdbTop.withPage("https://www.kkphim.com/api/movies/category/phim-le", 2),
                       "https://www.kkphim.com/api/movies/category/phim-le?page=2&limit=64")
        XCTAssertEqual(ImdbTop.withPage("https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24", 4),
                       "https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=4&limit=64")
        XCTAssertTrue(ImdbTop.canRank("https://www.kkphim.com/api/movies/category/phim-le?page=1&limit=24"))
        XCTAssertFalse(ImdbTop.canRank("https://vsmov.com/api/the-loai/hoat-hinh?page=1"))
    }
}
