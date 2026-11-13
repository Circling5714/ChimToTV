//! Cài đặt → Trang chủ → «Các hàng trên trang chủ» (bật / tắt từng hàng, ▲ ▼ đổi thứ tự, «＋ Thêm hàng…», ✕ xoá hàng tự
//! thêm, «Khôi phục mặc định»), «Thêm hàng» (chủ đề có sẵn hoặc loại phim + nước + thể loại + ngôn ngữ) và «Nguồn cho trang
//! chủ» (bật / tắt từng nguồn). Riêng từng người xem. Bản Windows: UI/HomeRowsPage.cs.

use super::{Page, PageBase};
use crate::app::app;
use crate::css::px;
use crate::keynav;
use crate::ui;
use cbtv::home_feed::{self as hf, CustomRow};
use cbtv::prefs;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    /// Hàng vừa thêm ở trang «Thêm hàng» (quay lại thì chọn sẵn hàng đó).
    static ADDED: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn focus_name(root: &gtk::Box, name: String) {
    let root = root.clone();
    ui::after(40, move || {
        let w = keynav::find_named(root.upcast_ref(), &name)
            .filter(|w| w.is_sensitive())
            .or_else(|| name.get(2..).and_then(|id| keynav::find_named(root.upcast_ref(), &format!("t:{id}"))));
        if let Some(w) = w {
            keynav::focus_and_show(&w);
        }
    });
}

fn who() -> String {
    if prefs::profiles().len() >= 2 {
        format!(" — {}", prefs::current_profile().name)
    } else {
        String::new()
    }
}

// ---- Các hàng ----

pub struct RowsPage {
    base: PageBase,
}

pub fn rows() -> Rc<dyn Page> {
    Rc::new(RowsPage { base: PageBase::new("settings") })
}

fn visible(id: &str) -> bool {
    id.starts_with("custom-") || if prefs::kids() { !hf::ADULT_ROWS.contains(&id) } else { !hf::KIDS_ROWS.contains(&id) }
}

fn movable(id: &str) -> bool {
    visible(id) && !hf::HEAD_ROWS.contains(&id)
}

fn label(id: &str) -> String {
    if id.starts_with("custom-") {
        return hf::home_custom().iter().find(|c| c.key() == id).map(|c| format!("＋ {}", hf::custom_title(c))).unwrap_or_else(|| id.to_string());
    }
    hf::row_label(id).replace("Vì …", &format!("Vì {}", prefs::current_profile().name))
}

impl RowsPage {
    fn toggle(self: &Rc<Self>, id: &str) {
        let p: Vec<(String, bool)> = hf::row_prefs().into_iter().map(|(i, on)| if i == id { (i, !on) } else { (i, on) }).collect();
        hf::save_rows(&p);
        self.build(Some(format!("t:{id}")));
    }

    fn mv(self: &Rc<Self>, id: &str, up: bool) {
        let mut p = hf::row_prefs();
        let Some(i) = p.iter().position(|x| x.0 == id) else { return };
        let j = if up { (0..i).rev().find(|&k| movable(&p[k].0)) } else { (i + 1..p.len()).find(|&k| movable(&p[k].0)) };
        let Some(j) = j else { return };
        p.swap(i, j);
        hf::save_rows(&p);
        self.build(Some(format!("{}{id}", if up { "u:" } else { "d:" })));
    }

    fn remove(self: &Rc<Self>, id: &str) {
        let me = self.clone();
        let id = id.to_string();
        ui::local(async move {
            let Some(c) = hf::home_custom().into_iter().find(|c| c.key() == id) else { return };
            if !app().confirm(&format!("Xoá hàng «{}»?", hf::custom_title(&c)), "", "Xoá", "Huỷ").await {
                return;
            }
            let rest: Vec<CustomRow> = hf::home_custom().into_iter().filter(|x| x.key() != id).collect();
            hf::set_home_custom(&rest);
            let p: Vec<(String, bool)> = hf::row_prefs().into_iter().filter(|x| x.0 != id).collect();
            hf::save_rows(&p);
            me.build(Some("add".into()));
        });
    }

