import XCTest

/// hoathinh3d.pw (11/2026): danh sách nạp bằng API JSON (cùng ca với HalimApiTest.kt của bản Android).
final class HalimApiTests: XCTestCase {
    private let pageHtml = #"""
    <main id="main-contents"><div class="mv-grid" id="mvGrid"></div></main>
    <script>(function(){ const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/halim\/v1\/movies"; const PER_PAGE = 20;
    const SORTBY = "default"; })();</script>
    """#

    func testFindsApiInPage() {
        XCTAssertEqual(Site.HalimApi.firstPage(pageHtml), "https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=1&per_page=20")
        XCTAssertEqual(Site.HalimApi.firstPage(#"const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/halim\/v1\/ongoing";"#),
                       "https://hoathinh3d.pw/wp-json/halim/v1/ongoing?page=1&per_page=20")
        XCTAssertNil(Site.HalimApi.firstPage(#"<div class="halim-item"></div>"#))
        XCTAssertEqual(Site.HalimApi.scheduleApi(#"const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/movie-schedule\/v1\/daily";"#),
                       "https://hoathinh3d.pw/wp-json/movie-schedule/v1/daily")
    }

    func testParsesItemsAndNextPage() {
        let json = #"""
        {"success":true,"page":1,"per_page":20,"total":33,"has_more":true,"items":[
        {"id":1,"title":"Tiên Nghịch","permalink":"https:\/\/hoathinh3d.pw\/tien-nghich","poster_url":"https:\/\/x\/a.jpg","average_rating":4.6,"latest_episode":"Tập 165"},
        {"id":2,"rank":2,"title":"Mục Thần Ký","permalink":"https:\/\/hoathinh3d.pw\/muc-than-ky","poster_url":"https:\/\/x\/b.jpg"},
        {"id":3,"title":"Phim","permalink":"https:\/\/hoathinh3d.pw\/phim","poster_url":"","latest_episode":"12"}]}
        """#
        let url = "https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=1&per_page=20"
        let p = Site.HalimApi.parse(Data(json.utf8), url)
        XCTAssertEqual(p.films.map { $0.title }, ["Tiên Nghịch", "Mục Thần Ký", "Phim"])
        XCTAssertEqual(p.films.map { $0.badge }, ["Tập 165", "Top 2", "Tập 12"])
        XCTAssertNil(p.films[2].poster)
        XCTAssertEqual(p.nextUrl, "https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=2&per_page=20")
        let last = json.replacingOccurrences(of: "\"has_more\":true", with: "\"has_more\":false")
        XCTAssertNil(Site.HalimApi.parse(Data(last.utf8), url).nextUrl)
    }

    func testParsesScheduleDay() {
        let json = #"""
        {"success":true,"day":"thu-nam","early_movies":[{"title":"Tru Tiên","permalink":"https:\/\/h\/tru-tien",
        "thumbnail":"https:\/\/x\/t.jpg","latest_episode":"Tập 91","is_early_screening":true,"early_screening_time":"18:00"}],
        "regular_movies":[{"title":"A","permalink":"https:\/\/h\/a","thumbnail":"","latest_episode":"Tập 17"}]}
        """#
        let films = Site.HalimApi.parseDay(Data(json.utf8))
        XCTAssertEqual(films.map { $0.title }, ["Tru Tiên", "A"])
        XCTAssertEqual(films[0].badge, "18:00 · Tập 91")
        XCTAssertEqual(films[0].poster, "https://x/t.jpg")
    }
}
