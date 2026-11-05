import Foundation

/// Giới hạn giờ xem theo người xem (Cài đặt → Người xem → Giới hạn giờ xem…): số phút xem mỗi ngày và giờ đi ngủ (từ
/// giờ đó tới 6 giờ sáng không xem được). Đếm thời gian phim thật sự chạy của người đang xem; hết giờ thì trình phát
/// dừng phim, xem thêm phải có mã PIN phụ huynh. Bản Android: WatchLimit.kt (cùng cách tính, cùng tên khoá).
enum WatchLimit {
    /// Hết giờ đi ngủ lúc 6:00 sáng.
    static let morning = 6 * 60

    /// Lựa chọn số phút mỗi ngày (0 = không giới hạn) và giờ đi ngủ (phút trong ngày, -1 = không).
    static let daily = [0, 30, 45, 60, 90, 120, 180]
    static let bedtimes = [-1, 19 * 60 + 30, 20 * 60, 20 * 60 + 30, 21 * 60, 21 * 60 + 30, 22 * 60, 23 * 60]

    /// Cho xem thêm (phút) khi hết giờ.
    static let grants = [15, 30, 60]

    enum Block { case daily, bedtime }

    /// Đồng hồ (ms; test thay được).
    static var clock: () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) }

    /// Giây xem chưa ghi xuống (ghi mỗi 30 giây và khi rời trình phát).
    private static var pending = 0.0
    private static var lastTick: Int64 = 0

    private static let dayFormat: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd"
        return f
    }()

    static func day(_ now: Int64) -> String { dayFormat.string(from: Date(timeIntervalSince1970: Double(now) / 1000)) }

    private static func minuteOfDay(_ now: Int64) -> Int {
        let c = Calendar.current.dateComponents([.hour, .minute], from: Date(timeIntervalSince1970: Double(now) / 1000))
        return (c.hour ?? 0) * 60 + (c.minute ?? 0)
    }

    /// Giây người đang xem đã xem hôm nay (kể cả phần chưa ghi).
    static func usedToday(_ now: Int64 = clock()) -> Int {
        (Prefs.watchDay == day(now) ? Prefs.watchSeconds : 0) + Int(pending)
    }

    /// Giây còn được xem hôm nay; nil = không giới hạn theo ngày.
    static func remaining(_ now: Int64 = clock()) -> Int? {
        let limit = Prefs.dailyLimit()
        guard limit > 0 else { return nil }
        let extra = Prefs.watchDay == day(now) ? Prefs.watchExtra : 0
        return (limit + extra) * 60 - usedToday(now)
    }

    /// Người đang xem có bị chặn không (giờ đi ngủ trước, rồi hết phút trong ngày); nil = xem được.
    static func block(_ now: Int64 = clock()) -> Block? {
        if now < Prefs.unblockUntil { return nil }
        let bed = Prefs.bedtime()
        if bed >= 0 {
            let m = minuteOfDay(now)
            if m >= bed || m < morning { return .bedtime }
        }
        guard let r = remaining(now) else { return nil }
        return r <= 0 ? .daily : nil
    }

    /// Gọi mỗi lần đọc trạng thái trình phát: phim đang chạy thì cộng thời gian từ lần trước (bỏ khoảng hở > 3 giây).
    static func tick(playing: Bool, now: Int64 = clock()) {
        let dt = Double(now - lastTick) / 1000
        lastTick = now
        guard playing, dt > 0, dt <= 3 else { return }
        pending += dt
        if pending >= 30 { flush(now) }
    }

    /// Ghi phần đã đếm (rời trình phát, đổi người xem, sang ngày mới).
    static func flush(_ now: Int64 = clock()) {
        let d = day(now)
        if Prefs.watchDay != d {
            Prefs.watchDay = d
            Prefs.watchSeconds = 0
            Prefs.watchExtra = 0
        }
        let whole = Int(pending)
        if whole > 0 { Prefs.watchSeconds += whole }
        pending -= Double(whole)
    }

    /// Bỏ phần đếm chưa ghi (đổi người xem — đã flush trước).
    static func reset() {
        pending = 0
        lastTick = 0
    }

    /// Cho xem thêm (đã nhập mã phụ huynh): đang giờ đi ngủ → mở khoá thêm từng ấy phút; hết phút trong ngày → cộng thêm.
    static func grant(_ minutes: Int, now: Int64 = clock()) {
        flush(now)
        if block(now) == .bedtime {
            Prefs.unblockUntil = now + Int64(minutes) * 60_000
        } else {
            Prefs.watchExtra += minutes
        }
    }

    /// Không giới hạn tới 6 giờ sáng (hôm nay nếu đang trước 6 giờ, không thì sáng mai).
    static func grantUntilMorning(_ now: Int64 = clock()) {
        let date = Date(timeIntervalSince1970: Double(now) / 1000)
        var six = Calendar.current.date(bySettingHour: morning / 60, minute: 0, second: 0, of: date) ?? date
        if six <= date { six = Calendar.current.date(byAdding: .day, value: 1, to: six) ?? six }
        Prefs.unblockUntil = Int64(six.timeIntervalSince1970 * 1000)
    }

    /// "30 phút", "1 giờ", "1 giờ 30 phút", "Không giới hạn".
    static func dailyLabel(_ minutes: Int) -> String {
        if minutes <= 0 { return "Không giới hạn" }
        if minutes < 60 { return "\(minutes) phút" }
        if minutes % 60 == 0 { return "\(minutes / 60) giờ" }
        return "\(minutes / 60) giờ \(minutes % 60) phút"
    }

    /// "21:00" hoặc "Không".
    static func bedtimeLabel(_ minute: Int) -> String {
        minute < 0 ? "Không" : String(format: "%d:%02d", minute / 60, minute % 60)
    }

    /// Giây còn lại → "25 phút" (làm tròn lên, tối thiểu 1 phút).
    static func minutesText(_ seconds: Int) -> String { dailyLabel(max(1, (seconds + 59) / 60)) }

    /// Tóm tắt giới hạn của một người xem, vd "1 giờ/ngày, ngủ lúc 21:00".
    static func summary(_ id: String) -> String {
        var parts: [String] = []
        let d = Prefs.dailyLimit(id), b = Prefs.bedtime(id)
        if d > 0 { parts.append("\(dailyLabel(d))/ngày") }
        if b >= 0 { parts.append("ngủ lúc \(bedtimeLabel(b))") }
        return parts.isEmpty ? "không giới hạn" : parts.joined(separator: ", ")
    }
}
