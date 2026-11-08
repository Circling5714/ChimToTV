import XCTest

/// Trang chủ tổng hợp — chuyển từ FeedTest.kt của bản Android (đọc danh sách, gộp phim trùng, sở thích, dựng hàng, tập mới,
/// hồ sơ trẻ em). Mỗi test một kho UserDefaults riêng.
final class FeedTests: XCTestCase {
    private var suite = ""
    private let now = Feed.time("2026-11-08T20:00:00+07:00")
    private let hour: Int64 = 3_600_000
    private var day: Int64 { 24 * hour }
    private let kk = "https://www.kkphim.com"
    private let nc = "https://phim.nguonc.com"

    override func setUp() {
        super.setUp()
        suite = "FeedTests-\(UUID().uuidString)"
        Prefs.store = UserDefaults(suiteName: suite)!
        Prefs.reload()
    }

    override func tearDown() {
        UserDefaults().removePersistentDomain(forName: suite)
        Prefs.store = .standard
        Prefs.reload()
        super.tearDown()
    }

    private func root(_ s: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(s.utf8)) as? [String: Any]) ?? [:]
    }

    private var kkItems: [FeedItem] {
        Feed.parseKk(root("""
        {"status":"success","data":{"APP_DOMAIN_CDN_IMAGE":"https://phimimg.com","items":[
          {"name":"Mục Thần Ký","slug":"muc-than-ky","origin_name":"Tales Of Herding Gods","type":"hoathinh",
           "poster_url":"upload/vod/a.jpg","episode_current":"Tập 107","year":2024,"modified":{"time":"2026-11-08T11:55:29.000Z"},
           "category":[{"name":"Phiêu Lưu","slug":"phieu-luu"},{"name":"Viễn Tưởng","slug":"vien-tuong"}],
           "country":[{"name":"Trung Quốc","slug":"trung-quoc"}]},
          {"name":"Đấu Phá Thương Khung (Phần 5)","slug":"dau-pha-thuong-khung-phan-5","type":"hoathinh",
           "poster_url":"https://img.example/dp5.jpg","episode_current":"Tập 150","year":2022,"modified":{"time":"2026-11-08T10:00:00.000Z"}},
          {"name":"Thôn Phệ Tinh Không","slug":"thon-phe-tinh-khong","type":"hoathinh","episode_current":"Tập 248","year":2020,
           "modified":{"time":"2026-11-08T09:00:00.000Z"}},
          {"name":"Thiên Long Bát Bộ","slug":"thien-long-bat-bo-2021","type":"series","episode_current":"Hoàn Tất (50/50)","year":2021,
           "modified":{"time":"2026-11-01T09:00:00.000Z"}}
        ]}}
        """), kk)
    }

    private var ncItems: [FeedItem] {
        Feed.parseNc(root("""
        {"status":"success","items":[
          {"name":"Mục Thần Ký","slug":"muc-than-ky","original_name":"Mu Shen Ji, Tales of Herding Gods","year":"2024",
           "current_episode":"Tập 108","thumb_url":"https://phim.nguonc.com/t.jpg","modified":"2026-11-08T12:30:00.000000Z"},
          {"name":"Đấu Phá Thương Khung 5","slug":"dau-pha-5","year":"2022","current_episode":"Tập 150","modified":"2026-11-08T08:00:00.000000Z"},
          {"name":"Thôn Phệ Tinh Không 2","slug":"thon-phe-2","year":"2024","current_episode":"Tập 222","modified":"2026-11-08T12:00:00.000000Z"}
        ]}
        """), nc)
    }

    private func web(_ titles: [(String, String?)]) -> [FeedItem] {
        Feed.fromFilms(titles.map { Film(url: "https://yanhh3d.ee/" + KkPhim.norm($0.0).replacingOccurrences(of: " ", with: "-"),
                                         title: $0.0, poster: nil, badge: $0.1) })
    }

    func testEpisodeNumbersAndTimes() {
        XCTAssertEqual(Feed.epOf("Tập 108"), 108)
        XCTAssertEqual(Feed.epOf("Hoàn Tất (16/16)"), 16)
        XCTAssertEqual(Feed.epOf("7/??"), 7)
        XCTAssertNil(Feed.epOf("Full"))
        XCTAssertNil(Feed.epOf("Top 1"))
        let utc = Feed.time("2026-11-08T14:12:36Z")
        XCTAssertEqual(utc, Feed.time("2026-11-08T21:12:36+07:00"))
        XCTAssertEqual(utc, Feed.time("2026-11-08T14:12:36.000000Z"))
        XCTAssertEqual(utc, Feed.time("2026-11-08 21:12:36"))
        XCTAssertEqual(0, Feed.time("không phải giờ"))
    }

    func testParsesKkAndNguonC() {
        let m = kkItems[0]
        XCTAssertEqual(m.url, "\(kk)/phim/muc-than-ky")
        XCTAssertEqual(m.poster, "https://phimimg.com/upload/vod/a.jpg")
        XCTAssertEqual(m.ep, 107)
        XCTAssertEqual(m.tags?.genres, ["phieu-luu", "vien-tuong"])
        XCTAssertEqual(m.tags?.type, "hoathinh")
        XCTAssertEqual(m.host, "kkphim.com")
        let n = ncItems[0]
        XCTAssertEqual(n.ep, 108)
        XCTAssertTrue(n.names.contains(KkPhim.norm("Mu Shen Ji")))
        XCTAssertEqual(FeedItem.of(m.json()), m)
    }

    func testMergesSameFilmAcrossSources() {
        let y = web([("Đấu Phá Thương Khung Phần 5", "Tập 150"), ("Mục Thần Ký", "Tập 107"), ("Thôn Phệ Tinh Không", "Tập 248")])
        let films = Feed.merge([y, kkItems, ncItems, Array(kkItems.prefix(1))], now: now)
        func find(_ n: String) -> [FeedFilm] { films.filter { $0.names.contains(KkPhim.norm(n)) } }
        XCTAssertEqual(find("Đấu Phá Thương Khung Phần 5").count, 1)
        XCTAssertEqual(find("Đấu Phá Thương Khung Phần 5")[0].items.count, 3)
        XCTAssertEqual(find("Mục Thần Ký").count, 1)
        XCTAssertEqual(find("Mục Thần Ký")[0].items.count, 3) // cùng phim trong hai danh sách KKPhim vẫn một nhóm
        XCTAssertEqual(find("Mục Thần Ký")[0].latest?.host, "phim.nguonc.com")
        XCTAssertEqual(find("Thôn Phệ Tinh Không").count, 1)
        XCTAssertEqual(find("Thôn Phệ Tinh Không 2").count, 1)
        // Một trang có hai mục cùng tên → không gộp
        let two = Feed.fromFilms([Film(url: "https://yanhh3d.ee/a", title: "Tiên Nghịch", poster: nil, badge: nil),
                                  Film(url: "https://yanhh3d.ee/b", title: "Tiên Nghịch", poster: nil, badge: nil)])
        XCTAssertEqual(Feed.merge([two], now: now).count, 2)
    }

    func testTasteFavoursRecentAndMuchWatched() {
        let tags = ["Tiên Nghịch": Tags(genres: ["vo-thuat", "hoat-hinh", "chinh-kich"], countries: ["trung-quoc"], type: "hoathinh"),
                    "Hạ Cánh Nơi Anh": Tags(genres: ["tinh-cam"], countries: ["han-quoc"], type: "series")]
        let history = [
            HistoryItem(filmUrl: "https://yanhh3d.ee/tien-nghich", title: "Tiên Nghịch", poster: nil, epTitle: "Tập 166", epUrl: "e1", time: now - day),
            HistoryItem(filmUrl: "https://www.kkphim.com/phim/ha-canh", title: "Hạ Cánh Nơi Anh", poster: nil, epTitle: "Tập 3", epUrl: "e2", time: now - 60 * day),
        ]
        let t = HomeFeed.taste(history, tagsOf: { tags[$0] }, watched: { $0.contains("tien") ? 20 : 3 }, now: now)
        XCTAssertGreaterThan(t.genre["vo-thuat"] ?? 0, 15)
        XCTAssertNil(t.genre["hoat-hinh"])
        XCTAssertEqual(t.genre["chinh-kich"] ?? 0, (t.genre["vo-thuat"] ?? 0) / 2, accuracy: 1e-9)
        XCTAssertEqual(t.combo.max { $0.value < $1.value }?.key, HomeFeed.Combo(type: "hoathinh", country: "trung-quoc", genre: "vo-thuat"))
    }

    private func part(_ role: HomeFeed.Role, _ key: String, _ host: String, _ items: [FeedItem], title: String? = nil, id: String = "",
                      state: Feed.State = .ok) -> (HomeFeed.Part, Feed.Result) {
        let r = Feed.Req(key: key, host: host) { Feed.Loaded(items: items) }
        return (HomeFeed.Part(role: role, req: r, title: title, id: id),
                Feed.Result(req: r, state: state, items: items, categories: nil, error: state == .ok ? nil : "lỗi 502", resting: false,
                            ms: 0, savedAt: now))
    }

    private func plan(_ parts: [(HomeFeed.Part, Feed.Result)], kids: Bool = false) -> HomeFeed.Plan {
        HomeFeed.Plan(parts: parts.map { $0.0 }, src: HomeFeed.Sources(kk: kk, nc: nc, vs: "https://vsmov.com", k20: nil, web: ["https://yanhh3d.ee"]),
                      kids: kids)
    }

    private func ctx(_ history: [HistoryItem], seen: [String: Double] = [:], follows: Set<String> = []) -> HomeFeed.Ctx {
        HomeFeed.Ctx(history: history, follows: follows, seen: { seen[$0] }, base: "yanhh3d.ee",
                     sites: ["https://yanhh3d.ee", kk, nc], now: now)
    }

    func testNewEpisodesOfFilmsBeingWatched() {
        let parts = [
            part(.web, "web|y", "yanhh3d.ee", web([("Mục Thần Ký", "Tập 107"), ("Tiên Nghịch", "Tập 168"), ("Thiên Long Bát Bộ", "Tập 45")])),
            part(.new, "new|kk", "kkphim.com", kkItems),
            part(.new, "new|nc", "phim.nguonc.com", ncItems),
        ]
        let history = [
            HistoryItem(filmUrl: "https://yanhh3d.ee/muc-than-ky", title: "Mục Thần Ký", poster: nil, epTitle: "Tập 107", epUrl: "e1", time: now - day),
            HistoryItem(filmUrl: "https://yanhh3d.ee/tien-nghich", title: "Tiên Nghịch", poster: nil, epTitle: "Tập 166", epUrl: "e2", time: now - 2 * day),
            HistoryItem(filmUrl: "https://yanhh3d.ee/thien-long-bat-bo", title: "Thiên Long Bát Bộ", poster: nil, epTitle: "Tập 30", epUrl: "e3", time: now - day),
        ]
        let fresh = Dictionary(uniqueKeysWithValues: HomeFeed.buildRows(plan(parts), parts, ctx(history, seen: [KkPhim.norm("Thiên Long Bát Bộ"): 45])).fresh
            .map { ($0.card.film.title, $0) })
        XCTAssertEqual(Set(fresh.keys), ["Mục Thần Ký", "Tiên Nghịch"])
        XCTAssertEqual(fresh["Mục Thần Ký"]?.card.film.badge, "Tập 108 mới")
        XCTAssertEqual(fresh["Mục Thần Ký"]?.card.chip, "có ở nguonc")
        XCTAssertEqual(fresh["Mục Thần Ký"]?.card.film.url, "\(nc)/phim/muc-than-ky")
        XCTAssertEqual(fresh["Tiên Nghịch"]?.card.film.badge, "+2 · Tập 168")
        // Đang xem dở từ trước (tập 31 / 108): không phải «tập mới»
        let behind = [HistoryItem(filmUrl: "https://yanhh3d.ee/muc-than-ky", title: "Mục Thần Ký", poster: nil, epTitle: "Tập 31", epUrl: "e1", time: now - 3 * day)]
        XCTAssertTrue(HomeFeed.buildRows(plan(parts), parts, ctx(behind)).fresh.isEmpty)
        // Đã thấy tập 108 → thôi báo
        let seen = HomeFeed.buildRows(plan(parts), parts, ctx(history, seen: [KkPhim.norm("Mục Thần Ký"): 108]))
        XCTAssertFalse(seen.fresh.contains { $0.card.film.title == "Mục Thần Ký" })
    }

    func testNewestRowMergesSourcesAndMarksBrokenOnes() {
        let parts = [
            part(.web, "web|y", "yanhh3d.ee", web([("Đấu Phá Thương Khung Phần 5", "Tập 150")])),
            part(.new, "new|kk", "kkphim.com", kkItems, state: .cached), // KKPhim lỗi, dùng bản lưu
            part(.new, "new|nc", "phim.nguonc.com", ncItems),
        ]
        let newest = HomeFeed.buildRows(plan(parts), parts, ctx([])).rows.first { $0.id == "newest" }!
        XCTAssertEqual(newest.cards.first { $0.film.title == "Thiên Long Bát Bộ" }?.chip, "bản lưu")
        let dp = newest.cards.first { KkPhim.norm($0.film.title) == KkPhim.norm("Đấu Phá Thương Khung Phần 5") }!
        XCTAssertEqual(dp.chip, "2 nguồn")
        XCTAssertEqual(dp.film.url, "https://yanhh3d.ee/dau-pha-thuong-khung-5") // trang đang dùng mở trước
        XCTAssertEqual(dp.links.count, 3)
    }

    func testKidsProfileKeepsOnlyChildFriendlyAnimation() {
        func item(_ slug: String, _ type: String, _ genres: [String]) -> String {
            #"{"name":"\#(slug)","slug":"\#(slug)","type":"\#(type)","modified":{"time":"2026-11-08T10:00:00Z"},"category":[\#(genres.map { "{\"slug\":\"\($0)\"}" }.joined(separator: ","))]}"#
        }
        let list = Feed.parseKk(root(#"{"data":{"items":["# + [
            item("doraemon", "hoathinh", ["gia-dinh"]), item("chiikawa", "hoathinh", ["tre-em"]), item("conan", "hoathinh", ["bi-an"]),
            item("edgerunners", "hoathinh", ["hinh-su"]), item("phim-ma", "hoathinh", ["kinh-di"]), item("nguoi-dong", "series", ["gia-dinh"]),
            item("Horror Collector", "hoathinh", ["tre-em"]),
        ].joined(separator: ",") + "]}}"), kk)
        let vs = Feed.fromFilms([Film(url: "https://vsmov.com/phim/na-tra", title: "Na Tra Truyền Kỳ", poster: nil, badge: nil)])
        let parts = [part(.kids, "list|kk", "kkphim.com", list, title: "Hoạt hình thiếu nhi", id: "kids-young"),
                     part(.kids, "list|vs", "vsmov.com", vs, title: "Hoạt hình thiếu nhi", id: "kids-young")]
        let rows = HomeFeed.buildRows(plan(parts, kids: true), parts, ctx([])).rows
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(Set(rows[0].cards.map { $0.film.title }), ["doraemon", "chiikawa", "conan", "Na Tra Truyền Kỳ"])
        XCTAssertEqual(HomeFeed.buildRows(plan(parts), parts, ctx([])).rows[0].cards.count, 8)
    }

    func testPrefsForTheFeed() {
        XCTAssertTrue(FilmTags.needsLookup("Tiên Nghịch"))
        FilmTags.remember(KkPhim.norm("Tiên Nghịch"), Tags(genres: ["vo-thuat"], countries: ["trung-quoc"], type: "hoathinh"))
        XCTAssertEqual(FilmTags.of("Tiên nghịch")?.type, "hoathinh")
        FilmTags.remember(KkPhim.norm("Phim Lạ"), nil)
        XCTAssertNil(FilmTags.of("Phim Lạ"))
        XCTAssertFalse(FilmTags.needsLookup("Phim Lạ"))

        Prefs.setSeenEp("muc than ky", 107)
        Prefs.setSeenEp("muc than ky", 105)
        XCTAssertEqual(Prefs.seenEp("muc than ky"), 107)
        Prefs.savePosition("https://yanhh3d.ee/muc-than-ky/tap-1", seconds: 0, filmUrl: "https://yanhh3d.ee/muc-than-ky")
        Prefs.savePosition("https://yanhh3d.ee/muc-than-ky/tap-2", seconds: 30, filmUrl: "https://yanhh3d.ee/muc-than-ky")
        XCTAssertEqual(Prefs.watchedCounts()["https://yanhh3d.ee/muc-than-ky"], 2)
        for i in 0..<45 { Prefs.recordWatchLang(i % 3 == 0 ? "vs" : "tm") }
        XCTAssertEqual(Prefs.watchLangs.count, 40)
        XCTAssertTrue(Prefs.homeMixOn)
        Prefs.homeMix = false
        XCTAssertFalse(Prefs.homeMixOn)
        Prefs.kids = true
        XCTAssertTrue(Prefs.homeMixOn)
        Prefs.homeTopics = ["cn", "kh"]
        XCTAssertEqual(Prefs.homeTopics, ["cn", "kh"])
    }
}
