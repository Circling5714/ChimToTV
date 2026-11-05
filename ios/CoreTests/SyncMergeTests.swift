import XCTest

/// Đồng bộ trong nhà: gộp lịch sử / chỗ đang xem / theo dõi / danh sách trang (cùng ca với SyncTest.kt của bản Android).
final class SyncMergeTests: XCTestCase {
    private func h(_ film: String, _ time: Int64, ep: String = "e1") -> SyncHistory {
        SyncHistory(film: film, title: film.uppercased(), poster: "", ep: ep, epUrl: "\(film)/\(ep)", time: time)
    }

    private func f(_ film: String, count: Int, seen: Int, t: Int64) -> SyncFollow {
        SyncFollow(film: film, title: film, poster: "", count: count, latest: "Tập \(count)", seen: seen, t: t)
    }

    func testHistoryNewestWinsAndUnion() {
        let a = SyncProfile(id: "", name: "Cả nhà", history: [h("x", 100, ep: "e1"), h("y", 50)])
        let b = SyncProfile(id: "", name: "Cả nhà", history: [h("x", 200, ep: "e2"), h("z", 150)])
        let m = SyncMerge.profile(a, b)
        XCTAssertEqual(m.history.map { $0.film }, ["x", "z", "y"])
        XCTAssertEqual(m.history[0].ep, "e2")
        // Gộp hai chiều cho cùng kết quả
        XCTAssertEqual(SyncMerge.profile(b, a).history, m.history)
    }

    func testDeletedHistoryStaysDeleted() {
        // Máy A xoá phim x lúc 300 (sau lần xem 200 ở máy B)
        let a = SyncProfile(id: "", name: "n", history: [], histDel: ["x": 300],
                            pos: [])
        let b = SyncProfile(id: "", name: "n", history: [h("x", 200)],
                            pos: [SyncPos(ep: "x/e1", sec: 30, film: "x", t: 200), SyncPos(ep: "y/e1", sec: 5, film: "y", t: 10)])
        let m = SyncMerge.profile(a, b)
        XCTAssertTrue(m.history.isEmpty)
        XCTAssertEqual(m.pos.map { $0.ep }, ["y/e1"])
        XCTAssertEqual(m.histDel["x"], 300)
        // Xem lại sau khi xoá thì hiện lại
        let c = SyncProfile(id: "", name: "n", history: [h("x", 400)])
        XCTAssertEqual(SyncMerge.profile(m, c).history.map { $0.film }, ["x"])
    }

    func testPositionsNewestWinsOldestFirst() {
        let a = SyncProfile(id: "", name: "n", pos: [SyncPos(ep: "old", sec: 1, film: "f", t: 0),
                                                     SyncPos(ep: "e", sec: 100, film: "f", t: 500)])
        let b = SyncProfile(id: "", name: "n", pos: [SyncPos(ep: "e", sec: 250, film: "f", t: 600),
                                                     SyncPos(ep: "n", sec: 7, film: "f", t: 50)])
        let m = SyncMerge.profile(a, b)
        XCTAssertEqual(m.pos.map { $0.ep }, ["old", "n", "e"])
        XCTAssertEqual(m.pos.last?.sec, 250)
    }

    func testFollowsMergeAndUnfollow() {
        let a = SyncProfile(id: "", name: "n", follows: [f("x", count: 10, seen: 10, t: 100), f("y", count: 3, seen: 1, t: 50)])
        let b = SyncProfile(id: "", name: "n", follows: [f("x", count: 12, seen: 9, t: 100)], followDel: ["y": 80])
        let m = SyncMerge.profile(a, b)
        XCTAssertEqual(m.follows.map { $0.film }, ["x"])
        XCTAssertEqual(m.follows[0].count, 12)
        XCTAssertEqual(m.follows[0].seen, 10)
        XCTAssertEqual(m.follows[0].latest, "Tập 12")
        // Theo dõi lại sau khi bỏ
        let c = SyncProfile(id: "", name: "n", follows: [f("y", count: 3, seen: 3, t: 90)])
        XCTAssertEqual(Set(SyncMerge.profile(m, c).follows.map { $0.film }), ["x", "y"])
    }

    func testMatchProfiles() {
        let local = [SyncProfile(id: "", name: "Cả nhà"), SyncProfile(id: "p1", name: "Bố"), SyncProfile(id: "p2", name: "Bé Na")]
        let remote = [SyncProfile(id: "p3", name: " bé na "), SyncProfile(id: "", name: "Nhà mình"), SyncProfile(id: "p1", name: "Mẹ")]
        let m = SyncMerge.match(local, remote)
        XCTAssertEqual(m.map { "\($0.local)-\($0.remote)" }, ["0-1", "2-0"])
    }

    func testSites() {
        let a = SyncSites(list: ["a", "b", "c"], add: ["c": 100], del: ["d": 100])
        let b = SyncSites(list: ["a", "d", "e"], add: ["d": 50, "e": 200], del: ["b": 300])
        let m = SyncMerge.sites(a, b)
        XCTAssertEqual(m.list, ["a", "c", "e"])
        XCTAssertEqual(SyncMerge.sites(b, a).list.sorted(), m.list.sorted())
    }

    func testTombstoneCap() {
        var big: [String: Int64] = [:]
        for i in 0..<(SyncMerge.maxTombstones + 20) { big["k\(i)"] = Int64(i) }
        let m = SyncMerge.tombstones(big, [:])
        XCTAssertEqual(m.count, SyncMerge.maxTombstones)
        XCTAssertNil(m["k0"])
        XCTAssertNotNil(m["k\(SyncMerge.maxTombstones + 19)"])
    }

    func testJSONShape() throws {
        let p = SyncPayload(device: "d", name: "iPad", platform: "ios",
                            profiles: [SyncProfile(id: "", name: "Cả nhà", history: [h("x", 1)])],
                            sites: SyncSites(list: ["a"]))
        let o = try JSONSerialization.jsonObject(with: JSONEncoder().encode(p)) as! [String: Any]
        let prof = (o["profiles"] as! [[String: Any]])[0]
        let hist = (prof["history"] as! [[String: Any]])[0]
        XCTAssertEqual(Set(hist.keys), ["film", "title", "poster", "ep", "epUrl", "time"])
        XCTAssertEqual(Set(prof.keys), ["id", "name", "history", "histDel", "pos", "follows", "followDel"])
        XCTAssertEqual(o["v"] as? Int, 1)
    }
}
