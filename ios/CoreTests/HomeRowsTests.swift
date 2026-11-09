import XCTest

/// Các hàng trang chủ người xem bật / tắt, sắp thứ tự, tự thêm (bản Android: HomeRowsTest.kt).
final class HomeRowsTests: XCTestCase {
    private let defaults = ["new-eps", "continue", "similar", "interest", "dominant", "newest", "movies"]

    private func ids(_ l: [(id: String, on: Bool)]) -> [String] { l.map { $0.id } }

    func testEmptyMeansDefaultAllOn() {
        let m = HomeFeed.mergeRows([], defaults)
        XCTAssertEqual(ids(m), defaults)
        XCTAssertTrue(m.allSatisfy { $0.on })
    }

    func testKeepsSavedOrderAndOff() {
        let m = HomeFeed.mergeRows(["new-eps", "continue", "newest", "-similar", "interest", "dominant", "movies"], defaults)
        XCTAssertEqual(ids(m), ["new-eps", "continue", "newest", "similar", "interest", "dominant", "movies"])
        XCTAssertEqual(m.first { $0.id == "similar" }?.on, false)
    }

    func testDropsUnknownAndDuplicates() {
        let m = HomeFeed.mergeRows(["bogus", "movies", "-movies", "similar"], defaults)
        XCTAssertEqual(m.filter { $0.id == "movies" }.count, 1)
        XCTAssertEqual(m.first { $0.id == "movies" }?.on, true)
        XCTAssertEqual(m.count, defaults.count)
    }

    /// Hàng mới ở bản sau (chưa có trong thứ tự đã lưu) chèn ngay sau hàng đứng trước nó trong thứ tự mặc định.
    func testNewRowsInsertedAfterTheirDefaultPredecessor() {
        let m = HomeFeed.mergeRows(["movies", "newest", "similar", "new-eps", "continue"], defaults)
        XCTAssertEqual(ids(m), ["movies", "newest", "similar", "interest", "dominant", "new-eps", "continue"])
    }

    func testCustomRowsDefaultAfterHead() {
        let custom = [CustomRow(id: "a", type: "hoathinh"), CustomRow(id: "t-kr", topic: "kr")]
        let d = HomeFeed.rowDefaults(custom)
        XCTAssertEqual(Array(d.prefix(5)), ["new-eps", "continue", "custom-a", "custom-t-kr", "similar"])
        let m = HomeFeed.mergeRows(["new-eps", "continue", "custom-a", "movies", "similar"], d)
        XCTAssertEqual(Array(ids(m).prefix(6)), ["new-eps", "continue", "custom-a", "custom-t-kr", "movies", "similar"])
    }

    func testCustomTitles() {
        XCTAssertEqual(HomeFeed.customTitle(CustomRow(id: "x", type: "hoathinh", country: "trung-quoc", genre: "phieu-luu", lang: "thuyet-minh")),
                       "Hoạt hình Trung Quốc · phiêu lưu · thuyết minh")
        XCTAssertEqual(HomeFeed.customTitle(CustomRow(id: "y", type: "series", country: "han-quoc")), "Phim bộ Hàn Quốc")
        XCTAssertEqual(HomeFeed.customTitle(CustomRow(id: "z", type: "single", genre: "hanh-dong")), "Phim lẻ · hành động")
        XCTAssertEqual(HomeFeed.customTitle(CustomRow(id: "t-kr", topic: "kr")), "Phim Hàn")
    }

    func testCustomRowRoundTrip() {
        let c = CustomRow(id: "c1", type: "series", country: "han-quoc", genre: "tinh-cam", lang: "")
        XCTAssertEqual(CustomRow(c.dict), c)
        XCTAssertNil(CustomRow(["t": "series"]))
        XCTAssertTrue(c.same(CustomRow(id: "khac", type: "series", country: "han-quoc", genre: "tinh-cam")))
    }

    func testArrangeFiltersAndOrders() {
        func row(_ id: String, _ slot: String) -> FeedRow { FeedRow(id: id, title: id, cards: [], more: nil, slot: slot) }
        let rows = [row("topic-cn", "topics"), row("similar", "similar"), row("interest-a", "interest"), row("newest", "newest"),
                    row("movies", "movies")]
        let prefs: [(id: String, on: Bool)] = [("new-eps", true), ("continue", true), ("movies", true), ("similar", false),
                                               ("newest", true), ("interest", true)]
        XCTAssertEqual(HomeFeed.arrange(rows, prefs).map { $0.id }, ["topic-cn", "movies", "newest", "interest-a"])
    }

