//! Thẻ phim: ảnh bìa 2:3, nhãn tập (góc trái), số tập mới (góc phải), điểm IMDb, thanh đã xem, tên phim 2 dòng, dòng phụ.
//! Đang chọn bằng phím: viền cam + tên màu cam (như bản TV / Windows). Vẽ thẳng bằng cairo — một widget mỗi thẻ cho nhẹ.

use crate::css::{px, rgb, BRAND, DIM, FOCUS, SURFACE2, TEXT};
use cbtv::Film;
use gdk::prelude::*;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;

pub struct CardData {
    pub film: Film,
    pub sub: Option<String>,
    pub fresh: Option<String>,
    pub progress: f64,
    pix: RefCell<Option<gdk_pixbuf::Pixbuf>>,
    hover: Cell<bool>,
}

pub type Action = Rc<dyn Fn()>;

/// Cách dùng thẻ: open = Enter / bấm; remove = Delete / chuột phải (xoá khỏi lịch sử…); menu = phím Menu / M / chuột phải
/// (sửa nhanh hàng trang chủ).
#[derive(Default, Clone)]
pub struct CardOpts {
    pub sub: Option<String>,
    pub fresh: Option<String>,
    pub progress: Option<f64>,
    pub remove: Option<Action>,
    pub menu: Option<Action>,
    /// Tên để chọn lại thẻ sau khi dựng lại trang (mặc định: địa chỉ phim).
    pub uid: Option<String>,
}

pub const W: f64 = 168.0;
pub const POSTER_H: f64 = 248.0;

