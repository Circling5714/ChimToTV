//! Cài đặt + lịch sử xem (bản Android: Prefs.kt, iPad: Prefs.swift, Windows: Prefs.cs — cùng tên khoá để đồng bộ trong nhà).
//! Lưu trong một tệp JSON ở ~/.local/share/conbangtv/prefs.json (biến CONBANG_DATA_DIR để thử bằng thư mục riêng); ghi lại
//! sau mỗi lần đổi (gộp các lần đổi liền nhau trong 400 ms).

use crate::models::{FilmDetail, Follow, HistoryItem, Position, Profile};
use crate::util::now_ms;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, OnceLock};

pub const DEFAULT_BASE: &str = "https://yanhh3d.ee";
/// Trang KKPhim — dữ liệu lấy thẳng từ API công khai phimapi.com (xem api_site).
pub const KK_SITE: &str = "https://www.kkphim.com";
/// NguonC (API phim.nguonc.com), VSMOV (API vsmov.com), CLB Phim Xưa (add-on Stremio k-20).
pub const NC_SITE: &str = "https://phim.nguonc.com";
pub const VS_SITE: &str = "https://vsmov.com";
pub const K20_SITE: &str = "https://sc.k-20.xyz";

/// Các trang mẫu có sẵn trong danh sách lần đầu cài.
pub const PRESET_SITES: &[&str] = &[
    DEFAULT_BASE,
    "https://hoathinh3d.pw",
    "https://hoathinh3d.ee",
    "https://hhkungfu.ee",
    "https://phimmois.fun",
    "https://mephim.ddns.net",
    KK_SITE,
    NC_SITE,
    VS_SITE,
    K20_SITE,
];

/// Trang mẫu thêm ở bản sau: thêm một lần vào danh sách đã lưu (xoá đi thì thôi, không thêm lại).
const ADDED_SITES: &[&str] = &[KK_SITE, NC_SITE, VS_SITE, K20_SITE];

pub const MAX_HISTORY: usize = 60;
pub const MAX_POSITIONS: usize = 400;

pub const MAX_PROFILES: usize = 8;
pub const PIN_LENGTH: usize = 4;
pub const FIRST_PROFILE_NAME: &str = "Cả nhà";

/// Màu vòng tròn của người xem (RGB, cùng thứ tự bản Android / iPad / Windows).
pub const PROFILE_COLORS: [u32; 8] = [0x2BAF85, 0xE5533D, 0x3B82F6, 0xF59E0B, 0x8B5CF6, 0x06B6D4, 0xEC4899, 0x84CC16];
pub const PROFILE_COLOR_NAMES: [&str; 8] = ["Xanh lá", "Đỏ cam", "Xanh dương", "Vàng", "Tím", "Xanh ngọc", "Hồng", "Xanh nõn chuối"];

/// Khoá riêng từng người xem (còn lại — danh sách trang, mốc nhạc mở đầu, cập nhật… — dùng chung).
const PROFILE_KEYS: &[&str] = &[
    "history", "positions_v2", "follows", "last_follow_check", "private_mode", "hist_del", "follow_del", "base_url", "seek_step",
    "auto_next", "video_fill", "limit_min", "bedtime", "watch_day", "watch_sec", "watch_extra", "unblock_until",
    // Trang chủ tổng hợp (home_feed): kiểu trang chủ, chủ đề đã chọn, tập mới nhất đã thấy, hồ sơ trẻ em, ngôn ngữ bản hay xem
    "home_mix", "home_topics", "seen_eps", "kids", "watch_langs",
    // Các hàng trang chủ (bật / tắt, thứ tự) và hàng tự thêm
    "home_rows", "home_custom", "home_src_off",
];
const PROFILE_PREFIXES: &[&str] = &["server_type@", "server_label@"];
/// Người thêm sau bắt đầu với trang phim / server ưu tiên / cách xem của người đang dùng.
const COPIED_KEYS: &[&str] = &["base_url", "seek_step", "auto_next", "video_fill", "home_mix"];

struct State {
    store: Map<String, Value>,
    file: Option<PathBuf>,
    dir: PathBuf,
    pid: Option<String>,
    history: Option<Vec<HistoryItem>>,
    positions: Option<Vec<Position>>,
    dirty: bool,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static SAVER: Condvar = Condvar::new();

fn st() -> std::sync::MutexGuard<'static, State> {
    STATE
        .get_or_init(|| {
            Mutex::new(State {
                store: Map::new(),
                file: None,
                dir: default_dir(),
                pid: None,
                history: None,
                positions: None,
                dirty: false,
            })
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn default_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("CONBANG_DATA_DIR") {
        return PathBuf::from(d);
    }
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("conbangtv")
}

/// Thư mục dữ liệu của app (prefs.json, log.txt, bộ nhớ đệm trang chủ…).
pub fn data_dir() -> PathBuf {
    st().dir.clone()
}

/// Gọi một lần lúc mở app (thử nghiệm: thư mục riêng).
pub fn load(dir: Option<PathBuf>) {
    {
        let mut s = st();
        if let Some(d) = dir {
            s.dir = d;
        }
        let _ = std::fs::create_dir_all(&s.dir);
        let file = s.dir.join("prefs.json");
        s.store = match std::fs::read(&file) {
            Ok(b) => match serde_json::from_slice::<Value>(&b) {
                Ok(Value::Object(o)) => o,
                _ => {
                    // Tệp hỏng (mất điện lúc ghi): giữ bản hỏng để xem lại, bắt đầu mới
                    let _ = std::fs::copy(&file, file.with_extension("json.bad"));
                    Map::new()
                }
            },
            Err(_) => Map::new(),
        };
        s.file = Some(file);
        s.pid = None;
        s.history = None;
        s.positions = None;
    }
    start_saver();
    add_new_presets();
}

fn start_saver() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::Builder::new()
            .name("prefs-save".into())
            .spawn(|| loop {
                {
                    let mut s = st();
                    while !s.dirty {
                        s = SAVER.wait(s).unwrap_or_else(|e| e.into_inner());
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(400));
                flush();
            })
            .ok();
    });
}

