import XCTest

/// Trang bật chống bot của Cloudflare (WebFetch.swift) — cùng ca với web_fetch.rs (lõi Rust) và WebFetchTest.kt của bản TV.
final class WebFetchTests: XCTestCase {
    override func tearDown() {
        WebFetch.forgetAll()
        super.tearDown()
    }

    func testChallengeDetection() {
        let page = Data("<!DOCTYPE html><html><head><title>Just a moment...</title><script>window._cf_chl_opt={}</script>".utf8)
        XCTAssertTrue(WebFetch.isChallenge(code: 403, cfMitigated: "challenge", body: nil))
        XCTAssertTrue(WebFetch.isChallenge(code: 200, cfMitigated: " Challenge ", body: nil))
        XCTAssertTrue(WebFetch.isChallenge(code: 403, cfMitigated: nil, body: page))
        XCTAssertTrue(WebFetch.isChallenge(code: 503, cfMitigated: nil, body: Data("x /cdn-cgi/challenge-platform/h/b/orchestrate".utf8)))
        // Lỗi thật của trang / trang chặn hẳn («you have been blocked») không phải lớp chặn tự qua được
        XCTAssertFalse(WebFetch.isChallenge(code: 404, cfMitigated: nil, body: page))
        XCTAssertFalse(WebFetch.isChallenge(code: 403, cfMitigated: nil, body: Data("<title>403 Forbidden</title>".utf8)))
        XCTAssertFalse(WebFetch.isChallenge(code: 403, cfMitigated: nil,
                                            body: Data("<title>Attention Required! | Cloudflare</title>Sorry, you have been blocked".utf8)))
        XCTAssertFalse(WebFetch.isChallenge(code: 200, cfMitigated: nil, body: page))
    }

    func testRememberedHostsExpireAfter12Hours() {
        let t0 = Date(timeIntervalSince1970: 1_800_000_000)
        XCTAssertFalse(WebFetch.needs("https://hoathinh3d.pw/phim/a", now: t0))
        WebFetch.remember("HoatHinh3D.pw", now: t0)
        XCTAssertTrue(WebFetch.needs("https://hoathinh3d.pw/phim/a", now: t0.addingTimeInterval(60)))
        XCTAssertTrue(WebFetch.needs("https://HOATHINH3D.PW/wp-content/x.jpg", now: t0.addingTimeInterval(11 * 3600)))
        XCTAssertFalse(WebFetch.needs("https://hoathinh3d.pw/", now: t0.addingTimeInterval(12 * 3600 + 1)))
        XCTAssertFalse(WebFetch.needs("https://img.hoathinh3d.pw/x.jpg", now: t0.addingTimeInterval(60)))
        XCTAssertEqual(WebFetch.rememberedHosts, ["hoathinh3d.pw"])
        WebFetch.forgetAll()
        XCTAssertFalse(WebFetch.needs("https://hoathinh3d.pw/", now: t0.addingTimeInterval(60)))
    }

    func testFetchWithoutProviderReportsChallenge() async {
        let old = WebFetch.provider
        WebFetch.provider = nil
        defer { WebFetch.provider = old }
        do {
            _ = try await WebFetch.fetch(.init(url: "https://hoathinh3d.pw/", method: "GET", body: nil, headers: [:]))
            XCTFail("phải báo lỗi chống bot")
        } catch let e as WebFetch.ChallengeError {
            XCTAssertEqual(e.host, "hoathinh3d.pw")
        } catch {
            XCTFail("sai loại lỗi: \(error)")
        }
    }

    func testProviderReplyStillChallengedOrHttpError() async {
        let old = WebFetch.provider
        defer { WebFetch.provider = old }
        let r = WebFetch.Request(url: "https://hoathinh3d.pw/x", method: "GET", body: nil, headers: [:])
        WebFetch.provider = { r in Http.Response(url: r.url, code: 200, data: Data("ok".utf8), contentType: "text/plain") }
        let ok = try? await WebFetch.fetch(r)
        XCTAssertEqual(ok?.body, "ok")
        WebFetch.provider = { r in Http.Response(url: r.url, code: 403, data: Data("<title>Just a moment...</title>".utf8), contentType: nil) }
        do { _ = try await WebFetch.fetch(r); XCTFail() } catch { XCTAssertTrue(error is WebFetch.ChallengeError) }
        WebFetch.provider = { r in Http.Response(url: r.url, code: 404, data: Data(), contentType: nil) }
        do { _ = try await WebFetch.fetch(r); XCTFail() } catch { XCTAssertEqual((error as? Http.HttpError)?.code, 404) }
    }

    /// Ghi chú phát hành lỡ lưu kèm BOM vẫn lấy đúng mục «Có gì mới».
    func testUpdateSummaryIgnoresBom() {
        let notes = "\u{FEFF}## Có gì mới\n- Đọc được hoathinh3d.pw\n\n## Kỹ thuật\n- SHA-256: abc"
        XCTAssertEqual(UpdateCheck.summary(notes), "Đọc được hoathinh3d.pw")
    }
}
