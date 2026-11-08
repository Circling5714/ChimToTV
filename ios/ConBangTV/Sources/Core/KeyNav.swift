import Foundation

/// Di chuyển ô đang chọn bằng phím mũi tên trên lưới chia nhóm (lưới phim trang chủ: "Xem tiếp", "Có tập mới", các
/// nhóm phim của trang; danh sách tập). Mỗi nhóm xếp `cols` ô một hàng; ▲ ▼ đi theo cột, qua hết nhóm thì sang hàng
/// gần nhất của nhóm trên / dưới (giữ cột, nhóm ngắn thì về ô cuối).
enum KeyNav {
    enum Dir { case left, right, up, down }

    /// strips[g] = true: nhóm g là một dải vuốt ngang (cả nhóm một dòng — trang chủ tổng hợp trên iPhone dọc). ▲ ▼ giữa
    /// hai dải giữ thứ tự ô (dải dưới tự cuộn tới ô đó).
    static func move(_ groups: [[String]], cols: Int, from current: String?, _ dir: Dir, strips: [Bool] = []) -> String? {
        let kept = groups.indices.filter { !groups[$0].isEmpty }
        let line = kept.map { strips.indices.contains($0) && strips[$0] }
        let groups = kept.map { groups[$0] }
        guard let first = groups.first?.first else { return nil }
        let cols = max(1, cols)
        guard let cur = current,
              let g = groups.firstIndex(where: { $0.contains(cur) }),
              let i = groups[g].firstIndex(of: cur) else { return first }
        let items = groups[g]
        let w = line[g] ? items.count : cols // số ô một hàng của nhóm này
        let col = line[g] ? i : i % cols
        switch dir {
        case .left:
            if i > 0 { return items[i - 1] }
            return g > 0 ? groups[g - 1].last : cur
        case .right:
            if i + 1 < items.count { return items[i + 1] }
            return g + 1 < groups.count ? groups[g + 1].first : cur
        case .down:
            if i + w < items.count { return items[i + w] }
            // Hàng cuối chưa đủ ô: còn hàng dưới thì xuống ô cuối
            if i / w < (items.count - 1) / w { return items.last }
            guard g + 1 < groups.count else { return cur }
            let next = groups[g + 1]
            return next[min(line[g + 1] ? col : min(col, cols - 1), next.count - 1)]
        case .up:
            if i >= w { return items[i - w] }
            guard g > 0 else { return cur }
            let prev = groups[g - 1]
            if line[g - 1] { return prev[min(col, prev.count - 1)] }
            let lastRow = (prev.count - 1) / cols * cols
            return prev[min(lastRow + min(col, cols - 1), prev.count - 1)]
        }
    }

    /// Số cột của LazyVGrid `.adaptive(minimum:)` trên bề rộng `width` (đã trừ lề), khoảng cách `spacing`.
    static func columns(width: Double, minimum: Double, spacing: Double) -> Int {
        max(1, Int((width + spacing) / (minimum + spacing)))
    }
}