/// Trang mẫu thêm ở bản sau (ADDED_SITES): người đã có danh sách trang thì thêm một lần.
fn add_new_presets() {
    for url in ADDED_SITES {
        let key = format!("preset_added@{url}");
        if get::<bool>(&key).unwrap_or(false) {
            continue;
        }
        if let Some(list) = get::<Vec<String>>("sites") {
            if !list.iter().any(|s| s == url) {
                let mut l = list.clone();
                l.push(url.to_string());
                set_sites(l);
            }
        }
        set(&key, Some(true));
    }
}

fn changed(s: &mut State) {
    s.dirty = true;
    SAVER.notify_all();
}

/// Ghi ngay (lúc thoát app).
pub fn flush() {
    let (text, file) = {
        let mut s = st();
        s.dirty = false;
        match &s.file {
            None => return,
            Some(f) => (serde_json::to_string(&s.store).unwrap_or_default(), f.clone()),
        }
    };
    let tmp = file.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &file);
    }
}

// ---- Đọc / ghi khoá ----

pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    let s = st();
    match s.store.get(key) {
        None | Some(Value::Null) => None,
        Some(v) => serde_json::from_value(v.clone()).ok(),
    }
}

pub fn set<T: Serialize>(key: &str, value: Option<T>) {
    let mut s = st();
    match value.and_then(|v| serde_json::to_value(v).ok()) {
        None => {
            s.store.remove(key);
        }
        Some(v) => {
            s.store.insert(key.to_string(), v);
        }
    }
    changed(&mut s);
}

pub fn has(key: &str) -> bool {
    st().store.contains_key(key)
}

pub fn remove(key: &str) {
    set::<Value>(key, None);
}

/// Cổng gỡ lỗi: đặt một khoá bằng JSON (thử giới hạn giờ xem…).
pub fn debug_set(key: &str, json: &str) {
    let mut s = st();
    if let Ok(v) = serde_json::from_str::<Value>(json) {
        s.store.insert(key.to_string(), v);
    }
    s.history = None;
    s.positions = None;
    changed(&mut s);
}

/// Toàn bộ kho (cổng gỡ lỗi /prefs).
pub fn debug_dump() -> String {
    serde_json::to_string_pretty(&st().store).unwrap_or_default()
}

fn get_or<T: DeserializeOwned>(key: &str, def: T) -> T {
    get(key).unwrap_or(def)
}

/// bool mặc định true khi chưa có khoá (kiểu `!Has(k) || Get<bool>(k)`).
fn flag_on(key: &str) -> bool {
    !has(key) || get_or(key, false)
}

// ---- Người xem (hồ sơ) — cùng cách lưu bản TV / iPad / Windows ----

/// Người xem đang dùng ("" = người đầu tiên, giữ khoá cũ không hậu tố).
pub fn profile_id() -> String {
    if let Some(p) = st().pid.clone() {
        return p;
    }
    let saved: String = get_or("profile", String::new());
    let list = profiles();
    let id = if list.iter().any(|p| p.id == saved) { saved } else { list[0].id.clone() };
    st().pid = Some(id.clone());
    id
}

/// Khoá của người xem id (người đầu tiên — id "" — giữ khoá cũ không hậu tố).
pub fn pk(id: &str, key: &str) -> String {
    if id.is_empty() {
        key.to_string()
    } else {
        format!("{key}#{id}")
    }
}

/// Khoá của người xem đang dùng.
fn k(key: &str) -> String {
    pk(&profile_id(), key)
}

fn is_profile_key(b: &str) -> bool {
    PROFILE_KEYS.contains(&b) || PROFILE_PREFIXES.iter().any(|p| b.starts_with(p))
}

/// Các khoá riêng đang có của một người xem: (khoá thật, khoá gốc không hậu tố).
fn own_keys(s: &State, id: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for key in s.store.keys() {
        let b = if id.is_empty() {
            if key.contains('#') {
                None
            } else {
                Some(key.clone())
            }
        } else if let Some(stripped) = key.strip_suffix(&format!("#{id}")) {
            Some(stripped.to_string())
        } else {
            None
        };
        if let Some(b) = b {
            if is_profile_key(&b) {
                out.push((key.clone(), b));
            }
        }
    }
    out
}

pub fn profiles() -> Vec<Profile> {
    let list: Vec<Profile> = get_or("profiles", Vec::new());
    if list.is_empty() {
        vec![Profile { id: String::new(), name: FIRST_PROFILE_NAME.into(), color: 0, pin: None }]
    } else {
        list
    }
}

fn save_profiles(list: &[Profile]) {
    set("profiles", Some(list));
}

/// Người xem đang dùng.
pub fn current_profile() -> Profile {
    let id = profile_id();
    let list = profiles();
    list.iter().find(|p| p.id == id).cloned().unwrap_or_else(|| list[0].clone())
}

pub fn switch_profile(id: &str) {
    if !profiles().iter().any(|p| p.id == id) {
        return;
    }
    {
        let mut s = st();
        s.pid = Some(id.to_string());
        s.history = None;
        s.positions = None;
    }
    set("profile", Some(id));
}

