//! Trang chủ tổng hợp (mặc định): phim từ mọi nguồn trong danh sách trang, gộp phim trùng, xếp hàng theo lịch sử xem của người
//! xem (cbtv::home_feed) — hiện ngay trang chủ lần trước, tải mới mọi nguồn (mỗi nguồn tối đa 6 giây) rồi thay vào; nguồn lỗi /
//! chậm ghi ở dòng trạng thái. Mỗi hàng một dải vuốt ngang (tới 12 phim, ô «Xem thêm» cuối dải). Bản Windows: Pages.cs HomePage.

use super::detail::{self, Link};
use super::lists::{self, CardItem};
use super::{Page, PageBase};
use crate::app::app;
use crate::card::{card, CardOpts};
use crate::css::px;
use crate::keynav;
use crate::ui;
use cbtv::feed::FeedItem;
use cbtv::home_feed::{self as hf, Card, FeedRow, Snapshot};
use cbtv::{prefs, Film};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Số phim tối đa một dải vuốt ngang (còn lại xem ở «Xem thêm»).
const STRIP_MAX: usize = 12;

pub struct HomeMix {
    base: PageBase,
    snap: RefCell<Option<Snapshot>>,
    kids: Cell<bool>,
    rows: RefCell<String>,
    site: RefCell<String>,
    feed_at: Cell<i64>,
    run: Cell<u64>,
    note: RefCell<Option<gtk::Label>>,
}

pub fn new() -> Rc<dyn Page> {
    Rc::new(HomeMix {
        base: PageBase::new("home"),
        snap: RefCell::new(None),
        kids: Cell::new(false),
        rows: RefCell::new(String::new()),
        site: RefCell::new(String::new()),
        feed_at: Cell::new(0),
        run: Cell::new(0),
        note: RefCell::new(None),
    })
}

pub fn links_of(items: &[FeedItem]) -> Vec<Link> {
    items.iter().map(|i| Link { url: i.url.clone(), title: i.title.clone(), poster: i.poster.clone(), badge: i.badge.clone() }).collect()
}

fn open_card(c: &Card) {
    app().push(detail::new(&c.film.url, Some(c.film.clone()), links_of(&c.links)));
}

impl HomeMix {
    fn set_note(&self, text: &str) {
        if let Some(n) = self.note.borrow().as_ref() {
            n.set_text(&format!("{}{text}", if self.kids.get() { "Hồ sơ trẻ em · " } else { "" }));
        }
    }

    fn load_mix(self: &Rc<Self>) {
        self.run.set(self.run.get() + 1);
        let run = self.run.get();
        let pid = prefs::profile_id();
        let me = self.clone();
        let shown = self.snap.borrow().is_some(); // làm mới: giữ trang đang hiện tới khi có bản mới
        ui::local(async move {
            let mut shown = shown;
            if !shown {
                me.base.loading("Đang tải trang chủ…");
                if let Some(Some(last)) = ui::run(async move { hf::last(&pid) }).await {
                    if run == me.run.get() {
                        shown = me.show(last);
                    }
                }
            }
            let Some(plan) = ui::run(async { hf::make_plan(false) }).await else { return };
            if run != me.run.get() {
                return;
            }
            me.set_note(&format!("Đang làm mới {} nguồn…", plan.src.count()));
            if !shown {
                me.base.loading(&format!("Đang tải trang chủ từ {} nguồn…", plan.src.count()));
                let p2 = plan.clone();
                if let Some(Some(c)) = ui::run(async move { hf::cached(&p2) }).await {
                    if run == me.run.get() {
                        shown = me.show(c);
                    }
                }
            }
            let Some(snap) = ui::run(hf::load(plan)).await else { return };
            if run != me.run.get() {
                return;
            }
            me.feed_at.set(cbtv::util::now_ms());
            let note = snap.note.clone();
            if !me.show(snap.clone()) && !shown {
                let m2 = me.clone();
                me.base.show_error(&format!("Không tải được phim từ nguồn nào.\n{note}"), Rc::new(move || m2.clone().start()));
            }
            // Nguồn chậm về muộn: cứ 4 giây xem bản lưu của chúng đã mới chưa, có thì dựng lại; thôi sau 5 lần
            let mut snap = snap;
            for _ in 0..5 {
                if !snap.pending {
                    break;
                }
                glib::timeout_future(std::time::Duration::from_millis(4000)).await;
                if run != me.run.get() {
                    return;
                }
                let s2 = snap.clone();
                if let Some(Some(next)) = ui::run(async move { hf::late(&s2).await }).await {
                    if run != me.run.get() {
                        return;
                    }
                    me.show(next.clone());
                    snap = next;
                }
            }
            if run == me.run.get() && snap.pending {
                me.set_note(&snap.note_gave_up);
            }
        });
    }

