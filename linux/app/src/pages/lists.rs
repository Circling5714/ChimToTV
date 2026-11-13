//! Danh sách phim của một mục (tự tải trang sau khi cuộn tới cuối), tìm kiếm, phim theo dõi, cả một hàng («Xem thêm»),
//! thể loại / quốc gia. Bản Windows: UI/Pages.cs (ListPage, SearchPage, FollowsPage, CardsPage, FilterPage).

use super::detail::{self, Link};
use super::{Page, PageBase};
use crate::app::app;
use crate::card::{card, CardOpts};
use crate::css::px;
use crate::keynav::{self, Dir};
use crate::ui;
use cbtv::{prefs, Film, FilmPage, Result};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub type Loader = Rc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<FilmPage>> + Send>>>;

pub fn film_card(f: &Film) -> gtk::Widget {
    let f2 = f.clone();
    card(f.clone(), Rc::new(move || detail::open_film(&f2)), CardOpts::default())
}

// ---- Danh sách ----

pub struct ListPage {
    base: PageBase,
    title: String,
    url: String,
    /// Cách tải riêng (vd «Điểm IMDb cao»); None = danh sách của trang (url).
    loader: Option<Loader>,
    /// Danh mục KKPhim đang xếp theo điểm IMDb (nút «Điểm IMDb cao» ở đầu danh mục).
    imdb: Cell<bool>,
    next: RefCell<Option<String>>,
    loading_more: Cell<bool>,
    grid: RefCell<Option<gtk::FlowBox>>,
    more: RefCell<Option<gtk::Button>>,
    seen: RefCell<HashSet<String>>,
}

pub fn list(title: &str, url: &str, menu_key: &str) -> Rc<dyn Page> {
    list_with(title, url, menu_key, None)
}

pub fn list_with(title: &str, url: &str, menu_key: &str, loader: Option<Loader>) -> Rc<dyn Page> {
    let p = Rc::new(ListPage {
        base: PageBase::new(menu_key),
        title: title.to_string(),
        url: url.to_string(),
        loader,
        imdb: Cell::new(false),
        next: RefCell::new(None),
        loading_more: Cell::new(false),
        grid: RefCell::new(None),
        more: RefCell::new(None),
        seen: RefCell::new(HashSet::new()),
    });
    let weak = Rc::downgrade(&p);
    p.base.scroller.vadjustment().connect_value_changed(move |a| {
        if let Some(p) = weak.upgrade() {
            if p.next.borrow().is_some() && !p.loading_more.get() && a.value() + a.page_size() > a.upper() - 700.0 {
                p.load_more();
            }
        }
    });
    p
}

impl ListPage {
    fn sortable(&self) -> bool {
        self.loader.is_none() && self.base.menu_key.borrow().starts_with("cat:") && crate::hooks::imdb_can_rank(&self.url)
    }

    fn show(self: &Rc<Self>, page: FilmPage) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header(&self.title));
        if self.sortable() {
            let imdb = self.imdb.get();
            let (m1, m2) = (self.clone(), self.clone());
            b.add(&ui::row(&[
                ui::pill_tag("Mới cập nhật", (!imdb).then_some("active"), move || m1.sort(false)).upcast(),
                ui::pill_tag("Điểm IMDb cao", imdb.then_some("active"), move || m2.sort(true)).upcast(),
            ]));
            let n = ui::dim(if imdb {
                "Xếp theo điểm IMDb (có tính số lượt chấm) trong khoảng 320 phim mới cập nhật của mục này."
            } else {
                "Phim mới cập nhật — chọn «Điểm IMDb cao» để xếp theo điểm IMDb."
            });
            ui::margins(&n, 4.0, 0.0, 10.0, 0.0);
            b.add(&n);
        }
        if !page.sections.is_empty() {
            for s in &page.sections {
                b.add(&ui::section(&s.title, None, s.films.iter().map(film_card).collect()));
            }
            return;
        }
        if page.films.is_empty() {
            b.add(&ui::dim(if self.imdb.get() || self.url == "imdb:top" { "Không có phim nào đủ lượt chấm IMDb." } else { "Không có phim nào." }));
            return;
        }
        let grid = ui::grid(Vec::new());
        b.add(&grid);
        *self.grid.borrow_mut() = Some(grid);
        self.append(page);
    }

    fn sort(self: &Rc<Self>, imdb: bool) {
        if self.imdb.get() == imdb {
            return;
        }
        self.imdb.set(imdb);
        self.clone().start();
    }

    fn append(self: &Rc<Self>, page: FilmPage) {
        let Some(grid) = self.grid.borrow().clone() else { return };
        if let Some(m) = self.more.borrow_mut().take() {
            self.base.body.remove(&m);
        }
        for f in &page.films {
            if self.seen.borrow_mut().insert(f.url.clone()) {
                let c = film_card(f);
                ui::add_flow(&grid, &c);
                c.show_all();
            }
        }
        *self.next.borrow_mut() = page.next_url.clone();
        if page.next_url.is_some() {
            let me = self.clone();
            let m = ui::pill("Tải thêm phim", move || me.load_more());
            m.set_halign(gtk::Align::Start);
            self.base.add(&m);
            *self.more.borrow_mut() = Some(m);
        }
    }

    fn load_more(self: &Rc<Self>) {
        let Some(next) = self.next.borrow().clone() else { return };
        if self.loading_more.replace(true) {
            return;
        }
        let had_focus = self.more.borrow().as_ref().map(|m| m.has_focus()).unwrap_or(false);
        if let Some(m) = self.more.borrow().as_ref() {
            m.set_label("Đang tải thêm…");
        }
        let me = self.clone();
        ui::spawn(async move { cbtv::site::list(&next).await }, move |r| {
            me.loading_more.set(false);
            match r {
                Ok(page) => {
                    let before = me.grid.borrow().as_ref().map(|g| g.children().len()).unwrap_or(0);
                    me.append(page);
                    if had_focus {
                        if let Some(g) = me.grid.borrow().as_ref() {
                            if let Some(c) = g.children().get(before).and_then(|c| c.clone().downcast::<gtk::Bin>().ok()).and_then(|b| b.child()) {
                                ui::after(30, move || keynav::focus_and_show(&c));
                            }
                        }
                    }
                }
                Err(e) => {
                    app().toast(&format!("Không tải thêm được: {}", cbtv::util::friendly(&e)));
                    if let Some(m) = me.more.borrow().as_ref() {
                        m.set_label("Tải thêm phim");
                    }
                }
            }
        });
    }
}

