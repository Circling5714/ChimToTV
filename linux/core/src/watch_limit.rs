//! Giới hạn giờ xem theo người xem (Cài đặt → Người xem → Giới hạn giờ xem): số phút xem mỗi ngày và giờ đi ngủ (từ giờ đó
//! tới 6 giờ sáng không xem được). Đếm thời gian phim thật sự chạy của người đang xem; hết giờ thì trình phát dừng phim, xem
//! thêm phải có mã PIN phụ huynh. Bản Android: WatchLimit.kt, iPad: WatchLimit.swift, Windows: WatchLimit.cs (cùng cách tính,
//! cùng tên khoá).

use crate::prefs;
use crate::util::now_ms;
use std::sync::Mutex;

/// Hết giờ đi ngủ lúc 6:00 sáng.
pub const MORNING: i64 = 6 * 60;

/// Lựa chọn số phút mỗi ngày (0 = không giới hạn) và giờ đi ngủ (phút trong ngày, -1 = không).
pub const DAILY: &[i64] = &[0, 30, 45, 60, 90, 120, 180];
pub const BEDTIMES: &[i64] = &[-1, 19 * 60 + 30, 20 * 60, 20 * 60 + 30, 21 * 60, 21 * 60 + 30, 22 * 60, 23 * 60];

/// Cho xem thêm (phút) khi hết giờ.
pub const GRANTS: &[i64] = &[15, 30, 60];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Daily,
    Bedtime,
}

struct State {
    /// Giây xem chưa ghi xuống (ghi mỗi 30 giây và khi rời trình phát).
    pending: f64,
    last_tick: i64,
    /// Đồng hồ (ms; test thay được).
    clock: fn() -> i64,
}

static STATE: Mutex<State> = Mutex::new(State { pending: 0.0, last_tick: 0, clock: now_ms });

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Đặt đồng hồ (test); None = giờ thật.
pub fn set_clock(clock: Option<fn() -> i64>) {
    state().clock = clock.unwrap_or(now_ms);
}

fn clock() -> i64 {
    let c = state().clock;
    c()
}

fn local_tm(ms: i64) -> libc::tm {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t: libc::time_t = ms.div_euclid(1000) as libc::time_t;
    unsafe {
        libc::localtime_r(&t, &mut tm);
    }
    tm
}

/// Mốc ms của một giờ địa phương (ngày tràn tháng tự chuyển, như mktime).
pub fn local_ms(year: i32, month: i32, day: i32, hour: i32, minute: i32) -> i64 {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = year - 1900;
    tm.tm_mon = month - 1;
    tm.tm_mday = day;
    tm.tm_hour = hour;
    tm.tm_min = minute;
    tm.tm_isdst = -1;
    let t = unsafe { libc::mktime(&mut tm) };
    t as i64 * 1000
}

/// Ngày địa phương "yyyy-MM-dd" của mốc ms.
pub fn day(now: i64) -> String {
    let tm = local_tm(now);
    format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday)
}

fn minute_of_day(now: i64) -> i64 {
    let tm = local_tm(now);
    (tm.tm_hour * 60 + tm.tm_min) as i64
}

/// Giây người đang xem đã xem hôm nay (kể cả phần chưa ghi).
pub fn used_today(at: Option<i64>) -> i64 {
    let now = at.unwrap_or_else(clock);
    let saved = if prefs::watch_day() == day(now) { prefs::watch_seconds() } else { 0 };
    saved + state().pending as i64
}

/// Giây còn được xem hôm nay; None = không giới hạn theo ngày.
pub fn remaining(at: Option<i64>) -> Option<i64> {
    let now = at.unwrap_or_else(clock);
    let limit = prefs::daily_limit(None);
    if limit <= 0 {
        return None;
    }
    let extra = if prefs::watch_day() == day(now) { prefs::watch_extra() } else { 0 };
    Some((limit + extra) * 60 - used_today(Some(now)))
}

