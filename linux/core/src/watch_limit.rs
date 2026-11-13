//! Giới hạn giờ xem theo người xem — TẠM (bản tối thiểu để dựng giao diện; bản đầy đủ + test do nhánh linux-sync chuyển từ
//! Windows WatchLimit.cs). Cùng cách tính, cùng tên khoá bản TV / iPad / Windows.

use crate::prefs;
use crate::util::now_ms;
use std::sync::Mutex;

/// Hết giờ đi ngủ lúc 6:00 sáng.
pub const MORNING: i64 = 6 * 60;
pub const DAILY: [i64; 7] = [0, 30, 45, 60, 90, 120, 180];
pub const BEDTIMES: [i64; 8] = [-1, 19 * 60 + 30, 20 * 60, 20 * 60 + 30, 21 * 60, 21 * 60 + 30, 22 * 60, 23 * 60];
pub const GRANTS: [i64; 3] = [15, 30, 60];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Daily,
    Bedtime,
}

struct Pending {
    sec: f64,
    last_tick: i64,
}

static PENDING: Mutex<Pending> = Mutex::new(Pending { sec: 0.0, last_tick: 0 });

fn local(ms: i64) -> libc::tm {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let tt: libc::time_t = (ms / 1000) as libc::time_t;
    unsafe {
        libc::localtime_r(&tt, &mut tm);
    }
    tm
}

pub fn day(now: i64) -> String {
    let t = local(now);
    format!("{:04}-{:02}-{:02}", t.tm_year + 1900, t.tm_mon + 1, t.tm_mday)
}

fn minute_of_day(now: i64) -> i64 {
    let t = local(now);
    (t.tm_hour * 60 + t.tm_min) as i64
}

pub fn used_today(at: Option<i64>) -> i64 {
    let now = at.unwrap_or_else(now_ms);
    (if prefs::watch_day() == day(now) { prefs::watch_seconds() } else { 0 }) + PENDING.lock().unwrap().sec as i64
}

pub fn remaining(at: Option<i64>) -> Option<i64> {
    let now = at.unwrap_or_else(now_ms);
    let limit = prefs::daily_limit(None);
    if limit <= 0 {
        return None;
    }
    let extra = if prefs::watch_day() == day(now) { prefs::watch_extra() } else { 0 };
    Some((limit + extra) * 60 - used_today(Some(now)))
}

pub fn blocked(at: Option<i64>) -> Option<Block> {
    let now = at.unwrap_or_else(now_ms);
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

pub fn tick(playing: bool, at: Option<i64>) {
    let now = at.unwrap_or_else(now_ms);
    let flush_now = {
        let mut p = PENDING.lock().unwrap();
        let dt = (now - p.last_tick) as f64 / 1000.0;
        p.last_tick = now;
        if !playing || dt <= 0.0 || dt > 3.0 {
            return;
        }
        p.sec += dt;
        p.sec >= 30.0
    };
    if flush_now {
        flush(Some(now));
    }
}

pub fn flush(at: Option<i64>) {
    let now = at.unwrap_or_else(now_ms);
    let d = day(now);
    if prefs::watch_day() != d {
        prefs::set_watch_day(&d);
        prefs::set_watch_seconds(0);
        prefs::set_watch_extra(0);
    }
    let whole = {
        let mut p = PENDING.lock().unwrap();
        let w = p.sec as i64;
        p.sec -= w as f64;
        w
    };
    if whole > 0 {
        prefs::set_watch_seconds(prefs::watch_seconds() + whole);
    }
}

pub fn reset() {
    let mut p = PENDING.lock().unwrap();
    p.sec = 0.0;
    p.last_tick = 0;
}

pub fn grant(minutes: i64, at: Option<i64>) {
    let now = at.unwrap_or_else(now_ms);
    flush(Some(now));
    if blocked(Some(now)) == Some(Block::Bedtime) {
        prefs::set_unblock_until(now + minutes * 60_000);
    } else {
        prefs::set_watch_extra(prefs::watch_extra() + minutes);
    }
}

pub fn grant_until_morning(at: Option<i64>) {
    let now = at.unwrap_or_else(now_ms);
    let t = local(now);
    let since_midnight = ((t.tm_hour * 60 + t.tm_min) as i64 * 60 + t.tm_sec as i64) * 1000 + now % 1000;
    let mut six = now - since_midnight + MORNING * 60_000;
    if six <= now {
        six += 86_400_000;
    }
    prefs::set_unblock_until(six);
}

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

pub fn bedtime_label(minute: i64) -> String {
    if minute < 0 {
        "Không".into()
    } else {
        format!("{}:{:02}", minute / 60, minute % 60)
    }
}

pub fn minutes_text(seconds: i64) -> String {
    daily_label(((seconds + 59) / 60).max(1))
}

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