impl Page for ListPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.base.loading(&format!("Đang tải {}…", self.title));
        self.seen.borrow_mut().clear();
        *self.grid.borrow_mut() = None;
        *self.more.borrow_mut() = None;
        *self.next.borrow_mut() = None;
        let t = self.base.next_token();
        let fut: Pin<Box<dyn Future<Output = Result<FilmPage>> + Send>> = match &self.loader {
            Some(l) => l(),
            None if self.imdb.get() && self.sortable() => crate::hooks::imdb_for_list(self.url.clone()),
            None => {
                let u = self.url.clone();
                Box::pin(async move { cbtv::site::list(&u).await })
            }
        };
        let me = self.clone();
        ui::spawn(fut, move |r| {
            if t != me.base.token() {
                return;
            }
            match r {
                Ok(p) => me.show(p),
                Err(e) => {
                    let m2 = me.clone();
                    me.base.show_error(&cbtv::util::friendly(&e), Rc::new(move || m2.clone().start()));
                }
            }
        });
    }

    fn scroll_edge(&self, dir: Dir) {
        let adj = self.base.scroller.vadjustment();
        if dir == Dir::Up {
            adj.set_value(adj.lower());
        } else {
            adj.set_value(adj.upper() - adj.page_size());
        }
    }
}

// ---- Tìm kiếm ----

thread_local! {
    static LAST_QUERY: RefCell<String> = const { RefCell::new(String::new()) };
    static ALL_SITES: Cell<bool> = const { Cell::new(true) };
}

pub struct SearchPage {
    base: PageBase,
    entry: gtk::Entry,
    results: gtk::Box,
    scope: gtk::Button,
    token: Cell<u64>,
}

pub fn search() -> Rc<dyn Page> {
    let entry = crate::ui::entry();
    entry.set_width_chars(36);
    entry.set_placeholder_text(Some("Tên phim…"));
    entry.set_text(&LAST_QUERY.with(|q| q.borrow().clone()));
    let p = Rc::new(SearchPage {
        base: PageBase::new("search"),
        entry,
        results: ui::vbox(0.0),
        scope: ui::pill("", || {}),
        token: Cell::new(0),
    });
    let weak = Rc::downgrade(&p);
    p.entry.connect_activate(move |_| {
        if let Some(p) = weak.upgrade() {
            p.search();
        }
    });
    let weak = Rc::downgrade(&p);
    p.scope.connect_clicked(move |_| {
        if let Some(p) = weak.upgrade() {
            ALL_SITES.with(|a| a.set(!a.get()));
            p.update_scope();
            if !p.entry.text().trim().is_empty() {
                p.search();
            }
        }
    });
    p.update_scope();
    p
}

impl SearchPage {
    fn update_scope(&self) {
        let all = ALL_SITES.with(|a| a.get());
        self.scope.set_label(&if all {
            "Tìm trên: mọi trang".to_string()
        } else {
            format!("Tìm trên: {}", cbtv::site::host_of(&prefs::base_url()).unwrap_or_default())
        });
    }

