import XCTest

/// Giới hạn giờ xem theo người xem — chuyển từ WatchLimitTest.kt của bản Android.
final class WatchLimitTests: XCTestCase {
    private var suite = ""
    private var now: Int64 = 0

    private func at(_ h: Int, _ m: Int, day: Int = 5) -> Int64 {
        let d = Calendar.current.date(from: DateComponents(year: 2026, month: 11, day: day, hour: h, minute: m))!
        return Int64(d.timeIntervalSince1970 * 1000)
    }

    /// Phim chạy `seconds` giây (trình phát báo trạng thái mỗi giây).
    private func watch(_ seconds: Int) {
        for _ in 0..<seconds {
            now += 1000
            WatchLimit.tick(playing: true, now: now)
        }
    }

    override func setUp() {
        super.setUp()
        suite = "WatchLimitTests-\(UUID().uuidString)"
        Prefs.store = UserDefaults(suiteName: suite)!
        Prefs.reload()
        WatchLimit.reset()
        WatchLimit.clock = { [unowned self] in self.now }
    }

    override func tearDown() {
        WatchLimit.clock = { Int64(Date().timeIntervalSince1970 * 1000) }
        WatchLimit.reset()
        UserDefaults().removePersistentDomain(forName: suite)
        Prefs.store = .standard
        Prefs.reload()
        super.tearDown()
    }

    func testNoLimitNeverBlocks() {
        now = at(23, 30)
        WatchLimit.tick(playing: false, now: now)
        watch(4000)
        XCTAssertNil(WatchLimit.block())
        XCTAssertNil(WatchLimit.remaining())
    }

    func testDailyLimitBlocksThenGrantAddsMinutesAndNextDayStartsOver() {
        Prefs.setDailyLimit(Prefs.profileId, 30)
        now = at(15, 0)
        WatchLimit.tick(playing: false, now: now)
        watch(29 * 60)
        XCTAssertNil(WatchLimit.block())
        XCTAssertEqual(WatchLimit.remaining(), 60)
        watch(61)
        XCTAssertEqual(WatchLimit.block(), .daily)
        WatchLimit.grant(15)
        XCTAssertNil(WatchLimit.block())
        XCTAssertTrue((14 * 60...15 * 60).contains(WatchLimit.remaining() ?? -1))
        now = at(15, 0, day: 6)
        WatchLimit.tick(playing: false, now: now)
        XCTAssertEqual(WatchLimit.remaining(), 30 * 60)
    }

    func testPausedTimeAndGapsDoNotCount() {
        Prefs.setDailyLimit(Prefs.profileId, 30)
        now = at(10, 0)
        for _ in 0..<100 {
            now += 1000
            WatchLimit.tick(playing: false, now: now)
        }
        now += 60_000 // app ngủ / trình phát đóng một lúc
        WatchLimit.tick(playing: true, now: now)
        XCTAssertEqual(WatchLimit.remaining(), 30 * 60)
    }

    func testBedtimeBlocksUntilSixInTheMorning() {
        Prefs.setBedtime(Prefs.profileId, 21 * 60)
        now = at(20, 59)
        XCTAssertNil(WatchLimit.block())
        now = at(21, 0)
        XCTAssertEqual(WatchLimit.block(), .bedtime)
        now = at(5, 59, day: 6)
        XCTAssertEqual(WatchLimit.block(), .bedtime)
        now = at(6, 0, day: 6)
        XCTAssertNil(WatchLimit.block())
    }

    func testGrantAtBedtimeUnblocksForAWhileOrUntilMorning() {
        Prefs.setBedtime(Prefs.profileId, 21 * 60)
        now = at(21, 30)
        WatchLimit.grant(30)
        XCTAssertNil(WatchLimit.block())
        now = at(22, 1)
        XCTAssertEqual(WatchLimit.block(), .bedtime)
        WatchLimit.grantUntilMorning()
        now = at(5, 59, day: 6)
        XCTAssertNil(WatchLimit.block())
        now = at(21, 0, day: 6)
        XCTAssertEqual(WatchLimit.block(), .bedtime)
    }

    func testLimitsBelongToEachProfile() {
        let kid = Prefs.addProfile("Bé Na")
        Prefs.setDailyLimit(kid.id, 45)
        XCTAssertNil(WatchLimit.remaining()) // "Cả nhà" không bị giới hạn
        XCTAssertTrue(Prefs.anyLimits)
        Prefs.switchProfile(kid.id)
        now = at(9, 0)
        XCTAssertEqual(WatchLimit.remaining(), 45 * 60)
        XCTAssertEqual(WatchLimit.summary(kid.id), "45 phút/ngày")
        Prefs.switchProfile("")
        Prefs.deleteProfile(kid.id)
        XCTAssertFalse(Prefs.anyLimits)
    }

    func testParentPin() {
        XCTAssertFalse(Prefs.hasParentPin)
        Prefs.setParentPin("1234")
        XCTAssertTrue(Prefs.hasParentPin)
        XCTAssertTrue(Prefs.checkParentPin("1234"))
        XCTAssertFalse(Prefs.checkParentPin("4321"))
        Prefs.setParentPin(nil)
        XCTAssertFalse(Prefs.hasParentPin)
    }

    func testLabels() {
        XCTAssertEqual(WatchLimit.dailyLabel(0), "Không giới hạn")
        XCTAssertEqual(WatchLimit.dailyLabel(45), "45 phút")
        XCTAssertEqual(WatchLimit.dailyLabel(120), "2 giờ")
        XCTAssertEqual(WatchLimit.dailyLabel(90), "1 giờ 30 phút")
        XCTAssertEqual(WatchLimit.bedtimeLabel(21 * 60 + 30), "21:30")
        XCTAssertEqual(WatchLimit.minutesText(5), "1 phút")
        XCTAssertEqual(WatchLimit.minutesText(300), "5 phút")
    }
}