    /// Hiện một lần dựng (giữ ô đang chọn). false nếu không có gì để hiện.
    fn show(self: &Rc<Self>, s: Snapshot) -> bool {
        if let Some((old, new)) = &s.moved_to {
            if *old == prefs::base_url() {
                if let Some(nu) = prefs::normalize_url(new) {
                    prefs::replace_site(old, &nu);
                    *self.site.borrow_mut() = nu.clone();
                    app().toast(&format!("Trang đã chuyển sang {} — app đã đổi theo.", cbtv::site::host_of(&nu).unwrap_or_default()));
                }
            }
        }
        if let Some(c) = s.categories.as_ref().filter(|c| !c.is_empty()) {
            app().set_categories(c.clone());
        }
        let focus = app().window.focused_widget().filter(|f| f.is_ancestor(&self.base.body)).map(|f| f.widget_name().to_string());
        *self.snap.borrow_mut() = Some(s);
        let any = self.build_mix();
        if let Some(name) = focus.filter(|n| !n.is_empty()) {
            self.focus_name(name);
        }
        any
    }

    fn focus_name(&self, name: String) {
        let body = self.base.body.clone();
        ui::after(40, move || {
            if let Some(w) = keynav::find_named(body.upcast_ref(), &name) {
                keynav::focus_and_show(&w);
            }
        });
    }

    /// Một hàng vuốt ngang; row_id: id hàng (nhấn phải / phím Menu / M trên phim → sửa nhanh hàng).
    fn strip(self: &Rc<Self>, title: &str, row_id: &str, cards: Vec<gtk::Widget>, more: Option<Rc<dyn Fn()>>) -> gtk::Box {
        let t = ui::label(title, &["h1"], false);
        let ev = gtk::EventBox::new();
        ev.add(&t);
        let (me, rid) = (self.clone(), row_id.to_string());
        ev.connect_button_release_event(move |_, e| {
            if e.button() == 3 {
                me.row_menu(&rid);
            }
            glib::Propagation::Proceed
        });
        ui::strip(ev.upcast_ref(), cards, more)
    }

    fn menu_action(self: &Rc<Self>, row_id: &str) -> Rc<dyn Fn()> {
        let (me, rid) = (self.clone(), row_id.to_string());
        Rc::new(move || me.row_menu(&rid))
    }

    fn card_widget(self: &Rc<Self>, c: &Card, row_id: &str) -> gtk::Widget {
        let c2 = c.clone();
        card(
            c.film.clone(),
            Rc::new(move || open_card(&c2)),
            CardOpts { sub: c.chip.clone(), menu: Some(self.menu_action(row_id)), ..Default::default() },
        )
    }

    /// Sửa nhanh một hàng: ẩn, ▲ chuyển lên / ▼ chuyển xuống (qua hàng đang hiện kề nó), «Các hàng…», «Nguồn…». Hồ sơ trẻ em:
    /// hỏi mã PIN phụ huynh trước (nếu đã đặt).
    fn row_menu(self: &Rc<Self>, row_id: &str) {
        let me = self.clone();
        let row_id = row_id.to_string();
        ui::local(async move {
            let a = app();
            if prefs::kids() && prefs::has_parent_pin() && !a.check_parent("Nhập mã PIN phụ huynh để sửa trang chủ.").await {
                return;
            }
            let arranged = me.snap.borrow().as_ref().map(|s| hf::arrange_slots(&s.rows, None)).unwrap_or_default();
            let (title, slot) = match row_id.as_str() {
                "new-eps" => ("Có tập mới".to_string(), row_id.clone()),
                "continue" => ("Xem tiếp".to_string(), row_id.clone()),
                _ => match arranged.iter().find(|x| x.0.id == row_id) {
                    Some((r, s)) if s != "topics" => (r.title.clone(), s.clone()),
                    _ => return,
                },
            };
            let shown: Vec<String> = arranged.iter().map(|x| x.1.clone()).collect();
            let prefs_now = hf::row_prefs();
            let at = arranged.iter().position(|x| x.0.id == row_id);
            let next_focus = at.and_then(|i| arranged.get(i + 1).or(if i > 0 { arranged.get(i - 1) } else { None })).and_then(|x| x.0.cards.first()).map(|c| c.film.url.clone());
            let mut labels: Vec<String> = vec!["Ẩn hàng này".into()];
            let up = hf::move_shown(&prefs_now, &shown, &slot, true);
            let down = hf::move_shown(&prefs_now, &shown, &slot, false);
            if up.is_some() {
                labels.push("▲  Chuyển lên".into());
            }
            if down.is_some() {
                labels.push("▼  Chuyển xuống".into());
            }
            labels.push("☰  Các hàng…".into());
            labels.push("Nguồn…".into());
            labels.push("Huỷ".into());
            let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
            let focus = a.window.focused_widget().map(|w| w.widget_name().to_string());
            let i = a.ask_async(&format!("Hàng «{title}»"), "", &refs, None, 0).await;
            if i < 0 {
                return;
            }
            let choice = labels[i as usize].clone();
            match choice.as_str() {
                "Ẩn hàng này" => {
                    me.apply(hf::hide(&prefs_now, &slot), next_focus.or(focus));
                    a.toast(&format!("Đã ẩn hàng «{title}» — bật lại trong Cài đặt → Các hàng trên trang chủ."));
                }
                "▲  Chuyển lên" => me.apply(up.unwrap_or_default(), focus),
                "▼  Chuyển xuống" => me.apply(down.unwrap_or_default(), focus),
                "☰  Các hàng…" => a.push(super::home_rows::rows()),
                "Nguồn…" => a.push(super::home_rows::sources()),
                _ => {}
            }
        });
    }

