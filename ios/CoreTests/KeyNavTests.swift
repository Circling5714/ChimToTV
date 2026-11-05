import XCTest

/// Phím mũi tên trên lưới phim chia nhóm.
final class KeyNavTests: XCTestCase {
    // Nhóm "Xem tiếp" 2 ô, nhóm phim 7 ô; 3 cột:
    //  a0 a1
    //  b0 b1 b2
    //  b3 b4 b5
    //  b6
    private let g = [["a0", "a1"], ["b0", "b1", "b2", "b3", "b4", "b5", "b6"]]

    private func mv(_ from: String?, _ d: KeyNav.Dir) -> String? { KeyNav.move(g, cols: 3, from: from, d) }

    func testStartsAtFirst() {
        XCTAssertEqual(mv(nil, .down), "a0")
        XCTAssertEqual(mv("gone", .right), "a0")
        XCTAssertNil(KeyNav.move([[], []], cols: 3, from: nil, .down))
    }

    func testLeftRightCrossGroups() {
        XCTAssertEqual(mv("a1", .right), "b0")
        XCTAssertEqual(mv("b0", .left), "a1")
        XCTAssertEqual(mv("a0", .left), "a0")
        XCTAssertEqual(mv("b6", .right), "b6")
    }

    func testDownKeepsColumn() {
        XCTAssertEqual(mv("a1", .down), "b1")
        XCTAssertEqual(mv("b1", .down), "b4")
        XCTAssertEqual(mv("b4", .down), "b6") // hàng cuối ngắn: về ô cuối
        XCTAssertEqual(mv("b6", .down), "b6")
    }

    func testUpToShorterGroup() {
        XCTAssertEqual(mv("b2", .up), "a1") // nhóm trên chỉ 2 ô
        XCTAssertEqual(mv("b0", .up), "a0")
        XCTAssertEqual(mv("b6", .up), "b3")
        XCTAssertEqual(mv("a0", .up), "a0")
    }

    func testColumns() {
        XCTAssertEqual(KeyNav.columns(width: 800, minimum: 150, spacing: 16), 4)
        XCTAssertEqual(KeyNav.columns(width: 100, minimum: 150, spacing: 16), 1)
    }
}