    fn build(self: &Rc<Self>, focus: Option<String>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header(&format!("Các hàng trên trang chủ{}", who())));
        let n = ui::dim(&format!(
            "Hàng tắt thì không tải. «Có tập mới» và «Xem tiếp» luôn ở đầu trang; hàng nào không đủ phim thì tự ẩn.{}",
            if prefs::kids() { " Hồ sơ trẻ em: chỉ các hàng hoạt hình hợp trẻ em." } else { "" }
        ));
        n.set_margin_bottom(px(14.0));
        b.add(&n);
        let shown: Vec<(String, bool)> = hf::row_prefs().into_iter().filter(|x| visible(&x.0)).collect();
        let mut ordered: Vec<(String, bool)> = shown.iter().filter(|x| hf::HEAD_ROWS.contains(&x.0.as_str())).cloned().collect();
        ordered.extend(shown.iter().filter(|x| !hf::HEAD_ROWS.contains(&x.0.as_str())).cloned());
        let others: Vec<String> = ordered.iter().filter(|x| !hf::HEAD_ROWS.contains(&x.0.as_str())).map(|x| x.0.clone()).collect();
        for (id, on) in &ordered {
            let row = ui::hbox(10.0);
            row.set_margin_bottom(px(8.0));
            let me = self.clone();
            let id2 = id.clone();
            let t = ui::pill(&format!("{}{}{}", if *on { "✓  " } else { "✗  " }, label(id), if *on { "" } else { "  (tắt)" }), move || me.toggle(&id2));
            t.set_widget_name(&format!("t:{id}"));
            t.set_size_request(px(560.0), -1);
            if let Some(l) = t.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
                l.set_xalign(0.0);
            }
            t.set_opacity(if *on { 1.0 } else { 0.6 });
            row.pack_start(&t, false, false, 0);
            if !hf::HEAD_ROWS.contains(&id.as_str()) {
                let at = others.iter().position(|x| x == id).unwrap_or(0);
                let cell = |text: &str, uid: String, enabled: bool, f: Box<dyn Fn()>| {
                    let c = ui::pill(text, f);
                    c.set_widget_name(&uid);
                    c.set_sensitive(enabled);
                    c.set_size_request(px(58.0), -1);
                    row.pack_start(&c, false, false, 0);
                };
                let (m1, i1) = (self.clone(), id.clone());
                cell("▲", format!("u:{id}"), at > 0, Box::new(move || m1.mv(&i1, true)));
                let (m2, i2) = (self.clone(), id.clone());
                cell("▼", format!("d:{id}"), at + 1 < others.len(), Box::new(move || m2.mv(&i2, false)));
                if id.starts_with("custom-") {
                    let (m3, i3) = (self.clone(), id.clone());
                    cell("✕", format!("x:{id}"), true, Box::new(move || m3.remove(&i3)));
                }
            }
            b.add(&row);
        }
        let n = hf::home_custom().len();
        let add = ui::pill(
            &if n < hf::CUSTOM_MAX { "＋  Thêm hàng…".to_string() } else { format!("Đã đủ {} hàng tự thêm", hf::CUSTOM_MAX) },
            || {
                if hf::home_custom().len() < hf::CUSTOM_MAX {
                    app().push(Rc::new(AddRowPage { base: PageBase::new("settings"), sel: RefCell::new(("hoathinh".into(), String::new(), String::new(), String::new())) }));
                }
            },
        );
        add.set_widget_name("add");
        add.set_opacity(if n < hf::CUSTOM_MAX { 1.0 } else { 0.5 });
        let me = self.clone();
        let reset = ui::pill("Khôi phục mặc định", move || {
            prefs::set_home_rows(&[]);
            me.build(Some("t:new-eps".into()));
        });
        let r = ui::row(&[add.upcast(), reset.upcast()]);
        ui::margins(&r, 10.0, 0.0, 6.0, 0.0);
        b.add(&r);
        b.add(&ui::dim(
            "Hàng tự thêm: chủ đề có sẵn, hoặc tự chọn loại phim, nước, thể loại, ngôn ngữ. «Khôi phục mặc định» trả thứ tự và bật lại \
             mọi hàng, giữ hàng tự thêm.",
        ));
        if let Some(f) = focus {
            focus_name(&b.body, f);
        }
    }
}

impl Page for RowsPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build(None);
    }

    fn returned(self: Rc<Self>) {
        let added = ADDED.with(|a| a.borrow_mut().take());
        self.build(Some(added.map(|k| format!("t:{k}")).unwrap_or_else(|| "add".into())));
    }
}

// ---- Thêm hàng ----

pub struct AddRowPage {
    base: PageBase,
    /// (loại phim, nước, thể loại, ngôn ngữ)
    sel: RefCell<(String, String, String, String)>,
}

