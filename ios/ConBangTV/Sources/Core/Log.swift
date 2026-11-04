import Foundation

/// Nhật ký trong bộ nhớ (vòng 2 000 dòng): bản thử đọc qua cổng gỡ lỗi (/log), thay cho logcat của bản Android.
enum Log {
    private static let lock = NSLock()
    private static var lines: [(seq: Int, text: String)] = []
    private static var seq = 0
    private static let fmt: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss.SSS"
        return f
    }()

    static func d(_ tag: String, _ message: String) {
        let line = "\(fmt.string(from: Date())) \(tag): \(message)"
        lock.lock()
        seq += 1
        lines.append((seq, line))
        if lines.count > 2000 { lines.removeFirst(lines.count - 2000) }
        lock.unlock()
        #if DEBUG
        print(line)
        #endif
    }

    /// Các dòng có số thứ tự > since; trả kèm số thứ tự cuối để lần sau hỏi tiếp.
    static func since(_ since: Int) -> (last: Int, lines: [String]) {
        lock.lock()
        defer { lock.unlock() }
        return (seq, lines.filter { $0.seq > since }.map { $0.text })
    }
}