    fn search(self: &Rc<Self>) {
        let q = self.entry.text().trim().to_string();
        if q.is_empty() {
            return;
        }
        LAST_QUERY.with(|x| *x.borrow_mut() = q.clone());
        self.token.set(self.token.get() + 1);
        let t = self.token.get();
        ui::clear(&self.results);
        let all = ALL_SITES.with(|a| a.get());
        let msg = if all { format!("Đang tìm «{q}» trên {} trang…", prefs::sites().len()) } else { format!("Đang tìm «{q}»…") };
        self.results.pack_start(&ui::dim(&msg), false, false, 0);
        self.results.show_all();
        let me = self.clone();
        let q2 = q.clone();
        ui::spawn(
            async move {
                if all {
                    cbtv::site::search_all(&q2).await
                } else {
                    cbtv::site::search(&q2, None).await
                }
            },
            move |r| {
                if t != me.token.get() {
                    return;
                }
                ui::clear(&me.results);
                match r {
                    Ok(page) => {
                        let mut sections = if page.sections.is_empty() {
                            vec![cbtv::FilmSection { title: "Kết quả".into(), films: page.films }]
                        } else {
                            page.sections
                        };
                        sections.retain(|s| !s.films.is_empty());
                        if sections.is_empty() {
                            me.results.pack_start(&ui::dim(&format!("Không tìm thấy phim nào cho «{q}».")), false, false, 0);
                        }
                        for s in &sections {
                            me.results.pack_start(&ui::section(&s.title, None, s.films.iter().map(film_card).collect()), false, false, 0);
                        }
                    }
                    Err(e) => me.results.pack_start(&ui::dim(&format!("Lỗi: {}", cbtv::util::friendly(&e))), false, false, 0),
                }
                me.results.show_all();
            },
        );
    }
}

impl Page for SearchPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Tìm kiếm"));
        let row = ui::hbox(10.0);
        row.set_margin_bottom(px(10.0));
        if let Some(p) = self.entry.parent() {
            p.downcast::<gtk::Container>().ok().map(|c| c.remove(&self.entry));
        }
        if let Some(p) = self.scope.parent() {
            p.downcast::<gtk::Container>().ok().map(|c| c.remove(&self.scope));
        }
        row.pack_start(&self.entry, false, false, 0);
        let me = self.clone();
        row.pack_start(&ui::pill("🔍  Tìm", move || me.search()), false, false, 0);
        row.pack_start(&self.scope, false, false, 0);
        b.add(&row);
        let n = ui::dim("Gõ tên phim (có dấu hoặc không) rồi nhấn Enter.");
        n.set_margin_bottom(px(18.0));
        b.add(&n);
        if let Some(p) = self.results.parent() {
            p.downcast::<gtk::Container>().ok().map(|c| c.remove(&self.results));
        }
        b.add(&self.results);
        let e = self.entry.clone();
        ui::after(40, move || {
            e.grab_focus();
        });
        if !self.entry.text().trim().is_empty() && self.results.children().is_empty() {
            self.search();
        }
    }

    fn first_target(&self) -> Option<gtk::Widget> {
        Some(self.entry.clone().upcast())
    }

    fn has_input(&self) -> bool {
        true
    }
}

// ---- Phim theo dõi ----

pub struct FollowsPage {
    base: PageBase,
    checking: Cell<bool>,
}

pub fn follows() -> Rc<dyn Page> {
    Rc::new(FollowsPage { base: PageBase::new("follows"), checking: Cell::new(false) })
}

impl FollowsPage {
    fn build(self: &Rc<Self>) {
        let focused = app().window.focused_widget().map(|w| w.widget_name().to_string());
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Phim theo dõi"));
        let mut list = prefs::follows();
        if list.is_empty() {
            b.add(&ui::dim("Chưa theo dõi phim nào. Mở một phim rồi bấm «☆ Theo dõi» để biết khi có tập mới."));
            return;
        }
        let n = ui::dim(if self.checking.get() { "Đang kiểm tra tập mới…" } else { "Delete hoặc chuột phải: bỏ theo dõi." });
        n.set_margin_bottom(px(16.0));
        b.add(&n);
        list.sort_by_key(|f| if f.new_count() > 0 { 0 } else { 1 });
        let mut cards = Vec::new();
        for (i, f) in list.into_iter().enumerate() {
            let me = self.clone();
            let f2 = f.clone();
            let remove: Rc<dyn Fn()> = Rc::new(move || {
                let me = me.clone();
                let f2 = f2.clone();
                ui::local(async move {
                    if !app().confirm(&format!("Bỏ theo dõi «{}»?", f2.title), "", "Bỏ theo dõi", "Huỷ").await {
                        return;
                    }
                    prefs::unfollow(&f2.film_url);
                    crate::startup::sync_soon(0);
                    me.build();
                    app().rebuild_menu();
                    super::focus_card_at(me.clone(), i);
                });
            });
            let film = Film::new(f.film_url.clone(), f.title.clone(), f.poster.clone(), cbtv::util::non_empty(Some(&f.latest)));
            let ff = film.clone();
            cards.push(card(
                film,
                Rc::new(move || detail::open_film(&ff)),
                CardOpts {
                    fresh: (f.new_count() > 0).then(|| format!("+{}", f.new_count())),
                    sub: cbtv::site::host_of(&f.film_url),
                    remove: Some(remove),
                    ..Default::default()
                },
            ));
        }
        b.add(&ui::grid(cards));
        if let Some(name) = focused {
            if let Some(w) = keynav::find_named(b.body.upcast_ref(), &name) {
                ui::after(30, move || keynav::focus_and_show(&w));
            }
        }
    }

