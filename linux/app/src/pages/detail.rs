//! Chi tiết phim: ảnh, thông tin, nút Xem tiếp / Xem từ đầu / Theo dõi, các bản (Vietsub / Thuyết Minh), danh sách tập,
//! phim liên quan. Bản Windows: UI/DetailPage.cs.

use super::{Page, PageBase};
use crate::app::app;
use crate::card::{card, poster, CardOpts};
use crate::css::px;
use crate::keynav;
use crate::ui;
use cbtv::util::format_time;
use cbtv::{prefs, EpisodeGroup, Film, FilmDetail};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Một nguồn có phim này (mở từ trang chủ tổng hợp) — có từ 2 nguồn thì có nút đổi nguồn.
#[derive(Clone, Debug)]
pub struct Link {
    pub url: String,
    pub title: String,
    pub poster: Option<String>,
    pub badge: Option<String>,
}

/// Một hàng phim liên quan («Cùng bộ», «Cùng thể loại»).
pub struct RelatedRow {
    pub title: String,
    pub films: Vec<Film>,
}

pub struct DetailPage {
    base: PageBase,
    url: String,
    hint: Option<Film>,
    links: Vec<Link>,
    d: RefCell<Option<FilmDetail>>,
    group: RefCell<Option<EpisodeGroup>>,
    related: RefCell<Option<Rc<Vec<RelatedRow>>>>,
    related_loading: std::cell::Cell<bool>,
    buttons: RefCell<Option<gtk::FlowBox>>,
    related_panel: RefCell<Option<gtk::Box>>,
}

pub fn new(url: &str, hint: Option<Film>, links: Vec<Link>) -> Rc<dyn Page> {
    let menu = app().current().map(|p| p.base().menu_key.borrow().clone()).unwrap_or_default();
    Rc::new(DetailPage {
        base: PageBase::new(&menu),
        url: url.to_string(),
        hint,
        links,
        d: RefCell::new(None),
        group: RefCell::new(None),
        related: RefCell::new(None),
        related_loading: std::cell::Cell::new(false),
        buttons: RefCell::new(None),
        related_panel: RefCell::new(None),
    })
}

/// Mở chi tiết phim.
pub fn open_film(f: &Film) {
    app().push(new(&f.url, Some(f.clone()), Vec::new()));
}

/// Phim lẻ (một bản, một tập): không cần ô tập — nút Xem là đủ, hàng phim liên quan lên ngay dưới.
fn is_movie(d: &FilmDetail) -> bool {
    d.groups.len() == 1 && d.episode_count() == 1
}

fn source_name(url: &str) -> String {
    match cbtv::site::host_of(url) {
        Some(h) => cbtv::site::tag(&h, &prefs::sites()),
        None => url.to_string(),
    }
}

impl DetailPage {
    fn loaded(self: &Rc<Self>, d: FilmDetail) {
        if prefs::follow_of(&d.url).is_some() {
            if let Some(last) = d.latest_episode() {
                prefs::update_follow(&d.url, d.episode_count() as i64, &last.title, false);
            }
        }
        crate::hooks::mark_seen(&d); // trang chủ tổng hợp: hết báo «tập mới» tới khi có tập sau
        let h = prefs::history_for(&d.url);
        let g = h.as_ref().and_then(|h| d.group_of(&h.ep_url).cloned()).or_else(|| d.groups.first().cloned());
        *self.group.borrow_mut() = g;
        *self.d.borrow_mut() = Some(d);
        self.build(true);
    }