/// Người đang xem có bị chặn không (giờ đi ngủ trước, rồi hết phút trong ngày); None = xem được.
pub fn blocked(at: Option<i64>) -> Option<Block> {
    let now = at.unwrap_or_else(clock);
    if now < prefs::unblock_until() {
        return None;
    }
    let bed = prefs::bedtime(None);
    if bed >= 0 {
        let m = minute_of_day(now);
        if m >= bed || m < MORNING {
            return Some(Block::Bedtime);
        }
    }
    match remaining(Some(now)) {
        Some(r) if r <= 0 => Some(Block::Daily),
        _ => None,
    }
}

/// Gọi mỗi lần đọc trạng thái trình phát: phim đang chạy thì cộng thời gian từ lần trước (bỏ khoảng hở > 3 giây).
pub fn tick(playing: bool, at: Option<i64>) {
    let now = at.unwrap_or_else(clock);
    let full = {
        let mut s = state();
        let dt = (now - s.last_tick) as f64 / 1000.0;
        s.last_tick = now;
        if !playing || dt <= 0.0 || dt > 3.0 {
            return;
        }
        s.pending += dt;
        s.pending >= 30.0
    };
    if full {
        flush(Some(now));
    }
}

/// Ghi phần đã đếm (rời trình phát, đổi người xem, sang ngày mới).
pub fn flush(at: Option<i64>) {
    let now = at.unwrap_or_else(clock);
    let d = day(now);
    if prefs::watch_day() != d {
        prefs::set_watch_day(&d);
        prefs::set_watch_seconds(0);
        prefs::set_watch_extra(0);
    }
    let whole = {
        let mut s = state();
        let whole = s.pending.trunc();
        s.pending -= whole;
        whole as i64
    };
    if whole > 0 {
        prefs::set_watch_seconds(prefs::watch_seconds() + whole);
    }
}

/// Bỏ phần đếm chưa ghi (đổi người xem — đã flush trước).
pub fn reset() {
    let mut s = state();
    s.pending = 0.0;
    s.last_tick = 0;
}

/// Cho xem thêm (đã nhập mã phụ huynh): đang giờ đi ngủ → mở khoá thêm từng ấy phút; hết phút trong ngày → cộng thêm.
pub fn grant(minutes: i64, at: Option<i64>) {
    let now = at.unwrap_or_else(clock);
    flush(Some(now));
    if blocked(Some(now)) == Some(Block::Bedtime) {
        prefs::set_unblock_until(now + minutes * 60_000);
    } else {
        prefs::set_watch_extra(prefs::watch_extra() + minutes);
    }
}

/// Không giới hạn tới 6 giờ sáng (hôm nay nếu đang trước 6 giờ, không thì sáng mai).
pub fn grant_until_morning(at: Option<i64>) {
    let now = at.unwrap_or_else(clock);
    let tm = local_tm(now);
    let (y, mo, d) = (tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday);
    let h = (MORNING / 60) as i32;
    let mut six = local_ms(y, mo, d, h, 0);
    if six <= now {
        six = local_ms(y, mo, d + 1, h, 0);
    }
    prefs::set_unblock_until(six);
}

/// "30 phút", "1 giờ", "1 giờ 30 phút", "Không giới hạn".
pub fn daily_label(minutes: i64) -> String {
    if minutes <= 0 {
        "Không giới hạn".into()
    } else if minutes < 60 {
        format!("{minutes} phút")
    } else if minutes % 60 == 0 {
        format!("{} giờ", minutes / 60)
    } else {
        format!("{} giờ {} phút", minutes / 60, minutes % 60)
    }
}

/// "21:00" hoặc "Không".
pub fn bedtime_label(minute: i64) -> String {
    if minute < 0 {
        "Không".into()
    } else {
        format!("{}:{:02}", minute / 60, minute % 60)
    }
}

/// Giây còn lại → "25 phút" (làm tròn lên, tối thiểu 1 phút).
pub fn minutes_text(seconds: i64) -> String {
    daily_label(((seconds + 59) / 60).max(1))
}

