import XCTest
import SwiftSoup

/// Đọc trang Next.js kiểu PhimMoi (phimmoir.fun) từ dữ liệu self.__next_f.push dựng tay (chuyển từ NextSiteTest.kt).
final class NextSiteTests: XCTestCase {
    let base = "https://phim.test"

    func quote(_ s: String) -> String {
        let d = try! JSONSerialization.data(withJSONObject: [s])
        return String(String(decoding: d, as: UTF8.self).dropFirst().dropLast())
    }

    /// Trang có dữ liệu cắt thành nhiều đoạn push (trang thật cắt ngay giữa đối tượng JSON).
    func page(_ url: String, _ data: String, parts: Int = 3) -> Document {
        let chars = Array(data)
        let size = (chars.count + parts - 1) / parts
        var scripts = ""
        var i = 0
        while i < chars.count {
            let chunk = String(chars[i..<min(chars.count, i + size)])
            scripts += "<script>self.__next_f.push([1,\(quote(chunk))])</script>"
            i += size
        }
        return try! SwiftSoup.parse("<html><head><script>self.__next_f=self.__next_f||[]</script></head><body>\(scripts)</body></html>", url)
    }

    func movie(_ id: Int, _ slug: String, _ name: String, _ status: String) -> String {
        #"{"id":\#(id),"name":"\#(name)","origin_name":"","slug":"\#(slug)","description":"$27","poster_url":"/public/images/posters/\#(slug).webp","# +
            #""display_status":"\#(status)","country":{"name":"Trung Quốc","slug":"trung-quoc"},"genres":[{"id":11,"name":"Hoạt Hình","slug":"hoat-hinh","is_active":true}]}"#
    }