    fn build(self: &Rc<Self>, focus: bool) {
        let Some(d) = self.d.borrow().clone() else { return };
        let b = &self.base;
        b.clear();
        let top = ui::hbox(30.0);
        top.set_margin_bottom(px(20.0));
        top.pack_start(&poster(d.poster.as_deref().or(self.hint.as_ref().and_then(|h| h.poster.as_deref())), 230.0, 345.0), false, false, 0);
        let info = ui::vbox(0.0);
        info.set_size_request(-1, -1);
        let title = ui::label(&d.title, &["big"], true);
        info.pack_start(&title, false, false, 0);
        if let Some(o) = &d.other_title {
            let l = ui::dim(o);
            l.set_margin_top(px(4.0));
            info.pack_start(&l, false, false, 0);
        }
        if let Some(i) = &d.info {
            let l = ui::dim(i);
            l.set_margin_top(px(10.0));
            info.pack_start(&l, false, false, 0);
        }
        let host = ui::dim(&cbtv::site::host_of(&d.url).unwrap_or_default());
        host.set_margin_top(px(6.0));
        info.pack_start(&host, false, false, 0);

        let buttons = ui::flow(10.0, 10.0);
        buttons.set_margin_top(px(18.0));
        let movie = is_movie(&d);
        let mut main: Option<gtk::Button> = None;
        let h = prefs::history_for(&d.url);
        let eps = self.group.borrow().as_ref().map(|g| g.episodes.clone()).unwrap_or_default();
        if let Some(h) = h.as_ref().filter(|h| d.group_of(&h.ep_url).is_some()) {
            let pos = prefs::position_of(&h.ep_url);
            let label = if prefs::finished(&h.ep_url) {
                format!("▶  Xem tiếp (sau {})", h.ep_title)
            } else {
                format!("▶  Xem tiếp {}{}", h.ep_title, if pos > 0 { format!(" ({})", format_time(pos as f64)) } else { String::new() })
            };
            let (fu, eu) = (d.url.clone(), h.ep_url.clone());
            let m = ui::pill_tag(&label, Some("active"), move || app().play(&fu, &eu, true));
            ui::add_flow(&buttons, m.upcast_ref());
            main = Some(m);
        }
        if !eps.is_empty() {
            let (fu, eu) = (d.url.clone(), eps[0].url.clone());
            let text = if eps.len() == 1 { "▶  Xem phim".to_string() } else { format!("▶  Xem từ {}", eps[0].title) };
            let first = ui::pill_tag(&text, main.is_none().then_some("active"), move || app().play(&fu, &eu, false));
            ui::add_flow(&buttons, first.upcast_ref());
            if main.is_none() {
                main = Some(first);
            }
            if eps.len() > 2 {
                let last = eps.last().unwrap().clone();
                let fu = d.url.clone();
                ui::add_flow(&buttons, ui::pill(&format!("Mới nhất: {}", last.title), move || app().play(&fu, &last.url, false)).upcast_ref());
            }
        }
        // Phim có ở nhiều nguồn (trang chủ tổng hợp): «Nguồn: … ▾» → chọn nguồn khác (mở lại trang phim)
        if self.links.len() >= 2 {
            let me = self.clone();
            ui::add_flow(&buttons, ui::pill(&format!("⇄  Nguồn: {} ▾", source_name(&self.url)), move || me.pick_source()).upcast_ref());
        }
        let followed = prefs::follow_of(&d.url).is_some();
        let me = self.clone();
        let d2 = d.clone();
        ui::add_flow(
            &buttons,
            ui::pill(if followed { "★  Đang theo dõi" } else { "☆  Theo dõi" }, move || {
                if prefs::follow_of(&d2.url).is_some() {
                    prefs::unfollow(&d2.url);
                    app().toast("Đã bỏ theo dõi.");
                } else {
                    prefs::follow_film(&d2);
                    app().toast("Đã theo dõi — có tập mới sẽ hiện ở Trang chủ và mục Theo dõi.");
                }
                app().rebuild_menu();
                crate::startup::sync_soon(0);
                me.build(false);
            })
            .upcast_ref(),
        );
        if !movie && self.related.borrow().as_ref().map(|r| !r.is_empty()).unwrap_or(false) {
            ui::add_flow(&buttons, self.related_button().upcast_ref());
        }
        let u = d.url.clone();
        ui::add_flow(&buttons, ui::pill("↗  Mở trên trình duyệt", move || ui::open_browser(&u)).upcast_ref());
        info.pack_start(&buttons, false, false, 0);
        *self.buttons.borrow_mut() = Some(buttons);
        if let Some(desc) = &d.description {
            let l = ui::dim(desc);
            l.set_margin_top(px(10.0));
            l.set_max_width_chars(110);
            info.pack_start(&l, false, false, 0);
        }
        top.pack_start(&info, true, true, 0);
        b.add(&top);

        if d.groups.len() > 1 {
            let hd = ui::header("Bản");
            hd.set_margin_top(px(8.0));
            b.add(&hd);
            let groups = ui::flow(10.0, 10.0);
            let cur = self.group.borrow().clone();
            for g in &d.groups {
                let me = self.clone();
                let g2 = g.clone();
                let p = ui::pill_tag(&format!("{} ({})", g.name, g.episodes.len()), (Some(g) == cur.as_ref()).then_some("active"), move || {
                    *me.group.borrow_mut() = Some(g2.clone());
                    me.build(false);
                });
                ui::add_flow(&groups, p.upcast_ref());
            }
            b.add(&groups);
        }

        if d.groups.is_empty() {
            let l = ui::dim("Phim này chưa có tập nào.");
            l.set_margin_top(px(10.0));
            b.add(&l);
        } else if !movie {
            let hd = ui::header(&if eps.len() > 1 { format!("Danh sách tập ({})", eps.len()) } else { "Tập phim".into() });
            hd.set_margin_top(px(14.0));
            b.add(&hd);
            let watched = prefs::watched_episodes();
            let grid = ui::flow(10.0, 10.0);
            for e in &eps {
                let is_last = h.as_ref().map(|h| h.ep_url == e.url).unwrap_or(false);
                let text = format!("{}{}", if prefs::finished(&e.url) { "✓ " } else { "" }, e.title);
                let tag = if is_last {
                    Some("active")
                } else if watched.contains(&e.url) {
                    Some("watched")
                } else {
                    None
                };
                let (fu, eu) = (d.url.clone(), e.url.clone());
                let p = ui::pill_tag(&text, tag, move || app().play(&fu, &eu, false));
                p.set_size_request(px(104.0), -1);
                p.set_tooltip_text(Some(&e.title));
                ui::add_flow(&grid, p.upcast_ref());
            }
            b.add(&grid);
        }
        if let Some(rows) = self.related.borrow().clone().filter(|r| !r.is_empty()) {
            let p = self.related_panel(&rows);
            b.add(&p);
        }
        if focus {
            if let Some(m) = main {
                ui::after(40, move || keynav::focus_and_show(m.upcast_ref()));
            }
        }
        if self.related.borrow().is_none() && !self.related_loading.get() {
            self.related_loading.set(true);
            self.load_related(&d);
        }
    }

