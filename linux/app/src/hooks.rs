//! Chỗ nối các trang / trình phát với lõi trang chủ tổng hợp (home_feed), phim liên quan (related), điểm IMDb (imdb_top),
//! DNS mã hoá, proxy.

use crate::pages::detail::RelatedRow;
use crate::pages::Page;
use cbtv::{home_feed as hf, prefs, FilmDetail, FilmPage, Result};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// Trang chủ tổng hợp: người xem đã mở phim → hết báo «tập mới» tới khi có tập sau.
pub fn mark_seen(d: &FilmDetail) {
    hf::mark_seen(d);
}

/// Các hàng phim liên quan («Cùng bộ», «Cùng thể loại») của một phim; done chạy trên luồng giao diện.
pub fn related(title: String, url: String, done: impl FnOnce(Vec<RelatedRow>) + 'static) {
    crate::ui::spawn(async move { cbtv::related::load(&title, &url).await }, move |rows| {
        done(rows.into_iter().map(|r| RelatedRow { title: r.title, films: r.films }).collect());
    });
}

/// Mục «Điểm IMDb cao» (các hàng Phim lẻ, Phim bộ, … xếp theo điểm có trọng số).
pub fn imdb_top_page() -> Rc<dyn Page> {
    let loader: crate::pages::lists::Loader = Rc::new(|| {
        let kids = prefs::kids();
        Box::pin(async move { cbtv::imdb_top::page(kids).await })
    });
    crate::pages::lists::list_with("Điểm IMDb cao", "imdb:top", "imdb", Some(loader))
}

/// Danh mục xếp được theo điểm IMDb (danh sách API kiểu KKPhim).
pub fn imdb_can_rank(url: &str) -> bool {
    cbtv::imdb_top::can_rank(url)
}

/// Danh mục xếp theo điểm IMDb (≈ 320 phim mới cập nhật của mục).
pub fn imdb_for_list(url: String) -> Pin<Box<dyn Future<Output = Result<FilmPage>> + Send>> {
    Box::pin(async move { cbtv::imdb_top::for_list(&url).await })
}

/// Cài đặt → «Các hàng trên trang chủ…».
pub fn home_rows_page() -> Rc<dyn Page> {
    crate::pages::home_rows::rows()
}

/// Cài đặt → «Nguồn cho trang chủ…».
pub fn home_sources_page() -> Rc<dyn Page> {
    crate::pages::home_rows::sources()
}

/// Chữ trên nút «Nguồn cho trang chủ: n / m…».
pub fn sources_label() -> String {
    let ids = hf::all_sources().ids();
    let off = prefs::home_src_off();
    let on = ids.iter().filter(|x| !off.contains(x.as_str())).count();
    format!("Nguồn cho trang chủ: {} / {}…", if on > 0 { on } else { ids.len() }, ids.len())
}

/// Thứ tự máy chủ DNS mã hoá («NextDNS → Cloudflare → Google»).
pub fn doh_providers() -> String {
    cbtv::doh::providers()
}

/// Chữ trên nút proxy trong Cài đặt.
pub fn proxy_label() -> String {
    if let Some(c) = cbtv::shadowsocks::config() {
        format!("Proxy Shadowsocks: {}:{}", c.host, c.port)
    } else if let Some(c) = cbtv::net::proxy() {
        format!("Proxy SOCKS5: {}:{}", c.host, c.port)
    } else {
        "Proxy: tắt".into()
    }
}

pub fn proxy_page() -> Rc<dyn Page> {
    crate::pages::proxy::new()
}

/// Id các hàng trang chủ đang bật ("new-eps", "continue", …).
pub fn rows_on() -> std::collections::HashSet<String> {
    hf::row_prefs().into_iter().filter(|x| x.1).map(|x| x.0).collect()
}

/// Cài đặt các hàng lúc tải (thứ tự / bật tắt + hàng tự thêm + nguồn tắt): đổi thì tải lại trang chủ.
pub fn rows_signature() -> String {
    let rows = prefs::home_rows().join(",");
    let custom: Vec<String> =
        hf::home_custom().iter().map(|c| format!("{};{};{};{};{};{}", c.id, c.topic.clone().unwrap_or_default(), c.kind, c.country, c.genre, c.lang)).collect();
    let mut off: Vec<String> = prefs::home_src_off().into_iter().collect();
    off.sort();
    format!("{rows}|{}|{}", custom.join(","), off.join(","))
}