    func eps(_ n: Int, _ lang: String) -> String {
        "[" + (1...n).map { #"{"id":\#($0),"episode_label":"Tập \#($0)","slug":"tap-\#($0)","is_embed":false,"language":"\#(lang)","name":"Tập \#($0)"}"# }
            .joined(separator: ",") + "]"
    }

    let sources = #"[{"server_name":"KK Phim","server_slug":"kk","language_name":"Vietsub","language_slug":"vietsub","total_episodes":3,"priority_order":2},"# +
        #"{"server_name":"NguonC","server_slug":"nc","language_name":"Vietsub","language_slug":"vietsub","total_episodes":3,"priority_order":3},"# +
        #"{"server_name":"KK Phim","server_slug":"kk","language_name":"Thuyết Minh","language_slug":"thuyetminh","total_episodes":2,"priority_order":2}]"#

    func testFilters() {
        func item(_ id: Int, _ name: String, _ slug: String, _ active: Bool = true) -> String {
            #"{"id":\#(id),"name":"\#(name)","slug":"\#(slug)","is_active":\#(active)}"#
        }
        let data = #"1:{"results":["# + movie(1, "a", "A", "Tập 1") + #"],"genres":["# +
            [item(1, "Âm Nhạc", "am-nhac"), item(2, "Cổ Trang", "co-trang"), item(3, "Ẩn", "an", false)].joined(separator: ",") +
            #"],"countries":["# +
            [item(54, "1983", "1983"), item(10, "Anh", "anh"), item(60, "KW", "kw"), item(61, "Hàn Quốc", "han-quoc"), item(62, "hàn quốc", "han-quoc-2")]
                .joined(separator: ",") + "]}\n"
        let doc = page("\(base)/loc-phim", data)
        let f = NextSite.parseFilters(doc, NextSite.data(doc)!)
        XCTAssertEqual(f.genres.map { $0.title }, ["Âm Nhạc", "Cổ Trang"])
        XCTAssertEqual(f.genres[1].url, "\(base)/loc-phim?genre=co-trang&sort=updated_at&order=desc")
        XCTAssertEqual(f.countries.map { $0.title }, ["Anh", "Hàn Quốc"])
        XCTAssertEqual(f.countries[1].url, "\(base)/loc-phim?country=han-quoc&sort=updated_at&order=desc")
    }

    func testNotNextJs() throws {
        let doc = try SwiftSoup.parse("<html><body><script>var a=1</script><div class=flw-item></div></body></html>", "\(base)/")
        XCTAssertNil(NextSite.data(doc))
    }

    func testHome() throws {
        let data = #"0:["$","div",null,{"children":["# + movie(1, "slide-a", "Phim Nổi Bật", "Vietsub 4") + "]}]\n" +
            #"5:["$","$L26","Trung Quốc Hot Nhất",{"movies":["# + movie(2, "tien-nghich", "Tiên Nghịch", "Thuyết Minh 107, Vietsub 165") + "," +
            movie(3, "phim-le", "Phim Lẻ", "Vietsub Full") + "]}]"
        let doc = page("\(base)/", data)
        try doc.body()?.append(#"<a href="/the-loai/hanh-dong">Hành Động</a>"#)
        let h = Site.parseHome(doc)
        XCTAssertEqual(h.sections.map { $0.title }, ["Nổi bật", "Trung Quốc Hot Nhất"])
        let f = h.sections[1].films
        XCTAssertEqual(f.map { $0.title }, ["Tiên Nghịch", "Phim Lẻ"])
        XCTAssertEqual(f[0].url, "\(base)/phim/tien-nghich")
        XCTAssertEqual(f[0].poster, "\(base)/public/images/posters/tien-nghich.webp")
        XCTAssertEqual(f.map { $0.badge }, ["Tập 165", "Full"])
        XCTAssertEqual(h.categories.first?.title, "Hoạt hình Trung Quốc")
        XCTAssertEqual(h.categories.last?.url, "\(base)/the-loai/hanh-dong")
    }

    func testListPaging() {
        let movies = (1...21).map { movie($0, "phim-\($0)", "Phim \($0)", "Vietsub \($0)") }.joined(separator: ",")
        let url = "\(base)/loc-phim?genre=hoat-hinh&sort=updated_at&order=desc"
        let p = Site.parseList(page(url, #"{"movies":["# + movies + #"],"total":50,"page":1,"limit":21,"filters":{}}"#), url)
        XCTAssertEqual(p.films.count, 21)
        XCTAssertEqual(p.nextUrl, "\(url)&page=2")
        let last = Site.parseList(page("\(url)&page=3", #"{"movies":["# + movie(1, "a", "A", "") + #"],"total":43,"page":3,"limit":21}"#), "\(url)&page=3")
        XCTAssertNil(last.nextUrl)
    }

    func testDetail() {
        let data = #"{"movie":{"id":283,"name":"Tiên Nghịch","origin_name":"Renegade Immortal","slug":"tien-nghich","# +
            #""description":"Cuộc hành trình của Vương Lâm, một thiếu niên bình thường.","poster_url":"/public/images/posters/p.webp","# +
            #""status_text":"Thuyết Minh 2, Vietsub 3","year":2023,"country":{"name":"Trung Quốc"},"genres":[{"name":"Hoạt Hình"}]},"# +
            #""episode":{"episodes":"# + eps(3, "vietsub") + #","available_languages":[],"available_sources":"# + sources +
            #","current_language":"vietsub","current_server":"kk"}}"#
        let d = Site.parseDetail(page("\(base)/phim/tien-nghich", data), "\(base)/phim/tien-nghich")
        XCTAssertEqual(d.title, "Tiên Nghịch")
        XCTAssertEqual(d.otherTitle, "Renegade Immortal")
        XCTAssertTrue(d.info?.hasPrefix("Thuyết Minh 2, Vietsub 3  •  Trung Quốc  •  2023") == true, d.info ?? "nil")
        XCTAssertEqual(d.groups.count, 1)
        XCTAssertEqual(d.groups[0].name, "Vietsub")
        XCTAssertEqual(d.groups[0].episodes.map { $0.title }, ["Tập 1", "Tập 2", "Tập 3"])
        XCTAssertEqual(d.groups[0].episodes.last?.url, "\(base)/xem-phim/tien-nghich/tap-3/vietsub")
        XCTAssertEqual(d.poster, "\(base)/public/images/posters/p.webp")
    }

    func testSource() {
        let epUrl = "\(base)/xem-phim/tien-nghich/tap-3/vietsub"
        let data = #"{"episode":{"episodes":"# + eps(3, "vietsub") + #","current_language":"vietsub","available_sources":"# + sources + "," +
            #""episode_sources":[{"server":{"id":3,"name":"KK Phim","slug":"kk"},"language":{"name":"Vietsub","slug":"vietsub"},"is_lock":false},"# +
            #"{"server":{"id":2,"name":"NguonC","slug":"nc"},"language":{"name":"Vietsub","slug":"vietsub"},"is_lock":false},"# +
            #"{"server":{"id":3,"name":"KK Phim","slug":"kk"},"language":{"name":"Thuyết Minh","slug":"thuyetminh"},"is_lock":false}],"# +
            #""current_server":{"id":3,"name":"KK Phim","slug":"kk"},"current_source":{"server":"$1f:props:data:episode:current_server"},"# +
            #""video":{"id":1,"link_embed":"https://player.test/?url=x","link_m3u8":"https://cdn.test/a/index.m3u8","is_embed":false}}}"#
        let s = Site.parseSource(page(epUrl, data), epUrl)
        XCTAssertTrue(s.next)
        XCTAssertFalse(s.pageMode)
        XCTAssertEqual(s.servers.map { "\($0.type)=\($0.label)" }, ["kk=KK Phim", "nc=NguonC"])
        XCTAssertEqual(s.links.keys.sorted(), ["kk"])
        XCTAssertEqual(s.links["kk"]?.url, "https://cdn.test/a/index.m3u8")
        XCTAssertNil(s.links["kk"]?.subtitle)
        XCTAssertEqual(s.ep, "tap-3")
        let embed = Site.parseSource(page(epUrl, data.replacingOccurrences(of: "https://cdn.test/a/index.m3u8", with: "")), epUrl)
        XCTAssertEqual(embed.links["kk"]?.url, "https://player.test/?url=x")
    }

    func testOwnServerWithSubtitles() {
        let epUrl = "\(base)/xem-phim/bac-si-x/tap-1/song-ngu"
        let data = #"{"episode":{"episodes":"# + eps(1, "song-ngu") + #","current_language":"song-ngu","# +
            #""episode_sources":[{"server":{"id":9,"name":"HD","slug":"hd"},"language":{"name":"Song Ngữ","slug":"song-ngu"},"is_lock":false}],"# +
            #""current_server":{"id":9,"name":"HD","slug":"hd"},"video":{"id":1,"link_embed":"/embed/v2/abc","link_m3u8":"/public/m3u8/job_1.m3u8","# +
            #""is_embed":false,"subtitles":[{"url":"https://sub.test/zh","lang":"chi","label":"Chinese","isDefault":false},"# +
            #"{"url":"https://sub.test/vi","lang":"vie","label":"Vietnamese","isDefault":true}]}}}"#
        let s = Site.parseSource(page(epUrl, data), epUrl)
        XCTAssertEqual(s.links["hd"]?.url, "\(base)/public/m3u8/job_1.m3u8")
        XCTAssertEqual(s.links["hd"]?.subtitle, "https://sub.test/vi")
    }

    func testOddPosterDoesNotBreakList() {
        let odd = movie(1, "phim-a", "Phim A", "Vietsub 3").replacingOccurrences(of: "/public/images/posters/phim-a.webp", with: "data:image/png;base64,AAAA")
        let url = "\(base)/loc-phim?type=phim-bo"
        let p = Site.parseList(page(url, #"{"movies":["# + odd + "," + movie(2, "phim-b", "Phim B", "Full") + #"],"total":2,"page":1,"limit":21}"#), url)
        XCTAssertEqual(p.films.map { $0.title }, ["Phim A", "Phim B"])
        XCTAssertNil(p.films[0].poster)
        XCTAssertEqual(p.films[1].poster, "\(base)/public/images/posters/phim-b.webp")
    }

    func testBadge() {
        XCTAssertEqual(NextSite.badge("Song Ngữ 4, Thuyết Minh 8, Vietsub 8"), "Tập 8")
        XCTAssertEqual(NextSite.badge("Thuyết Minh Full, Vietsub Full"), "Full")
        XCTAssertNil(NextSite.badge(""))
    }
}
