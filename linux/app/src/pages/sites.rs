//! Đổi trang phim: danh sách trang, đo nhanh, thêm / xoá, thêm lại trang mẫu. Bản Windows: SettingsPages.cs SitesPage.

use super::{Page, PageBase};
use crate::app::app;
use crate::css::px;
use crate::keynav;
use crate::ui;
use cbtv::prefs;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct SitesPage {
    base: PageBase,
    checks: RefCell<HashMap<String, String>>,
    add: gtk::Entry,
}

pub fn new() -> Rc<dyn Page> {
    let add = gtk::Entry::new();
    add.set_width_chars(30);
    add.set_placeholder_text(Some("vd yanhh3d.ee"));
    let p = Rc::new(SitesPage { base: PageBase::new("sites"), checks: RefCell::new(HashMap::new()), add });
    let weak = Rc::downgrade(&p);
    p.add.connect_activate(move |_| {
        if let Some(p) = weak.upgrade() {
            p.add_site();
        }
    });
    p
}

impl SitesPage {
    fn build(self: &Rc<Self>) {
        let focused = app().window.focused_widget().and_then(|w| w.downcast::<gtk::Button>().ok()).and_then(|b| b.label()).map(|s| s.to_string());
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Đổi trang phim"));
        let n = ui::dim("Trang đang dùng có màu xanh. Trang chậm / không vào được (nhà mạng chặn, đổi tên miền): bấm «Đo» rồi chọn trang khác.");
        n.set_margin_bottom(px(16.0));
        b.add(&n);
        let cur = prefs::base_url();
        for s in prefs::sites() {
            let row = ui::hbox(10.0);
            row.set_margin_bottom(px(10.0));
            let host = cbtv::site::host_of(&s).unwrap_or_else(|| s.clone());
            let name = format!("{}{}", if s == cur { "●  " } else { "" }, cbtv::site::display_name(&host));
            let s1 = s.clone();
            let p = ui::pill_tag(&name, (s == cur).then_some("active"), move || use_site(&s1));
            p.set_size_request(px(260.0), -1);
            row.pack_start(&p, false, false, 0);
            let (me, s2) = (self.clone(), s.clone());
            row.pack_start(&ui::pill("Đo", move || me.check(&s2)), false, false, 0);
            let (me, s3, h3) = (self.clone(), s.clone(), host.clone());
            row.pack_start(
                &ui::pill("Xoá", move || {
                    let (me, s3, h3) = (me.clone(), s3.clone(), h3.clone());
                    ui::local(async move {
                        if !app().confirm(&format!("Xoá {h3} khỏi danh sách?"), "", "Xoá", "Huỷ").await {
                            return;
                        }
                        if !prefs::remove_site(&s3) {
                            app().toast("Phải còn ít nhất một trang.");
                        }
                        app().rebuild_menu();
                        me.build();
                    });
                }),
                false,
                false,
                0,
            );
            if let Some(r) = self.checks.borrow().get(&s) {
                let t = ui::dim(r);
                t.set_valign(gtk::Align::Center);
                row.pack_start(&t, false, false, 0);
            }
            b.add(&row);
        }
        let me = self.clone();
        b.add(&ui::row(&[ui::pill("Đo tất cả", move || {
            for s in prefs::sites() {
                me.check(&s);
            }
        })
        .upcast()]));

        let h = ui::header("Thêm trang");
        h.set_margin_top(px(22.0));
        b.add(&h);
        let add = ui::hbox(10.0);
        add.set_margin_bottom(px(10.0));
        if let Some(p) = self.add.parent() {
            if let Ok(c) = p.downcast::<gtk::Container>() {
                c.remove(&self.add);
            }
        }
        add.pack_start(&self.add, false, false, 0);
        let me = self.clone();
        add.pack_start(&ui::pill("Thêm", move || me.add_site()), false, false, 0);
        b.add(&add);
        let n = ui::dim(
            "Gõ tên miền, vd yanhh3d.ee. App đọc được trang kiểu yanhh3d, theme HaLim (hoathinh3d…), kiểu PhimMoi (phimmois.fun), \
             mephim.ddns.net, KKPhim (kkphim.com), NguonC (phim.nguonc.com), VSMOV (vsmov.com) và CLB Phim Xưa (sc.k-20.xyz).",
        );
        n.set_margin_bottom(px(10.0));
        b.add(&n);
        let missing = prefs::missing_presets();
        if !missing.is_empty() {
            let l = ui::dim("Thêm lại trang mẫu:");
            ui::margins(&l, 8.0, 0.0, 8.0, 0.0);
            b.add(&l);
            let row = ui::flow(10.0, 10.0);
            for m in missing {
                let me = self.clone();
                let m2 = m.clone();
                let p = ui::pill(&format!("+ {}", cbtv::site::host_of(&m).unwrap_or_default()), move || {
                    prefs::add_site(&m2);
                    me.build();
                });
                ui::add_flow(&row, p.upcast_ref());
            }
            b.add(&row);
        }
        if let Some(text) = focused {
            let body = b.body.clone();
            ui::after(30, move || {
                if let Some(w) = keynav::targets(body.upcast_ref())
                    .into_iter()
                    .find(|w| w.clone().downcast::<gtk::Button>().ok().and_then(|b| b.label()).map(|l| l == text.as_str()).unwrap_or(false))
                {
                    w.grab_focus();
                }
            });
        }
    }

    fn add_site(self: &Rc<Self>) {
        let Some(u) = prefs::normalize_url(&self.add.text()) else {
            app().toast("Tên trang không hợp lệ.");
            return;
        };
        prefs::add_site(&u);
        self.add.set_text("");
        self.build();
        self.check(&u);
    }

    fn check(self: &Rc<Self>, s: &str) {
        self.checks.borrow_mut().insert(s.to_string(), "đang đo…".into());
        self.build();
        let me = self.clone();
        let s2 = s.to_string();
        ui::spawn(
            {
                let s = s2.clone();
                async move { cbtv::util::with_timeout(25.0, cbtv::site::check(&s)).await }
            },
            move |r| {
                let text = match r {
                    Ok(c) if c.films > 0 => format!("{} ms · {} phim", c.ms, c.films),
                    Ok(c) => format!("{} ms · không thấy phim (bị chặn / đổi giao diện?)", c.ms),
                    Err(e) => format!("lỗi: {}", cbtv::util::friendly(&e)),
                };
                me.checks.borrow_mut().insert(s2, text);
                me.build();
            },
        );
    }
}

fn use_site(s: &str) {
    let a = app();
    if s == prefs::base_url() {
        a.show_root(crate::menu::home_page());
        return;
    }
    prefs::set_base_url(s);
    a.set_categories(Vec::new());
    a.show_root(crate::menu::home_page());
    a.toast(&format!("Đã đổi sang {}.", cbtv::site::host_of(s).unwrap_or_default()));
}

impl Page for SitesPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build();
    }

    fn has_input(&self) -> bool {
        true
    }
}
