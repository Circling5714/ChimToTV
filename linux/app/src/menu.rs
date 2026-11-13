//! Các mục menu bên trái (như bản TV / Windows) — mỗi mục mở một trang gốc.

use crate::app::{App, MenuEntry};
use crate::pages::{self, Page};
use cbtv::prefs;
use std::rc::Rc;

pub fn home_page() -> Rc<dyn Page> {
    pages::history::home()
}

pub fn search_page() -> Rc<dyn Page> {
    pages::placeholder::new("search", "Tìm kiếm")
}

/// Tên trang đang dùng ở đầu menu.
pub fn site_display(base: &str) -> String {
    cbtv::site::host_of(base).unwrap_or_else(|| base.to_string())
}

pub fn entries(a: &App) -> Vec<MenuEntry> {
    let mut v = Vec::new();
    let mut add = |key: &str, title: String, open: Rc<dyn Fn()>| v.push(MenuEntry { key: key.into(), title, open });
    add("home", "🏠  Trang chủ".into(), Rc::new(|| crate::app::app().show_root(home_page())));
    add("search", "🔍  Tìm kiếm".into(), Rc::new(|| crate::app::app().show_root(search_page())));
    add("history", "🕘  Xem tiếp / Lịch sử".into(), Rc::new(|| crate::app::app().show_root(pages::history::history())));
    let fresh = prefs::follows().iter().filter(|f| f.new_count() > 0).count();
    add(
        "follows",
        format!("★  Theo dõi{}", if fresh > 0 { format!("  ({fresh} mới)") } else { String::new() }),
        Rc::new(|| crate::app::app().show_root(pages::placeholder::new("follows", "Phim theo dõi"))),
    );
    for c in a.categories.borrow().iter() {
        let (t, u) = (c.title.clone(), c.url.clone());
        add(&format!("cat:{u}"), c.title.clone(), Rc::new(move || crate::app::app().show_root(pages::placeholder::new(&format!("cat:{u}"), &t))));
    }
    add("sites", "🌐  Đổi trang phim".into(), Rc::new(|| crate::app::app().show_root(pages::placeholder::sites())));
    add("sync", "⇄  Đồng bộ trong nhà".into(), Rc::new(|| crate::app::app().show_root(pages::placeholder::new("sync", "Đồng bộ trong nhà"))));
    add("settings", "⚙  Cài đặt".into(), Rc::new(|| crate::app::app().show_root(pages::placeholder::new("settings", "Cài đặt"))));
    v
}