impl AddRowPage {
    fn row(&self) -> CustomRow {
        let s = self.sel.borrow();
        CustomRow::filter(&format!("c{}", cbtv::util::now_ms()), &s.0, &s.1, &s.2, &s.3)
    }

    fn build(self: &Rc<Self>, focus: Option<String>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Thêm hàng trên trang chủ"));
        let kids = prefs::kids();
        let existing = hf::home_custom();
        let topics: Vec<(&str, &str)> = hf::TOPICS
            .iter()
            .filter(|t| (!kids || hf::KIDS_TOPICS.contains(&t.0)) && existing.iter().all(|c| c.topic.as_deref() != Some(t.0)))
            .copied()
            .collect();
        if !topics.is_empty() {
            let t = ui::label("Chủ đề có sẵn", &["h2"], true);
            ui::margins(&t, 4.0, 0.0, 8.0, 0.0);
            b.add(&t);
            let wrap = ui::flow(12.0, 12.0);
            for (id, lab) in topics {
                let me = self.clone();
                let id2 = id.to_string();
                ui::add_flow(&wrap, ui::pill(lab, move || me.add(CustomRow::for_topic(&id2))).upcast_ref());
            }
            b.add(&wrap);
        }
        let t = ui::label("Tự chọn", &["h2"], true);
        ui::margins(&t, 10.0, 0.0, 8.0, 0.0);
        b.add(&t);
        let types: Vec<(String, String)> = hf::TYPES.iter().take(if kids { 1 } else { hf::TYPES.len() }).map(|t| (t.1.to_string(), t.0.to_string())).collect();
        let mut countries: Vec<(String, String)> = vec![("Mọi nước".into(), String::new())];
        countries.extend(cbtv::api_site::country_list().iter().map(|(n, s)| (n.to_string(), s.to_string())));
        let mut genres: Vec<(String, String)> = vec![("Mọi thể loại".into(), String::new())];
        genres.extend(cbtv::api_site::genre_list().iter().filter(|g| !kids || !hf::adult_genres().contains(&g.1)).map(|(n, s)| (n.to_string(), s.to_string())));
        let langs: Vec<(String, String)> = hf::LANGS.iter().map(|l| (l.1.to_string(), l.0.to_string())).collect();
        let sel = self.sel.borrow().clone();
        self.field("Loại phim", types, &sel.0, 0);
        self.field("Nước", countries, &sel.1, 1);
        self.field("Thể loại", genres, &sel.2, 2);
        self.field("Ngôn ngữ", langs, &sel.3, 3);
        let me = self.clone();
        let add = ui::pill_tag(&format!("＋  Thêm hàng «{}»", hf::custom_title(&self.row())), Some("active"), move || me.add(me.row()));
        add.set_widget_name("add");
        add.set_halign(gtk::Align::Start);
        ui::margins(&add, 8.0, 0.0, 10.0, 0.0);
        b.add(&add);
        b.add(&ui::dim("Phim lấy từ KKPhim (lọc được cả bốn mục), không có KKPhim thì VSMOV; phim cùng tên ở các trang khác tự gộp vào."));
        if let Some(f) = focus {
            focus_name(&b.body, f);
        }
    }

    /// Một dòng chọn: bấm mở hộp chọn của app (đi bằng phím), chọn xong vẽ lại, giữ ô ở dòng đó.
    fn field(self: &Rc<Self>, title: &str, options: Vec<(String, String)>, value: &str, which: usize) {
        let name = options.iter().find(|o| o.1 == value).map(|o| o.0.clone()).unwrap_or_else(|| value.to_string());
        let me = self.clone();
        let (t2, v2) = (title.to_string(), value.to_string());
        let b = ui::pill(&format!("{title}: {name}  ▾"), move || {
            let (me, t2, v2, options) = (me.clone(), t2.clone(), v2.clone(), options.clone());
            ui::local(async move {
                let list = ui::flow(10.0, 10.0);
                list.set_size_request(px(760.0), -1);
                let mut active: Option<gtk::Button> = None;
                for (i, o) in options.iter().enumerate() {
                    let p = ui::pill_tag(&o.0, (o.1 == v2).then_some("active"), move || app().close_dialog(100 + i as i32));
                    if o.1 == v2 {
                        active = Some(p.clone());
                    }
                    ui::add_flow(&list, p.upcast_ref());
                }
                let sw = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
                sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                sw.set_size_request(-1, px(420.0));
                sw.add(&list);
                app().dialog_width(px(820.0));
                let rx = app().ask(&t2, "", &["Huỷ"], Some(sw.upcast()), 0);
                if let Some(a) = active {
                    ui::after(60, move || keynav::focus_and_show(a.upcast_ref()));
                }
                let r = rx.await.unwrap_or(-1);
                app().dialog_width(px(380.0));
                if r < 100 {
                    return;
                }
                let v = options[(r - 100) as usize].1.clone();
                {
                    let mut s = me.sel.borrow_mut();
                    match which {
                        0 => s.0 = v,
                        1 => s.1 = v,
                        2 => s.2 = v,
                        _ => s.3 = v,
                    }
                }
                me.build(Some(format!("f:{t2}")));
            });
        });
        b.set_widget_name(&format!("f:{title}"));
        b.set_halign(gtk::Align::Start);
        b.set_margin_bottom(px(8.0));
        self.base.add(&b);
    }

