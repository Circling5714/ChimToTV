//! Các mục menu bên trái (như bản TV / Windows MainWindow.MenuEntries) — mỗi mục mở một trang gốc.

use crate::app::{app, App, MenuEntry};
use crate::pages::{self, Page};
use cbtv::prefs;
use std::rc::Rc;

pub fn home_page() -> Rc<dyn Page> {
    pages::home::new()
}

pub fn search_page() -> Rc<dyn Page> {
    pages::lists::search()
}

/// Tên trang đang dùng ở đầu menu.
pub fn site_display(base: &str) -> String {
    let host = cbtv::site::host_of(base).unwrap_or_else(|| base.to_string());
    cbtv::site::display_name(&host)
}

pub fn entries(a: &App) -> Vec<MenuEntry> {
    let mut v = Vec::new();
    let mut add = |key: &str, title: String, open: Rc<dyn Fn()>| v.push(MenuEntry { key: key.into(), title, open });
    add("home", "🏠  Trang chủ".into(), Rc::new(|| app().show_root(home_page())));
    add("search", "🔍  Tìm kiếm".into(), Rc::new(|| app().show_root(search_page())));
    add("history", "🕘  Xem tiếp / Lịch sử".into(), Rc::new(|| app().show_root(pages::history::history())));
    let fresh = prefs::follows().iter().filter(|f| f.new_count() > 0).count();
    add(
        "follows",
        format!("★  Theo dõi{}", if fresh > 0 { format!("  ({fresh} mới)") } else { String::new() }),
        Rc::new(|| app().show_root(pages::lists::follows())),
    );
    add("imdb", "⭐  Điểm IMDb cao".into(), Rc::new(|| app().show_root(crate::hooks::imdb_top_page())));
    for c in a.categories.borrow().iter() {
        let (t, u) = (c.title.clone(), c.url.clone());
        let key = format!("cat:{u}");
        let k2 = key.clone();
        add(&key, c.title.clone(), Rc::new(move || app().show_root(pages::lists::list(&t, &u, &k2))));
    }
    let b = prefs::base_url();
    if cbtv::next_site::is_known(&b) || cbtv::api_site::is_known(&b) {
        add("genres", "Thể loại…".into(), Rc::new(|| app().show_root(pages::lists::filter(true))));
        add("countries", "Quốc gia…".into(), Rc::new(|| app().show_root(pages::lists::filter(false))));
    }
    add("sites", "🌐  Đổi trang phim".into(), Rc::new(|| app().show_root(pages::sites::new())));
    add("sync", "⇄  Đồng bộ trong nhà".into(), Rc::new(|| app().show_root(pages::sync_page::new())));
    add("settings", "⚙  Cài đặt".into(), Rc::new(|| app().show_root(pages::settings::new())));
    v
}
