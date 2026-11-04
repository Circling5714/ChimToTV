import XCTest

/// Chọn / gợi ý server theo tên server thật của yanhh3d (chuyển từ PlayStatsTest.kt của bản Android).
final class PlayStatsTests: XCTestCase {
    let v1 = Server(label: "VIETSUB 1080 V1", type: "a")
    let v2 = Server(label: "VIETSUB 1080 V2", type: "b")
    let k1 = Server(label: "VIETSUB 4K V1", type: "c")
    let k2 = Server(label: "VIETSUB 4K V2", type: "d")
    let tm = Server(label: "Thuyết minh 1080 V1", type: "e")
    var all: [Server] { [v1, v2, k1, k2, tm] }

    let unknown = PlayStats.Score(level: -1, startSec: nil, stallPct: 0)
    let smooth = PlayStats.Score(level: 0, startSec: 4, stallPct: 0)
    let bad = PlayStats.Score(level: 2, startSec: 20, stallPct: 0.12)

    func scores(_ s: [(Server, PlayStats.Score)] = []) -> (Server) -> PlayStats.Score {
        var m: [Server: PlayStats.Score] = [:]
        for (k, v) in s { m[k] = v }
        let u = unknown
        return { m[$0] ?? u }
    }

    func testReadsLanguageAndResolution() {
        XCTAssertEqual(PlayStats.lang("VIETSUB 1080 V1"), "vs")
        XCTAssertEqual(PlayStats.lang("Thuyết minh 1080 V1"), "tm")
        // Trang có thể trả chữ tiếng Việt dạng tổ hợp (NFD)
        XCTAssertEqual(PlayStats.lang("Thuyết Minh".decomposedStringWithCanonicalMapping), "tm")
        XCTAssertEqual(PlayStats.lang("Lồng Tiếng"), "lt")
        XCTAssertNil(PlayStats.lang("VIP"))
        XCTAssertEqual(PlayStats.resolution("VIETSUB 4K V1"), 2160)
        XCTAssertEqual(PlayStats.resolution("VIETSUB 1080 V2"), 1080)
        XCTAssertEqual(PlayStats.resolution("HD 720p"), 720)
        XCTAssertNil(PlayStats.resolution("VIP"))
    }

    func testSuggestsSameResolutionMirrorFirst() {
        XCTAssertEqual(PlayStats.alternative(all, v1, scores())?.0, v2)
        XCTAssertEqual(PlayStats.alternative(all, k1, scores())?.0, k2)
    }

    func testNeverSuggestsSharperUntestedServerOrOtherLanguage() {
        XCTAssertNil(PlayStats.alternative(all, v2, scores([(v1, bad)])))
    }

    func testStepsDownWhenSameResolutionIsBad() {
        XCTAssertEqual(PlayStats.alternative(all, k1, scores([(k2, bad)]))?.0, v1)
    }

    func testRatedSmoothServerBeatsUntested() {
        XCTAssertEqual(PlayStats.alternative(all, v1, scores([(k1, smooth)]))?.0, k1)
    }

    func testRatesRealMeasurements() {
        XCTAssertEqual(PlayStats.level(sessions: 1, fails: 0, play: 95, stall: 1, stalls: 1), 0)
        XCTAssertEqual(PlayStats.level(sessions: 3, fails: 0, play: 151, stall: 24, stalls: 13), 2)
        XCTAssertEqual(PlayStats.level(sessions: 4, fails: 0, play: 1788, stall: 12, stalls: 6), 1)
        XCTAssertEqual(PlayStats.level(sessions: 1, fails: 0, play: 40, stall: 5, stalls: 2), -1)
        XCTAssertEqual(PlayStats.level(sessions: 2, fails: 1, play: 300, stall: 0, stalls: 0), 3)
    }

    func testChoosesBetterServerOnlyWhenUsualIsBad() {
        XCTAssertEqual(PlayStats.choose(all, v1, scores([(v2, smooth)])), v1)
        XCTAssertEqual(PlayStats.choose(all, v1, scores([(v1, bad), (v2, smooth)])), v2)
        // Không tự đổi sang Thuyết minh dù chấm tốt hơn
        XCTAssertEqual(PlayStats.choose(all, v1, scores([(v1, bad), (tm, smooth)])), v1)
    }

    func testLeavesFailingServerForAnyWorkingOne() {
        let dead = PlayStats.Score(level: 3, startSec: nil, stallPct: 0)
        XCTAssertEqual(PlayStats.choose(all, v1, scores([(v1, dead), (v2, bad)])), v2)
        XCTAssertEqual(PlayStats.choose([v1, v2], v1, scores([(v1, dead)])), v2)
        XCTAssertEqual(PlayStats.choose([v1, k1, tm], v1, scores([(v1, dead), (tm, smooth)])), v1)
        XCTAssertEqual(PlayStats.choose([v1, v2], v1, scores([(v1, dead), (v2, dead)])), v1)
        XCTAssertEqual(PlayStats.choose([v1, v2], v1, scores([(v1, bad)])), v1)
    }

    func testSessionCountsStallsNotSeeks() {
        let s = PlaySession(host: "x", server: "a", openedAt: 0)
        s.update(1000, paused: false, waiting: false, ready: true, droppedFrames: 0, totalFrames: 0) // bắt đầu phát
        for i in 2...30 { s.update(Int64(i) * 1000, paused: false, waiting: false, ready: true, droppedFrames: 0, totalFrames: 0) }
        s.seeked()
        s.update(31_000, paused: false, waiting: true, ready: true, droppedFrames: 0, totalFrames: 0) // chờ sau lệnh tua: không tính
        s.update(32_000, paused: false, waiting: false, ready: true, droppedFrames: 0, totalFrames: 0)
        s.update(33_000, paused: false, waiting: true, ready: true, droppedFrames: 0, totalFrames: 0) // giật thật
        XCTAssertEqual(s.describe(), "mở 1 s · giật 1 lần (1 s)")
        XCTAssertFalse(s.struggling(33_000))
    }
}
