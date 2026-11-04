import Foundation

/// Chấm điểm từng server video theo những lần xem thật trên chính máy này (bản Android: PlayStats.kt — giữ y logic).
/// Giữ vài lần xem gần nhất cho mỗi trang + server, nên điểm tự đổi theo giờ cao điểm / nhà mạng.
enum PlayStats {
    private static let keep = 8

    /// level: 0 mượt, 1 hơi giật, 2 hay giật, 3 hay lỗi (không chạy được), -1 chưa đủ dữ liệu.
    struct Score {
        let level: Int
        let startSec: Int?
        let stallPct: Double

        var label: String? {
            switch level {
            case 0: return "mượt"
            case 1: return "hơi giật"
            case 2: return "hay giật"
            case 3: return "hay lỗi"
            default: return nil
            }
        }

        /// Mô tả ngắn cho nút server, vd "mượt, mở 4 s".
        var summary: String? {
            let parts = [label, startSec.map { "mở \($0) s" }].compactMap { $0 }
            return parts.isEmpty ? nil : parts.joined(separator: ", ")
        }
    }

    private static func key(_ host: String?, _ server: String) -> String { "\(host ?? "null")|\(server)" }

    /// startMs < 0: không đo được thời gian mở.
    static func record(_ host: String?, _ server: String, startMs: Int64, play: Int, stall: Int, stalls: Int, failed: Bool) {
        var all = Prefs.playStats
        let k = key(host, server)
        var list = all[k] ?? []
        list.append(Prefs.Stat(s: startMs, p: play, w: stall, n: stalls, x: failed))
        if list.count > keep { list.removeFirst(list.count - keep) }
        all[k] = list
        Prefs.playStats = all
    }

    static func score(_ host: String?, _ server: String) -> Score { scoreOf(Prefs.playStats[key(host, server)]) }

    static func scoreOf(_ list: [Prefs.Stat]?) -> Score {
        guard let list = list, !list.isEmpty else { return Score(level: -1, startSec: nil, stallPct: 0) }
        var play = 0, stall = 0, stalls = 0, fails = 0
        var starts: [Int64] = []
        for o in list {
            if o.x { fails += 1; continue }
            play += o.p
            stall += o.w
            stalls += o.n
            if o.s > 0 { starts.append(o.s) }
        }
        let watched = play + stall
        let start = starts.isEmpty ? nil : max(1, Int((Double(starts.reduce(0, +)) / Double(starts.count) / 1000).rounded()))
        return Score(level: level(sessions: list.count, fails: fails, play: play, stall: stall, stalls: stalls),
                     startSec: start, stallPct: watched > 0 ? Double(stall) / Double(watched) : 0)
    }

    /// Mức của một server từ tổng các lần xem gần đây. Số lần giật tính trên ít nhất 10 phút xem.
    static func level(sessions: Int, fails: Int, play: Int, stall: Int, stalls: Int) -> Int {
        let watched = play + stall
        let pct = watched > 0 ? Double(stall) / Double(watched) : 0
        let per10min = Double(stalls) * 600 / Double(max(watched, 600))
        if fails > 0 && fails * 2 >= sessions { return 3 }
        if watched < 60 { return -1 }
        if pct < 0.02 && per10min < 2 { return 0 }
        if pct < 0.06 && per10min < 5 { return 1 }
        return 2
    }

    /// Server nên mở cho một tập: giữ nguyên, trừ khi nó hay giật / hay lỗi mà có server cùng ngôn ngữ đã chấm tốt.
    static func choose(_ host: String?, _ servers: [Server], _ current: Server?) -> Server? {
        choose(servers, current) { score(host, $0.type) }
    }

