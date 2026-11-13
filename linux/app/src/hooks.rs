//! Chỗ nối trang / trình phát với trang chủ tổng hợp và phim liên quan (cbtv::home_feed, cbtv::related — chuyển sau).

use crate::pages::detail::RelatedRow;
use cbtv::FilmDetail;

/// Trang chủ tổng hợp: người xem đã mở phim → hết báo «tập mới» tới khi có tập sau.
pub fn mark_seen(_d: &FilmDetail) {}

/// Id các hàng trang chủ đang bật ("new-eps", "continue", …).
pub fn rows_on() -> std::collections::HashSet<String> {
    ["new-eps", "continue"].iter().map(|s| s.to_string()).collect()
}

/// Cài đặt các hàng lúc tải (đổi thì tải lại trang chủ).
pub fn rows_signature() -> String {
    let rows = cbtv::prefs::home_rows().join(",");
    let custom = cbtv::prefs::home_custom_raw().map(|v| v.to_string()).unwrap_or_default();
    let mut off: Vec<String> = cbtv::prefs::home_src_off().into_iter().collect();
    off.sort();
    format!("{rows}|{custom}|{}", off.join(","))
}

/// Mục «Điểm IMDb cao» (cbtv::imdb_top — chuyển sau).
pub fn imdb_top_page() -> std::rc::Rc<dyn crate::pages::Page> {
    crate::pages::placeholder::new("imdb", "Điểm IMDb cao")
}

/// Danh mục xếp được theo điểm IMDb (danh sách API kiểu KKPhim).
pub fn imdb_can_rank(_url: &str) -> bool {
    false
}

/// Danh mục xếp theo điểm IMDb (≈ 320 phim mới cập nhật của mục).
pub fn imdb_for_list(url: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = cbtv::Result<cbtv::FilmPage>> + Send>> {
    Box::pin(async move { cbtv::site::list(&url).await })
}

/// Các hàng phim liên quan («Cùng bộ», «Cùng thể loại») của một phim; done chạy trên luồng giao diện.
pub fn related(_title: String, _url: String, done: impl FnOnce(Vec<RelatedRow>) + 'static) {
    done(Vec::new());
}
