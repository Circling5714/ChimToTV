//! Trang chủ. Kiểu theo trang: Xem tiếp, phim theo dõi có tập mới, rồi các nhóm phim của trang đang dùng. Kiểu tổng hợp
//! (mặc định): phim từ mọi nguồn, gộp phim trùng, xếp hàng theo lịch sử xem (home_mix.rs). Bản Windows: Pages.cs HomePage.

use super::lists::film_card;
use super::{Page, PageBase};
use crate::app::app;
use crate::card::{card, CardOpts};
use crate::ui;
use cbtv::{prefs, Film, Home};
use std::cell::RefCell;
use std::rc::Rc;

pub struct HomePage {
    base: PageBase,
    home: RefCell<Option<Home>>,
    site: RefCell<String>,
    mix: std::cell::Cell<bool>,
    kids: std::cell::Cell<bool>,
    rows: RefCell<String>,
}

pub fn new() -> Rc<dyn Page> {
    if prefs::home_mix_on() {
        return super::home_mix::new();
    }
    new_site()
}

/// Trang chủ kiểu theo trang đang dùng.
pub fn new_site() -> Rc<dyn Page> {
    Rc::new(HomePage {
        base: PageBase::new("home"),
        home: RefCell::new(None),
        site: RefCell::new(String::new()),
        mix: std::cell::Cell::new(false),
        kids: std::cell::Cell::new(false),
        rows: RefCell::new(String::new()),
    })
}

impl HomePage {
    fn build(self: &Rc<Self>) {
        let Some(home) = self.home.borrow().clone() else { return };
        let b = &self.base;
        b.clear();
        let on = crate::hooks::rows_on();
        let history: Vec<_> = if on.contains("continue") { prefs::history().into_iter().take(12).collect() } else { Vec::new() };
        if !history.is_empty() {
            let cards = history
                .iter()
                .map(|h| {
                    let (fu, eu) = (h.film_url.clone(), h.ep_url.clone());
                    card(
                        Film::new(h.film_url.clone(), h.title.clone(), h.poster.clone(), None),
                        Rc::new(move || app().play(&fu, &eu, true)),
                        CardOpts { sub: Some(h.ep_title.clone()), ..Default::default() },
                    )
                })
                .collect();
            b.add(&ui::section("Xem tiếp", Some("Enter: xem tiếp ngay"), cards));
        }
        let fresh: Vec<_> = if on.contains("new-eps") { prefs::follows().into_iter().filter(|f| f.new_count() > 0).collect() } else { Vec::new() };
        if !fresh.is_empty() {
            let cards = fresh
                .iter()
                .map(|f| {
                    let film = Film::new(f.film_url.clone(), f.title.clone(), f.poster.clone(), cbtv::util::non_empty(Some(&f.latest)));
                    let ff = film.clone();
                    card(film, Rc::new(move || super::detail::open_film(&ff)), CardOpts { fresh: Some(format!("+{}", f.new_count())), ..Default::default() })
                })
                .collect();
            b.add(&ui::section("Phim theo dõi có tập mới", None, cards));
        }
        for s in &home.sections {
            b.add(&ui::section(&s.title, None, s.films.iter().map(film_card).collect()));
        }
        if home.sections.is_empty() {
            let t = ui::label("Trang không có phim nào", &["big"], true);
            t.set_margin_top(crate::css::px(30.0));
            b.add(&t);
            b.add(&ui::dim("Có thể trang đã đổi giao diện, hoặc nhà mạng chặn trang này (trang chặn không có phim). Thử trang khác:"));
            b.add(&ui::row(&[ui::pill("🌐  Đổi trang phim", || app().show_root(super::sites::new())).upcast()]));
        }
    }
}

use gtk::prelude::*;

impl Page for HomePage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.mix.set(prefs::home_mix_on());
        self.kids.set(prefs::kids());
        *self.rows.borrow_mut() = crate::hooks::rows_signature();
        let base = prefs::base_url();
        *self.site.borrow_mut() = base.clone();
        self.base.loading(&format!("Đang tải {}…", cbtv::site::host_of(&base).unwrap_or_default()));
        let t = self.base.next_token();
        let me = self.clone();
        let b2 = base.clone();
        ui::spawn(async move { cbtv::site::home_of(Some(&b2)).await }, move |r| {
            if t != me.base.token() {
                return;
            }
            match r {
                Ok(home) => {
                    if let Some(m) = home.moved_to.as_deref().and_then(prefs::normalize_url) {
                        if m != base {
                            prefs::replace_site(&base, &m);
                            app().toast(&format!("Trang đã chuyển sang {} — app đã đổi theo.", cbtv::site::host_of(&m).unwrap_or_default()));
                            *me.site.borrow_mut() = m;
                        }
                    }
                    app().set_categories(home.categories.clone());
                    *me.home.borrow_mut() = Some(home);
                    me.build();
                }
                Err(e) => {
                    let m2 = me.clone();
                    me.base.show_error(&cbtv::util::friendly(&e), Rc::new(move || m2.clone().start()));
                }
            }
        });
    }

    fn returned(self: Rc<Self>) {
        // Đổi kiểu trang chủ / hồ sơ trẻ em / trang phim trong Cài đặt → tải lại
        if prefs::home_mix_on() != self.mix.get() || prefs::kids() != self.kids.get() || prefs::base_url() != *self.site.borrow()
            || crate::hooks::rows_signature() != *self.rows.borrow()
        {
            if prefs::home_mix_on() {
                app().replace(super::home_mix::new());
                return;
            }
            self.start();
            return;
        }
        self.build();
    }
}