pub fn card(film: Film, open: Action, o: CardOpts) -> gtk::Widget {
    let has_sub = o.sub.as_deref().map(|s| !s.is_empty()).unwrap_or(false);
    let h = POSTER_H + 7.0 + 40.0 + if has_sub { 19.0 } else { 0.0 };
    let da = gtk::DrawingArea::new();
    da.set_size_request(px(W), px(h));
    da.set_can_focus(true);
    da.set_tooltip_text(Some(&film.title));
    da.set_widget_name(o.uid.as_deref().unwrap_or(&film.url));
    da.add_events(
        gdk::EventMask::BUTTON_PRESS_MASK
            | gdk::EventMask::BUTTON_RELEASE_MASK
            | gdk::EventMask::ENTER_NOTIFY_MASK
            | gdk::EventMask::LEAVE_NOTIFY_MASK
            | gdk::EventMask::KEY_PRESS_MASK
            | gdk::EventMask::FOCUS_CHANGE_MASK,
    );
    let d = Rc::new(CardData {
        film,
        sub: o.sub.clone().filter(|s| !s.is_empty()),
        fresh: o.fresh.clone(),
        progress: o.progress.unwrap_or(-1.0),
        pix: RefCell::new(None),
        hover: Cell::new(false),
    });
    {
        let d = d.clone();
        da.connect_draw(move |w, cr| {
            draw(w, cr, &d);
            glib::Propagation::Stop
        });
    }
    da.connect_focus_in_event(|w, _| {
        w.queue_draw();
        glib::Propagation::Proceed
    });
    da.connect_focus_out_event(|w, _| {
        w.queue_draw();
        glib::Propagation::Proceed
    });
    {
        let d = d.clone();
        da.connect_enter_notify_event(move |w, _| {
            d.hover.set(true);
            w.queue_draw();
            glib::Propagation::Proceed
        });
    }
    {
        let d = d.clone();
        da.connect_leave_notify_event(move |w, _| {
            d.hover.set(false);
            w.queue_draw();
            glib::Propagation::Proceed
        });
    }
    {
        let open = open.clone();
        let remove = o.remove.clone();
        let menu = o.menu.clone();
        da.connect_button_release_event(move |w, ev| {
            match ev.button() {
                1 => {
                    w.grab_focus();
                    open();
                }
                3 => {
                    w.grab_focus();
                    if let Some(m) = &menu {
                        m();
                    } else if let Some(r) = &remove {
                        r();
                    }
                }
                _ => {}
            }
            glib::Propagation::Stop
        });
    }
    da.connect_button_press_event(|_, _| glib::Propagation::Stop);
    {
        let remove = o.remove.clone();
        let menu = o.menu.clone();
        da.connect_key_press_event(move |_, ev| {
            use gdk::keys::constants as k;
            let key = ev.keyval();
            let shift = ev.state().contains(gdk::ModifierType::SHIFT_MASK);
            if key == k::Return || key == k::KP_Enter || key == k::space {
                open();
                return glib::Propagation::Stop;
            }
            if key == k::Delete {
                if let Some(r) = &remove {
                    r();
                    return glib::Propagation::Stop;
                }
            }
            if key == k::Menu || key == k::m || key == k::M || (key == k::F10 && shift) {
                if let Some(m) = &menu {
                    m();
                    return glib::Propagation::Stop;
                } else if let Some(r) = &remove {
                    r();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
    }
    // Ảnh bìa
    if let Some(url) = d.film.poster.clone().filter(|u| !u.is_empty()) {
        let s = crate::css::scale();
        let (pw, ph) = (((W - 6.0) * s).round() as i32, ((POSTER_H - 6.0) * s).round() as i32);
        let weak = da.downgrade();
        let d2 = d.clone();
        crate::images::load(&url, pw, ph, move |p| {
            if let (Some(p), Some(w)) = (p, weak.upgrade()) {
                *d2.pix.borrow_mut() = Some(p);
                w.queue_draw();
            }
        });
    }
    da.upcast()
}

/// Ảnh bìa lớn (trang phim): bo góc, cắt kiểu «lấp đầy», chữ 鵬 khi chưa có ảnh.
pub fn poster(url: Option<&str>, w: f64, h: f64) -> gtk::Widget {
    let da = gtk::DrawingArea::new();
    let (pw, ph) = (px(w), px(h));
    da.set_size_request(pw, ph);
    da.set_valign(gtk::Align::Start);
    let pix: Rc<RefCell<Option<gdk_pixbuf::Pixbuf>>> = Rc::new(RefCell::new(None));
    {
        let pix = pix.clone();
        da.connect_draw(move |w, cr| {
            let s = crate::css::scale();
            let (ww, hh) = (w.allocated_width() as f64, w.allocated_height() as f64);
            rounded(cr, 0.0, 0.0, ww, hh, 12.0 * s);
            cr.clip();
            set(cr, SURFACE2, 1.0);
            let _ = cr.paint();
            if let Some(p) = pix.borrow().as_ref() {
                cr.set_source_pixbuf(p, 0.0, 0.0);
                let _ = cr.paint();
            } else {
                let l = layout(w.upcast_ref(), "鵬", 70.0 * s, false);
                let (tw, th) = l.pixel_size();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.2);
                cr.move_to((ww - tw as f64) / 2.0, (hh - th as f64) / 2.0);
                pangocairo::functions::show_layout(cr, &l);
            }
            glib::Propagation::Stop
        });
    }
    if let Some(u) = url.filter(|u| !u.is_empty()) {
        let weak = da.downgrade();
        crate::images::load(u, pw, ph, move |p| {
            if let (Some(p), Some(w)) = (p, weak.upgrade()) {
                *pix.borrow_mut() = Some(p);
                w.queue_draw();
            }
        });
    }
    da.upcast()
}

fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, 1.5 * PI);
    cr.close_path();
}

fn set(cr: &gtk::cairo::Context, hex: &str, a: f64) {
    let (r, g, b) = rgb(hex);
    cr.set_source_rgba(r, g, b, a);
}

fn layout(w: &gtk::Widget, text: &str, size: f64, bold: bool) -> pango::Layout {
    let l = w.create_pango_layout(Some(text));
    let mut fd = w.style_context().font(gtk::StateFlags::NORMAL);
    fd.set_absolute_size(size * pango::SCALE as f64);
    fd.set_weight(if bold { pango::Weight::Semibold } else { pango::Weight::Normal });
    l.set_font_description(Some(&fd));
    l
}

/// Nhãn nhỏ bo góc (nhãn tập, tập mới, IMDb).
fn chip(w: &gtk::Widget, cr: &gtk::cairo::Context, text: &str, s: f64, x: f64, y: f64, right: bool, bottom: bool, bg: (&str, f64), fg: &str) {
    let l = layout(w, text, 12.5 * s, true);
    let (tw, th) = l.pixel_size();
    let (pw, ph) = (7.0 * s, 3.0 * s);
    let (bw, bh) = (tw as f64 + 2.0 * pw, th as f64 + 2.0 * ph);
    let bx = if right { x - bw } else { x };
    let by = if bottom { y - bh } else { y };
    set(cr, bg.0, bg.1);
    rounded(cr, bx, by, bw, bh, 6.0 * s);
    let _ = cr.fill();
    set(cr, fg, 1.0);
    cr.move_to(bx + pw, by + ph);
    pangocairo::functions::show_layout(cr, &l);
}

fn draw(w: &gtk::DrawingArea, cr: &gtk::cairo::Context, d: &CardData) {
    let s = crate::css::scale();
    let focused = w.has_focus();
    let width = w.allocated_width() as f64;
    let ph = POSTER_H * s;
    let b = 3.0 * s;
    // Khung + viền (cam khi đang chọn, sáng khi rê chuột)
    set(cr, SURFACE2, 1.0);
    rounded(cr, 0.0, 0.0, width, ph, 10.0 * s);
    let _ = cr.fill();
    // Ảnh bìa (cắt bo góc)
    let _ = cr.save();
    rounded(cr, b, b, width - 2.0 * b, ph - 2.0 * b, 8.0 * s);
    cr.clip();
    let pix = d.pix.borrow();
    if let Some(p) = pix.as_ref() {
        cr.set_source_pixbuf(p, b, b);
        let _ = cr.paint();
    } else {
        let l = layout(w.upcast_ref(), "鵬", 54.0 * s, false);
        let (tw, th) = l.pixel_size();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.2);
        cr.move_to((width - tw as f64) / 2.0, (ph - th as f64) / 2.0);
        pangocairo::functions::show_layout(cr, &l);
    }
    // Thanh đã xem
    if d.progress >= 0.0 {
        let bh = 5.0 * s;
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.4);
        cr.rectangle(b, ph - b - bh, width - 2.0 * b, bh);
        let _ = cr.fill();
        set(cr, BRAND, 1.0);
        cr.rectangle(b, ph - b - bh, (width - 2.0 * b) * d.progress.clamp(0.03, 1.0), bh);
        let _ = cr.fill();
    }
    let _ = cr.restore();
    if let Some(badge) = d.film.badge.as_deref().filter(|x| !x.is_empty()) {
        chip(w.upcast_ref(), cr, badge, s, b + 6.0 * s, b + 6.0 * s, false, false, ("#000000", 0.9), "#FFFFFF");
    }
    if let Some(fresh) = d.fresh.as_deref() {
        chip(w.upcast_ref(), cr, fresh, s, width - b - 6.0 * s, b + 6.0 * s, true, false, ("#E5533D", 1.0), "#FFFFFF");
    }
    if let Some(i) = d.film.imdb {
        chip(w.upcast_ref(), cr, &i.label(), s, width - b - 6.0 * s, ph - b - 10.0 * s, true, true, ("#F5C518", 1.0), "#000000");
    }
    if focused || d.hover.get() {
        if focused {
            set(cr, FOCUS, 1.0);
        } else {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.4);
        }
        cr.set_line_width(b);
        rounded(cr, b / 2.0, b / 2.0, width - b, ph - b, 10.0 * s - b / 2.0);
        let _ = cr.stroke();
    }
    // Tên phim (2 dòng) + dòng phụ
    let tl = layout(w.upcast_ref(), &d.film.title, 14.5 * s, false);
    tl.set_width(((width - 4.0 * s) * pango::SCALE as f64) as i32);
    tl.set_wrap(pango::WrapMode::WordChar);
    tl.set_ellipsize(pango::EllipsizeMode::End);
    tl.set_height(-2);
    set(cr, if focused { FOCUS } else { TEXT }, 1.0);
    cr.move_to(2.0 * s, ph + 7.0 * s);
    pangocairo::functions::show_layout(cr, &tl);
    if let Some(sub) = d.sub.as_deref() {
        let sl = layout(w.upcast_ref(), sub, 12.5 * s, false);
        sl.set_width(((width - 4.0 * s) * pango::SCALE as f64) as i32);
        sl.set_ellipsize(pango::EllipsizeMode::End);
        set(cr, DIM, 1.0);
        cr.move_to(2.0 * s, ph + 7.0 * s + 41.0 * s);
        pangocairo::functions::show_layout(cr, &sl);
    }
}