    /// Tải các hàng phim liên quan rồi thêm vào trang đang hiện (không dựng lại — giữ ô đang chọn).
    fn load_related(self: &Rc<Self>, d: &FilmDetail) {
        let me = self.clone();
        let url = d.url.clone();
        crate::hooks::related(d.title.clone(), d.url.clone(), move |rows| {
            let rows = Rc::new(rows);
            *me.related.borrow_mut() = Some(rows.clone());
            let same = me.d.borrow().as_ref().map(|x| x.url == url).unwrap_or(false);
            if rows.is_empty() || !same {
                return;
            }
            let p = me.related_panel(&rows);
            me.base.add(&p);
            let movie = me.d.borrow().as_ref().map(is_movie).unwrap_or(true);
            if !movie {
                if let Some(bt) = me.buttons.borrow().as_ref() {
                    let n = bt.children().len() as i32;
                    let rb = me.related_button();
                    bt.insert(&rb, (n - 1).max(0));
                    if let Some(c) = rb.parent() {
                        c.set_can_focus(false);
                    }
                    rb.show_all();
                }
            }
        });
    }

    /// Nút «▼ Phim liên quan» (phim bộ): cuộn xuống hàng phim liên quan đầu tiên, chọn phim đầu.
    fn related_button(self: &Rc<Self>) -> gtk::Button {
        let me = self.clone();
        ui::pill("▼  Phim liên quan", move || {
            if let Some(p) = me.related_panel.borrow().as_ref() {
                if let Some(f) = keynav::first(p.upcast_ref()) {
                    keynav::focus_and_show(&f);
                }
            }
        })
    }

