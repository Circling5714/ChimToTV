import XCTest

/// Phim liên quan (11/2026): tên gốc, số phần, ghép cùng bộ, hàng cùng thể loại — cùng ca với RelatedTest.kt / RelatedTests.cs.
final class RelatedTests: XCTestCase {
    private func f(_ title: String, _ url: String? = nil) -> Film {
        Film(url: url ?? "https://x.test/" + KkPhim.norm(title).replacingOccurrences(of: " ", with: "-"), title: title, poster: nil, badge: nil)
    }

    func testSearchNameDropsPartYearAndEpisodes() {
        XCTAssertEqual(Related.searchName("Tiên Nghịch (Phần 2) 2024 – 20 Tập"), "Tiên Nghịch")
        XCTAssertEqual(Related.searchName("Đấu Phá Thương Khung Phần 5"), "Đấu Phá Thương Khung")
        XCTAssertEqual(Related.searchName("Nhất Nguyệt Thần Kiếm 1990 (Phần 01) – 20 Tập"), "Nhất Nguyệt Thần Kiếm")
        XCTAssertEqual(Related.searchName("Avatar: Dòng Chảy Của Nước"), "Avatar")
        XCTAssertEqual(Related.searchName("1917"), "1917")
    }

    func testPartNumbers() {
        XCTAssertEqual(Related.part("Đấu Phá Thương Khung Phần 5"), 5)
        XCTAssertEqual(Related.part("Nhất Nguyệt Thần Kiếm 1990 (Phần 01) – 20 Tập"), 1)
        XCTAssertEqual(Related.part("Squid Game Season 2"), 2)
        XCTAssertEqual(Related.part("Thế Giới Hoàn Mỹ 3"), 3)
        XCTAssertEqual(Related.part("Đấu Phá Thương Khung 4: Đào Vong"), 4)
        XCTAssertNil(Related.part("Tiên Nghịch"))
        XCTAssertNil(Related.part("Trái Tim Quái Thú 2025"))
    }

    func testSameSeriesMatchesOtherPartsOnly() {
        XCTAssertTrue(Related.sameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung Phần 1"))
        XCTAssertTrue(Related.sameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung"))
        XCTAssertTrue(Related.sameSeries("Avatar", "Avatar: Dòng Chảy Của Nước"))
        XCTAssertFalse(Related.sameSeries("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung Phần 5"))
        XCTAssertFalse(Related.sameSeries("Tiên Nghịch", "Tiên Nghịch Truyện Kỳ"))
        XCTAssertFalse(Related.sameSeries("Yêu", "Yêu 2"))
    }

    func testPickSeriesOrdersByPart() {
        let found = [f("Đấu Phá Thương Khung Phần 3"), f("Đấu Phá Thương Khung Phần 5", "https://x.test/me"), f("Đấu Phá Thương Khung"),
                     f("Đấu Phá Thương Khung Phần 3", "https://kk.test/dpk3"), f("Kiếm Lai"), f("Đấu Phá Thương Khung Phần 2")]
        XCTAssertEqual(Related.pickSeries("Đấu Phá Thương Khung Phần 5", url: "https://x.test/me", found).map(\.title),
                       ["Đấu Phá Thương Khung", "Đấu Phá Thương Khung Phần 2", "Đấu Phá Thương Khung Phần 3"])
        let more = Related.pickSeries("Đấu Phá Thương Khung (Phần 5)", url: "https://x.test/me",
                                      [f("Đấu Phá Thương Khung: Thức Tỉnh"), f("Đấu Phá Thương Khung 2: Ngừng Chiến"), f("Đấu Phá Thương Khung (Phần 2)")])
        XCTAssertEqual(more.map(\.title), ["Đấu Phá Thương Khung: Thức Tỉnh", "Đấu Phá Thương Khung 2: Ngừng Chiến", "Đấu Phá Thương Khung (Phần 2)"])
    }

    func testPickSimilarSkipsSelfSeriesAndPutsWatchedLast() {
        let found = [f("Tiên Nghịch"), f("Phàm Nhân Tu Tiên"), f("Tiên Nghịch Phần 2"), f("Thế Giới Hoàn Mỹ"), f("Phàm Nhân Tu Tiên")]
        let got = Related.pickSimilar("Tiên Nghịch", url: "https://x.test/me", found, watched: [KkPhim.norm("Phàm Nhân Tu Tiên")])
        XCTAssertEqual(got.map(\.title), ["Thế Giới Hoàn Mỹ", "Phàm Nhân Tu Tiên"])
    }
}
