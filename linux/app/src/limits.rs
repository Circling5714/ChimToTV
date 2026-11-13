//! Giới hạn giờ xem — phần dính giao diện (lõi: cbtv::watch_limit). Hết giờ: người lớn nhập mã PIN phụ huynh để cho xem
//! thêm (rồi mở lại đúng tập, xem tiếp), hoặc thôi. Bản Windows: MainWindow.LimitReached.

use crate::app::app;
use cbtv::prefs;
use cbtv::watch_limit as wl;
use gtk::prelude::*;

pub use wl::Block;

pub fn tick(playing: bool, now: i64) {
    wl::tick(playing, Some(now));
}

pub fn blocked() -> Option<Block> {
    wl::blocked(None)
}

pub fn remaining() -> Option<i64> {
    wl::remaining(None)
}

pub fn minutes_text(sec: i64) -> String {
    wl::minutes_text(sec)
}

pub fn flush() {
    wl::flush(None);
}

/// Ghi số giây đã xem và bắt đầu đếm lại (đổi người xem).
pub fn flush_and_reset() {
    wl::flush(None);
    wl::reset();
}

/// Hết giờ xem: hỏi mã PIN phụ huynh để cho xem thêm.
pub fn limit_reached(b: Block, film: String, ep: String) {
    crate::ui::local(async move {
        let a = app();
        a.window.present();
        let who = prefs::current_profile().name;
        let why = if b == Block::Bedtime {
            format!("Đã tới giờ đi ngủ của {who} (từ {} tới 6 giờ sáng).", wl::bedtime_label(prefs::bedtime(None)))
        } else {
            format!("{who} đã xem hết {} của hôm nay.", wl::daily_label(prefs::daily_limit(None)))
        };
        loop {
            let r = a.ask_async("Hết giờ xem", &format!("{why}\nNgười lớn nhập mã PIN phụ huynh để cho xem thêm."), &["Cho xem thêm…", "Thoát"], None, 1).await;
            if r != 0 {
                return;
            }
            let Some(pin) = a.ask_pin("Mã PIN phụ huynh", "Nhập mã PIN phụ huynh (4 số) để cho xem thêm.").await else { continue };
            if !prefs::check_parent_pin(&pin) {
                a.toast("Sai mã PIN phụ huynh.");
                continue;
            }
            let mut labels: Vec<String> = wl::GRANTS.iter().map(|m| format!("Thêm {}", wl::daily_label(*m))).collect();
            labels.push("Tới 6 giờ sáng".into());
            let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
            let g = a.ask_async("Cho xem thêm bao lâu?", &format!("Người xem: {who}."), &refs, None, 0).await;
            if g < 0 {
                continue;
            }
            if (g as usize) < wl::GRANTS.len() {
                wl::grant(wl::GRANTS[g as usize], None);
            } else {
                wl::grant_until_morning(None);
            }
            a.toast("Đã cho xem thêm.");
            a.play(&film, &ep, false);
            return;
        }
    });
}