/// Tóm tắt giới hạn của một người xem, vd "1 giờ/ngày, ngủ lúc 21:00".
pub fn summary(id: &str) -> String {
    let mut parts = Vec::new();
    let (d, b) = (prefs::daily_limit(Some(id)), prefs::bedtime(Some(id)));
    if d > 0 {
        parts.push(format!("{}/ngày", daily_label(d)));
    }
    if b >= 0 {
        parts.push(format!("ngủ lúc {}", bedtime_label(b)));
    }
    if parts.is_empty() {
        "không giới hạn".into()
    } else {
        parts.join(", ")
    }
}

// ---- Lời báo cho giao diện (cùng câu chữ bản Windows: MainWindow.LimitReached, PlayerView.CheckLimit) ----

/// Lý do bị chặn, vd "Đã tới giờ đi ngủ của Bé Na (từ 21:00 tới 6 giờ sáng)." (hộp «Hết giờ xem»).
pub fn block_reason(b: Block) -> String {
    let who = prefs::current_profile().name;
    match b {
        Block::Bedtime => format!("Đã tới giờ đi ngủ của {who} (từ {} tới 6 giờ sáng).", bedtime_label(prefs::bedtime(None))),
        Block::Daily => format!("{who} đã xem hết {} của hôm nay.", daily_label(prefs::daily_limit(None))),
    }
}

/// Nội dung hộp «Hết giờ xem» (nút: «Cho xem thêm…», «Thoát» — chọn sẵn «Thoát»).
pub fn block_message(b: Block) -> String {
    format!("{}\nNgười lớn nhập mã PIN phụ huynh để cho xem thêm.", block_reason(b))
}

/// Nhắc khi còn ≤ 5 phút (trình phát hiện một lần): "Còn 5 phút xem hôm nay (Bé Na)."; None = chưa cần nhắc.
pub fn warning(at: Option<i64>) -> Option<String> {
    if blocked(at).is_some() {
        return None;
    }
    match remaining(at) {
        Some(r) if r > 0 && r <= 300 => {
            Some(format!("Còn {} xem hôm nay ({}).", minutes_text(r), prefs::current_profile().name))
        }
        _ => None,
    }
}

/// Các nút hộp «Cho xem thêm bao lâu?»: "Thêm 15 phút", "Thêm 30 phút", "Thêm 1 giờ", "Tới 6 giờ sáng".
pub fn grant_labels() -> Vec<String> {
    GRANTS.iter().map(|m| format!("Thêm {}", daily_label(*m))).chain(std::iter::once("Tới 6 giờ sáng".to_string())).collect()
}

/// Áp lựa chọn thứ i của `grant_labels()` (đã nhập đúng mã phụ huynh).
pub fn apply_grant(index: usize) {
    match GRANTS.get(index) {
        Some(m) => grant(*m, None),
        None => grant_until_morning(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(daily_label(0), "Không giới hạn");
        assert_eq!(daily_label(45), "45 phút");
        assert_eq!(daily_label(120), "2 giờ");
        assert_eq!(daily_label(90), "1 giờ 30 phút");
        assert_eq!(bedtime_label(21 * 60 + 30), "21:30");
        assert_eq!(bedtime_label(21 * 60), "21:00");
        assert_eq!(bedtime_label(-1), "Không");
        assert_eq!(minutes_text(5), "1 phút");
        assert_eq!(minutes_text(300), "5 phút");
        assert_eq!(grant_labels(), ["Thêm 15 phút", "Thêm 30 phút", "Thêm 1 giờ", "Tới 6 giờ sáng"]);
    }

    #[test]
    fn local_day_and_minutes() {
        let t = local_ms(2026, 11, 5, 21, 30);
        assert_eq!(day(t), "2026-11-05");
        assert_eq!(minute_of_day(t), 21 * 60 + 30);
        // ngày tràn tháng
        assert_eq!(day(local_ms(2026, 11, 31, 6, 0)), "2026-12-01");
    }
}