    fn add(self: &Rc<Self>, c: CustomRow) {
        let cur = hf::home_custom();
        if cur.iter().any(|x| x.same(&c)) {
            app().toast(&format!("Đã có hàng «{}».", hf::custom_title(&c)));
            return;
        }
        if cur.len() >= hf::CUSTOM_MAX {
            return;
        }
        let mut list = cur;
        list.push(c.clone());
        hf::set_home_custom(&list);
        ADDED.with(|a| *a.borrow_mut() = Some(c.key()));
        app().back();
    }
}

impl Page for AddRowPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build(None);
    }
}

// ---- Nguồn cho trang chủ ----

pub struct SourcesPage {
    base: PageBase,
}

pub fn sources() -> Rc<dyn Page> {
    Rc::new(SourcesPage { base: PageBase::new("settings") })
}

impl SourcesPage {
    fn toggle(self: &Rc<Self>, id: &str, ids: &[String]) {
        let mut off = prefs::home_src_off();
        if !off.remove(id) {
            if ids.iter().filter(|x| !off.contains(*x)).count() <= 1 {
                app().toast("Cần ít nhất một nguồn cho trang chủ.");
                return;
            }
            off.insert(id.to_string());
        }
        prefs::set_home_src_off(&off);
        self.build(Some(id.to_string()));
    }

    fn build(self: &Rc<Self>, focus: Option<String>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header(&format!("Nguồn cho trang chủ{}", who())));
        let n = ui::dim(&format!(
            "Nguồn tắt thì trang chủ không tải phim của nguồn đó. Hàng lọc theo thể loại / nước lấy từ KKPhim — tắt KKPhim thì lấy VSMOV. \
             Phim theo dõi vẫn báo tập mới. Cần ít nhất một nguồn; nguồn không có trong danh sách trang thì không hiện ở đây.{}",
            if prefs::home_mix_on() { "" } else { "\nTrang chủ đang theo trang đang dùng — cài đặt này dùng khi chọn «Tổng hợp mọi nguồn»." }
        ));
        n.set_margin_bottom(px(14.0));
        b.add(&n);
        let all = hf::all_sources();
        let ids = all.ids();
        let off = prefs::home_src_off();
        for (id, name) in hf::SOURCE_NAMES {
            if !ids.iter().any(|x| x == id) {
                continue;
            }
            let on = !off.contains(*id);
            let lab = if *id == "web" {
                format!("{name}: {}", all.web.iter().filter_map(|w| cbtv::site::host_of(w)).collect::<Vec<_>>().join(", "))
            } else {
                name.to_string()
            };
            let me = self.clone();
            let (id2, ids2) = (id.to_string(), ids.clone());
            let p = ui::pill(&format!("{}{lab}{}", if on { "✓  " } else { "✗  " }, if on { "" } else { "  (tắt)" }), move || me.toggle(&id2, &ids2));
            p.set_widget_name(id);
            p.set_halign(gtk::Align::Start);
            p.set_size_request(px(560.0), -1);
            if let Some(l) = p.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
                l.set_xalign(0.0);
            }
            p.set_opacity(if on { 1.0 } else { 0.6 });
            p.set_margin_bottom(px(8.0));
            b.add(&p);
        }
        if let Some(f) = focus {
            focus_name(&b.body, f);
        }
    }
}

impl Page for SourcesPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build(None);
    }
}
