//! Người xem (hồ sơ), cài đặt, giới hạn giờ xem, đồng bộ nhiều người xem — cùng ca với ProfilesTests.cs (Windows) /
//! ProfilesTests.swift / ProfilesTest.kt, WatchLimitTests.swift. Kho cài đặt / bộ đếm giờ xem là một cho cả tiến trình → các
//! test chạy lần lượt (khoá LOCK), mỗi test một thư mục dữ liệu riêng.

use cbtv::prefs;
use cbtv::sync_data::{self, SyncHistory, SyncPayload, SyncProfile, SyncSites};
use cbtv::sync_service;
use cbtv::watch_limit::{self, Block};
use cbtv::{Episode, EpisodeGroup, FilmDetail, HistoryItem};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());
static N: AtomicUsize = AtomicUsize::new(0);

struct Fresh {
    dir: PathBuf,
    _g: MutexGuard<'static, ()>,
}

impl Drop for Fresh {
    fn drop(&mut self) {
        watch_limit::set_clock(None);
        watch_limit::reset();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Kho cài đặt mới tinh (thư mục riêng).
fn fresh() -> Fresh {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("cbtv-profiles-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    prefs::load(Some(dir.clone()));
    watch_limit::set_clock(None);
    watch_limit::reset();
    Fresh { dir, _g: g }
}

fn history(slug: &str) -> HistoryItem {
    HistoryItem {
        film_url: format!("https://yanhh3d.ee/{slug}"),
        title: slug.into(),
        poster: None,
        ep_title: "Tập 1".into(),
        ep_url: format!("https://yanhh3d.ee/{slug}/tap-1"),
        time: 1,
    }
}

fn film() -> FilmDetail {
    FilmDetail {
        url: "https://yanhh3d.ee/tien-nghich".into(),
        title: "Tiên Nghịch".into(),
        groups: vec![EpisodeGroup {
            name: "Vietsub".into(),
            episodes: (1..=3).map(|i| Episode::new(format!("Tập {i}"), format!("https://yanhh3d.ee/tien-nghich/tap-{i}"))).collect(),
        }],
        ..Default::default()
    }
}

const EP1: &str = "https://yanhh3d.ee/tien-nghich/tap-1";

fn titles() -> Vec<String> {
    prefs::history().into_iter().map(|h| h.title).collect()
}

// ---- Người xem ----

#[test]
fn first_profile_keeps_old_keys() {
    let _f = fresh();
    prefs::add_history(history("tien-nghich"));
    prefs::save_position(EP1, 120, &film().url);
    prefs::set_base_url("https://phimmoiw.fun");
    assert_eq!(prefs::profiles().len(), 1);
    assert_eq!(prefs::profile_id(), "");
    assert_eq!(prefs::current_profile().name, "Cả nhà");
    assert_eq!(prefs::history().len(), 1);
    assert_eq!(prefs::position_of(EP1), 120);
    assert_eq!(prefs::base_url(), "https://phimmoiw.fun");
    // khoá cũ không hậu tố (đồng bộ / chuyển máy với bản TV)
    assert!(prefs::has("history") && prefs::has("positions_v2") && prefs::has("base_url"));
}

#[test]
fn each_profile_has_own_history_follows_and_settings() {
    let _f = fresh();
    let film = film();
    prefs::add_history(history("tien-nghich"));
    prefs::save_position(EP1, 120, &film.url);
    prefs::follow_film(&film);
    prefs::set_base_url("https://hoathinh3d.ee");
    prefs::set_seek_step(30);
    prefs::set_server(Some("hoathinh3d.ee"), Some("tm"), Some("Thuyết Minh"));
    prefs::set_intro(&film.url, 0, 90);

    let bo = prefs::add_profile("  Bố ");
    assert_eq!(bo.name, "Bố");
    assert_eq!(bo.id, "p1");
    assert_eq!(prefs::profile_id(), ""); // thêm chưa chuyển sang
    prefs::switch_profile(&bo.id);

    assert!(prefs::history().is_empty());
    assert_eq!(prefs::position_of(EP1), 0);
    assert!(prefs::watched_episodes().is_empty());
    assert!(prefs::follows().is_empty());
    assert_eq!(prefs::base_url(), "https://hoathinh3d.ee");
    assert_eq!(prefs::seek_step(), 30);
    assert_eq!(prefs::server_type(Some("hoathinh3d.ee")).as_deref(), Some("tm"));
    assert_eq!(prefs::intro(&film.url).map(|x| x.1), Some(90)); // mốc nhạc mở đầu: dùng chung

    prefs::add_history(history("vu-than"));
    prefs::set_base_url("https://mephim.ddns.net");
    prefs::set_server(Some("hoathinh3d.ee"), Some("vs"), Some("Vietsub"));
    prefs::set_seek_step(10);
    prefs::switch_profile("");
    assert_eq!(titles(), ["tien-nghich"]);
    assert_eq!(prefs::position_of(EP1), 120);
    assert_eq!(prefs::follows().len(), 1);
    assert_eq!(prefs::base_url(), "https://hoathinh3d.ee");
    assert_eq!(prefs::server_type(Some("hoathinh3d.ee")).as_deref(), Some("tm"));
    assert_eq!(prefs::seek_step(), 30);

    // Mở lại app: nhớ người xem lần trước
    prefs::switch_profile(&bo.id);
    prefs::flush();
    prefs::load(Some(prefs::data_dir()));
    assert_eq!(prefs::profile_id(), bo.id);
    assert_eq!(titles(), ["vu-than"]);
}

#[test]
fn delete_removes_only_that_profile_data() {
    let _f = fresh();
    let film = film();
    prefs::add_history(history("tien-nghich"));
    let me = prefs::add_profile("Mẹ");
    prefs::switch_profile(&me.id);
    prefs::add_history(history("vu-than"));
    prefs::set_server(Some("yanhh3d.ee"), Some("v1"), Some("VIP 1"));
    prefs::follow_film(&film);
    prefs::set_intro(&film.url, 0, 80);

    prefs::delete_profile(&me.id);
    assert_eq!(prefs::profile_id(), "");
    assert_eq!(prefs::profiles().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), [""]);
    assert_eq!(titles(), ["tien-nghich"]);
    assert!(prefs::follows().is_empty());
    assert_eq!(prefs::intro(&film.url).map(|x| x.1), Some(80));
    assert!(!prefs::has(&prefs::pk(&me.id, "history")));
    prefs::delete_profile("");
    assert_eq!(prefs::profiles().len(), 1);
}

#[test]
fn pin_lock_stores_only_hash() {
    let _f = fresh();
    let bo = prefs::add_profile("Bố");
    assert!(!bo.locked());
    assert!(prefs::check_pin(&bo, ""));
    prefs::set_profile_pin(&bo.id, Some("1234"));
    let locked = prefs::profiles().into_iter().find(|p| p.id == bo.id).unwrap();
    assert!(locked.locked());
    assert!(!locked.pin.as_deref().unwrap().contains("1234"));
    assert!(prefs::check_pin(&locked, "1234"));
    assert!(!prefs::check_pin(&locked, "4321"));
    // Cùng cách băm bản TV / iPad / Windows (đồng bộ / chuyển máy về sau không phải đặt lại)
    assert_eq!(locked.pin, Some(prefs::pin_hash("p1", "1234")));
    prefs::set_profile_pin(&bo.id, None);
    assert_eq!(prefs::profiles().into_iter().find(|p| p.id == bo.id).unwrap().pin, None);
}

#[test]
fn new_profiles_get_free_colors_and_ids() {
    let _f = fresh();
    let a = prefs::add_profile("A");
    let b = prefs::add_profile("B");
    assert_eq!(prefs::profiles().iter().map(|p| p.color).collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(prefs::profiles().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["", "p1", "p2"]);
    prefs::delete_profile(&a.id);
    let c = prefs::add_profile("C");
    assert_eq!(c.id, "p1");
    assert_eq!(c.color, 1);
    prefs::rename_profile(&b.id, "Bé Na");
    prefs::set_profile_color(&b.id, 6);
    let nb = prefs::profiles().into_iter().find(|p| p.id == b.id).unwrap();
    assert_eq!((nb.name.as_str(), nb.color), ("Bé Na", 6));
}

// ---- Đồng bộ nhiều người xem (kho cài đặt thật) ----

#[test]
fn sync_carries_every_profile() {
    let _f = fresh();
    prefs::add_history(history("tien-nghich"));
    let na = prefs::add_profile("Bé Na");
    let payload = sync_service::payload();
    assert_eq!(payload.profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Cả nhà", "Bé Na"]);
    assert_eq!(payload.platform, if cfg!(windows) { "windows" } else { "linux" });
    assert_eq!(payload.device, prefs::device_id());
    // Máy kia (TV) có người xem "bé na" (khác id) xem phim x → vào đúng người xem ở máy này
    let remote = SyncPayload {
        device: "tv".into(),
        profiles: vec![
            SyncProfile { id: "".into(), name: "Cả nhà".into(), ..Default::default() },
            SyncProfile {
                id: "p7".into(),
                name: " bé na ".into(),
                history: vec![SyncHistory::new("x", "X", "", "Tập 2", "x/2", 50)],
                ..Default::default()
            },
        ],
        sites: sync_data::sync_sites_data(),
        ..Default::default()
    };
    assert!(sync_service::absorb(&remote));
    assert_eq!(titles(), ["tien-nghich"]);
    prefs::switch_profile(&na.id);
    assert_eq!(titles(), ["X"]);
    let hist = prefs::history();
    let h = &hist[0];
    assert_eq!((h.film_url.as_str(), h.ep_title.as_str(), h.ep_url.as_str(), h.time, h.poster.as_deref()), ("x", "Tập 2", "x/2", 50, None));
    // gộp lại lần nữa: đã khớp
    assert!(!sync_service::absorb(&remote));
}

#[test]
fn deletes_and_sites_travel_with_sync() {
    let _f = fresh();
    let film = film();
    prefs::add_history(history("tien-nghich"));
    prefs::save_position(EP1, 30, &film.url);
    prefs::follow_film(&film);
    let mine = sync_data::sync_profile_data(&prefs::current_profile());
    assert_eq!(mine.history.len(), 1);
    assert!(mine.pos[0].t > 0 && mine.follows[0].t > 0);
    assert_eq!(mine.follows[0].count, 3);
    assert_eq!(mine.follows[0].latest, "Tập 3");

    // Máy kia đã xoá phim khỏi lịch sử + bỏ theo dõi sau đó, bỏ một trang, thêm một trang
    let later = cbtv::util::now_ms() + 1000;
    let mut sites = sync_data::sync_sites_data();
    let gone = sites.list[1].clone();
    sites.list.retain(|s| *s != gone);
    sites.list.push("https://moi.example".into());
    sites.del.insert(gone.clone(), later);
    sites.add.insert("https://moi.example".into(), later);
    let remote = SyncPayload {
        profiles: vec![SyncProfile {
            name: "Nhà".into(),
            hist_del: [(film.url.clone(), later)].into_iter().collect(),
            follow_del: [(film.url.clone(), later)].into_iter().collect(),
            ..Default::default()
        }],
        sites,
        ..Default::default()
    };
    assert!(sync_service::absorb(&remote));
    assert!(prefs::history().is_empty());
    assert_eq!(prefs::position_of(EP1), 0);
    assert!(prefs::follows().is_empty());
    assert!(!prefs::sites().contains(&gone));
    assert_eq!(prefs::sites().last().map(|s| s.as_str()), Some("https://moi.example"));
    assert_eq!(prefs::stamps("hist_del")[&film.url], later);
    // xem lại sau lần xoá đó thì có lại
    prefs::add_history(HistoryItem { time: later + 1, ..history("tien-nghich") });
    assert!(!sync_service::absorb(&remote));
    assert_eq!(titles(), ["tien-nghich"]);
    // danh sách trang gộp ra trống thì không ghi
    assert!(!sync_data::apply_sites(&SyncSites::default()));
    // người xem không có ở máy này thì bỏ qua
    assert!(!sync_data::apply_sync(&SyncProfile { id: "p9".into(), ..Default::default() }));
}

// ---- Giới hạn giờ xem (WatchLimitTests.cs / WatchLimitTests.swift) ----

static NOW: AtomicI64 = AtomicI64::new(0);

fn now() -> i64 {
    NOW.load(Ordering::SeqCst)
}

fn set_now(ms: i64) {
    NOW.store(ms, Ordering::SeqCst);
}

/// Giờ địa phương ngày `day`/11/2026.
fn at(h: i32, m: i32, day: i32) -> i64 {
    watch_limit::local_ms(2026, 11, day, h, m)
}

fn limits() -> Fresh {
    let f = fresh();
    watch_limit::set_clock(Some(now));
    f
}

fn watch(seconds: i64) {
    for _ in 0..seconds {
        set_now(now() + 1000);
        watch_limit::tick(true, Some(now()));
    }
}

#[test]
fn no_limit_never_blocks() {
    let _f = limits();
    set_now(at(23, 30, 5));
    watch_limit::tick(false, Some(now()));
    watch(4000);
    assert_eq!(watch_limit::blocked(None), None);
    assert_eq!(watch_limit::remaining(None), None);
}

#[test]
fn daily_limit_blocks_then_grant_adds_minutes_and_next_day_starts_over() {
    let _f = limits();
    prefs::set_daily_limit(&prefs::profile_id(), 30);
    set_now(at(15, 0, 5));
    watch_limit::tick(false, Some(now()));
    watch(29 * 60);
    assert_eq!(watch_limit::blocked(None), None);
    assert_eq!(watch_limit::remaining(None), Some(60));
    assert_eq!(watch_limit::warning(None).as_deref(), Some("Còn 1 phút xem hôm nay (Cả nhà)."));
    watch(61);
    assert_eq!(watch_limit::blocked(None), Some(Block::Daily));
    assert_eq!(watch_limit::block_reason(Block::Daily), "Cả nhà đã xem hết 30 phút của hôm nay.");
    assert_eq!(watch_limit::warning(None), None);
    watch_limit::grant(15, None);
    assert_eq!(watch_limit::blocked(None), None);
    let r = watch_limit::remaining(None).unwrap();
    assert!((14 * 60..=15 * 60).contains(&r), "còn {r}");
    set_now(at(15, 0, 6));
    watch_limit::tick(false, Some(now()));
    assert_eq!(watch_limit::remaining(None), Some(30 * 60));
}

#[test]
fn paused_time_and_gaps_do_not_count() {
    let _f = limits();
    prefs::set_daily_limit(&prefs::profile_id(), 30);
    set_now(at(10, 0, 5));
    for _ in 0..100 {
        set_now(now() + 1000);
        watch_limit::tick(false, Some(now()));
    }
    set_now(now() + 60_000);
    watch_limit::tick(true, Some(now()));
    assert_eq!(watch_limit::remaining(None), Some(30 * 60));
}

#[test]
fn bedtime_blocks_until_six_in_the_morning() {
    let _f = limits();
    prefs::set_bedtime(&prefs::profile_id(), 21 * 60);
    set_now(at(20, 59, 5));
    assert_eq!(watch_limit::blocked(None), None);
    set_now(at(21, 0, 5));
    assert_eq!(watch_limit::blocked(None), Some(Block::Bedtime));
    assert_eq!(
        watch_limit::block_message(Block::Bedtime),
        "Đã tới giờ đi ngủ của Cả nhà (từ 21:00 tới 6 giờ sáng).\nNgười lớn nhập mã PIN phụ huynh để cho xem thêm."
    );
    set_now(at(5, 59, 6));
    assert_eq!(watch_limit::blocked(None), Some(Block::Bedtime));
    set_now(at(6, 0, 6));
    assert_eq!(watch_limit::blocked(None), None);
}

#[test]
fn grant_at_bedtime_unblocks_for_a_while_or_until_morning() {
    let _f = limits();
    prefs::set_bedtime(&prefs::profile_id(), 21 * 60);
    set_now(at(21, 30, 5));
    watch_limit::grant(30, None);
    assert_eq!(watch_limit::blocked(None), None);
    set_now(at(22, 1, 5));
    assert_eq!(watch_limit::blocked(None), Some(Block::Bedtime));
    watch_limit::grant_until_morning(None);
    set_now(at(5, 59, 6));
    assert_eq!(watch_limit::blocked(None), None);
    set_now(at(21, 0, 6));
    assert_eq!(watch_limit::blocked(None), Some(Block::Bedtime));
    // trước 6 giờ sáng: tới 6 giờ sáng cùng ngày; nút cuối của «Cho xem thêm bao lâu?» là «Tới 6 giờ sáng»
    set_now(at(2, 0, 7));
    watch_limit::apply_grant(watch_limit::GRANTS.len());
    assert_eq!(prefs::unblock_until(), at(6, 0, 7));
}

#[test]
fn limits_belong_to_each_profile() {
    let _f = limits();
    let kid = prefs::add_profile("Bé Na");
    prefs::set_daily_limit(&kid.id, 45);
    assert_eq!(watch_limit::remaining(None), None);
    assert!(prefs::any_limits());
    prefs::switch_profile(&kid.id);
    set_now(at(9, 0, 5));
    assert_eq!(watch_limit::remaining(None), Some(45 * 60));
    assert_eq!(watch_limit::summary(&kid.id), "45 phút/ngày");
    prefs::set_bedtime(&kid.id, 21 * 60 + 30);
    assert_eq!(watch_limit::summary(&kid.id), "45 phút/ngày, ngủ lúc 21:30");
    assert_eq!(watch_limit::summary(""), "không giới hạn");
    prefs::switch_profile("");
    prefs::delete_profile(&kid.id);
    assert!(!prefs::any_limits());
}

#[test]
fn parent_pin() {
    let _f = fresh();
    assert!(!prefs::has_parent_pin());
    prefs::set_parent_pin(Some("1234"));
    assert!(prefs::has_parent_pin());
    assert!(prefs::check_parent_pin("1234"));
    assert!(!prefs::check_parent_pin("4321"));
    prefs::set_parent_pin(None);
    assert!(!prefs::has_parent_pin());
}

#[test]
fn watch_time_is_written_every_30_seconds() {
    let _f = limits();
    set_now(at(12, 0, 5));
    watch_limit::tick(false, Some(now()));
    watch(29);
    assert_eq!(prefs::watch_seconds(), 0);
    assert_eq!(watch_limit::used_today(None), 29);
    watch(1);
    assert_eq!(prefs::watch_seconds(), 30);
    assert_eq!(prefs::watch_day(), "2026-11-05");
    watch(5);
    watch_limit::flush(None);
    assert_eq!(prefs::watch_seconds(), 35);
}

// ---- Miễn trừ trách nhiệm (khoá disclaimer_v, theo từng máy) ----

#[test]
fn disclaimer_needed_once() {
    let _f = fresh();
    assert!(cbtv::disclaimer::needed());
    cbtv::disclaimer::accept();
    assert!(!cbtv::disclaimer::needed());
    assert_eq!(prefs::disclaimer_version(), cbtv::disclaimer::VERSION);
}
