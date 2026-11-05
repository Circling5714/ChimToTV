import XCTest

/// Trang đổi tên miền (phimmoir.fun → phimmois.fun): dữ liệu đã lưu tự đổi sang tên mới lúc mở app.
final class MovedHostsTests: XCTestCase {
    private var suite = ""

    override func setUp() {
        super.setUp()
        suite = "MovedHostsTests-\(UUID().uuidString)"
        Prefs.store = UserDefaults(suiteName: suite)!
        Prefs.reload()
    }

    override func tearDown() {
        UserDefaults().removePersistentDomain(forName: suite)
        Prefs.store = .standard
        Prefs.reload()
        super.tearDown()
    }

    func testRenamesSavedData() {
        let st = Prefs.store
        st.set(["https://yanhh3d.ee", "https://phimmoir.fun", "https://phimmois.fun"], forKey: "sites")
        st.set("https://phimmoir.fun", forKey: "base_url")
        st.set("kk", forKey: "server_type@phimmoir.fun")
        st.set("nguonc", forKey: "server_type@phimmois.fun#p1")
        st.set("ophim", forKey: "server_type@phimmoir.fun#p1") // khoá tên mới đã có → giữ khoá tên mới
        st.set(Data(#"[{"filmUrl":"https:\/\/phimmoir.fun\/phim\/a"}]"#.utf8), forKey: "history")
        Prefs.migrateMovedHosts()
        XCTAssertEqual(Prefs.sites, ["https://yanhh3d.ee", "https://phimmois.fun"])
        XCTAssertEqual(Prefs.baseUrl, "https://phimmois.fun")
        XCTAssertEqual(st.string(forKey: "server_type@phimmois.fun"), "kk")
        XCTAssertEqual(st.string(forKey: "server_type@phimmois.fun#p1"), "nguonc")
        XCTAssertNil(st.object(forKey: "server_type@phimmoir.fun"))
        XCTAssertEqual(String(data: st.data(forKey: "history") ?? Data(), encoding: .utf8), #"[{"filmUrl":"https:\/\/phimmois.fun\/phim\/a"}]"#)
    }

    func testReplaceSiteKeepsPlace() {
        Prefs.sites = ["https://yanhh3d.ee", "https://phimmoir.fun", "https://hhkungfu.ee"]
        Prefs.replaceSite("https://phimmoir.fun", with: "https://phimmois.fun")
        XCTAssertEqual(Prefs.sites, ["https://yanhh3d.ee", "https://phimmois.fun", "https://hhkungfu.ee"])
        XCTAssertEqual(Prefs.baseUrl, "https://phimmois.fun")
    }
}