/// Thêm người xem (chưa chuyển sang): lịch sử trống; trang phim / server ưu tiên / cách xem chép từ người đang dùng.
pub fn add_profile(name: &str) -> Profile {
    let mut list = profiles();
    let mut n = 1;
    while list.iter().any(|p| p.id == format!("p{n}")) {
        n += 1;
    }
    let id = format!("p{n}");
    let used: HashSet<i64> = list.iter().map(|p| p.color).collect();
    let color = (0..PROFILE_COLORS.len() as i64).find(|i| !used.contains(i)).unwrap_or(list.len() as i64 % PROFILE_COLORS.len() as i64);
    let cur = profile_id();
    let base = base_url();
    {
        let mut s = st();
        for (key, b) in own_keys(&s, &cur) {
            if !COPIED_KEYS.contains(&b.as_str()) && !PROFILE_PREFIXES.iter().any(|p| b.starts_with(p)) {
                continue;
            }
            if let Some(v) = s.store.get(&key).cloned() {
                s.store.insert(format!("{b}#{id}"), v);
            }
        }
        s.store.insert(format!("base_url#{id}"), Value::String(base));
        changed(&mut s);
    }
    let p = Profile { id, name: name.trim().to_string(), color, pin: None };
    list.push(p.clone());
    save_profiles(&list);
    p
}

pub fn rename_profile(id: &str, name: &str) {
    let mut list = profiles();
    for p in list.iter_mut().filter(|p| p.id == id) {
        p.name = name.trim().to_string();
    }
    save_profiles(&list);
}

pub fn set_profile_color(id: &str, color: i64) {
    let mut list = profiles();
    for p in list.iter_mut().filter(|p| p.id == id) {
        p.color = color;
    }
    save_profiles(&list);
}

/// Đặt mã PIN (4 số); None = tắt khoá.
pub fn set_profile_pin(id: &str, pin: Option<&str>) {
    let mut list = profiles();
    for p in list.iter_mut().filter(|p| p.id == id) {
        p.pin = pin.map(|x| pin_hash(id, x));
    }
    save_profiles(&list);
}

pub fn check_pin(p: &Profile, pin: &str) -> bool {
    match &p.pin {
        None => true,
        Some(h) => *h == pin_hash(&p.id, pin),
    }
}