    static func choose(_ servers: [Server], _ current: Server?, _ score: (Server) -> Score) -> Server? {
        guard let current = current, score(current).level >= 2 else { return current }
        let same = servers.filter { sameLang($0, current) }.map { ($0, score($0)) }
        if let best = same.filter({ (0...1).contains($0.1.level) })
            .min(by: { ($0.1.level, $0.1.stallPct) < ($1.1.level, $1.1.stallPct) }) {
            return best.0
        }
        if score(current).level < 3 { return current }
        let r = resolution(current.label)
        let candidates = same.filter { s, sc in
            s != current && sc.level != 3 && (sc.level >= 0 || r == nil || (resolution(s.label) ?? r!) <= r!)
        }
        let rank: (Score) -> Int = { $0.level < 0 ? 3 : $0.level }
        return candidates.min(by: { (rank($0.1), $0.1.stallPct) < (rank($1.1), $1.1.stallPct) })?.0 ?? current
    }

    /// Server nên thử thay cho server đang giật, cùng ngôn ngữ; không gợi ý server chưa thử mà nét hơn.
    static func alternative(_ host: String?, _ servers: [Server], _ current: Server) -> (Server, Score)? {
        alternative(servers, current) { score(host, $0.type) }
    }

    static func alternative(_ servers: [Server], _ current: Server, _ score: (Server) -> Score) -> (Server, Score)? {
        let r = resolution(current.label)
        let lower: (Server) -> Int = { s in (r != nil && (resolution(s.label) ?? r!) < r!) ? 1 : 0 }
        let rank: (Score) -> Int = { $0.level < 0 ? 2 : $0.level }
        return servers.filter { $0 != current && sameLang($0, current) }.map { ($0, score($0)) }
            .filter { s, sc in (0...1).contains(sc.level) || (sc.level < 0 && (r == nil || (resolution(s.label) ?? r!) <= r!)) }
            .min(by: { a, b in
                (rank(a.1), lower(a.0), a.1.stallPct) < (rank(b.1), lower(b.0), b.1.stallPct)
            })
    }

    /// Tên server bỏ dấu, chữ thường: "Thuyết minh 1080 V1" → "thuyet minh 1080 v1".
    private static func plain(_ label: String) -> String {
        label.folding(options: [.diacriticInsensitive], locale: nil).lowercased()
    }

