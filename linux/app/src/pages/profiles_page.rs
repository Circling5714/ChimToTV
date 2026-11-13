//! Người xem: mỗi người một lịch sử / Xem tiếp / theo dõi / trang phim; khoá bằng mã PIN; giới hạn giờ xem (cần mã PIN phụ
//! huynh). Khi đã có giới hạn giờ xem, thêm / xoá người xem cũng cần mã PIN phụ huynh. Bản Windows: UI/ProfilesPage.cs.

use super::{Page, PageBase};
use crate::app::app;
use crate::css::{px, rgb};
use crate::ui;
use cbtv::models::Profile;
use cbtv::prefs;
use cbtv::watch_limit as wl;
use gtk::prelude::*;
use std::rc::Rc;

pub struct ProfilesPage {
    base: PageBase,
}

pub fn new() -> Rc<dyn Page> {
    Rc::new(ProfilesPage { base: PageBase::new("profile") })
}

fn circle(p: &Profile, size: f64) -> gtk::DrawingArea {
    let color = prefs::PROFILE_COLORS[(p.color.max(0) as usize).min(prefs::PROFILE_COLORS.len() - 1)];
    let hex = format!("#{color:06X}");
    let initial = p.initial();
    let da = gtk::DrawingArea::new();
    da.set_size_request(px(size), px(size));
    da.set_valign(gtk::Align::Center);
    da.connect_draw(move |w, cr| {
        let s = w.allocated_width().min(w.allocated_height()) as f64;
        let (r, g, b) = rgb(&hex);
        cr.set_source_rgb(r, g, b);
        cr.arc(s / 2.0, s / 2.0, s / 2.0, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
        let l = w.create_pango_layout(Some(&initial));
        let mut fd = pango::FontDescription::new();
        fd.set_absolute_size(s * 0.45 * pango::SCALE as f64);
        fd.set_weight(pango::Weight::Bold);
        l.set_font_description(Some(&fd));
        let (tw, th) = l.pixel_size();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.move_to((s - tw as f64) / 2.0, (s - th as f64) / 2.0);
        pangocairo::functions::show_layout(cr, &l);
        glib::Propagation::Stop
    });
    da
}

impl ProfilesPage {
    fn rebuild(self: &Rc<Self>) {
        let text = ui::focused_label();
        self.build();
        app().rebuild_menu();
        ui::refocus_label(self.base.body.upcast_ref(), text);
    }

    fn build(self: &Rc<Self>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Người xem"));
        let n = ui::dim(
            "Mỗi người một lịch sử, Xem tiếp, chỗ đang xem, phim theo dõi và trang phim riêng. Khoá bằng mã PIN để trẻ em khỏi vào \
             nhầm; giới hạn giờ xem mỗi ngày / giờ đi ngủ cần mã PIN phụ huynh. Đồng bộ trong nhà ghép người xem cùng tên (và người \
             đầu tiên của mỗi máy).",
        );
        n.set_margin_bottom(px(14.0));
        b.add(&n);
        let list = prefs::profiles();
        let cur = prefs::profile_id();
        for p in &list {
            let row = ui::hbox(10.0);
            row.set_margin_bottom(px(10.0));
            row.pack_start(&circle(p, 40.0), false, false, 0);
            let name = ui::label(
                &format!("{}{}{}", p.name, if p.locked() { " 🔒" } else { "" }, if p.id == cur { "  (đang xem)" } else { "" }),
                &["h2"],
                false,
            );
            name.set_size_request(px(170.0), -1);
            row.pack_start(&name, false, false, 0);
            let add = |label: &str, f: Box<dyn Fn()>| {
                row.pack_start(&ui::pill(label, f), false, false, 0);
            };
            let me = self.clone();
            if p.id != cur {
                let (me, pp) = (me.clone(), p.clone());
                add("Dùng", Box::new(move || me.use_profile(pp.clone())));
            }
            let (m, pp) = (me.clone(), p.clone());
            add("Đổi tên", Box::new(move || m.rename(pp.clone())));
            let (m, pp) = (me.clone(), p.clone());
            add("Màu", Box::new(move || m.pick_color(pp.clone())));
            let (m, pp) = (me.clone(), p.clone());
            add(if p.locked() { "Bỏ khoá" } else { "Khoá bằng mã PIN" }, Box::new(move || m.toggle_lock(pp.clone())));
            let (m, pp) = (me.clone(), p.clone());
            add(&format!("Giới hạn: {}", wl::summary(&p.id)), Box::new(move || m.limits(pp.clone())));
            if list.len() > 1 {
                let (m, pp) = (me.clone(), p.clone());
                add("Xoá", Box::new(move || m.delete(pp.clone())));
            }
            b.add(&row);
        }
        if list.len() < prefs::MAX_PROFILES {
            let me = self.clone();
            b.add(&ui::row(&[ui::pill("＋  Thêm người xem", move || me.add()).upcast()]));
        }
        let t = ui::label("Hỏi «Ai đang xem?» khi mở app", &["h2"], true);
        ui::margins(&t, 18.0, 0.0, 8.0, 0.0);
        b.add(&t);
        let ask = prefs::ask_profile();
        let (m1, m2) = (self.clone(), self.clone());
        b.add(&ui::row(&[
            ui::pill_tag("Bật", ask.then_some("active"), move || {
                prefs::set_ask_profile(true);
                m1.rebuild();
            })
            .upcast(),
            ui::pill_tag("Tắt", (!ask).then_some("active"), move || {
                prefs::set_ask_profile(false);
                m2.rebuild();
            })
            .upcast(),
        ]));
        b.add(&ui::dim("Khi có từ 2 người xem; hỏi lại khi rời app hơn 30 phút. Người có khoá luôn phải nhập mã PIN."));
        let t = ui::label("Mã PIN phụ huynh", &["h2"], true);
        ui::margins(&t, 18.0, 0.0, 8.0, 0.0);
        b.add(&t);
        let mut items: Vec<gtk::Widget> = Vec::new();
        if !prefs::has_parent_pin() {
            let me = self.clone();
            items.push(
                ui::pill("Đặt mã PIN phụ huynh", move || {
                    let me = me.clone();
                    ui::local(async move {
                        if app().check_parent("").await {
                            me.rebuild();
                        }
                    });
                })
                .upcast(),
            );
        } else {
            items.push(
                ui::pill("Đổi mã PIN phụ huynh", || {
                    ui::local(async {
                        let a = app();
                        if !a.check_parent("Nhập mã PIN phụ huynh hiện tại.").await {
                            return;
                        }
                        let Some(np) = a.ask_new_pin("Mã PIN phụ huynh mới").await else { return };
                        prefs::set_parent_pin(Some(&np));
                        a.toast("Đã đổi mã PIN phụ huynh.");
                    });
                })
                .upcast(),
            );
            let me = self.clone();
            items.push(
                ui::pill("Xoá mã PIN phụ huynh", move || {
                    let me = me.clone();
                    ui::local(async move {
                        let a = app();
                        if prefs::any_limits() {
                            a.toast("Đang có giới hạn giờ xem — bỏ hết giới hạn trước rồi mới xoá được mã phụ huynh.");
                            return;
                        }
                        if !a.check_parent("Nhập mã PIN phụ huynh để xoá.").await {
                            return;
                        }
                        prefs::set_parent_pin(None);
                        me.rebuild();
                    });
                })
                .upcast(),
            );
        }
        b.add(&ui::row(&items));
        b.add(&ui::dim("Dùng để đặt giới hạn giờ xem và cho xem thêm khi hết giờ. Khác mã PIN khoá từng người xem."));
    }

    fn use_profile(self: &Rc<Self>, p: Profile) {
        ui::local(async move {
            let a = app();
            if p.locked() {
                let Some(pin) = a.ask_pin(&format!("Mã PIN của {}", p.name), "Nhập mã PIN 4 số.").await else { return };
                if !prefs::check_pin(&p, &pin) {
                    a.toast("Sai mã PIN.");
                    return;
                }
            }
            crate::profiles::switch_to(&p);
        });
    }

    async fn ask_name(title: &str, current: &str) -> Option<String> {
        let e = crate::ui::entry();
        e.set_text(current);
        e.set_max_length(24);
        e.set_width_chars(26);
        e.connect_activate(|_| app().close_dialog(0));
        let rx = app().ask(title, "", &["Lưu", "Huỷ"], Some(e.clone().upcast()), 0);
        let e2 = e.clone();
        ui::after(30, move || {
            e2.grab_focus();
            e2.select_region(0, -1);
        });
        let r = rx.await.unwrap_or(-1);
        let name = e.text().trim().to_string();
        (r == 0 && !name.is_empty()).then_some(name)
    }

    fn add(self: &Rc<Self>) {
        let me = self.clone();
        ui::local(async move {
            let a = app();
            if prefs::any_limits() && !a.check_parent("Đang có giới hạn giờ xem — nhập mã PIN phụ huynh để thêm người xem.").await {
                return;
            }
            let Some(name) = Self::ask_name("Thêm người xem", "").await else { return };
            prefs::add_profile(&name);
            a.toast(&format!("Đã thêm {name} — chọn «Dùng» để chuyển sang."));
            me.rebuild();
            crate::startup::sync_soon(0);
        });
    }

    /// Người có khoá: hỏi mã PIN của người đó (hoặc mã phụ huynh) trước khi sửa.
    async fn unlock(p: &Profile) -> bool {
        Self::unlock_by(p).await.is_some()
    }

    /// Mở khoá người xem: None = huỷ / sai; Some(true) = vừa nhập đúng mã PIN phụ huynh (khỏi hỏi lại ngay sau đó).
    async fn unlock_by(p: &Profile) -> Option<bool> {
        if !p.locked() {
            return Some(false);
        }
        let a = app();
        let pin = a.ask_pin(&format!("Mã PIN của {}", p.name), "Nhập mã PIN của người xem này (hoặc mã PIN phụ huynh).").await?;
        if prefs::has_parent_pin() && prefs::check_parent_pin(&pin) {
            return Some(true);
        }
        if prefs::check_pin(p, &pin) {
            return Some(false);
        }
        a.toast("Sai mã PIN.");
        None
    }

    fn rename(self: &Rc<Self>, p: Profile) {
        let me = self.clone();
        ui::local(async move {
            if !Self::unlock(&p).await {
                return;
            }
            let Some(name) = Self::ask_name("Đổi tên người xem", &p.name).await else { return };
            prefs::rename_profile(&p.id, &name);
            me.rebuild();
        });
    }

    fn pick_color(self: &Rc<Self>, p: Profile) {
        let me = self.clone();
        ui::local(async move {
            let tiles = ui::flow(10.0, 10.0);
            for (i, name) in prefs::PROFILE_COLOR_NAMES.iter().enumerate() {
                let b = ui::pill_tag(name, (i as i64 == p.color).then_some("active"), move || app().close_dialog(100 + i as i32));
                b.set_size_request(px(120.0), -1);
                if let Some(l) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
                    l.set_markup(&format!("<span foreground='#{:06X}'>{}</span>", prefs::PROFILE_COLORS[i], glib::markup_escape_text(name)));
                }
                ui::add_flow(&tiles, b.upcast_ref());
            }
            let r = app().ask_async(&format!("Màu của {}", p.name), "", &["Huỷ"], Some(tiles.upcast()), 0).await;
            if r < 100 {
                return;
            }
            prefs::set_profile_color(&p.id, (r - 100) as i64);
            me.rebuild();
        });
    }

    fn toggle_lock(self: &Rc<Self>, p: Profile) {
        let me = self.clone();
        ui::local(async move {
            let a = app();
            if p.locked() {
                if !Self::unlock(&p).await {
                    return;
                }
                prefs::set_profile_pin(&p.id, None);
                a.toast(&format!("Đã bỏ khoá {}.", p.name));
            } else {
                let Some(pin) = a.ask_new_pin(&format!("Khoá {} bằng mã PIN", p.name)).await else { return };
                prefs::set_profile_pin(&p.id, Some(&pin));
                a.toast(&format!("Đã khoá {} — chọn người này phải nhập mã PIN.", p.name));
            }
            me.rebuild();
        });
    }

    fn limits(self: &Rc<Self>, p: Profile) {
        ui::local(async move {
            if !app().check_parent(&format!("Nhập mã PIN phụ huynh để đặt giới hạn giờ xem cho {}.", p.name)).await {
                return;
            }
            app().push(Rc::new(LimitsPage { base: PageBase::new("profile"), id: p.id.clone() }));
        });
    }

    fn delete(self: &Rc<Self>, p: Profile) {
        let me = self.clone();
        ui::local(async move {
            let a = app();
            let Some(by_parent) = Self::unlock_by(&p).await else { return };
            if !by_parent && prefs::any_limits() && !a.check_parent("Đang có giới hạn giờ xem — nhập mã PIN phụ huynh để xoá người xem.").await {
                return;
            }
            if !a.confirm(&format!("Xoá người xem «{}»?", p.name), "Xoá cả lịch sử, Xem tiếp, phim theo dõi và cài đặt riêng của người này.", "Xoá", "Huỷ").await {
                return;
            }
            let was_current = p.id == prefs::profile_id();
            let old_site = prefs::base_url();
            if was_current {
                if let Some(pl) = a.player.borrow().clone() {
                    pl.close(false);
                }
                crate::limits::flush_and_reset();
            }
            prefs::delete_profile(&p.id);
            if was_current {
                a.toast(&format!("Đang xem: {}.", prefs::current_profile().name));
                // Mục trang phim trong menu đi theo trang của người xem: người mới dùng trang khác thì về Trang chủ để nạp lại
                // (như khi đổi người xem); cùng trang thì giữ menu, ở lại trang này
                if prefs::base_url() != old_site {
                    a.set_categories(Vec::new());
                    a.show_root(crate::menu::home_page());
                    return;
                }
                a.rebuild_menu();
            }
            me.rebuild();
        });
    }
}

