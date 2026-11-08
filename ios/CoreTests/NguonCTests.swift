import XCTest

/// NguonC + VSMOV + CLB Phim Xưa (k-20) (11/2026) — cùng ca với NguonCTest.kt / StremioTest.kt / ApiSiteTest.kt.
final class NguonCTests: XCTestCase {
    private let o = "https://phim.nguonc.com"
    private let k = "https://sc.k-20.xyz"

    private func json(_ s: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(s.utf8)) as? [String: Any]) ?? [:]
    }

    private lazy var detail = json(#"""
    {"status":"success","movie":{"name":"Mục Thần Ký","original_name":"Mu Shen Ji, Tales of Herding Gods",
    "slug":"muc-than-ky","description":"<p>Tần Mục lớn lên ở <b>Tàn Lão Thôn</b>.</p>","total_episodes":120,
    "current_episode":"Tập 108","time":"21 phút/tập","quality":"HD","language":"Vietsub","year":2023,
    "thumb_url":"https://img.nguonc.com/images/a.jpg","poster_url":"https://img.nguonc.com/images/b.jpg",
    "category":{"1":{"group":{"name":"Định dạng"},"list":[{"name":"Phim bộ"}]},
                "2":{"group":{"name":"Thể loại"},"list":[{"name":"Hành động"},{"name":"Hoạt hình"}]},
                "4":{"group":{"name":"Quốc gia"},"list":[{"name":"Trung Quốc"}]}},
    "episodes":[
      {"server_name":"Vietsub #1","items":[
        {"name":"1","slug":"tap-1","embed":"https://embed1.streamc.xyz/embed.php?hash=a1"},
        {"name":"107","slug":"tap-107","embed":"https://embed14.streamc.xyz/embed.php?hash=a107"},
        {"name":"108","slug":"tap-108","embed":"https://embed10.streamc.xyz/embed.php?hash=a108"},
        {"name":"109","slug":"tap-109","embed":""}]},
      {"server_name":"Thuyết Minh #1","items":[
        {"name":"108","slug":"tap-108","embed":"https://embed10.streamc.xyz/embed.php?hash=t108"}]}]}}
    """#)

    func testParsesListWithPortraitPostersAndPaging() {
        let root = json(#"""
        {"status":"success","paginate":{"current_page":2,"total_page":3365},"items":[
         {"name":"Mục Thần Ký","slug":"muc-than-ky","current_episode":"Tập 108",
          "thumb_url":"https://img.nguonc.com/images/thumb.jpg","poster_url":"https://img.nguonc.com/images/wide.jpg"},
         {"name":"Doraemon","slug":"doraemon","current_episode":"Hoàn tất (26/26)","poster_url":"https://img.nguonc.com/images/d.jpg"},
         {"name":"","slug":"rong"}]}
        """#)
        let url = "\(o)/api/films/danh-sach/hoat-hinh?page=2"
        let p = NguonC.parseList(root, url)
        XCTAssertEqual(p.films.map { $0.title }, ["Mục Thần Ký", "Doraemon"])
        XCTAssertEqual(p.films[0].url, "\(o)/phim/muc-than-ky")
        XCTAssertEqual(p.films[0].poster, "https://img.nguonc.com/images/thumb.jpg")
        XCTAssertEqual(p.films[1].poster, "https://img.nguonc.com/images/d.jpg")
        XCTAssertEqual(p.films.map { $0.badge }, ["Tập 108", "Tập 26"])
        XCTAssertEqual(p.nextUrl, "\(o)/api/films/danh-sach/hoat-hinh?page=3")
    }

    func testParsesDetailGroupsAndInfo() throws {
        let d = try NguonC.parseDetail(detail, "\(o)/phim/muc-than-ky")
        XCTAssertEqual(d.title, "Mục Thần Ký")
        XCTAssertEqual(d.otherTitle, "Mu Shen Ji, Tales of Herding Gods")
        XCTAssertEqual(d.description, "Tần Mục lớn lên ở Tàn Lão Thôn.")
        XCTAssertEqual(d.info, "Tập 108  •  21 phút/tập  •  Vietsub  •  Trung Quốc  •  2023  •  Hành động, Hoạt hình")
        XCTAssertEqual(d.groups.map { $0.name }, ["Vietsub #1", "Thuyết Minh #1"])
        XCTAssertEqual(d.groups[0].episodes.map { $0.title }, ["Tập 1", "Tập 107", "Tập 108"])
        XCTAssertEqual(d.groups[0].episodes[2].url, "\(o)/phim/muc-than-ky/tap-108?sv=0")
        XCTAssertEqual(d.groups[1].episodes[0].url, "\(o)/phim/muc-than-ky/tap-108?sv=1")
        XCTAssertEqual(try NguonC.filmUrl("\(o)/phim/muc-than-ky/tap-108?sv=1"), "\(o)/phim/muc-than-ky")
    }

    func testBackupPicksSameEpisodeInLanguageBeingWatched() async throws {
        let m = J.obj(detail, "movie")!
        let all = NguonC.pick(m, 108, nil)
        XCTAssertEqual(all.map { $0.0.type }, ["nc-vs", "nc-tm"])
        XCTAssertEqual(all.map { $0.0.label }, ["NguonC Vietsub", "NguonC Thuyết Minh"])
        XCTAssertEqual(NguonC.pick(m, 108, "tm").map { $0.0.type }, ["nc-tm"])
        XCTAssertTrue(NguonC.pick(m, 109, nil).isEmpty)
        XCTAssertTrue(KkPhim.matches(m, [KkPhim.norm("Tales of Herding Gods")]))
        // Server NguonC phát từ link có sẵn
        let ep = "https://yanhh3d.ee/tu-tien/muc-than-ky/tap-108.html"
        let site = EpisodeSource(epUrl: ep, servers: [Server(label: "VIETSUB 1080 V1", type: "tiktik")], postId: "1", ep: "tap-108",
                                 sv: "1", playerUrl: "https://yanhh3d.ee/player/player.php", directEmbed: nil, pageMode: false)
        let s = KkPhim.attach(site, all)
        XCTAssertEqual(s.servers.map { $0.type }, ["tiktik", "nc-vs", "nc-tm"])
        let v = try await Site.video(s, s.servers[2])
        XCTAssertEqual(v.url, "https://embed10.streamc.xyz/embed.php?hash=t108")
    }

    func testSlowBackupDoesNotHideTheFastOne() async throws {
        let ep = "https://yanhh3d.ee/tu-tien/muc-than-ky/tap-108.html"
        let site = EpisodeSource(epUrl: ep, servers: [Server(label: "VIETSUB 1080 V1", type: "tiktik")], postId: "1", ep: "tap-108",
                                 sv: "1", playerUrl: "https://yanhh3d.ee/player/player.php", directEmbed: nil, pageMode: false)
        let nc = [(Server(label: "NguonC Vietsub", type: "nc-vs"), Video(url: "https://embed1.streamc.xyz/embed.php?hash=x"))]
        let kk = [(Server(label: "KKPhim Vietsub", type: "kk-vs"), Video(url: "https://a.kvp726.com/108/index.m3u8"))]
        let t0 = nowMs()
        let s = try await KkPhim.combineAll(ep, site: { site }, backups: [
            { try await Task.sleep(nanoseconds: 3_000_000_000); return kk }, { nc },
        ], waitMs: 400)
        XCTAssertEqual(s.servers.map { $0.type }, ["tiktik", "nc-vs"])
        XCTAssertLessThan(nowMs() - t0, 2_000)
        let both = try await KkPhim.combineAll(ep, site: { site }, backups: [{ kk }, { nc }])
        XCTAssertEqual(both.servers.map { $0.type }, ["tiktik", "kk-vs", "nc-vs"])
        let down = try await KkPhim.combineAll(ep, site: { throw AppError("HTTP 522") }, backups: [{ throw AppError("KK lỗi") }, { nc }])
        XCTAssertEqual(down.servers.map { $0.type }, ["nc-vs"])
    }

    // MARK: - VSMOV

    func testVsmovListsAndUrls() throws {
        let vs = "https://vsmov.com"
        XCTAssertTrue(ApiSite.isKnown("\(vs)/") && ApiSite.isVs("\(vs)/phim/tien-nghich"))
        XCTAssertEqual(ApiSite.newestUrl(vs), "\(vs)/api/danh-sach/phim-moi-cap-nhat?page=1")
        XCTAssertEqual(ApiSite.filters(vs).genres.first { $0.title == "Kiếm Hiệp" }?.url, "\(vs)/api/the-loai/kiem-hiep?page=1")
        let root = json(#"""
        {"status":true,"items":[{"name":"Túy Linh Lung","slug":"tuy-linh-lung","poster_url":"https://vsmov.com/storage/images/e6.jpg"}],
         "pagination":{"totalItems":2804,"totalItemsPerPage":20,"currentPage":1,"totalPages":141}}
        """#)
        let p = ApiSite.parseList(root, "\(vs)/api/the-loai/hoat-hinh?country=trung-quoc&page=1")
        XCTAssertEqual(p.films.first?.url, "\(vs)/phim/tuy-linh-lung")
        XCTAssertEqual(p.nextUrl, "\(vs)/api/the-loai/hoat-hinh?country=trung-quoc&page=2")
        let d = try ApiSite.parseDetail(json(#"""
        {"status":true,"movie":{"name":"Túy Linh Lung"},"episodes":[{"server_name":"Lồng tiếng #1","server_data":[
         {"name":"1","slug":"tap-1","link_embed":"https://v15.streamvsmov.com/video/fc1d03d9"}]}]}
        """#), "\(vs)/phim/tuy-linh-lung")
        XCTAssertEqual(d.groups.first?.episodes.first?.url, "\(vs)/phim/tuy-linh-lung/tap-1?sv=0")
        XCTAssertEqual(d.groups.first?.episodes.first?.title, "Tập 1")
    }

    func testVsmovHlsFromPlayerPage() {
        let signed = #"enableSignedUrl: true, signedMasterUrl: "https://v2.streamvsmov.com/stream/55010e78/master.m3u8?expires=1794131570&signature=ca8c", volume: 100 }; const baseUrl = "https://v2.streamvsmov.com"; const videoHash = "55010e78";"#
        XCTAssertEqual(ApiSite.vsHlsFrom(signed), "https://v2.streamvsmov.com/stream/55010e78/master.m3u8?expires=1794131570&signature=ca8c")
        let plain = #"enableSignedUrl: false, signedMasterUrl: "", const baseUrl = "https://v15.streamvsmov.com"; const videoHash = "fc1d03d9-0667";"#
        XCTAssertEqual(ApiSite.vsHlsFrom(plain), "https://v15.streamvsmov.com/stream/fc1d03d9-0667/master.m3u8")
        XCTAssertNil(ApiSite.vsHlsFrom("<html>lỗi</html>"))
    }

    // MARK: - CLB Phim Xưa (k-20)

    func testStremioCatalogUrlsAndPaging() {
        XCTAssertEqual(Stremio.catalog(k), "\(k)/catalog/movie/clbpx-movie.json")
        XCTAssertEqual(Stremio.catalog(k, genre: "Kiếm Hiệp", skip: 30),
                       "\(k)/catalog/movie/clbpx-movie/genre=Th%E1%BB%83%20lo%E1%BA%A1i%3A%20Ki%E1%BA%BFm%20Hi%E1%BB%87p&skip=30.json")
        XCTAssertEqual(Stremio.searchUrl(k, " thiên long "), "\(k)/catalog/movie/clbpx-movie/search=thi%C3%AAn%20long.json")
        let data = Data(#"""
        {"metas":[
          {"id":"clbpx:nch1993","name":"Nguyên Chấn Hiệp 1993 – 20 Tập","poster":"https://clbphimxua.com/a.jpg"},
          {"id":"clbpx:harry-5","name":"Harry Bẩn Thỉu 5: Mồ Chôn Tội Ác 1988","poster":"https://clbphimxua.com/b.jpg"},
          {"id":"nguonc:khac","name":"Không phải CLBPX"}]}
        """#.utf8)
        let url = Stremio.catalog(k, genre: "Kiếm Hiệp", skip: 30)
        let p = Stremio.parseList(data, url)
        XCTAssertEqual(p.films.map { $0.url }, ["\(k)/clbpx/nch1993", "\(k)/clbpx/harry-5"])
        XCTAssertEqual(p.films.map { $0.badge }, ["20 tập", nil])
        XCTAssertEqual(p.nextUrl, Stremio.catalog(k, genre: "Kiếm Hiệp", skip: 32))
        XCTAssertEqual(Stremio.parseList(data, Stremio.catalog(k)).nextUrl, Stremio.catalog(k, skip: 2))
        XCTAssertNil(Stremio.parseList(data, Stremio.searchUrl(k, "x")).nextUrl)
    }

    func testStremioDetailAndMp4Servers() async throws {
        let series = Data(#"""
        {"meta":{"id":"clbpx:nch1993","name":"Nguyên Chấn Hiệp 1993 – 20 Tập","poster":"https://clbphimxua.com/p.jpg","releaseInfo":"1993",
         "videos":[{"id":"clbpx:nch1993:HFvsf0tTx","title":"Tập 02","season":1,"episode":2},
                   {"id":"clbpx:nch1993:LM4bPc-3a","title":"Tập 01","season":1,"episode":1}]}}
        """#.utf8)
        let d = try Stremio.parseDetail(series, "\(k)/clbpx/nch1993")
        XCTAssertEqual(d.info, "1993")
        XCTAssertEqual(d.groups.first?.episodes.map { $0.title }, ["Tập 1", "Tập 2"])
        XCTAssertEqual(d.groups.first?.episodes.first?.url, "\(k)/clbpx/nch1993/LM4bPc-3a")
        XCTAssertEqual(try Stremio.filmUrl("\(k)/clbpx/nch1993/LM4bPc-3a"), "\(k)/clbpx/nch1993")
        let movie = try Stremio.parseDetail(Data(#"{"meta":{"id":"clbpx:harry-5","name":"Harry 5"}}"#.utf8), "\(k)/clbpx/harry-5")
        XCTAssertEqual(movie.groups.first?.episodes.map { $0.url }, ["\(k)/clbpx/harry-5/full"])
        let streams = Data(#"""
        {"streams":[
          {"title":"1080p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=5&size=3069863340"},
          {"title":"1080p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=5&size=1938835536"},
          {"title":"360p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=2&size=395465856"},
          {"title":"720p","url":""}]}
        """#.utf8)
        let s = try Stremio.parseSource(streams, "\(k)/clbpx/harry-5/full")
        XCTAssertEqual(s.servers.map { $0.label }, ["1080p · 3,1 GB", "1080p · 1,9 GB", "360p"])
        let v = try await Site.video(s, s.servers[2])
        XCTAssertEqual(v.url, "https://sc.k-20.xyz/hx-mp4?embed=x&res=2&size=395465856")
        XCTAssertTrue(v.file)
    }

    func testShortSiteNames() {
        XCTAssertEqual(Site.shortName("phim.nguonc.com"), "nguonc")
        XCTAssertEqual(Site.shortName("yanhh3d.ee"), "yanhh3d")
        XCTAssertEqual(Site.shortName("sc.k-20.xyz"), "CLB Phim Xưa")
        XCTAssertEqual(Site.displayName("sc.k-20.xyz"), "CLB Phim Xưa (sc.k-20.xyz)")
    }
}
