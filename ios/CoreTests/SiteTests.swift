import XCTest
import SwiftSoup

/// Bộ đọc trang HTML (yanhh3d / HaLim) trên HTML dựng tay theo đúng bộ chọn của Site — kiểm SwiftSoup hiểu các bộ
/// chọn giống jsoup. Và bộ đọc JSON tại chỗ (JSONScan).
final class SiteTests: XCTestCase {
    let base = "https://hh.test"

    func testHomeCardsUnderHeadings() throws {
        let html = """
        <html><body>
        <div class="block_area"><h2 class="cat-heading">Mới cập nhật</h2>
          <div class="flw-item"><div class="film-poster"><img data-src="/img/a.jpg" alt="Ảnh A">
            <a class="film-poster-ahref" href="/phim/a"></a></div><div class="tick-rate">Tập 5</div>
            <h3 class="film-name"><a href="/phim/a">Phim A</a></h3></div>
          <div class="flw-item"><a href="/phim/b" title="Phim B"><img src="https://cdn.test/b.jpg"></a></div>
        </div>
        <div class="block_area-realtime"><h2 class="cat-heading"></h2>
          <li class="item-top"><a href="/phim/c"><h4>Phim C</h4></a></li>
        </div>
        <nav><ul><li class="menu-item"><a href="/the-loai/tien-hiep">Tiên Hiệp</a></li>
          <li class="menu-item"><a href="#">Thể loại</a></li><li class="menu-item"><a href="/dang-nhap">Đăng nhập</a></li></ul></nav>
        </body></html>
        """
        let h = Site.parseHome(try SwiftSoup.parse(html, base + "/"))
        XCTAssertEqual(h.sections.map { $0.title }, ["Mới cập nhật", "Xem nhiều"])
        let a = h.sections[0].films[0]
        XCTAssertEqual(a.title, "Phim A")
        XCTAssertEqual(a.url, "\(base)/phim/a")
        XCTAssertEqual(a.poster, "\(base)/img/a.jpg")
        XCTAssertEqual(a.badge, "Tập 5")
        XCTAssertEqual(h.sections[0].films[1].title, "Phim B")
        XCTAssertEqual(h.sections[1].films.map { $0.title }, ["Phim C"])
        XCTAssertEqual(h.categories.map { $0.title }, ["Tiên Hiệp"])
    }

    func testDetailEpisodesSortedAndGrouped() throws {
        let html = """
        <html><head><meta property="og:image" content="https://cdn.test/p.jpg"></head><body>
        <h1 class="film-name">Tiên Nghịch</h1>
        <div class="film-description"><div class="text">Cuộc hành trình của Vương Lâm, một thiếu niên bình thường.</div></div>
        <div class="halim-server"><span class="halim-server-name">#Thuyết Minh:</span><ul class="halim-list-eps">
          <li><a href="/xem/tn/tap-2-tm">2</a></li><li><a href="/xem/tn/tap-1-tm">1</a></li><li><a href="/xem/tn/tap-10-tm">10</a></li></ul></div>
        <div class="halim-server"><span class="halim-server-name">Vietsub</span><ul class="halim-list-eps">
          <li><a href="/xem/tn/tap-1">Tập 1</a></li></ul></div>
        <div class="category"><a rel="category tag" href="/the-loai/a">Tiên Hiệp</a></div>
        </body></html>
        """
        let d = Site.parseDetail(try SwiftSoup.parse(html, "\(base)/phim/tn"), "\(base)/phim/tn")
        XCTAssertEqual(d.title, "Tiên Nghịch")
        XCTAssertEqual(d.poster, "https://cdn.test/p.jpg")
        XCTAssertEqual(d.groups.map { $0.name }, ["Thuyết Minh", "Vietsub"])
        XCTAssertEqual(d.groups[0].episodes.map { $0.title }, ["Tập 1", "Tập 2", "Tập 10"])
        XCTAssertEqual(d.info, "Tiên Hiệp")
        XCTAssertEqual(d.latestEpisode?.title, "Tập 10")
    }

    func testSourceWithPlayerPhp() throws {
        let html = """
        <html><body>
        <div id="list_sv"><a data-type="tiktik">VIETSUB 1080 V1</a><a data-type="vip2">VIETSUB 1080 V2</a><a data-type="">x</a></div>
        <ul><li class="active"><a class="ep-item active" data-post-id="123" data-ep="165" data-sv="1" href="/xem/tn/tap-165">165</a></li></ul>
        <script>jQuery.ajax({ url: '/player/player.php', data: { action: 'dox_ajax_player' } });</script>
        </body></html>
        """
        let epUrl = "\(base)/xem/tn/tap-165"
        let s = Site.parseSource(try SwiftSoup.parse(html, epUrl), epUrl)
        XCTAssertFalse(s.pageMode)
        XCTAssertEqual(s.servers.map { $0.type }, ["tiktik", "vip2"])
        XCTAssertEqual(s.postId, "123")
        XCTAssertEqual(s.ep, "165")
        XCTAssertEqual(s.playerUrl, "\(base)/player/player.php")
    }

    func testPageModeWithoutPlayer() throws {
        let epUrl = "\(base)/xem/tn/tap-1.html"
        let s = Site.parseSource(try SwiftSoup.parse("<html><body><div id='player'></div></body></html>", epUrl), epUrl)
        XCTAssertTrue(s.pageMode)
        XCTAssertEqual(s.ep, "tap-1")
    }

    func testJSONScan() {
        let s = #"xx{"a":"b\"cạ😀","n":[1,2.5,-3e2,true,null],"o":{"k":"v"}} trailing"#
        let v = JSONScan.value(s, at: 2) as? [String: Any]
        XCTAssertEqual(v?["a"] as? String, "b\"cạ😀")
        let n = v?["n"] as? [Any]
        XCTAssertEqual(n?[0] as? Int, 1)
        XCTAssertEqual(n?[1] as? Double, 2.5)
        XCTAssertEqual(n?[2] as? Double, -300)
        XCTAssertEqual(n?[3] as? Bool, true)
        XCTAssertTrue(n?[4] is NSNull)
        XCTAssertEqual((v?["o"] as? [String: Any])?["k"] as? String, "v")
        XCTAssertNil(JSONScan.value(#"{"a":"#, at: 0))
    }

    func testUpdateSummaryAndVersions() {
        XCTAssertTrue(UpdateCheck.isNewer("1.0.1", than: "1.0"))
        XCTAssertFalse(UpdateCheck.isNewer("1.0", than: "1.0"))
        XCTAssertTrue(UpdateCheck.isNewer("1.10", than: "1.9"))
        let notes = "Mở đầu\n## Có gì mới\n- **Theo dõi** phim\n- `x` y\n## Kỹ thuật\n- SHA-256 abc"
        XCTAssertEqual(UpdateCheck.summary(notes), "Theo dõi phim\nx y")
        XCTAssertEqual(UpdateCheck.trollStoreURL("https://g.test/a b.ipa")?.absoluteString,
                       "apple-magnifier://install?url=https%3A%2F%2Fg.test%2Fa%20b.ipa")
    }

    func testAdBlockRules() throws {
        XCTAssertTrue(AdBlock.isAd("static.cloudflareinsights.com"))
        XCTAssertFalse(AdBlock.isAd("ssl.p.jwpcdn.com"))
        let rules = try JSONSerialization.jsonObject(with: Data(AdBlock.ruleListJSON.utf8)) as? [[String: Any]]
        XCTAssertEqual(rules?.count, AdBlock.hosts.count)
    }
}