impl Page for ProfilesPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build();
    }

    fn returned(self: Rc<Self>) {
        self.build();
    }
}

/// Giới hạn giờ xem của một người xem (đã nhập mã PIN phụ huynh): số phút mỗi ngày, giờ đi ngủ.
pub struct LimitsPage {
    base: PageBase,
    id: String,
}

impl LimitsPage {
    fn build(self: &Rc<Self>) {
        let Some(p) = prefs::profiles().into_iter().find(|x| x.id == self.id) else { return };
        let b = &self.base;
        b.clear();
        b.add(&ui::header(&format!("Giới hạn giờ xem — {}", p.name)));
        let n = ui::dim(
            "Đếm thời gian phim thật sự chạy (dừng / đang tải không tính). Hết giờ: phim dừng, xem thêm phải nhập mã PIN phụ huynh. \
             Tính theo từng máy.",
        );
        n.set_margin_bottom(px(10.0));
        b.add(&n);
        let t = ui::label("Mỗi ngày được xem", &["h2"], true);
        ui::margins(&t, 10.0, 0.0, 8.0, 0.0);
        b.add(&t);
        let d = ui::flow(10.0, 10.0);
        let cur_d = prefs::daily_limit(Some(&self.id));
        for m in wl::DAILY.iter().copied() {
            let me = self.clone();
            let p = ui::pill_tag(&wl::daily_label(m), (cur_d == m).then_some("active"), move || {
                let text = ui::focused_label();
                prefs::set_daily_limit(&me.id, m);
                me.build();
                ui::refocus_label(me.base.body.upcast_ref(), text);
            });
            p.set_size_request(px(100.0), -1);
            ui::add_flow(&d, p.upcast_ref());
        }
        b.add(&d);
        let t = ui::label("Giờ đi ngủ (không xem được tới 6 giờ sáng)", &["h2"], true);
        ui::margins(&t, 14.0, 0.0, 8.0, 0.0);
        b.add(&t);
        let bd = ui::flow(10.0, 10.0);
        let cur_b = prefs::bedtime(Some(&self.id));
        for m in wl::BEDTIMES.iter().copied() {
            let me = self.clone();
            let p = ui::pill_tag(&wl::bedtime_label(m), (cur_b == m).then_some("active"), move || {
                let text = ui::focused_label();
                prefs::set_bedtime(&me.id, m);
                me.build();
                ui::refocus_label(me.base.body.upcast_ref(), text);
            });
            p.set_size_request(px(90.0), -1);
            ui::add_flow(&bd, p.upcast_ref());
        }
        b.add(&bd);
        if self.id == prefs::profile_id() {
            if let Some(r) = wl::remaining(None) {
                let l = ui::dim(&format!("Hôm nay còn {}.", if r > 0 { wl::minutes_text(r) } else { "0 phút".into() }));
                l.set_margin_top(px(10.0));
                b.add(&l);
            }
        }
        b.add(&ui::row(&[ui::pill("Xong", || app().back()).upcast()]));
    }
}

impl Page for LimitsPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build();
    }
}