    private static let tmWord = Rx(#"\btm\b"#)
    private static let resWord = Rx(#"\b(1080|720|480|360)p?\b"#)

    /// Ngôn ngữ ghi trong tên server; nil = không rõ (coi như cùng ngôn ngữ).
    static func lang(_ label: String) -> String? {
        let s = plain(label)
        if s.contains("thuyet minh") || tmWord.matches(s) { return "tm" }
        if s.contains("long tieng") { return "lt" }
        if s.contains("sub") { return "vs" }
        return nil
    }

    private static func sameLang(_ a: Server, _ b: Server) -> Bool {
        let x = lang(a.label), y = lang(b.label)
        return x == nil || y == nil || x == y
    }

    /// Độ phân giải ghi trong tên server (4K → 2160); nil = không rõ.
    static func resolution(_ label: String) -> Int? {
        let s = plain(label)
        if s.contains("4k") || s.contains("2160") { return 2160 }
        if s.contains("2k") || s.contains("1440") { return 1440 }
        return resWord.group(1, in: s).flatMap { Int($0) }
    }

    /// Điểm server tốt nhất của một trang, cho danh sách "Đổi trang phim", vd "video mượt".
    static func siteSummary(_ host: String?) -> String? {
        let prefix = "\(host ?? "null")|"
        let best = Prefs.playStats.filter { $0.key.hasPrefix(prefix) }.map { scoreOf($0.value) }
            .filter { $0.level >= 0 }.min { $0.level < $1.level }
        return best?.label.map { "video \($0)" }
    }
}

/// Theo dõi một lần phát (một server của một tập) từ trạng thái trình phát báo lên mỗi giây.
/// "Giật" = đang phát mà trình phát đứng chờ tải; chờ tải ngay sau lệnh tua là bình thường nên không tính.
final class PlaySession {
    private let host: String?
    private let server: String
    private let openedAt: Int64
    private var firstPlay: Int64 = -1
    private var play = 0, stall = 0, stalls = 0
    private var savedPlay = 0, savedStall = 0, savedStalls = 0
    private var startSaved = false, failSaved = false
    private var stalling = false
    private var stallRun = 0
    private var afterSeek = false
    private var lastPaused = false
    private var inAd = false
    private var recent: [Int64] = []
    private var frames0 = -1, dropped0 = 0, frames = 0, dropped = 0

    init(host: String?, server: String, openedAt: Int64) {
        self.host = host
        self.server = server
        self.openedAt = openedAt
    }

    /// Vừa ra lệnh tua: trình phát chờ tải ngay sau đó là bình thường, không tính là giật.
    func seeked() { afterSeek = true }

    /// Trình phát đang chạy quảng cáo: chưa phải phim, rời đi lúc này không tính là server lỗi.
    func ad() { inAd = true }

    func update(_ now: Int64, paused: Bool, waiting: Bool, ready: Bool, droppedFrames: Int, totalFrames: Int) {
        inAd = false
        lastPaused = paused
        if totalFrames > 0 {
            if frames0 < 0 || totalFrames < frames0 {
                frames0 = totalFrames
                dropped0 = droppedFrames
            }
            frames = totalFrames - frames0
            dropped = droppedFrames - dropped0
        }
        if firstPlay < 0 {
            if ready && !paused && !waiting { firstPlay = now - openedAt }
            return
        }
        if paused {
            stalling = false
            stallRun = 0
        } else if waiting {
            if !afterSeek {
                if !stalling {
                    stalling = true
                    stalls += 1
                    recent.append(now)
                }
                stall += 1
                stallRun += 1
            }
        } else {
            play += 1
            stalling = false
            stallRun = 0
            afterSeek = false
        }
    }

    /// Giật nhiều: ≥ 3 lần trong 3 phút, hoặc đứng hình liền ≥ 10 giây.
    func struggling(_ now: Int64) -> Bool {
        recent.removeAll { now - $0 > 180_000 }
        return recent.count >= 3 || stallRun >= 10
    }

    /// Mô tả lần xem này cho menu, vd "mở 4 s · giật 2 lần (9 s)".
    func describe() -> String? {
        if firstPlay < 0 { return nil }
        var parts = ["mở \(max(1, firstPlay / 1000)) s"]
        parts.append(stalls == 0 ? "chưa giật lần nào" : "giật \(stalls) lần (\(stall) s)")
        if frames >= 300 && dropped * 10 >= frames {
            parts.append("máy rớt \(dropped * 100 / frames)% khung hình (máy xử lý không kịp, đổi server không đỡ)")
        }
        return parts.joined(separator: " · ")
    }

    /// Trình phát báo lỗi hẳn → tính là lỗi ngay.
    func fail() {
        if firstPlay >= 0 || failSaved { return }
        failSaved = true
        PlayStats.record(host, server, startMs: -1, play: 0, stall: 0, stalls: 0, failed: true)
    }

    /// Ghi phần chưa ghi vào thống kê (khi đổi server / tập, hoặc rời màn hình).
    func save(_ now: Int64) {
        if firstPlay < 0 {
            // Chờ ≥ 20 giây mà video vẫn chưa chạy (không do người xem dừng, không đang quảng cáo) → tính là lỗi
            if !failSaved && !lastPaused && !inAd && now - openedAt >= 20_000 {
                failSaved = true
                PlayStats.record(host, server, startMs: -1, play: 0, stall: 0, stalls: 0, failed: true)
            }
            return
        }
        let p = play - savedPlay, w = stall - savedStall
        if p + w < 20 { return }
        let start: Int64 = (startSaved || firstPlay > 60_000) ? -1 : firstPlay
        PlayStats.record(host, server, startMs: start, play: p, stall: w, stalls: stalls - savedStalls, failed: false)
        startSaved = true
        savedPlay = play
        savedStall = stall
        savedStalls = stalls
    }
}
