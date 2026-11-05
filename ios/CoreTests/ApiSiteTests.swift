import XCTest

/// mephim.ddns.net (11/2026): ứng dụng React, API JSON /api/movies/… (định dạng KKPhim) — cùng ca với ApiSiteTest.kt.
final class ApiSiteTests: XCTestCase {
    private let o = "https://mephim.ddns.net"

    private func json(_ s: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(s.utf8)) as? [String: Any]) ?? [:]
    }

    func testParsesListWithRelativePostersAndPaging() {
        let body = #"""
        {"status":true,"msg":"done","data":{"items":[
        {"name":"Tiên Nghịch","slug":"tien-nghich","poster_url":"uploads/movies/20260720/tien-nghich-poster.webp",
         "thumb_url":"uploads/movies/20260720/tien-nghich-thumb.webp","episode_current":"Tập 165"},
        {"name":"Avatar","slug":"avatar","poster_url":"https://phimimg.com/upload/vod/a.jpg","episode_current":"Full"},
        {"name":"Hết","slug":"het","thumb_url":"uploads/movies/x-thumb.webp","episode_current":"Hoàn Tất (12/12)"}],
        "params":{"pagination":{"totalItems":3629,"totalItemsPerPage":24,"currentPage":2,"totalPages":152}},
        "APP_DOMAIN_CDN_IMAGE":"https://phimimg.com"}}
        """#
        let url = "\(o)/api/movies/category/hoat-hinh?country=trung-quoc&page=2&limit=24"
        let p = ApiSite.parseList(json(body), url)
        XCTAssertEqual(p.films.map { $0.title }, ["Tiên Nghịch", "Avatar", "Hết"])
        XCTAssertEqual(p.films[0].url, "\(o)/phim/tien-nghich")
        XCTAssertEqual(p.films[0].poster, "https://phimimg.com/uploads/movies/20260720/tien-nghich-poster.webp")
        XCTAssertEqual(p.films[1].poster, "https://phimimg.com/upload/vod/a.jpg")
        XCTAssertEqual(p.films[2].poster, "https://phimimg.com/uploads/movies/x-thumb.webp")
        XCTAssertEqual(p.films.map { $0.badge }, ["Tập 165", "Full", "Tập 12"])
        XCTAssertEqual(p.nextUrl, "\(o)/api/movies/category/hoat-hinh?country=trung-quoc&page=3&limit=24")
        let last = body.replacingOccurrences(of: "\"currentPage\":2", with: "\"currentPage\":152")
        XCTAssertNil(ApiSite.parseList(json(last), url).nextUrl)
    }

    func testParsesTrendingItemsAtRoot() {
        let body = #"""
        {"status":true,"cdnDomain":"https://phimimg.com/uploads/movies","items":[
        {"name":"VisionQuest","slug":"vision","poster_url":"https://image.tmdb.org/t/p/w500/x.jpg","episode_current":"Tập 6"}]}
        """#
        let films = ApiSite.parseItems(json(body), o)
        XCTAssertEqual(films.map { $0.url }, ["\(o)/phim/vision"])
        XCTAssertEqual(films.first?.poster, "https://image.tmdb.org/t/p/w500/x.jpg")
    }

    func testParsesDetailGroupsPerLanguage() throws {
        let body = #"""
        {"status":true,"movie":{"name":"Tiên Nghịch","origin_name":"Renegade Immortal",
        "content":"<p>Phim kể về <b>Vương Lâm</b>, một thiếu niên bình thường.</p>",
        "poster_url":"https://phimimg.com/uploads/movies/20260720/tien-nghich-poster.webp","episode_current":"Tập 165",
        "time":"23 phút/tập","lang":"Vietsub + Thuyết Minh","year":2023,
        "country":[{"name":"Trung Quốc"}],"category":[{"name":"Cổ Trang"},{"name":"Hoạt Hình"}]},
        "episodes":[
         {"server_name":"Vietsub","server_data":[
           {"name":"Tập 01","slug":"tap-01","link_embed":"https://player.phimapi.com/player/?url=a","link_m3u8":"https://s4.phim1280.tv/a/index.m3u8"},
           {"name":"Tập 02","slug":"tap-02","link_embed":"","link_m3u8":"https://s4.phim1280.tv/b/index.m3u8"},
           {"name":"Tập 03","slug":"tap-03","link_embed":"","link_m3u8":""}]},
         {"server_name":"Thuyết Minh","server_data":[
           {"name":"Tập 01","slug":"tap-01","link_embed":"","link_m3u8":"https://s5.phim1280.tv/c/index.m3u8"}]}]}
        """#
        let d = try ApiSite.parseDetail(json(body), "\(o)/phim/tien-nghich")
        XCTAssertEqual(d.title, "Tiên Nghịch")
        XCTAssertEqual(d.otherTitle, "Renegade Immortal")
        XCTAssertEqual(d.description, "Phim kể về Vương Lâm, một thiếu niên bình thường.")
        XCTAssertEqual(d.info, "Tập 165  •  23 phút/tập  •  Vietsub + Thuyết Minh  •  Trung Quốc  •  2023  •  Cổ Trang, Hoạt Hình")
        XCTAssertEqual(d.groups.map { $0.name }, ["Vietsub", "Thuyết Minh"])
        XCTAssertEqual(d.groups[0].episodes.map { $0.title }, ["Tập 1", "Tập 2"]) // tập 3 chưa có video → bỏ
        XCTAssertEqual(d.groups[0].episodes[0].url, "\(o)/phim/tien-nghich/tap-01?sv=0")
        XCTAssertEqual(d.groups[1].episodes[0].url, "\(o)/phim/tien-nghich/tap-01?sv=1")
    }

    func testFallsBackToKkPhimApi() {
        XCTAssertEqual(ApiSite.fallback("\(o)/api/movies/newest?page=2&limit=24"),
                       "https://phimapi.com/v1/api/danh-sach/phim-moi-cap-nhat?page=2&limit=24")
        XCTAssertEqual(ApiSite.fallback("\(o)/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24"),
                       "https://phimapi.com/v1/api/danh-sach/hoat-hinh?country=trung-quoc&page=1&limit=24")
        XCTAssertEqual(ApiSite.fallback("\(o)/api/movies/the-loai/co-trang?page=1&limit=24"),
                       "https://phimapi.com/v1/api/the-loai/co-trang?page=1&limit=24")
        XCTAssertEqual(ApiSite.fallback("\(o)/api/movies/search?keyword=ti%C3%AAn&page=1&limit=24"),
                       "https://phimapi.com/v1/api/tim-kiem?keyword=ti%C3%AAn&page=1&limit=24")
        XCTAssertEqual(ApiSite.fallback("\(o)/api/movies/detail/tien-nghich"), "https://phimapi.com/phim/tien-nghich")
        XCTAssertNil(ApiSite.fallback("\(o)/api/movies/trending"))
    }

    func testFilmUrlFromEpisode() throws {
        XCTAssertEqual(try ApiSite.filmUrl("\(o)/phim/tien-nghich/tap-01?sv=1"), "\(o)/phim/tien-nghich")
        XCTAssertEqual(try ApiSite.filmUrl("\(o)/phim/tien-nghich"), "\(o)/phim/tien-nghich")
        XCTAssertTrue(ApiSite.isKnown("\(o)/"))
        XCTAssertEqual(ApiSite.searchUrl(o, " tiên nghịch "), "\(o)/api/movies/search?keyword=ti%C3%AAn+ngh%E1%BB%8Bch&page=1&limit=24")
    }
}
