import Foundation

/// Hai nhãn ở đáy ảnh bìa của thẻ phim: nhãn tập (sát mép trái) và điểm IMDb (sát mép phải). Đủ chỗ thì nằm chung một hàng;
/// thẻ hẹp hoặc nhãn tập dài («Hoàn Tất (16/16)», iPhone cầm dọc) thì nhãn IMDb lên hàng trên, vẫn sát mép phải. Trước 1.16.1
/// hai nhãn là hai lớp phủ riêng nên đè lên nhau. Cùng quy tắc với BadgeRow của bản TV (Ui.kt).
enum BadgeRow {
    /// Hai nhãn rộng `left` và `right` (0 = không có nhãn đó) có phải xếp hai hàng trong bề ngang `width` không.
    static func needsTwoRows(width: CGFloat, left: CGFloat, right: CGFloat, gap: CGFloat) -> Bool {
        left > 0 && right > 0 && left + gap + right > width
    }
}
