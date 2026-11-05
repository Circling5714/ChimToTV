import XCTest

/// Người xem (hồ sơ) — chuyển từ ProfilesTest.kt của bản Android. Mỗi test một kho UserDefaults riêng.
final class ProfilesTests: XCTestCase {
    private var suite = ""

    private let film = FilmDetail(
        url: "https://yanhh3d.ee/tien-nghich", title: "Tiên Nghịch", otherTitle: nil, poster: nil, description: nil, info: nil,
        groups: [EpisodeGroup(name: "Vietsub", episodes: (1...3).map { Episode(title: "Tập \($0)", url: "https://yanhh3d.ee/tien-nghich/tap-\($0)") })]
    )

    private func history(_ slug: String) -> HistoryItem {
        HistoryItem(filmUrl: "https://yanhh3d.ee/\(slug)", title: slug, poster: nil, epTitle: "Tập 1",
                    epUrl: "https://yanhh3d.ee/\(slug)/tap-1", time: 1)
    }

    override func setUp() {
        super.setUp()
        suite = "ProfilesTests-\(UUID().uuidString)"
        Prefs.store = UserDefaults(suiteName: suite)!
        Prefs.reload()
    }

    override func tearDown() {
        UserDefaults().removePersistentDomain(forName: suite)
        Prefs.store = .standard
        Prefs.reload()
        super.tearDown()
    }

    /// Bản cũ (chưa có người xem): dữ liệu là của người xem đầu tiên "Cả nhà", khoá giữ nguyên.
    func testOldDataBelongsToFirstProfile() {
        Prefs.addHistory(history("tien-nghich"))
        Prefs.savePosition("https://yanhh3d.ee/tien-nghich/tap-1", seconds: 120, filmUrl: film.url)
        Prefs.baseUrl = "https://phimmoir.fun"
        XCTAssertNotNil(Prefs.store.object(forKey: "history"))
        XCTAssertNotNil(Prefs.store.object(forKey: "positions_v2"))
        XCTAssertEqual(Prefs.store.string(forKey: "base_url"), "https://phimmoir.fun")

        Prefs.reload() // mở lại app (bản mới)
        XCTAssertEqual(Prefs.profiles, [Profile(id: "", name: "Cả nhà", color: 0, pin: nil)])
        XCTAssertEqual(Prefs.profileId, "")
        XCTAssertEqual(Prefs.history.count, 1)
        XCTAssertEqual(Prefs.position("https://yanhh3d.ee/tien-nghich/tap-1"), 120)
        XCTAssertEqual(Prefs.baseUrl, "https://phimmoir.fun")
    }

    func testEachProfileHasOwnHistoryFollowsAndSettings() {
        Prefs.addHistory(history("tien-nghich"))
        Prefs.savePosition("https://yanhh3d.ee/tien-nghich/tap-1", seconds: 120, filmUrl: film.url)
        Prefs.follow(film)
        Prefs.baseUrl = "https://hoathinh3d.ee"
        Prefs.seekStep = 30
        Prefs.setServer("hoathinh3d.ee", type: "tm", label: "Thuyết Minh")
        Prefs.setIntro(film.url, start: 0, end: 90)
        Prefs.privateMode = true

        let bo = Prefs.addProfile("  Bố ")
        XCTAssertEqual(bo.name, "Bố")
        XCTAssertEqual(bo.id, "p1")
        XCTAssertEqual(Prefs.profileId, "") // thêm chưa chuyển sang
        Prefs.switchProfile(bo.id)

        // Lịch sử, chỗ xem, theo dõi, Xem riêng: trống; trang / bước tua / server ưu tiên: chép từ người cũ
        XCTAssertTrue(Prefs.history.isEmpty)
        XCTAssertEqual(Prefs.position("https://yanhh3d.ee/tien-nghich/tap-1"), 0)
        XCTAssertTrue(Prefs.watchedEpisodes().isEmpty)
        XCTAssertTrue(Prefs.follows.isEmpty)
        XCTAssertFalse(Prefs.privateMode)
        XCTAssertEqual(Prefs.baseUrl, "https://hoathinh3d.ee")
        XCTAssertEqual(Prefs.seekStep, 30)
        XCTAssertEqual(Prefs.serverType("hoathinh3d.ee"), "tm")
        // Mốc nhạc mở đầu của phim: dùng chung
        XCTAssertEqual(Prefs.intro(film.url)?.end, 90)

        // Người mới đổi gì cũng không đụng tới người cũ
        Prefs.addHistory(history("vu-than"))
        Prefs.baseUrl = "https://phimmoir.fun"
        Prefs.setServer("hoathinh3d.ee", type: "vs", label: "Vietsub")
        Prefs.seekStep = 10
        Prefs.switchProfile("")
        XCTAssertEqual(Prefs.history.map { $0.title }, ["tien-nghich"])
        XCTAssertEqual(Prefs.position("https://yanhh3d.ee/tien-nghich/tap-1"), 120)
        XCTAssertEqual(Prefs.follows.count, 1)
        XCTAssertTrue(Prefs.privateMode)
        XCTAssertEqual(Prefs.baseUrl, "https://hoathinh3d.ee")
        XCTAssertEqual(Prefs.serverType("hoathinh3d.ee"), "tm")
        XCTAssertEqual(Prefs.seekStep, 30)

        // Mở lại app: nhớ người xem lần trước
        Prefs.switchProfile(bo.id)
        Prefs.reload()
        XCTAssertEqual(Prefs.profileId, bo.id)
        XCTAssertEqual(Prefs.history.map { $0.title }, ["vu-than"])
    }

