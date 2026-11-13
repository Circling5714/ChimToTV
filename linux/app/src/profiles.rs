//! «Ai đang xem?» và đổi người xem (như bản TV / Windows MainWindow.PickProfile / SwitchTo).

use crate::app::app;
use crate::css::{px, rgb};
use crate::ui;
use cbtv::models::Profile;
use cbtv::prefs;
use gtk::prelude::*;
use std::cell::Cell;

thread_local! {
    static PICKING: Cell<bool> = const { Cell::new(false) };
}

pub fn pick_profile(cancellable: bool) {
    ui::local(pick_profile_async(cancellable));
}

/// «Ai đang xem?»: chọn người xem (người có khoá phải nhập mã PIN). cancellable = false: người đang dùng có khoá → phải chọn
/// được ai đó mới vào app (nền đen kín, không lộ lịch sử phía sau).
pub async fn pick_profile_async(cancellable: bool) {
    if PICKING.with(|p| p.replace(true)) {
        return;
    }
    let a = app();
    if !cancellable {
        a.dialog_solid(true);
    }
    loop {
        let list = prefs::profiles();
        let tiles = ui::flow(12.0, 12.0);
        tiles.set_margin_top(px(6.0));
        let cur = list.iter().position(|p| p.id == prefs::profile_id());
        let mut first: Option<gtk::Button> = None;
        for (i, p) in list.iter().enumerate() {
            let b = profile_tile(p, move || app().close_dialog(100 + i as i32));
            if Some(i) == cur {
                first = Some(b.clone());
            }
            ui::add_flow(&tiles, b.upcast_ref());
        }
        let buttons: &[&str] = if cancellable { &["Quản lý người xem…", "Huỷ"] } else { &["Quản lý người xem…"] };
        let rx = a.ask("Ai đang xem?", "", buttons, Some(tiles.upcast()), 0);
        if let Some(b) = first {
            ui::after(40, move || b.grab_focus());
        }
        let r = rx.await.unwrap_or(-1);
        if r == 0 {
            if cancellable || !prefs::current_profile().locked() {
                a.show_root(crate::pages::profiles_page::new());
                break;
            }
            a.toast("Chọn người xem trước.");
            continue;
        }
        if r < 100 {
            if !cancellable && prefs::current_profile().locked() {
                continue;
            }
            break;
        }
        let p = list[(r - 100) as usize].clone();
        if p.locked() && !(cancellable && p.id == prefs::profile_id()) {
            let Some(pin) = a.ask_pin(&format!("Mã PIN của {}", p.name), "Nhập mã PIN 4 số.").await else { continue };
            if !prefs::check_pin(&p, &pin) {
                a.toast("Sai mã PIN.");
                continue;
            }
        }
        switch_to(&p);
        break;
    }
    a.dialog_solid(false);
    PICKING.with(|p| p.set(false));
}

/// Đổi sang người xem p: đóng phim đang xem, về Trang chủ (trang phim / lịch sử của người đó).
pub fn switch_to(p: &Profile) {
    if p.id == prefs::profile_id() {
        return;
    }
    let a = app();
    if let Some(pl) = a.player.borrow().clone() {
        pl.close(false);
    }
    crate::limits::flush_and_reset();
    prefs::switch_profile(&p.id);
    a.stack().set_visible_child_name("shell");
    a.categories.borrow_mut().clear();
    a.rebuild_menu();
    a.show_root(crate::menu::home_page());
    a.toast(&format!("Đang xem: {}.", p.name));
}

/// Ô người xem: vòng tròn màu + chữ cái đầu, tên, 🔒 nếu có khoá.
pub fn profile_tile(p: &Profile, on_click: impl Fn() + 'static) -> gtk::Button {
    let color = prefs::PROFILE_COLORS[(p.color.max(0) as usize).min(prefs::PROFILE_COLORS.len() - 1)];
    let hex = format!("#{color:06X}");
    let initial = p.initial();
    let circle = gtk::DrawingArea::new();
    let d = px(84.0);
    circle.set_size_request(d, d);
    circle.set_halign(gtk::Align::Center);
    circle.connect_draw(move |w, cr| {
        let (r, g, b) = rgb(&hex);
        let s = w.allocated_width().min(w.allocated_height()) as f64;
        cr.set_source_rgb(r, g, b);
        cr.arc(s / 2.0, s / 2.0, s / 2.0, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
        let l = w.create_pango_layout(Some(&initial));
        let mut fd = pango::FontDescription::new();
        fd.set_absolute_size(38.0 * crate::css::scale() * pango::SCALE as f64);
        fd.set_weight(pango::Weight::Bold);
        l.set_font_description(Some(&fd));
        let (tw, th) = l.pixel_size();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.move_to((s - tw as f64) / 2.0, (s - th as f64) / 2.0);
        pangocairo::functions::show_layout(cr, &l);
        glib::Propagation::Stop
    });
    let panel = ui::vbox(8.0);
    panel.set_size_request(px(120.0), -1);
    panel.pack_start(&circle, false, false, 0);
    let name = ui::label(&format!("{}{}", p.name, if p.locked() { " 🔒" } else { "" }), &[], false);
    name.set_halign(gtk::Align::Center);
    name.set_xalign(0.5);
    name.set_max_width_chars(12);
    panel.pack_start(&name, false, false, 0);
    let b = gtk::Button::new();
    b.style_context().add_class("profile-tile");
    b.add(&panel);
    b.connect_clicked(move |_| on_click());
    b
}
