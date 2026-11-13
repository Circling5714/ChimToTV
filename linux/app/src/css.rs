//! Giao diện: màu giống bản TV / Windows (res/values/colors.xml, App.xaml). Cỡ chữ / khoảng cách nhân theo «Cỡ giao diện».

use gtk::prelude::*;
use std::cell::RefCell;

pub const BG: &str = "#0E1014";
pub const SURFACE: &str = "#171A21";
pub const SURFACE2: &str = "#232833";
pub const SURFACE3: &str = "#2E3442";
pub const TEXT: &str = "#F2F4F8";
pub const DIM: &str = "#9AA3B2";
pub const FOCUS: &str = "#FFB020";
pub const ON_FOCUS: &str = "#14110A";
pub const BRAND: &str = "#2BAF85";
pub const WATCHED: &str = "#3A6B5C";

thread_local! {
    static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Cỡ giao diện đang dùng (1.0 = máy tính; 1.25 / 1.5 = tivi ngồi xa).
pub fn scale() -> f64 {
    cbtv::prefs::ui_scale()
}

/// Số điểm ảnh theo cỡ giao diện.
pub fn px(v: f64) -> i32 {
    (v * scale()).round() as i32
}

fn css(s: f64) -> String {
    let p = |v: f64| format!("{}px", (v * s).round() as i64);
    let f = |v: f64| format!("{:.1}px", v * s);
    format!(
        r#"
* {{ outline-width: 0; -gtk-outline-radius: 0; }}
window, tooltip, popover, menu {{ font-family: {font_text}; }}
window, .bg {{ background-color: {BG}; color: {TEXT}; }}
label {{ color: {TEXT}; }}
.dim, label.dim {{ color: {DIM}; }}
.sidebar {{ background-color: {SURFACE}; }}
.app-name {{ font-size: {f19}; font-weight: bold; }}
.site-name {{ font-size: {f13}; color: {DIM}; }}
.h1, .big, .dialog-title, .osd-title, .hud label {{ font-family: {font_display}; }}
.h1 {{ font-size: {f21}; font-weight: 600; }}
.h2 {{ font-size: {f16}; font-weight: 600; }}
.big {{ font-size: {f24}; font-weight: 600; }}
.separator-line {{ background-color: {SURFACE3}; min-height: 1px; }}

button.menu-item {{ background: transparent; background-image: none; border: none; box-shadow: none; text-shadow: none;
    border-radius: {p8}; padding: {p10} {p16}; margin: 1px {p8}; color: {TEXT}; font-size: {f16}; }}
button.menu-item label {{ color: {TEXT}; }}
button.menu-item:hover {{ background-color: {SURFACE2}; }}
button.menu-item.active {{ background-color: {SURFACE2}; box-shadow: inset {p4} 0 0 0 {BRAND}; }}
button.menu-item.active label {{ font-weight: 600; }}
button.menu-item:focus {{ background-color: {FOCUS}; box-shadow: none; }}
button.menu-item:focus label {{ color: {ON_FOCUS}; }}

button.pill {{ background-color: {SURFACE2}; background-image: none; color: {TEXT}; border-radius: {p8};
    padding: {p9} {p16}; border: 2px solid transparent; box-shadow: none; text-shadow: none; font-size: {f15}; min-height: 0; }}
button.pill label {{ color: {TEXT}; }}
button.pill:hover {{ border-color: rgba(255,255,255,0.33); }}
button.pill.active {{ background-color: {BRAND}; }}
button.pill.watched {{ background-color: {WATCHED}; }}
button.pill:focus {{ background-color: {FOCUS}; }}
button.pill:focus label {{ color: {ON_FOCUS}; }}
button.pill:disabled {{ opacity: 0.45; }}
button.small {{ padding: {p7} {p10}; }}
button.link {{ background: transparent; border: none; box-shadow: none; padding: 2px {p8}; }}
button.link label {{ color: {BRAND}; font-weight: 600; font-size: {f15}; }}
button.more-tile {{ background-color: {SURFACE2}; background-image: none; border-radius: {p10}; border: 3px solid transparent;
    box-shadow: none; padding: {p6}; }}
button.more-tile:hover {{ border-color: rgba(255,255,255,0.4); }}
button.more-tile:focus {{ border-color: {FOCUS}; }}
button.more-tile:focus label {{ color: {FOCUS}; }}
button.profile-tile {{ background: transparent; background-image: none; border: 2px solid transparent; border-radius: {p12};
    box-shadow: none; padding: {p10}; }}
button.profile-tile:hover {{ border-color: rgba(255,255,255,0.33); }}
button.profile-tile:focus {{ background-color: {FOCUS}; }}
button.profile-tile:focus label {{ color: {ON_FOCUS}; }}

entry {{ background-color: {SURFACE2}; background-image: none; color: {TEXT}; border: 2px solid {SURFACE3};
    border-radius: {p8}; padding: {p8} {p10}; font-size: {f17}; box-shadow: none; caret-color: {TEXT}; }}
entry:focus {{ border-color: {FOCUS}; }}
entry selection {{ background-color: {BRAND}; }}

.dialog-layer {{ background-color: rgba(0,0,0,0.70); }}
.dialog-layer.solid {{ background-color: {BG}; }}
.dialog {{ background-color: {SURFACE}; border-radius: {p14}; padding: {p24} {p28}; border: 1px solid {SURFACE3}; }}
.dialog-title {{ font-size: {f21}; font-weight: 600; }}
.toast {{ background-color: rgba(35,40,51,0.94); border-radius: {p10}; padding: {p12} {p20}; border: 1px solid {SURFACE3}; }}
.toast label {{ font-size: {f15}; }}

scrollbar {{ background: transparent; border: none; }}
scrollbar slider {{ background-color: rgba(255,255,255,0.33); border-radius: 4px; min-width: 6px; min-height: 6px; border: none; }}
progressbar trough {{ background-color: {SURFACE2}; min-height: 4px; border: none; }}
progressbar progress {{ background-color: {BRAND}; min-height: 4px; border: none; }}

/* Trình phát */
.player {{ background-color: #000; }}
.osd {{ background-image: linear-gradient(to bottom, rgba(0,0,0,0), rgba(0,0,0,0.85)); padding: {p46} {p26} {p18} {p26}; }}
.topbar {{ background-image: linear-gradient(to bottom, rgba(0,0,0,0.8), rgba(0,0,0,0)); padding: {p16} {p22} {p40} {p22}; }}
.osd-time {{ font-size: {f16}; font-weight: 600; }}
.osd-state {{ font-size: {f14}; color: rgba(255,255,255,0.7); }}
.osd-title {{ font-size: {f20}; font-weight: 600; }}
.box-dark {{ background-color: rgba(0,0,0,0.75); border-radius: {p14}; padding: {p22} {p30}; }}
.notice {{ background-color: rgba(35,40,51,0.9); border-radius: {p10}; padding: {p12} {p20}; }}
.notice label {{ font-size: {f16}; }}
.hud {{ background-color: rgba(0,0,0,0.69); border-radius: {p14}; padding: {p14} {p26}; }}
.hud label {{ font-size: {f28}; font-weight: 600; }}
.panel {{ background-color: rgba(23,26,33,0.94); border-left: 1px solid {SURFACE3}; }}
.panel-title {{ font-size: {f17}; font-weight: 600; }}
scale.seek trough {{ background-color: rgba(255,255,255,0.33); min-height: {p5}; border-radius: 3px; border: none; }}
scale.seek highlight {{ background-color: {BRAND}; border-radius: 3px; border: none; }}
scale.seek slider {{ background-color: #FFFFFF; min-width: {p16}; min-height: {p16}; border-radius: 50%; border: none; box-shadow: none; }}
"#,
        font_text = crate::fonts::TEXT,
        font_display = crate::fonts::DISPLAY,
        f13 = f(13.0),
        f14 = f(14.0),
        f15 = f(15.0),
        f16 = f(16.0),
        f17 = f(17.0),
        f19 = f(19.0),
        f20 = f(20.0),
        f21 = f(21.0),
        f24 = f(24.0),
        f28 = f(28.0),
        p4 = p(4.0),
        p5 = p(5.0),
        p6 = p(6.0),
        p7 = p(7.0),
        p8 = p(8.0),
        p9 = p(9.0),
        p10 = p(10.0),
        p12 = p(12.0),
        p14 = p(14.0),
        p16 = p(16.0),
        p18 = p(18.0),
        p20 = p(20.0),
        p22 = p(22.0),
        p24 = p(24.0),
        p26 = p(26.0),
        p28 = p(28.0),
        p30 = p(30.0),
        p40 = p(40.0),
        p46 = p(46.0),
    )
}

/// Nạp (lại) giao diện theo cỡ giao diện hiện tại.
pub fn install() {
    if let Some(settings) = gtk::Settings::default() {
        settings.set_property("gtk-application-prefer-dark-theme", true);
    }
    let Some(screen) = gdk::Screen::default() else { return };
    PROVIDER.with(|p| {
        if let Some(old) = p.borrow_mut().take() {
            gtk::StyleContext::remove_provider_for_screen(&screen, &old);
        }
        let provider = gtk::CssProvider::new();
        if let Err(e) = provider.load_from_data(css(scale()).as_bytes()) {
            cbtv::log::d("ui", &format!("CSS lỗi: {e}"));
        }
        gtk::StyleContext::add_provider_for_screen(&screen, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        *p.borrow_mut() = Some(provider);
    });
}

/// "#RRGGBB" → (r, g, b) 0..1 cho cairo.
pub fn rgb(hex: &str) -> (f64, f64, f64) {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    (((v >> 16) & 255) as f64 / 255.0, ((v >> 8) & 255) as f64 / 255.0, (v & 255) as f64 / 255.0)
}