    fn check_all(self: &Rc<Self>) {
        if self.checking.replace(true) {
            return;
        }
        let list = prefs::follows();
        let me = self.clone();
        ui::spawn(
            async move {
                let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(3));
                let mut tasks = Vec::new();
                for f in list {
                    let gate = gate.clone();
                    tasks.push(tokio::spawn(async move {
                        let _p = gate.acquire().await.ok()?;
                        let d = cbtv::site::detail(&f.film_url).await.ok()?;
                        let last = d.latest_episode()?.title.clone();
                        Some((f.film_url.clone(), d.episode_count() as i64, last))
                    }));
                }
                let mut out = Vec::new();
                for t in tasks {
                    if let Ok(Some(x)) = t.await {
                        out.push(x);
                    }
                }
                out
            },
            move |res| {
                for (url, n, last) in res {
                    prefs::update_follow(&url, n, &last, false);
                }
                me.checking.set(false);
                if me.base.scroller.is_mapped() {
                    me.build();
                }
                app().rebuild_menu();
            },
        );
    }
}

impl Page for FollowsPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build();
        self.check_all();
    }

    fn returned(self: Rc<Self>) {
        self.build();
    }
}

// ---- Cả một hàng của trang chủ tổng hợp («Xem thêm») ----

pub struct CardItem {
    pub film: Film,
    pub chip: Option<String>,
    pub links: Vec<Link>,
}

pub struct CardsPage {
    base: PageBase,
    title: String,
    cards: Vec<CardItem>,
}

pub fn cards(title: &str, cards: Vec<CardItem>) -> Rc<dyn Page> {
    Rc::new(CardsPage { base: PageBase::new("home"), title: title.to_string(), cards })
}

impl Page for CardsPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header(&self.title));
        let cards = self
            .cards
            .iter()
            .map(|c| {
                let (f, links) = (c.film.clone(), c.links.clone());
                card(
                    c.film.clone(),
                    Rc::new(move || app().push(detail::new(&f.url, Some(f.clone()), links.clone()))),
                    CardOpts { sub: c.chip.clone(), ..Default::default() },
                )
            })
            .collect();
        b.add(&ui::grid(cards));
    }
}

// ---- Thể loại / Quốc gia ----

pub struct FilterPage {
    base: PageBase,
    genres: bool,
}

pub fn filter(genres: bool) -> Rc<dyn Page> {
    Rc::new(FilterPage { base: PageBase::new(if genres { "genres" } else { "countries" }), genres })
}

impl Page for FilterPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.base.loading("Đang tải…");
        let t = self.base.next_token();
        let b = prefs::base_url();
        let me = self.clone();
        ui::spawn(async move { if cbtv::api_site::is_known(&b) { Ok(cbtv::api_site::filters(&b)) } else { cbtv::next_site::filters(&b).await } }, move |r| {
            if t != me.base.token() {
                return;
            }
            match r {
                Ok((g, c)) => {
                    let items = if me.genres { g } else { c };
                    me.base.clear();
                    me.base.add(&ui::header(if me.genres { "Thể loại" } else { "Quốc gia" }));
                    let grid = ui::flow(10.0, 10.0);
                    let key = me.base.menu_key.borrow().clone();
                    for c in items {
                        let k = key.clone();
                        let (t, u) = (c.title.clone(), c.url.clone());
                        let p = ui::pill(&c.title, move || app().push(list(&t, &u, &k)));
                        p.set_size_request(px(150.0), -1);
                        ui::add_flow(&grid, p.upcast_ref());
                    }
                    me.base.add(&grid);
                }
                Err(e) => {
                    let m2 = me.clone();
                    me.base.show_error(&cbtv::util::friendly(&e), Rc::new(move || m2.clone().start()));
                }
            }
        });
    }
}