    /// Các hàng phim liên quan: dải vuốt ngang như trang chủ (phim ở trang khác ghi tên trang ở góc).
    fn related_panel(&self, rows: &[RelatedRow]) -> gtk::Box {
        let host = cbtv::site::host_of(&self.url);
        let panel = ui::vbox(0.0);
        panel.set_margin_top(px(18.0));
        for row in rows {
            let cards = row
                .films
                .iter()
                .map(|f| {
                    let other = cbtv::site::host_of(&f.url).filter(|h| Some(h) != host.as_ref()).map(|h| cbtv::site::tag(&h, &prefs::sites()));
                    let f2 = f.clone();
                    card(f.clone(), Rc::new(move || open_film(&f2)), CardOpts { sub: other, ..Default::default() })
                })
                .collect();
            let t = ui::label(&row.title, &["h1"], false);
            panel.pack_start(&ui::strip(t.upcast_ref(), cards, None), false, false, 0);
        }
        *self.related_panel.borrow_mut() = Some(panel.clone());
        panel
    }

    /// Hộp chọn nguồn (đi bằng phím mũi tên): chọn nguồn khác thì mở trang phim ở nguồn đó.
    fn pick_source(self: &Rc<Self>) {
        let me = self.clone();
        ui::local(async move {
            let list = ui::vbox(10.0);
            let mut other: Option<gtk::Button> = None;
            for (i, l) in me.links.iter().enumerate() {
                let cur = l.url == me.url;
                let text = format!(
                    "{}{}{}",
                    source_name(&l.url),
                    l.badge.as_ref().map(|b| format!(" — {b}")).unwrap_or_default(),
                    if cur { "  ✓" } else { "" }
                );
                let b = ui::pill_tag(&text, cur.then_some("active"), move || app().close_dialog(100 + i as i32));
                b.set_size_request(px(360.0), -1);
                list.pack_start(&b, false, false, 0);
                if !cur && other.is_none() {
                    other = Some(b);
                }
            }
            let rx = app().ask("Mở phim ở nguồn khác", "", &["Huỷ"], Some(list.upcast()), 0);
            if let Some(o) = other {
                ui::after(60, move || o.grab_focus());
            }
            let r = rx.await.unwrap_or(-1);
            if r < 100 || (r - 100) as usize >= me.links.len() {
                return;
            }
            let link = me.links[(r - 100) as usize].clone();
            if link.url == me.url {
                return;
            }
            let poster = link.poster.clone().or_else(|| me.hint.as_ref().and_then(|h| h.poster.clone()));
            app().replace(new(&link.url, Some(Film::new(link.url.clone(), link.title.clone(), poster, None)), me.links.clone()));
        });
    }
}

impl Page for DetailPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        let title = self.hint.as_ref().map(|h| format!("Đang tải «{}»…", h.title)).unwrap_or_else(|| "Đang tải phim…".into());
        self.base.loading(&title);
        if let Some(d) = cbtv::site::cached_detail(&self.url) {
            self.loaded(d);
            return;
        }
        let t = self.base.next_token();
        let url = self.url.clone();
        let me = self.clone();
        ui::spawn(async move { cbtv::site::detail(&url).await }, move |r| {
            if t != me.base.token() {
                return;
            }
            match r {
                Ok(d) => me.loaded(d),
                Err(e) => {
                    let me2 = me.clone();
                    me.base.show_error(&cbtv::util::friendly(&e), Rc::new(move || me2.clone().start()));
                }
            }
        });
    }

    fn returned(self: Rc<Self>) {
        let Some(d) = self.d.borrow().clone() else { return };
        if let Some(h) = prefs::history_for(&d.url) {
            if let Some(g) = d.group_of(&h.ep_url) {
                *self.group.borrow_mut() = Some(g.clone());
            }
        }
        self.build(false);
    }
}