    /// Trang chủ lưu từ bản cũ (chưa ghi slot): đoán theo id — hai hàng «Vì … hay xem» theo thứ tự xuất hiện.
    func testArrangeLegacyRows() {
        func row(_ id: String) -> FeedRow { FeedRow(id: id, title: id, cards: []) }
        let rows = [row("similar"), row("interest-hanh-dong"), row("dominant"), row("interest-tien-hiep"), row("newest")]
        let prefs: [(id: String, on: Bool)] = [("interest2", true), ("similar", true), ("interest", false), ("dominant", true),
                                               ("newest", true)]
        XCTAssertEqual(HomeFeed.arrange(rows, prefs).map { $0.id }, ["interest-tien-hiep", "similar", "dominant", "newest"])
    }

    /// Sửa nhanh trên trang chủ: chuyển qua hàng đang hiện kề nó (hàng tắt / không hiện nằm giữa bị vượt qua).
    func testMoveShownSkipsHiddenRows() {
        let prefs: [(id: String, on: Bool)] = [("new-eps", true), ("continue", true), ("similar", true), ("interest", false),
                                               ("dominant", true), ("newest", true), ("movies", true)]
        let shown = ["similar", "dominant", "movies"] // «Mới cập nhật» không đủ phim nên không hiện
        XCTAssertEqual(HomeFeed.moveShown(prefs, shown, "dominant", up: true).map(ids),
                       ["new-eps", "continue", "dominant", "similar", "interest", "newest", "movies"])
        let down = HomeFeed.moveShown(prefs, shown, "dominant", up: false)
        XCTAssertEqual(down.map(ids), ["new-eps", "continue", "similar", "interest", "newest", "movies", "dominant"])
        XCTAssertEqual(down?.first { $0.id == "interest" }?.on, false)
        XCTAssertNil(HomeFeed.moveShown(prefs, shown, "similar", up: true))
        XCTAssertNil(HomeFeed.moveShown(prefs, shown, "movies", up: false))
        XCTAssertNil(HomeFeed.moveShown(prefs, ["continue", "similar"], "continue", up: false))
        XCTAssertNil(HomeFeed.moveShown(prefs, shown, "newest", up: true))
    }

    func testHideTurnsOffOnlyThatRow() {
        let h = HomeFeed.hide([("similar", true), ("custom-a", true), ("movies", false)], "custom-a")
        XCTAssertEqual(h.map { "\($0.id)=\($0.on)" }, ["similar=true", "custom-a=false", "movies=false"])
    }

    func testArrangeSlotsKeepsSlotOfLegacyRows() {
        func row(_ id: String, _ slot: String = "") -> FeedRow { FeedRow(id: id, title: id, cards: [], more: nil, slot: slot) }
        let rows = [row("interest-a"), row("custom-x", "custom-x"), row("interest-b")]
        let prefs: [(id: String, on: Bool)] = [("custom-x", true), ("interest2", true), ("interest", true)]
        XCTAssertEqual(HomeFeed.arrangeSlots(rows, prefs).map { "\($0.row.id)=\($0.slot)" },
                       ["custom-x=custom-x", "interest-b=interest2", "interest-a=interest"])
    }

    /// Nguồn tắt thì bỏ; tắt hết nguồn đang có thì coi như bật hết (trang chủ không trống).
    func testSourcesOff() {
        let all = HomeFeed.Sources(kk: "https://kk", nc: "https://nc", vs: nil, k20: "https://k20", web: ["https://web1", "https://web2"])
        XCTAssertEqual(all.ids, ["kk", "nc", "k20", "web"])
        let s = HomeFeed.withoutOff(all, ["kk", "web", "vs"])
        XCTAssertEqual(s.ids, ["nc", "k20"])
        XCTAssertNil(s.kk)
        XCTAssertEqual(s.web, [])
        XCTAssertEqual(HomeFeed.withoutOff(all, ["kk", "nc", "k20", "web"]).ids, all.ids)
        XCTAssertEqual(HomeFeed.withoutOff(HomeFeed.Sources(kk: nil, nc: "https://nc", vs: nil, k20: nil, web: []), ["kk"]).ids, ["nc"])
    }
}