    func testDeleteRemovesOnlyThatProfileData() {
        Prefs.addHistory(history("tien-nghich"))
        let me = Prefs.addProfile("Mẹ")
        Prefs.switchProfile(me.id)
        Prefs.addHistory(history("vu-than"))
        Prefs.setServer("yanhh3d.ee", type: "v1", label: "VIP 1")
        Prefs.follow(film)
        Prefs.setIntro(film.url, start: 0, end: 80)

        Prefs.deleteProfile(me.id)
        XCTAssertEqual(Prefs.profileId, "") // xoá người đang dùng → sang người còn lại
        XCTAssertEqual(Prefs.profiles.map { $0.id }, [""])
        XCTAssertTrue(Prefs.store.dictionaryRepresentation().keys.allSatisfy { !$0.hasSuffix("#\(me.id)") })
        XCTAssertEqual(Prefs.history.map { $0.title }, ["tien-nghich"])
        XCTAssertEqual(Prefs.intro(film.url)?.end, 80) // dùng chung, còn nguyên
        // Còn một người thì không xoá được
        Prefs.deleteProfile("")
        XCTAssertEqual(Prefs.profiles.count, 1)
    }

    func testDeleteFirstProfileKeepsSharedSettings() {
        Prefs.addHistory(history("tien-nghich"))
        Prefs.sites = ["https://yanhh3d.ee", "https://phimmoir.fun"]
        Prefs.setIntro(film.url, start: 0, end: 80)
        let be = Prefs.addProfile("Bé")
        Prefs.deleteProfile("")
        XCTAssertEqual(Prefs.profileId, be.id)
        XCTAssertNil(Prefs.store.object(forKey: "history"))
        XCTAssertEqual(Prefs.sites, ["https://yanhh3d.ee", "https://phimmoir.fun"])
        XCTAssertEqual(Prefs.intro(film.url)?.end, 80)
    }

    func testPinLock() {
        let bo = Prefs.addProfile("Bố")
        XCTAssertFalse(bo.locked)
        XCTAssertTrue(Prefs.checkPin(bo, "")) // không khoá: mã nào cũng qua
        Prefs.setProfilePin(bo.id, "1234")
        let locked = Prefs.profiles.first { $0.id == bo.id }!
        XCTAssertTrue(locked.locked)
        XCTAssertFalse(locked.pin!.contains("1234")) // chỉ lưu mã băm
        XCTAssertTrue(Prefs.checkPin(locked, "1234"))
        XCTAssertFalse(Prefs.checkPin(locked, "4321"))
        XCTAssertFalse(Prefs.checkPin(locked, ""))
        Prefs.setProfilePin(bo.id, nil)
        XCTAssertNil(Prefs.profiles.first { $0.id == bo.id }!.pin)
    }

    func testNewProfilesGetFreeColorsAndIds() {
        let a = Prefs.addProfile("A")
        let b = Prefs.addProfile("B")
        XCTAssertEqual(Prefs.profiles.map { $0.color }, [0, 1, 2])
        XCTAssertEqual(Prefs.profiles.map { $0.id }, ["", "p1", "p2"])
        Prefs.deleteProfile(a.id)
        let c = Prefs.addProfile("C")
        XCTAssertEqual(c.id, "p1") // id trống thì dùng lại (dữ liệu của người cũ đã xoá hết)
        XCTAssertEqual(c.color, 1)
        Prefs.renameProfile(b.id, "Bé Na")
        Prefs.setProfileColor(b.id, 6)
        XCTAssertEqual(Prefs.profiles.first { $0.id == b.id }, Profile(id: "p2", name: "Bé Na", color: 6, pin: nil))
    }
}