/// Không lưu mã PIN thật, chỉ lưu mã băm (khoá cho trẻ em khỏi vào nhầm, không phải mật khẩu bảo mật). Cùng cách băm
/// bản TV / iPad / Windows.
pub fn pin_hash(id: &str, pin: &str) -> String {
    let d = Sha256::digest(format!("chimto-pin:{id}:{pin}").as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Xoá người xem cùng toàn bộ dữ liệu riêng; còn một người thì không xoá được.
pub fn delete_profile(id: &str) {
    let list = profiles();
    if list.len() <= 1 || !list.iter().any(|p| p.id == id) {
        return;
    }
    {
        let mut s = st();
        for (key, _) in own_keys(&s, id) {
            s.store.remove(&key);
        }
        changed(&mut s);
    }
    let rest: Vec<Profile> = list.into_iter().filter(|p| p.id != id).collect();
    save_profiles(&rest);
    if profile_id() == id {
        switch_profile(&rest[0].id);
    }
}

// ---- Giới hạn giờ xem (xem watch_limit) — cùng tên khoá bản TV / iPad / Windows ----

/// Số phút xem mỗi ngày của người xem id (None = người đang dùng; 0 = không giới hạn).
pub fn daily_limit(id: Option<&str>) -> i64 {
    let id = id.map(|s| s.to_string()).unwrap_or_else(profile_id);
    get_or(&pk(&id, "limit_min"), 0)
}

pub fn set_daily_limit(id: &str, minutes: i64) {
    set(&pk(id, "limit_min"), Some(minutes));
}

/// Giờ đi ngủ (phút trong ngày, vd 21:00 = 1260; -1 = không): từ giờ đó tới 6 giờ sáng không xem được.
pub fn bedtime(id: Option<&str>) -> i64 {
    let id = id.map(|s| s.to_string()).unwrap_or_else(profile_id);
    let key = pk(&id, "bedtime");
    if has(&key) {
        get_or(&key, -1)
    } else {
        -1
    }
}

pub fn set_bedtime(id: &str, minute: i64) {
    set(&pk(id, "bedtime"), Some(minute));
}

pub fn watch_day() -> String {
    get_or(&k("watch_day"), String::new())
}
pub fn set_watch_day(v: &str) {
    set(&k("watch_day"), Some(v));
}
pub fn watch_seconds() -> i64 {
    get_or(&k("watch_sec"), 0)
}
pub fn set_watch_seconds(v: i64) {
    set(&k("watch_sec"), Some(v));
}
pub fn watch_extra() -> i64 {
    get_or(&k("watch_extra"), 0)
}
pub fn set_watch_extra(v: i64) {
    set(&k("watch_extra"), Some(v));
}
/// Người đang xem được xem không giới hạn tới thời điểm này (ms) — phụ huynh cho thêm lúc giờ đi ngủ.
pub fn unblock_until() -> i64 {
    get_or(&k("unblock_until"), 0)
}
pub fn set_unblock_until(v: i64) {
    set(&k("unblock_until"), Some(v));
}

/// Có ai đang bị giới hạn giờ xem (khi đó thêm / xoá người xem cũng cần mã PIN phụ huynh).
pub fn any_limits() -> bool {
    profiles().iter().any(|p| daily_limit(Some(&p.id)) > 0 || bedtime(Some(&p.id)) >= 0)
}

/// Mã PIN phụ huynh (cả nhà): đặt / đổi giới hạn giờ xem, cho xem thêm khi hết giờ.
pub fn has_parent_pin() -> bool {
    get::<String>("parent_pin").is_some()
}

pub fn set_parent_pin(pin: Option<&str>) {
    match pin {
        Some(p) => set("parent_pin", Some(pin_hash("parent", p))),
        None => remove("parent_pin"),
    }
}

pub fn check_parent_pin(pin: &str) -> bool {
    get::<String>("parent_pin").map(|h| h == pin_hash("parent", pin)).unwrap_or(false)
}

/// Hỏi "Ai đang xem?" khi mở app (từ 2 người xem).
pub fn ask_profile() -> bool {
    flag_on("ask_profile")
}
pub fn set_ask_profile(v: bool) {
    set("ask_profile", Some(v));
}

/// Thêm server dự phòng KKPhim cho các trang tự có trình phát (xem kk_phim). Dùng chung cả máy.
pub fn kk_backup() -> bool {
    flag_on("kk_backup")
}
pub fn set_kk_backup(v: bool) {
    set("kk_backup", Some(v));
}

/// Server dự phòng NguonC cho yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw. Dùng chung cả máy.
pub fn nc_backup() -> bool {
    flag_on("nc_backup")
}
pub fn set_nc_backup(v: bool) {
    set("nc_backup", Some(v));
}

// ---- Trang chủ tổng hợp (home_feed) ----

/// true: trang chủ gộp phim từ mọi nguồn, xếp theo lịch sử xem; false: trang chủ của trang đang dùng.
pub fn home_mix() -> bool {
    flag_on(&k("home_mix"))
}
pub fn set_home_mix(v: bool) {
    set(&k("home_mix"), Some(v));
}

/// Hồ sơ trẻ em: trang chủ (luôn kiểu tổng hợp) chỉ có phim hoạt hình hợp trẻ em. Tắt cần mã PIN phụ huynh (nếu có).
pub fn kids() -> bool {
    get_or(&k("kids"), false)
}
pub fn set_kids(v: bool) {
    set(&k("kids"), Some(v));
}

/// Trang chủ tổng hợp đang dùng (người xem chọn, hoặc hồ sơ trẻ em).
pub fn home_mix_on() -> bool {
    home_mix() || kids()
}

/// Nguồn trang chủ tổng hợp người xem đã tắt (id: kk, nc, vs, k20, web); rỗng = mọi nguồn.
pub fn home_src_off() -> HashSet<String> {
    get_or::<Vec<String>>(&k("home_src_off"), Vec::new()).into_iter().collect()
}
pub fn set_home_src_off(v: &HashSet<String>) {
    let mut l: Vec<&String> = v.iter().collect();
    l.sort();
    set(&k("home_src_off"), Some(l));
}

/// Các hàng trang chủ theo thứ tự người xem chọn: "id" = bật, "-id" = tắt; rỗng = mặc định.
pub fn home_rows() -> Vec<String> {
    get_or(&k("home_rows"), Vec::new())
}
pub fn set_home_rows(v: &[String]) {
    set(&k("home_rows"), Some(v));
}

/// Hàng tự thêm (JSON thô — kiểu CustomRow ở home_feed). Trước 1.6 chủ đề người mới chọn lưu ở "home_topics".
pub fn home_custom_raw() -> Option<Value> {
    let key = k("home_custom");
    if !has(&key) {
        let topics: Vec<String> = get_or(&k("home_topics"), Vec::new());
        return Some(Value::Array(
            topics.into_iter().map(|t| serde_json::json!({"id": format!("t-{t}"), "topic": t})).collect(),
        ));
    }
    get(&key)
}
pub fn set_home_custom_raw(v: Value) {
    set(&k("home_custom"), Some(v));
}

/// Tập mới nhất người xem đã thấy của từng phim (khoá = tên đã chuẩn hoá — chung mọi nguồn).
pub fn seen_ep(key: &str) -> Option<f64> {
    get::<HashMap<String, f64>>(&k("seen_eps")).and_then(|d| d.get(key).copied())
}

pub fn set_seen_ep(key: &str, ep: f64) {
    if key.is_empty() {
        return;
    }
    let kk = k("seen_eps");
    let mut d: Map<String, Value> = get_or(&kk, Map::new());
    if let Some(old) = d.get(key).and_then(|v| v.as_f64()) {
        if old >= ep {
            return;
        }
    }
    d.insert(key.to_string(), serde_json::json!(ep));
    // giữ 300 mục (bỏ mục cũ nhất — thứ tự chèn)
    while d.len() > 300 {
        if let Some(first) = d.keys().next().cloned() {
            d.remove(&first);
        }
    }
    set(&kk, Some(d));
}

/// Số tập đã xem của mọi phim (theo chỗ đang xem đã lưu) — dùng tính sở thích.
pub fn watched_counts() -> HashMap<String, usize> {
    let mut m = HashMap::new();
    for p in positions() {
        if !p.film.is_empty() {
            *m.entry(p.film).or_insert(0) += 1;
        }
    }
    m
}

/// Ghi ngôn ngữ của bản vừa mở ("vs" / "tm" / "lt"), giữ 40 lần gần nhất.
pub fn record_watch_lang(lang: &str) {
    let mut list: Vec<String> = get_or(&k("watch_langs"), Vec::new());
    list.push(lang.to_string());
    let skip = list.len().saturating_sub(40);
    set(&k("watch_langs"), Some(&list[skip..]));
}

pub fn watch_langs() -> Vec<String> {
    get_or(&k("watch_langs"), Vec::new())
}

/// Tình trạng nguồn của trang chủ tổng hợp (feed) — chung cả máy.
pub fn feed_health() -> String {
    get_or("feed_health", "{}".to_string())
}
pub fn set_feed_health(v: &str) {
    set("feed_health", Some(v));
}

/// Thể loại / quốc gia của phim đã xem (FilmTags) — chung cả máy.
pub fn film_tags() -> String {
    get_or("film_tags", "{}".to_string())
}
pub fn set_film_tags(v: &str) {
    set("film_tags", Some(v));
}

/// Thu nhỏ cửa sổ app lúc đang xem → phim chạy tiếp ở cửa sổ nhỏ trên màn hình.
pub fn mini_on_minimize() -> bool {
    flag_on("mini_on_minimize")
}
pub fn set_mini_on_minimize(v: bool) {
    set("mini_on_minimize", Some(v));
}

// ---- Trang phim ----

/// Trang phim đang dùng (địa chỉ gốc, không có dấu / ở cuối). Đặt trang mới thì tự thêm vào danh sách.
pub fn base_url() -> String {
    get(&k("base_url")).or_else(|| get("base_url")).unwrap_or_else(|| DEFAULT_BASE.to_string())
}

pub fn set_base_url(v: &str) {
    set(&k("base_url"), Some(v));
    let list = sites();
    if !list.iter().any(|s| s == v) {
        let mut l = list;
        l.push(v.to_string());
        set_sites(l);
    }
}

/// Danh sách trang phim để chọn nhanh (mặc định: các trang mẫu).
pub fn sites() -> Vec<String> {
    get("sites").unwrap_or_else(|| PRESET_SITES.iter().map(|s| s.to_string()).collect())
}

pub fn set_sites(value: Vec<String>) {
    // Đồng bộ trong nhà: ghi lúc thêm / bỏ từng trang
    let old: HashSet<String> = sites().into_iter().collect();
    let mut list: Vec<String> = Vec::new();
    for s in value {
        if !list.contains(&s) {
            list.push(s);
        }
    }
    let now = now_ms();
    let mut add = stamps("sites_add");
    let mut del = stamps("sites_del");
    for s in list.iter().filter(|s| !old.contains(*s)) {
        add.insert(s.clone(), now);
    }
    for s in old.iter().filter(|s| !list.contains(s)) {
        del.insert(s.clone(), now);
    }
    set_stamps("sites_add", add);
    set_stamps("sites_del", del);
    set("sites", Some(list));
}

/// Trang tự chuyển sang tên miền mới: thay tên cũ trong danh sách (giữ chỗ), rồi dùng tên mới.
pub fn replace_site(old: &str, now: &str) {
    set_sites(sites().into_iter().map(|s| if s == old { now.to_string() } else { s }).collect());
    set_base_url(now);
}

pub fn add_site(url: &str) {
    let list = sites();
    if !list.iter().any(|s| s == url) {
        let mut l = list;
        l.push(url.to_string());
        set_sites(l);
    }
}

/// Bỏ trang khỏi danh sách (còn ít nhất một trang); đang dùng trang đó thì sang trang đầu danh sách.
pub fn remove_site(url: &str) -> bool {
    let rest: Vec<String> = sites().into_iter().filter(|s| s != url).collect();
    if rest.is_empty() {
        return false;
    }
    let first = rest[0].clone();
    set_sites(rest);
    if base_url() == url {
        set_base_url(&first);
    }
    true
}

/// Trang mẫu không còn trong danh sách (người xem đã xoá) — để thêm lại nhanh.
pub fn missing_presets() -> Vec<String> {
    let list = sites();
    PRESET_SITES.iter().filter(|p| !list.iter().any(|s| s == *p)).map(|s| s.to_string()).collect()
}

/// Server người xem chọn lần gần nhất trên từng trang (vd "tiktik").
pub fn server_type(host: Option<&str>) -> Option<String> {
    get(&k(&format!("server_type@{}", host.unwrap_or(""))))
}

pub fn set_server(host: Option<&str>, kind: Option<&str>, label: Option<&str>) {
    let h = host.unwrap_or("");
    set(&k(&format!("server_type@{h}")), kind);
    set(&k(&format!("server_label@{h}")), label);
}

pub fn seek_step() -> i64 {
    let key = k("seek_step");
    if has(&key) {
        get_or(&key, 10)
    } else {
        10
    }
}
pub fn set_seek_step(v: i64) {
    set(&k("seek_step"), Some(v));
}

pub fn auto_next() -> bool {
    flag_on(&k("auto_next"))
}
pub fn set_auto_next(v: bool) {
    set(&k("auto_next"), Some(v));
}

/// Chuẩn hoá chuỗi người dùng gõ ("yanhh3d.ee", "https://yanhh3d.ee/") → "https://yanhh3d.ee"; sai thì None.
pub fn normalize_url(input: &str) -> Option<String> {
    let mut s = input.trim().trim_end_matches('/').to_string();
    if s.is_empty() || s.contains(' ') {
        return None;
    }
    let lower = s.to_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        s = format!("https://{s}");
    }
    let u = url::Url::parse(&s).ok()?;
    let host = u.host_str()?;
    if !host.contains('.') {
        return None;
    }
    // Nhà mạng chặn tên miền trần kkphim.com: gõ «kkphim.com» là trang KKPhim mẫu (www)
    if host.eq_ignore_ascii_case("kkphim.com") {
        return Some(KK_SITE.to_string());
    }
    Some(s)
}

// ---- Giao diện (riêng bản máy tính) ----

/// Cỡ chữ / cỡ giao diện: 1.0 (máy tính), 1.25 / 1.5 (tivi ngồi xa).
pub fn ui_scale() -> f64 {
    let v: f64 = get_or("ui_scale", 0.0);
    if v == 0.0 {
        1.0
    } else {
        v.clamp(0.8, 2.0)
    }
}
pub fn set_ui_scale(v: f64) {
    set("ui_scale", Some(v));
}

/// Mở app là vào toàn màn hình (máy cắm tivi).
pub fn start_fullscreen() -> bool {
    get_or("start_fullscreen", false)
}
pub fn set_start_fullscreen(v: bool) {
    set("start_fullscreen", Some(v));
}

/// Cỡ cửa sổ lần trước: [rộng, cao, phóng to (1/0)].
pub fn window_bounds() -> Option<Vec<f64>> {
    get("window_bounds")
}
pub fn set_window_bounds(v: &[f64]) {
    set("window_bounds", Some(v));
}

/// Vượt chặn nhà mạng (xem unblock): 0 = tắt, 1 = tách gói bắt tay, 2 = tách + gửi đảo thứ tự. Cùng khoá bản TV.
/// Bật sẵn (chưa có khoá = bật mức 1).
pub fn unblock_level() -> i64 {
    if flag_on("unblock") {
        if get_or::<i64>("unblock_level", 1) == 2 {
            2
        } else {
            1
        }
    } else {
        0
    }
}

pub fn set_unblock_level(v: i64) {
    set("unblock", Some(v > 0));
    if v > 0 {
        set("unblock_level", Some(v));
    }
}

/// Trang (tên miền gốc) đã tự lên Vượt chặn Mức 2 → lúc ghi nhận (ms). Dùng chung cả nhà.
pub fn level2_hosts() -> HashMap<String, i64> {
    get_or("unblock_l2", HashMap::new())
}
pub fn set_level2_hosts(v: &HashMap<String, i64>) {
    set("unblock_l2", Some(v));
}

/// DNS mã hoá (xem doh: DoH rồi DoT, NextDNS → Cloudflare → Google, lỗi hết thì DNS của máy). Bật sẵn: khoá "doh" =
/// "AUTO" / "OFF", cùng khoá bản TV.
pub fn secure_dns() -> bool {
    get::<String>("doh").as_deref() != Some("OFF")
}
pub fn set_secure_dns(v: bool) {
    set("doh", Some(if v { "AUTO" } else { "OFF" }));
}

// ---- Proxy SOCKS5 / Shadowsocks (xem net) — cùng khoá bản TV / iPad / Windows ----

pub fn proxy_on() -> bool {
    get_or("proxy_on", false)
}
pub fn set_proxy_on(v: bool) {
    set("proxy_on", Some(v));
}
pub fn proxy_host() -> String {
    get_or("proxy_host", String::new())
}
pub fn set_proxy_host(v: &str) {
    set("proxy_host", Some(v.trim()));
}
pub fn proxy_port() -> i64 {
    if has("proxy_port") {
        get_or("proxy_port", 1080)
    } else {
        1080
    }
}
pub fn set_proxy_port(v: i64) {
    set("proxy_port", Some(v));
}
pub fn proxy_user() -> String {
    get_or("proxy_user", String::new())
}
pub fn set_proxy_user(v: &str) {
    set("proxy_user", Some(v));
}
/// Mật khẩu proxy — lưu trong tệp bí mật riêng (secret.rs, quyền 600), không nằm trong prefs.json.
pub fn proxy_pass() -> String {
    crate::secret::get("proxy_pass")
}
pub fn set_proxy_pass(v: &str) {
    crate::secret::set("proxy_pass", v);
}
/// DNS của proxy (gửi tên miền cho proxy) — mặc định; tắt = máy tự phân giải.
pub fn proxy_remote_dns() -> bool {
    flag_on("proxy_remote_dns")
}
pub fn set_proxy_remote_dns(v: bool) {
    set("proxy_remote_dns", Some(v));
}
/// Chỉ trang phim đi qua proxy (video / ảnh CDN đi thẳng).
pub fn proxy_sites_only() -> bool {
    get_or("proxy_sites_only", false)
}
pub fn set_proxy_sites_only(v: bool) {
    set("proxy_sites_only", Some(v));
}
/// Kiểu proxy: "socks5" hoặc "ss" (Shadowsocks).
pub fn proxy_kind() -> String {
    if get::<String>("proxy_kind").as_deref() == Some("ss") {
        "ss".into()
    } else {
        "socks5".into()
    }
}
pub fn set_proxy_kind(v: &str) {
    set("proxy_kind", Some(v));
}
pub fn ss_host() -> String {
    get_or("ss_host", String::new())
}
pub fn set_ss_host(v: &str) {
    set("ss_host", Some(v.trim()));
}
pub fn ss_port() -> i64 {
    get_or("ss_port", 0)
}
pub fn set_ss_port(v: i64) {
    set("ss_port", Some(v));
}
/// Kiểu mã hoá Shadowsocks (id, vd "2022-blake3-aes-256-gcm").
pub fn ss_method_id() -> String {
    get("ss_method").unwrap_or_else(|| "2022-blake3-aes-256-gcm".to_string())
}
pub fn set_ss_method_id(v: &str) {
    set("ss_method", Some(v));
}
/// Khoá / mật khẩu Shadowsocks — tệp bí mật riêng như mật khẩu proxy.
pub fn ss_key() -> String {
    crate::secret::get("ss_key")
}
pub fn set_ss_key(v: &str) {
    crate::secret::set("ss_key", v);
}

/// Bản miễn trừ trách nhiệm đã bấm «Đồng ý»; 0 = chưa. Theo từng máy, không đồng bộ.
pub fn disclaimer_version() -> i64 {
    get_or("disclaimer_v", 0)
}
pub fn set_disclaimer_version(v: i64) {
    set("disclaimer_v", Some(v));
}

/// Mặc định về mạng: máy mới cũng bật Vượt chặn mức 1 (đang mức 2 thì giữ) và DNS mã hoá — làm một lần; sau đó người
/// xem tắt thì giữ theo lựa chọn đó. Gọi lúc mở app.
pub fn apply_net_defaults() {
    if get_or("net_defaults_v1", false) {
        return;
    }
    set_unblock_level(if unblock_level() == 2 { 2 } else { 1 });
    set_secure_dns(true);
    set("net_defaults_v1", Some(true));
}

// ---- Cập nhật ----

pub fn auto_update() -> bool {
    flag_on("auto_update")
}
pub fn set_auto_update(v: bool) {
    set("auto_update", Some(v));
}
pub fn last_update_check() -> i64 {
    get_or("last_update_check", 0)
}
pub fn set_last_update_check(v: i64) {
    set("last_update_check", Some(v));
}
pub fn snooze_update(version: &str, until: i64) {
    set("snooze_version", Some(version));
    set("snooze_until", Some(until));
}
pub fn is_update_snoozed(version: &str) -> bool {
    get::<String>("snooze_version").as_deref() == Some(version) && now_ms() < get_or("snooze_until", 0)
}

// ---- Lịch sử xem ----

pub fn history() -> Vec<HistoryItem> {
    if let Some(h) = st().history.clone() {
        return h;
    }
    let h: Vec<HistoryItem> = get_or(&k("history"), Vec::new());
    st().history = Some(h.clone());
    h
}

fn save_history(list: Vec<HistoryItem>) {
    st().history = Some(list.clone());
    set(&k("history"), Some(list));
}

pub fn history_for(film_url: &str) -> Option<HistoryItem> {
    history().into_iter().find(|h| h.film_url == film_url)
}

pub fn add_history(item: HistoryItem) {
    let mut list = vec![item.clone()];
    list.extend(history().into_iter().filter(|h| h.film_url != item.film_url));
    list.truncate(MAX_HISTORY);
    save_history(list);
}

pub fn clear_history() {
    let key = k("hist_del");
    let mut del = stamps(&key);
    let now = now_ms();
    for f in history().into_iter().map(|h| h.film_url).chain(positions().into_iter().map(|p| p.film)) {
        if !f.is_empty() {
            del.insert(f, now);
        }
    }
    set_stamps(&key, del);
    save_history(Vec::new());
    save_positions(Vec::new());
}

/// Xoá một phim khỏi lịch sử, kèm chỗ đang xem / đánh dấu đã xem của các tập phim đó.
pub fn remove_history(film_url: &str) {
    let item = history_for(film_url);
    let key = k("hist_del");
    let mut del = stamps(&key);
    del.insert(film_url.to_string(), now_ms());
    set_stamps(&key, del);
    let mut eps: HashSet<String> = positions().into_iter().filter(|p| p.film == film_url).map(|p| p.ep).collect();
    if let Some(i) = item {
        eps.insert(i.ep_url);
    }
    save_positions(positions().into_iter().filter(|p| !eps.contains(&p.ep)).collect());
    save_history(history().into_iter().filter(|h| h.film_url != film_url).collect());
}

// ---- Phim theo dõi ----

pub fn follows() -> Vec<Follow> {
    get_or(&k("follows"), Vec::new())
}
pub fn set_follows(v: &[Follow]) {
    set(&k("follows"), Some(v));
}

pub fn follow_of(film_url: &str) -> Option<Follow> {
    follows().into_iter().find(|f| f.film_url == film_url)
}

pub fn follow_film(d: &FilmDetail) {
    let n = d.episode_count() as i64;
    let f = Follow {
        film_url: d.url.clone(),
        title: d.title.clone(),
        poster: d.poster.clone(),
        count: n,
        latest: d.latest_episode().map(|e| e.title.clone()).unwrap_or_default(),
        seen: n,
        t: Some(now_ms()),
    };
    let mut list = vec![f];
    list.extend(follows().into_iter().filter(|x| x.film_url != d.url));
    set_follows(&list);
}

pub fn unfollow(film_url: &str) {
    let key = k("follow_del");
    let mut del = stamps(&key);
    del.insert(film_url.to_string(), now_ms());
    set_stamps(&key, del);
    let list: Vec<Follow> = follows().into_iter().filter(|f| f.film_url != film_url).collect();
    set_follows(&list);
}

/// Cập nhật số tập mới nhất; seen = true: người xem vừa mở phim (hết "tập mới").
pub fn update_follow(film_url: &str, count: i64, latest: &str, seen: bool) {
    let mut list = follows();
    let Some(f) = list.iter_mut().find(|x| x.film_url == film_url) else { return };
    let was = f.seen;
    f.count = count;
    f.latest = latest.to_string();
    f.seen = if seen || count < was { count } else { was };
    set_follows(&list);
}

// ---- Nhạc mở đầu (mốc riêng từng phim, giây) ----

/// (đầu, hết) nhạc mở đầu; hết = -1 là mới đánh dấu đầu.
pub fn intro(film_url: &str) -> Option<(i64, i64)> {
    let s: String = get(&format!("intro@{film_url}"))?;
    let mut p = s.split(',');
    let start = p.next()?.trim().parse().ok()?;
    let end = p.next().and_then(|x| x.trim().parse().ok()).unwrap_or(-1);
    Some((start, end))
}
pub fn set_intro(film_url: &str, start: i64, end: i64) {
    set(&format!("intro@{film_url}"), Some(format!("{start},{end}")));
}
pub fn clear_intro(film_url: &str) {
    remove(&format!("intro@{film_url}"));
}

/// Lấp đầy màn hình: video rộng (21:9) phóng cho hết viền đen, cắt bớt hai bên.
pub fn video_fill() -> bool {
    get_or(&k("video_fill"), false)
}
pub fn set_video_fill(v: bool) {
    set(&k("video_fill"), Some(v));
}

// ---- Vị trí đang xem từng tập (giây) — thứ tự lưu: cũ nhất trước ----

fn positions() -> Vec<Position> {
    if let Some(p) = st().positions.clone() {
        return p;
    }
    let p: Vec<Position> = get_or(&k("positions_v2"), Vec::new());
    st().positions = Some(p.clone());
    p
}

fn save_positions(list: Vec<Position>) {
    st().positions = Some(list.clone());
    set(&k("positions_v2"), Some(list));
}

pub fn position_of(ep_url: &str) -> i64 {
    positions().iter().rev().find(|p| p.ep == ep_url).map(|p| p.sec).unwrap_or(0)
}

/// Tập đã xem hết (lưu vị trí 0 khi xem tới ~90 giây cuối).
pub fn finished(ep_url: &str) -> bool {
    positions().iter().rev().find(|p| p.ep == ep_url).map(|p| p.sec == 0).unwrap_or(false)
}

pub fn watched_episodes() -> HashSet<String> {
    positions().into_iter().map(|p| p.ep).collect()
}

pub fn save_position(ep_url: &str, seconds: i64, film_url: &str) {
    let mut list: Vec<Position> = positions().into_iter().filter(|p| p.ep != ep_url).collect();
    list.push(Position { ep: ep_url.to_string(), sec: seconds, film: film_url.to_string(), t: Some(now_ms()) });
    if list.len() > MAX_POSITIONS {
        list.drain(0..list.len() - MAX_POSITIONS);
    }
    save_positions(list);
}

// ---- Đồng bộ trong nhà (sync_data.rs) — cùng khoá bản TV / iPad / Windows ----

/// Mốc thời gian theo khoá (lúc xoá / lúc thêm): {"khoá": ms}.
pub fn stamps(key: &str) -> BTreeMap<String, i64> {
    get_or(key, BTreeMap::new())
}

/// Giữ tối đa bấy nhiêu "bia mộ" mỗi loại (mới nhất).
pub const MAX_TOMBSTONES: usize = 300;

/// Gộp hai bảng mốc (lấy mốc mới hơn), giữ MAX_TOMBSTONES mục mới nhất (SyncMerge.Tombstones).
pub fn tombstones(a: &BTreeMap<String, i64>, b: &BTreeMap<String, i64>) -> BTreeMap<String, i64> {
    let mut m = a.clone();
    for (k, v) in b {
        if m.get(k).map(|old| v > old).unwrap_or(true) {
            m.insert(k.clone(), *v);
        }
    }
    if m.len() > MAX_TOMBSTONES {
        let mut v: Vec<(String, i64)> = m.into_iter().collect();
        v.sort_by(|x, y| y.1.cmp(&x.1).then_with(|| x.0.cmp(&y.0)));
        v.truncate(MAX_TOMBSTONES);
        m = v.into_iter().collect();
    }
    m
}

pub fn set_stamps(key: &str, m: BTreeMap<String, i64>) {
    let t = tombstones(&m, &BTreeMap::new());
    set(key, Some(t));
}

/// Mã máy (ngẫu nhiên, tạo lần đầu) — máy kia nhận ra máy này qua mã này.
pub fn device_id() -> String {
    if let Some(id) = get::<String>("device_id").filter(|s| !s.is_empty()) {
        return id;
    }
    let b: [u8; 16] = rand::random();
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let id = format!("{}-{}-4{}-{}-{}", &h[0..8], &h[8..12], &h[13..16], &h[16..20], &h[20..32]);
    set("device_id", Some(&id));
    id
}

/// Lịch sử / chỗ xem / theo dõi thô của người xem id (đồng bộ).
pub fn raw_profile_data(id: &str) -> (Vec<HistoryItem>, Vec<Position>, Vec<Follow>) {
    (get_or(&pk(id, "history"), Vec::new()), get_or(&pk(id, "positions_v2"), Vec::new()), get_or(&pk(id, "follows"), Vec::new()))
}

/// Ghi dữ liệu đã gộp cho người xem id (đồng bộ).
pub fn write_profile_data(
    id: &str,
    history: Vec<HistoryItem>,
    pos: Vec<Position>,
    follows: Vec<Follow>,
    hist_del: BTreeMap<String, i64>,
    follow_del: BTreeMap<String, i64>,
) {
    set(&pk(id, "history"), Some(history));
    set(&pk(id, "positions_v2"), Some(pos));
    set(&pk(id, "follows"), Some(follows));
    set_stamps(&pk(id, "hist_del"), hist_del);
    set_stamps(&pk(id, "follow_del"), follow_del);
    let mut s = st();
    s.history = None;
    s.positions = None;
}

/// Ghi danh sách trang đã gộp (không ghi lại mốc thêm / bỏ như khi người xem tự sửa).
pub fn write_sites_data(list: Vec<String>, add: BTreeMap<String, i64>, del: BTreeMap<String, i64>) {
    set_stamps("sites_add", add);
    set_stamps("sites_del", del);
    set("sites", Some(list));
}

/// Máy đã ghép (JSON thô — kiểu SyncPeer ở sync_service).
pub fn sync_peers_raw() -> Value {
    get("sync_peers").unwrap_or(Value::Array(Vec::new()))
}
pub fn set_sync_peers_raw(v: Value) {
    set("sync_peers", Some(v));
}

/// Tự đồng bộ (mở app, thoát phim, theo dõi…).
pub fn sync_auto() -> bool {
    flag_on("sync_auto")
}
pub fn set_sync_auto(v: bool) {
    set("sync_auto", Some(v));
}

/// Lần kiểm phim theo dõi gần nhất (ms) của người xem đang dùng.
pub fn last_follow_check() -> i64 {
    get_or(&k("last_follow_check"), 0)
}
pub fn set_last_follow_check(v: i64) {
    set(&k("last_follow_check"), Some(v));
}

#[cfg(test)]
pub(crate) fn test_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("cbtv-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        load(Some(d.clone()));
        d
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_hash_matches_other_platforms() {
        // cùng công thức bản TV / iPad / Windows: sha256("chimto-pin:<id>:<pin>")
        assert_eq!(pin_hash("", "1234").len(), 64);
        assert_eq!(pin_hash("p1", "0000"), pin_hash("p1", "0000"));
        assert_ne!(pin_hash("p1", "0000"), pin_hash("p2", "0000"));
    }

    #[test]
    fn normalize() {
        assert_eq!(normalize_url("yanhh3d.ee/").as_deref(), Some("https://yanhh3d.ee"));
        assert_eq!(normalize_url("kkphim.com").as_deref(), Some(KK_SITE));
        assert_eq!(normalize_url("bad host"), None);
        assert_eq!(normalize_url("localhost"), None);
    }

    #[test]
    fn tombstones_keep_newest() {
        let mut a = BTreeMap::new();
        for i in 0..310 {
            a.insert(format!("k{i:03}"), i as i64);
        }
        let mut b = BTreeMap::new();
        b.insert("k000".to_string(), 1000);
        let m = tombstones(&a, &b);
        assert_eq!(m.len(), MAX_TOMBSTONES);
        assert_eq!(m["k000"], 1000);
        assert!(!m.contains_key("k001"));
    }
}
