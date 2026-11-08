import XCTest

/// Server dự phòng KKPhim + trang KKPhim (11/2026) — cùng ca với KkPhimTest.kt / ApiSiteTest.kt.
final class KkPhimTests: XCTestCase {
    private func json(_ s: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(s.utf8)) as? [String: Any]) ?? [:]
    }

    private func movie(_ name: String, _ origin: String = "", type: String = "hoathinh", slug: String = "x") -> [String: Any] {
        ["name": name, "origin_name": origin, "type": type, "slug": slug]
    }

    func testNormalizesSeasonsAndAccents() {
        XCTAssertEqual(KkPhim.norm("Đấu Phá Thương Khung Phần 5"), KkPhim.norm("Đấu Phá Thương Khung (Phần 5)"))
        XCTAssertEqual(KkPhim.norm("Đấu Phá Thương Khung (Phần 5)"), "dau pha thuong khung 5")
        XCTAssertEqual(KkPhim.norm("Sword of Coming SS2"), KkPhim.norm("Sword of Coming (Season 2)"))
        XCTAssertEqual(KkPhim.norm("Fights Break Sphere 5"), KkPhim.norm("Fights Break Sphere (Season 5)"))
        XCTAssertEqual(KkPhim.norm("Đấu La Đại Lục 2: Tuyệt Thế Đường Môn"), KkPhim.norm("Đấu La Đại Lục 2 (Tuyệt Thế Đường Môn)"))
        XCTAssertEqual(KkPhim.norm("Spare Me, Great Lord! Season 3"), KkPhim.norm("Spare Me, Great Lord! (Season 3)"))
    }

    func testMatchesByNameOrOriginalName() {
        let mine: Set = [KkPhim.norm("Già Thiên"), KkPhim.norm("Zhe Tian")]
        XCTAssertTrue(KkPhim.matches(movie("Già Thiên", "Shrouding The Heavens"), mine))
        XCTAssertTrue(KkPhim.matches(movie("Che Trời", "Zhe Tian"), mine))
        XCTAssertFalse(KkPhim.matches(movie("Già Thiên Movie", "Shrouding The Heavens Movie"), mine))
        let doctor = movie("Thần Y Cổ Đại Ở Đô Thị (Đô Thị Cổ Tiên Y)", "Urban Miracle Doctor")
        XCTAssertTrue(KkPhim.matches(doctor, [KkPhim.norm("Thần Y Cổ Đại Ở Đô Thị")]))
        XCTAssertTrue(KkPhim.matches(doctor, [KkPhim.norm("Đô Thị Cổ Tiên Y")]))
        // Trong ngoặc chỉ là số phần: phần 5 không được ghép với phim "Đấu Phá Thương Khung" (phần 1)
        let s5 = movie("Đấu Phá Thương Khung (Phần 5)", "Fights Break Sphere (Season 5)")
        XCTAssertFalse(KkPhim.matches(s5, [KkPhim.norm("Đấu Phá Thương Khung")]))
        XCTAssertTrue(KkPhim.matches(s5, [KkPhim.norm("Đấu Phá Thương Khung Phần 5")]))
    }

    func testPrefersAnimationAmongSameNameResults() {
        let mine: Set = [KkPhim.norm("Già Thiên")]
        let live = movie("Già Thiên", type: "series", slug: "gia-thien-live")
        let anim = movie("Già Thiên", type: "hoathinh", slug: "gia-thien")
        XCTAssertEqual(KkPhim.choose([live, anim], mine).map { J.str($0, "slug") }, "gia-thien")
        XCTAssertEqual(KkPhim.choose([live], mine).map { J.str($0, "slug") }, "gia-thien-live")
        XCTAssertNil(KkPhim.choose([movie("Tiên Nghịch")], mine))
    }

    func testReadsSlugFromFilmUrl() {
        XCTAssertEqual(KkPhim.slugOf("https://yanhh3d.ee/tu-tien/tien-nghich"), "tien-nghich")
        XCTAssertEqual(KkPhim.slugOf("https://hoathinh3d.ee/dau-pha-thuong-khung-phan-5/"), "dau-pha-thuong-khung-phan-5")
        XCTAssertNil(KkPhim.slugOf("https://example.com/"))
    }

    private lazy var detail = json(#"""
    {"status":true,"movie":{"name":"Tiên Nghịch","slug":"tien-nghich","type":"hoathinh"},"episodes":[
    {"server_name":"Vietsub #Hà Nội","is_ai":false,"server_data":[
      {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://a.kvp726.com/164/index.m3u8"},
      {"name":"Tập 165","slug":"tap-165","link_m3u8":"https://a.kvp726.com/165/index.m3u8"}]},
    {"server_name":"Thuyết Minh","is_ai":false,"server_data":[
      {"name":"Tập 08","slug":"tap-08","link_m3u8":"https://s6.kkphimplayer6.com/8/index.m3u8"},
      {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://s6.kkphimplayer6.com/164/index.m3u8"}]},
    {"server_name":"Thuyết Minh","is_ai":true,"server_data":[
      {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://ai.example/164/index.m3u8"}]},
    {"server_name":"Lồng Tiếng","is_ai":false,"server_data":[
      {"name":"Tập 164","slug":"tap-164","link_m3u8":"","link_embed":"https://player.phimapi.com/player/?url=x"}]}
    ]}
    """#)

    func testPicksSameEpisodeNumberInEveryLanguage() {
        let all = KkPhim.pick(detail, 164, nil)
        XCTAssertEqual(all.map { $0.0.type }, ["kk-vs", "kk-tm", "kk-tm-ai"])
        XCTAssertEqual(all.map { $0.0.label }, ["KKPhim Vietsub", "KKPhim Thuyết Minh", "KKPhim Thuyết Minh (AI)"])
        XCTAssertEqual(all[0].1.url, "https://a.kvp726.com/164/index.m3u8")
        XCTAssertEqual(KkPhim.pick(detail, 8, nil).map { $0.0.type }, ["kk-tm"])
        XCTAssertEqual(KkPhim.pick(detail, 165, nil).map { $0.0.type }, ["kk-vs"])
        XCTAssertTrue(KkPhim.pick(detail, 166, nil).isEmpty)
    }

    func testOnlyLanguageOfTheVersionBeingWatched() {
        XCTAssertEqual(KkPhim.pick(detail, 164, PlayStats.lang("Việt Sub")).map { $0.0.type }, ["kk-vs"])
        XCTAssertEqual(KkPhim.pick(detail, 164, PlayStats.lang("Thuyết Minh")).map { $0.0.type }, ["kk-tm", "kk-tm-ai"])
        XCTAssertEqual(PlayStats.lang("4K Việt Sub"), "vs")
        XCTAssertNil(PlayStats.lang("Tập phim"))
    }

    func testAttachesAfterSiteServersAndPlaysFromLinks() async throws {
        let kk = KkPhim.pick(detail, 164, nil)
        let yan = EpisodeSource(epUrl: "https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html", servers: [Server(label: "VIETSUB 1080 V1", type: "tiktik")],
                                postId: "1010", ep: "tap-164", sv: "1", playerUrl: "https://yanhh3d.ee/player/player.php",
                                directEmbed: nil, pageMode: false)
        let s = KkPhim.attach(yan, kk)
        XCTAssertEqual(s.servers.map { $0.type }, ["tiktik", "kk-vs", "kk-tm", "kk-tm-ai"])
        XCTAssertFalse(s.pageOnly)
        let v = try await Site.video(s, s.servers[2])
        XCTAssertEqual(v.url, "https://s6.kkphimplayer6.com/164/index.m3u8")
        XCTAssertEqual(KkPhim.attach(yan, []).servers.map { $0.type }, ["tiktik"])
        // Trang mở thẳng trang tập (hoathinh3d.pw): trình phát của trang thành server đầu tiên
        let pw = EpisodeSource(epUrl: "https://hoathinh3d.pw/xem-phim-gia-thien/tap-2-sv1.html", servers: [], postId: "", ep: "", sv: "",
                               playerUrl: nil, directEmbed: nil, pageMode: true)
        XCTAssertTrue(pw.pageOnly)
        let p = KkPhim.attach(pw, kk)
        XCTAssertEqual(p.servers.first?.type, EpisodeSource.pageServer)
        XCTAssertFalse(p.pageOnly)
        XCTAssertTrue(p.pageMode)
    }

    func testSiteDownStillPlaysFromKkAndSlowKkIsSkipped() async throws {
        let ep = "https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html"
        let kk = KkPhim.pick(detail, 164, nil)
        let site = EpisodeSource(epUrl: ep, servers: [Server(label: "VIETSUB 1080 V1", type: "tiktik")], postId: "1", ep: "tap-164",
                                 sv: "1", playerUrl: "https://yanhh3d.ee/player/player.php", directEmbed: nil, pageMode: false)
        // Trang lỗi → chỉ còn server KKPhim, phát được
        let only = try await KkPhim.combine(ep, site: { throw AppError("Trang trả về lỗi HTTP 522") }, kk: { kk })
        XCTAssertEqual(only.servers.map { $0.type }, ["kk-vs", "kk-tm", "kk-tm-ai"])
        let v = try await Site.video(only, only.servers[0])
        XCTAssertEqual(v.url, "https://a.kvp726.com/164/index.m3u8")
        // Trang lỗi và KKPhim cũng không có → báo lỗi của trang
        do {
            _ = try await KkPhim.combine(ep, site: { throw AppError("HTTP 522") }, kk: { [] })
            XCTFail("phải báo lỗi")
        } catch {
            XCTAssertEqual(friendly(error), "HTTP 522")
        }
        // KKPhim lỗi / quá chậm → phát bằng server của trang như cũ, không chờ mãi
        let failed = try await KkPhim.combine(ep, site: { site }, kk: { throw AppError("KK lỗi") })
        XCTAssertEqual(failed.servers.map { $0.type }, ["tiktik"])
        let t0 = nowMs()
        let slow = try await KkPhim.combine(ep, site: { site }, kk: {
            try await Task.sleep(nanoseconds: 3_000_000_000)
            return kk
        }, waitMs: 300)
        XCTAssertEqual(slow.servers.map { $0.type }, ["tiktik"])
        XCTAssertLessThan(nowMs() - t0, 2_000)
        let both = try await KkPhim.combine(ep, site: { site }, kk: { kk })
        XCTAssertEqual(both.servers.count, 4)
    }

    func testSuggestsKkWhenEveryMirrorStalls() {
        let v1 = Server(label: "VIETSUB 1080 V1", type: "tiktik")
        let v2 = Server(label: "VIETSUB 1080 V2", type: "pro")
        let tm = Server(label: "Thuyết minh 1080 V1", type: "tiktm")
        let kk = KkPhim.pick(detail, 164, nil).map { $0.0 }
        let bad = PlayStats.Score(level: 2, startSec: 20, stallPct: 0.12)
        let unknown = PlayStats.Score(level: -1, startSec: nil, stallPct: 0)
        let all = [v1, v2, tm] + kk
        XCTAssertEqual(PlayStats.alternative(all, v2) { $0 == v1 ? bad : unknown }?.0.type, "kk-vs")
        XCTAssertEqual(PlayStats.alternative(all, v1) { _ in unknown }?.0, v2)
    }

    /// Trang KKPhim (1.6): địa chỉ trong app theo www.kkphim.com, dữ liệu gọi thẳng phimapi.com.
    func testKkPhimSiteUsesPublicApi() {
        let kk = Prefs.kkSite
        XCTAssertTrue(ApiSite.isKnown("\(kk)/"))
        XCTAssertTrue(ApiSite.isKk("\(kk)/phim/tien-nghich/tap-165?sv=0"))
        XCTAssertTrue(ApiSite.isKk("https://kkphim.com"))
        XCTAssertFalse(ApiSite.isKk("https://mephim.ddns.net/phim/tien-nghich"))
        XCTAssertEqual(ApiSite.fallback(ApiSite.newestUrl(kk)), "https://phimapi.com/v1/api/danh-sach/phim-moi-cap-nhat?page=1&limit=24")
        XCTAssertEqual(ApiSite.fallback(ApiSite.searchUrl(kk, "già thiên")),
                       "https://phimapi.com/v1/api/tim-kiem?keyword=gi%C3%A0+thi%C3%AAn&page=1&limit=24")
        XCTAssertEqual(ApiSite.fallback("\(kk)/api/movies/category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh&page=1&limit=24"),
                       "https://phimapi.com/v1/api/danh-sach/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh&page=1&limit=24")
        XCTAssertEqual(try ApiSite.filmUrl("\(kk)/phim/tien-nghich/tap-165?sv=0"), "\(kk)/phim/tien-nghich")
        let body = #"""
        {"status":true,"data":{"items":[{"name":"Đấu Phá Thương Khung (Phần 5)","slug":"dau-pha-thuong-khung-phan-5",
        "poster_url":"upload/vod/20240822-1/58a4.jpg","episode_current":"Tập 218"}],
        "params":{"pagination":{"totalItems":462,"totalItemsPerPage":24,"currentPage":1,"totalPages":20}},
        "APP_DOMAIN_CDN_IMAGE":"https://phimimg.com"}}
        """#
        let p = ApiSite.parseList(json(body), "\(kk)/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24")
        XCTAssertEqual(p.films.map { $0.url }, ["https://www.kkphim.com/phim/dau-pha-thuong-khung-phan-5"])
        XCTAssertEqual(p.films.first?.poster, "https://phimimg.com/upload/vod/20240822-1/58a4.jpg")
        XCTAssertEqual(p.nextUrl, "\(kk)/api/movies/category/hoat-hinh?country=trung-quoc&page=2&limit=24")
    }

    func testBareKkDomainBecomesTheWwwSite() {
        // kkphim.com trần bị nhà mạng chặn, và trùng với trang mẫu www.kkphim.com trong danh sách
        XCTAssertEqual(Prefs.normalizeUrl("kkphim.com"), Prefs.kkSite)
        XCTAssertEqual(Prefs.normalizeUrl("https://KKPhim.com/"), Prefs.kkSite)
        XCTAssertEqual(Prefs.normalizeUrl("www.kkphim.com"), Prefs.kkSite)
        XCTAssertEqual(Prefs.normalizeUrl("yanhh3d.ee/"), "https://yanhh3d.ee")
    }
}
