import Foundation

// Đồng bộ trong nhà (TV ↔ iPad / iPhone): dữ liệu trao đổi và cách gộp — giống hệt bản Android (Sync.kt), cùng định
// dạng JSON. Mỗi máy gửi toàn bộ dữ liệu của mình, máy kia gộp rồi trả lại bản đã gộp; gộp xong hai máy như nhau.
// Xoá cũng được đồng bộ nhờ "bia mộ" (thời điểm xoá): phim xoá khỏi lịch sử / bỏ theo dõi / trang bỏ khỏi danh sách ở máy
// này không bị máy kia trả về, trừ khi bên kia xem / theo dõi / thêm lại sau thời điểm đó.

struct SyncHistory: Codable, Equatable {
    var film: String
    var title: String
    var poster: String
    var ep: String
    var epUrl: String
    var time: Int64
}

/// Chỗ đang xem một tập (giây; 0 = đã xem hết), t = lúc lưu (0 = lưu từ bản cũ, không rõ lúc nào).
struct SyncPos: Codable, Equatable {
    var ep: String
    var sec: Int
    var film: String
    var t: Int64
}

struct SyncFollow: Codable, Equatable {
    var film: String
    var title: String
    var poster: String
    var count: Int
    var latest: String
    var seen: Int
    /// Lúc bấm theo dõi (0 = bản cũ).
    var t: Int64
}

/// Dữ liệu riêng một người xem.
struct SyncProfile: Codable, Equatable {
    var id: String
    var name: String
    var history: [SyncHistory] = []
    var histDel: [String: Int64] = [:]
    var pos: [SyncPos] = []
    var follows: [SyncFollow] = []
    var followDel: [String: Int64] = [:]
}

/// Danh sách trang phim (dùng chung cả máy): thứ tự, lúc thêm, lúc bỏ.
struct SyncSites: Codable, Equatable {
    var list: [String]
    var add: [String: Int64] = [:]
    var del: [String: Int64] = [:]
}

struct SyncPayload: Codable {
    var v = 1
    var device: String
    var name: String
    var platform: String
    var profiles: [SyncProfile]
    var sites: SyncSites
}

enum SyncMerge {
    static let maxHistory = 60
    static let maxPositions = 400
    /// Giữ tối đa bấy nhiêu "bia mộ" mỗi loại (mới nhất).
    static let maxTombstones = 300

    /// Ghép người xem hai máy: người đầu tiên (id "") với nhau, còn lại theo tên (không phân biệt hoa / thường, khoảng trắng).
    static func match(_ local: [SyncProfile], _ remote: [SyncProfile]) -> [(local: Int, remote: Int)] {
        func key(_ s: String) -> String { s.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() }
        var out: [(Int, Int)] = []
        var used = Set<Int>()
        if let l = local.firstIndex(where: { $0.id.isEmpty }), let r = remote.firstIndex(where: { $0.id.isEmpty }) {
            out.append((l, r))
            used.insert(r)
        }
        for (i, p) in local.enumerated() where !p.id.isEmpty {
            if let j = remote.indices.first(where: { !used.contains($0) && !remote[$0].id.isEmpty && key(remote[$0].name) == key(p.name) }) {
                out.append((i, j))
                used.insert(j)
            }
        }
        return out
    }

    static func tombstones(_ a: [String: Int64], _ b: [String: Int64]) -> [String: Int64] {
        var m = a
        for (k, v) in b where v > (m[k] ?? Int64.min) { m[k] = v }
        if m.count > maxTombstones {
            let keep = m.sorted { $0.value > $1.value || ($0.value == $1.value && $0.key < $1.key) }.prefix(maxTombstones)
            m = Dictionary(uniqueKeysWithValues: keep.map { ($0.key, $0.value) })
        }
        return m
    }

    /// Gộp dữ liệu một người xem (local: máy này, remote: máy kia). Kết quả không phụ thuộc bên nào là local, trừ khi
    /// hai bên trùng thời điểm (lấy local).
    static func profile(_ local: SyncProfile, _ remote: SyncProfile) -> SyncProfile {
        var out = local
        let histDel = tombstones(local.histDel, remote.histDel)
        out.histDel = histDel

        // Lịch sử: mỗi phim một mục, lấy lần xem mới hơn; bỏ phim đã xoá sau lần xem đó
        var hist: [String: SyncHistory] = [:]
        for h in local.history + remote.history {
            if let old = hist[h.film], old.time >= h.time { continue }
            hist[h.film] = h
        }
        out.history = Array(hist.values.filter { h in histDel[h.film].map { $0 < h.time } ?? true }
            .sorted { $0.time > $1.time || ($0.time == $1.time && $0.film < $1.film) }
            .prefix(maxHistory))

        // Chỗ đang xem: mỗi tập lấy lần lưu mới hơn; phim đã xoá khỏi lịch sử sau lần lưu đó thì bỏ
        var pos: [String: SyncPos] = [:]
        var order: [String] = []
        for p in local.pos + remote.pos {
            if let old = pos[p.ep] {
                if p.t > old.t { pos[p.ep] = p }
            } else {
                pos[p.ep] = p
                order.append(p.ep)
            }
        }
        let kept = order.compactMap { pos[$0] }.filter { p in histDel[p.film].map { $0 < p.t } ?? true }
        // Thứ tự lưu: cũ nhất trước (bản cũ không rõ lúc lưu — t = 0 — đứng đầu, giữ thứ tự cũ)
        let sorted = kept.enumerated().sorted { a, b in
            a.element.t != b.element.t ? a.element.t < b.element.t : a.offset < b.offset
        }.map { $0.element }
        out.pos = Array(sorted.suffix(maxPositions))

        // Phim theo dõi: hợp hai bên; số tập / số tập đã thấy lấy số lớn hơn; bỏ phim đã bỏ theo dõi sau lần theo dõi đó
        let followDel = tombstones(local.followDel, remote.followDel)
        out.followDel = followDel
        var fol: [String: SyncFollow] = [:]
        var forder: [String] = []
        for f in local.follows + remote.follows {
            guard var old = fol[f.film] else {
                fol[f.film] = f
                forder.append(f.film)
                continue
            }
            if f.count > old.count { old.latest = f.latest }
            old.count = max(old.count, f.count)
            old.seen = max(old.seen, f.seen)
            if f.t > old.t {
                old.t = f.t
                old.title = f.title
                if !f.poster.isEmpty { old.poster = f.poster }
            }
            fol[f.film] = old
        }
        out.follows = forder.compactMap { fol[$0] }
            .filter { f in followDel[f.film].map { $0 < f.t } ?? true }
            .enumerated()
            .sorted { a, b in a.element.t != b.element.t ? a.element.t > b.element.t : a.offset < b.offset }
            .map { $0.element }
        return out
    }

    /// Gộp danh sách trang: trang có mặt nếu lần thêm mới hơn lần bỏ (trang có sẵn từ bản cũ: lúc thêm = 0). Thứ tự: trang
    /// của máy này trước, trang mới của máy kia sau.
    static func sites(_ local: SyncSites, _ remote: SyncSites) -> SyncSites {
        let add = tombstones(local.add, remote.add)
        let del = tombstones(local.del, remote.del)
        var list: [String] = []
        for s in local.list + remote.list where !list.contains(s) {
            if let d = del[s], d >= (add[s] ?? 0) { continue }
            list.append(s)
        }
        if list.isEmpty { list = local.list } // không để trống danh sách
        return SyncSites(list: list, add: add, del: del)
    }
}