    /// Lưu cài đặt các hàng rồi xếp lại từ lần tải gần nhất (không tải lại).
    fn apply(self: &Rc<Self>, p: Vec<(String, bool)>, focus: Option<String>) {
        hf::save_rows(&p);
        *self.rows.borrow_mut() = crate::hooks::rows_signature();
        self.build_mix();
        match focus {
            Some(f) => self.focus_name(f),
            None => {
                let me = self.clone();
                ui::after(40, move || super::focus_first(me.as_ref()));
            }
        }
    }

    /// Dựng trang chủ tổng hợp từ lần tải gần nhất: Có tập mới, Xem tiếp, (chọn chủ đề), mỗi hàng một dải vuốt ngang.
    fn build_mix(self: &Rc<Self>) -> bool {
        let Some(s) = self.snap.borrow().clone() else { return false };
        let b = &self.base;
        b.clear();
        let note = ui::dim(&format!("{}{}", if self.kids.get() { "Hồ sơ trẻ em · " } else { "" }, s.note));
        note.set_margin_bottom(px(10.0));
        b.add(&note);
        *self.note.borrow_mut() = Some(note);
        let mut count = 0;
        let on: std::collections::HashSet<String> = hf::row_prefs().into_iter().filter(|x| x.1).map(|x| x.0).collect();

        // Có tập mới: phim theo dõi + phim đang xem dở có tập mới ở bất kỳ nguồn nào (đã mở phim từ lúc dựng trang thì thôi)
        let follows: Vec<_> = prefs::follows().into_iter().filter(|f| f.new_count() > 0).collect();
        let followed: std::collections::HashSet<String> = prefs::follows().iter().map(|f| cbtv::kk_phim::norm(&f.title)).collect();
        let auto: Vec<_> = s.fresh.iter().filter(|a| prefs::seen_ep(&a.key).unwrap_or(-1.0) < a.ep && !followed.contains(&a.key)).cloned().collect();
        if follows.len() + auto.len() > 0 && on.contains("new-eps") {
            let mut cards: Vec<gtk::Widget> = Vec::new();
            for f in &follows {
                let film = Film::new(f.film_url.clone(), f.title.clone(), f.poster.clone(), cbtv::util::non_empty(Some(&f.latest)));
                let ff = film.clone();
                cards.push(card(
                    film,
                    Rc::new(move || detail::open_film(&ff)),
                    CardOpts { fresh: Some(format!("+{}", f.new_count())), menu: Some(self.menu_action("new-eps")), ..Default::default() },
                ));
            }
            for a in &auto {
                cards.push(self.card_widget(&a.card, "new-eps"));
            }
            b.add(&self.strip("Có tập mới", "new-eps", cards, None));
            count += 1;
        }
        let history: Vec<_> = if on.contains("continue") { prefs::history().into_iter().take(STRIP_MAX).collect() } else { Vec::new() };
        if !history.is_empty() {
            let cards = history
                .iter()
                .map(|h| {
                    let (fu, eu) = (h.film_url.clone(), h.ep_url.clone());
                    card(
                        Film::new(h.film_url.clone(), h.title.clone(), h.poster.clone(), None),
                        Rc::new(move || app().play(&fu, &eu, true)),
                        CardOpts {
                            sub: Some(h.ep_title.clone()),
                            menu: Some(self.menu_action("continue")),
                            uid: Some(format!("resume:{}", h.film_url)),
                            ..Default::default()
                        },
                    )
                })
                .collect();
            b.add(&self.strip("Xem tiếp", "continue", cards, None));
            count += 1;
        }
        // Người xem chưa có lịch sử: chọn chủ đề
        if prefs::history().is_empty() && !self.kids.get() {
            let chosen = hf::home_topics();
            let row = ui::flow(12.0, 12.0);
            for (id, label) in hf::TOPICS {
                let on = chosen.contains(*id);
                let me = self.clone();
                let id2 = id.to_string();
                let p = ui::pill_tag(&format!("{}{label}", if on { "✓ " } else { "" }), on.then_some("active"), move || me.toggle_topic(&id2));
                p.set_size_request(px(200.0), -1);
                p.set_widget_name(&format!("chip:{id}"));
                ui::add_flow(&row, p.upcast_ref());
            }
            let sec = ui::vbox(0.0);
            let h = ui::label("Chọn vài chủ đề bạn thích — trang chủ sẽ gợi ý theo đó", &["h1"], true);
            ui::margins(&h, 6.0, 0.0, 14.0, 0.0);
            sec.pack_start(&h, false, false, 0);
            sec.pack_start(&row, false, false, 0);
            sec.set_margin_bottom(px(12.0));
            b.add(&sec);
        }
        // Thứ tự / bật tắt theo cài đặt hiện tại của người xem
        for r in hf::arrange(&s.rows, None) {
            let n = r.cards.len();
            let mut cards: Vec<gtk::Widget> = r.cards.iter().take(STRIP_MAX).map(|c| self.card_widget(c, &r.id)).collect();
            let mut more: Option<Rc<dyn Fn()>> = None;
            if n > STRIP_MAX || r.more.is_some() {
                let row = r.clone();
                let m: Rc<dyn Fn()> = Rc::new(move || open_more(&row));
                let m2 = m.clone();
                let count = format!("{n} phim");
                let tile = ui::more_tile(move || m2(), if r.more.is_none() { Some(count.as_str()) } else { None });
                tile.set_widget_name(&format!("more:{}", r.id));
                cards.push(tile.upcast());
                more = Some(m);
            }
            b.add(&self.strip(&r.title, &r.id, cards, more));
            count += 1;
        }
        count > 0
    }

    /// Chọn / bỏ một chủ đề rồi tải các hàng theo chủ đề.
    fn toggle_topic(self: &Rc<Self>, id: &str) {
        let mut set = hf::home_topics();
        if !set.remove(id) {
            set.insert(id.to_string());
        }
        hf::set_home_topics(&set);
        self.build_mix();
        self.focus_name(format!("chip:{id}"));
        self.clone().start();
    }
}

/// «Xem thêm»: cả hàng (danh sách có phân trang của nguồn nếu có, không thì mọi phim đã gộp).
fn open_more(r: &FeedRow) {
    match &r.more {
        Some(url) => app().push(lists::list(&r.title, url, "home")),
        None => app().push(lists::cards(
            &r.title,
            r.cards.iter().map(|c| CardItem { film: c.film.clone(), chip: c.chip.clone(), links: links_of(&c.links) }).collect(),
        )),
    }
}

impl Page for HomeMix {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.kids.set(prefs::kids());
        *self.site.borrow_mut() = prefs::base_url();
        *self.rows.borrow_mut() = crate::hooks::rows_signature();
        self.load_mix();
    }

    fn returned(self: Rc<Self>) {
        // Đổi kiểu trang chủ / hồ sơ trẻ em / trang phim / các hàng trong Cài đặt → tải lại
        if !prefs::home_mix_on() {
            app().replace(super::home::new_site());
            return;
        }
        if prefs::kids() != self.kids.get() || prefs::base_url() != *self.site.borrow() || crate::hooks::rows_signature() != *self.rows.borrow() {
            *self.snap.borrow_mut() = None;
            self.start();
            return;
        }
        // Mở lâu (quay lại sau 20 phút): làm mới; không thì dựng lại hàng đầu (xem xong / vừa mở phim có tập mới)
        if cbtv::util::now_ms() - self.feed_at.get() > 20 * 60_000 {
            self.start();
            return;
        }
        let name = self.base.last_focus.borrow().as_ref().map(|w| w.widget_name().to_string());
        self.build_mix();
        *self.base.last_focus.borrow_mut() = name.and_then(|n| keynav::find_named(self.base.body.upcast_ref(), &n));
    }
}
