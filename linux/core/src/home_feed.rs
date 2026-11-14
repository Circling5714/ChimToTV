//! Trang chủ tổng hợp (từ Windows 1.5) — bản Windows: HomeFeed.cs (Android: HomeFeed.kt, iPad: HomeFeed.swift — giữ cùng tên
//! hàm để sửa song song). Lập danh sách cần tải (make_plan — chỉ đọc cài đặt), tải các nguồn (load — feed::load), dựng các
//! hàng (build_rows); các hàng người xem bật / tắt, sắp thứ tự, tự thêm (Cài đặt → Trang chủ → Các hàng); trang chủ lần trước
//! (last / save_last); thể loại phim đã xem (film_tags).
//!
//! Riêng bản Rust: phim đã gộp (FeedFilm) trong lúc dựng hàng được gọi theo chỉ số trong danh sách đã gộp (bản Windows so
//! sánh theo chính đối tượng).

use crate::feed::{self, FeedItem, LoadResult, Loaded, Req, State, Tags};
use crate::json::{self, Obj};
use crate::models::*;
use crate::util::{local_hm, non_empty, now_ms, Error, Result, Rx};
use crate::{api_site, http, kk_phim, nguon_c, prefs, site, stremio};
use serde_json::{json, Map, Value};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;

const DAY: i64 = 86_400_000;
/// Phim xem trong 30 ngày qua mới được tự báo tập mới; tập mới phải ra trong 7 ngày qua.
const FRESH_WATCHED: i64 = 30 * DAY;
const FRESH_NEW: i64 = 7 * DAY;
/// Số phim tối đa mỗi hàng (giữ cho «Xem thêm»).
const ROW_MAX: usize = 40;

/// Thẻ phim của trang chủ tổng hợp. links: các nguồn có phim này, nguồn mở trước đứng đầu.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub film: Film,
    pub chip: Option<String>,
    pub links: Vec<FeedItem>,
}

/// Một hàng. more: địa chỉ danh sách đầy đủ (None = «Xem thêm» hiện mọi phim đã có trong cards); slot: loại hàng trong cài
/// đặt của người xem (ROWS: "similar", "interest", "newest"…; "custom-&lt;id&gt;" = hàng tự thêm; "topics" = bản cũ).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedRow {
    pub id: String,
    pub title: String,
    pub cards: Vec<Card>,
    pub more: Option<String>,
    pub slot: String,
}

impl FeedRow {
    pub fn new(id: impl Into<String>, title: impl Into<String>, cards: Vec<Card>, more: Option<String>, slot: impl Into<String>) -> FeedRow {
        FeedRow { id: id.into(), title: title.into(), cards, more, slot: slot.into() }
    }
}

/// Hàng người xem tự thêm (Cài đặt → Trang chủ → Các hàng): chủ đề có sẵn (topic — TOPICS) hoặc lọc theo loại phim
/// (hoathinh / series / single / tvshows) + nước + thể loại (slug KKPhim, rỗng = mọi) + ngôn ngữ (thuyet-minh / long-tieng).
/// Lưu JSON như bản Windows: {"id", "topic", "type", "country", "genre", "lang"}.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomRow {
    pub id: String,
    pub topic: Option<String>,
    /// Loại phim (khoá JSON "type").
    #[serde(rename = "type")]
    pub kind: String,
    pub country: String,
    pub genre: String,
    pub lang: String,
}

impl CustomRow {
    /// Hàng lọc theo loại phim + nước + thể loại + ngôn ngữ.
    pub fn filter(id: impl Into<String>, kind: &str, country: &str, genre: &str, lang: &str) -> CustomRow {
        CustomRow { id: id.into(), topic: None, kind: kind.into(), country: country.into(), genre: genre.into(), lang: lang.into() }
    }

    /// Hàng chủ đề (id "t-&lt;chủ đề&gt;").
    pub fn for_topic(topic: &str) -> CustomRow {
        CustomRow { id: format!("t-{topic}"), topic: Some(topic.to_string()), ..Default::default() }
    }

    /// Mã trong cài đặt thứ tự các hàng.
    pub fn key(&self) -> String {
        format!("custom-{}", self.id)
    }

    /// Cùng nội dung (bỏ qua id) — khỏi thêm trùng.
    pub fn same(&self, o: &CustomRow) -> bool {
        self.topic == o.topic && self.kind == o.kind && self.country == o.country && self.genre == o.genre && self.lang == o.lang
    }

    /// JSON như bản Windows (tên khoá camelCase, topic null khi không phải chủ đề).
    pub fn json(&self) -> Value {
        let mut o = Map::new();
        o.insert("id".into(), json!(self.id));
        o.insert("topic".into(), self.topic.as_ref().map(|t| json!(t)).unwrap_or(Value::Null));
        o.insert("type".into(), json!(self.kind));
        o.insert("country".into(), json!(self.country));
        o.insert("genre".into(), json!(self.genre));
        o.insert("lang".into(), json!(self.lang));
        Value::Object(o)
    }

    /// Đọc một hàng đã lưu (thiếu khoá = rỗng); không phải đối tượng → None.
    pub fn of(v: &Value) -> Option<CustomRow> {
        let o = v.as_object()?;
        let s = |k: &str| match o.get(k) {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        let topic = match o.get("topic") {
            Some(Value::String(t)) => Some(t.clone()),
            _ => None,
        };
        Some(CustomRow { id: s("id"), topic, kind: s("type"), country: s("country"), genre: s("genre"), lang: s("lang") })
    }
}

/// Phim đang xem dở vừa có tập mới (key: tên đã chuẩn hoá; ep: số tập mới nhất).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreshCard {
    pub card: Card,
    pub key: String,
    pub ep: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Web,
    New,
    Similar,
    Interest,
    Dominant,
    Dub,
    Kids,
    Movies,
    Custom,
    Tags,
}

/// Một phần cần tải: vai trò + tiêu đề hàng / địa chỉ «Xem thêm» (nếu có).
#[derive(Clone)]
pub struct Part {
    pub role: Role,
    pub req: Req,
    pub title: Option<String>,
    pub more: Option<String>,
    pub id: String,
    pub slot: String,
}

impl Part {
    pub fn new(role: Role, req: Req, title: Option<String>, more: Option<String>, id: impl Into<String>, slot: impl Into<String>) -> Part {
        Part { role, req, title, more, id: id.into(), slot: slot.into() }
    }

    pub fn key(&self) -> &str {
        &self.req.key
    }

    pub fn row_id(&self) -> &str {
        if self.id.is_empty() {
            &self.req.key
        } else {
            &self.id
        }
    }
}

// ---- Các hàng người xem bật / tắt, sắp thứ tự, tự thêm (Cài đặt → Trang chủ → Các hàng) ----

/// Theo thứ tự mặc định. «Có tập mới» / «Xem tiếp» (HEAD_ROWS) chỉ bật / tắt — luôn ở đầu trang; hàng trẻ em (KIDS_ROWS)
/// chỉ có ở hồ sơ trẻ em.
pub const ROWS: &[(&str, &str)] = &[
    ("new-eps", "Có tập mới"),
    ("continue", "Xem tiếp"),
    ("similar", "Giống «phim vừa xem»"),
    ("interest", "Vì … hay xem <thể loại> (sở thích 1)"),
    ("dominant", "Loại phim hay xem · mới cập nhật"),
    ("dub", "Thuyết minh / lồng tiếng mới"),
    ("kids-young", "Hoạt hình thiếu nhi"),
    ("kids-family", "Hoạt hình gia đình"),
    ("kids-dub", "Hoạt hình lồng tiếng"),
    ("interest2", "Vì … hay xem <thể loại> (sở thích 2)"),
    ("newest", "Mới cập nhật · mọi nguồn"),
    ("movies", "Phim lẻ mới"),
];
pub const HEAD_ROWS: &[&str] = &["new-eps", "continue"];
pub const KIDS_ROWS: &[&str] = &["kids-young", "kids-family", "kids-dub"];
/// Hàng hồ sơ trẻ em không có (chỉ hoạt hình hợp trẻ em).
pub const ADULT_ROWS: &[&str] = &["dominant", "dub", "newest", "movies"];
/// Tối đa số hàng tự thêm.
pub const CUSTOM_MAX: usize = 6;
/// Chủ đề hợp hồ sơ trẻ em (chỉ hoạt hình).
pub const KIDS_TOPICS: &[&str] = &["cn", "jp", "kid"];
/// Loại phim / ngôn ngữ cho hàng tự thêm (type, sort_lang của KKPhim).
pub const TYPES: &[(&str, &str)] = &[("hoathinh", "Hoạt hình"), ("series", "Phim bộ"), ("single", "Phim lẻ"), ("tvshows", "TV show")];
pub const LANGS: &[(&str, &str)] = &[("", "Mọi bản"), ("thuyet-minh", "Thuyết minh"), ("long-tieng", "Lồng tiếng")];

/// Thể loại không hợp trẻ em (hàng tự thêm của hồ sơ trẻ em không chọn được).
pub fn adult_genres() -> &'static [&'static str] {
    ADULT
}

/// Nhãn của một hàng trong ROWS (không có thì chính id).
pub fn row_label(id: &str) -> &str {
    ROWS.iter().find(|x| x.0 == id).map(|x| x.1).unwrap_or(id)
}

/// Tên hàng tự thêm: «Hoạt hình Trung Quốc · phiêu lưu · thuyết minh», chủ đề thì tên chủ đề.
pub fn custom_title(r: &CustomRow) -> String {
    if let Some(t) = &r.topic {
        return TOPICS.iter().find(|x| x.0 == t).map(|x| x.1.to_string()).unwrap_or_else(|| t.clone());
    }
    let mut head = TYPES.iter().find(|x| x.0 == r.kind).map(|x| x.1).unwrap_or("Phim").to_string();
    if !r.country.is_empty() {
        head.push(' ');
        head.push_str(&api_site::country_name(&r.country));
    }
    let mut parts = vec![head];
    if !r.genre.is_empty() {
        parts.push(api_site::genre_name(&r.genre));
    }
    if !r.lang.is_empty() {
        if let Some(l) = LANGS.iter().find(|x| x.0 == r.lang) {
            parts.push(l.1.to_lowercase());
        }
    }
    parts.join(" · ")
}

/// Thứ tự mặc định: «Có tập mới», «Xem tiếp», các hàng tự thêm (theo lúc thêm), rồi các hàng app tự chọn.
pub fn row_defaults(custom: Option<&[CustomRow]>) -> Vec<String> {
    let owned;
    let custom = match custom {
        Some(c) => c,
        None => {
            owned = home_custom();
            &owned[..]
        }
    };
    ROWS[..2]
        .iter()
        .map(|x| x.0.to_string())
        .chain(custom.iter().map(|c| c.key()))
        .chain(ROWS[2..].iter().map(|x| x.0.to_string()))
        .collect()
}

/// Ghép thứ tự đã lưu ("id" bật, "-id" tắt) với các hàng mặc định: bỏ id lạ / trùng, hàng mặc định chưa có (hàng mới ở bản
/// sau, hàng vừa tự thêm) chèn ngay sau hàng đứng trước nó trong thứ tự mặc định.
pub fn merge_rows(saved: &[String], defaults: Option<&[String]>) -> Vec<(String, bool)> {
    let owned: Vec<String>;
    let defaults = match defaults {
        Some(d) => d,
        None => {
            owned = ROWS.iter().map(|x| x.0.to_string()).collect();
            &owned[..]
        }
    };
    let mut o: Vec<(String, bool)> = Vec::new();
    for s in saved {
        let on = !s.starts_with('-');
        let id = if on { s.as_str() } else { &s[1..] };
        if defaults.iter().any(|d| d == id) && o.iter().all(|x| x.0 != id) {
            o.push((id.to_string(), on));
        }
    }
    for (i, id) in defaults.iter().enumerate() {
        if o.iter().any(|x| x.0 == *id) {
            continue;
        }
        let prev = defaults[..i].iter().rev().find(|p| o.iter().any(|x| x.0 == **p));
        let at = prev.and_then(|p| o.iter().position(|x| x.0 == *p)).map(|k| k + 1).unwrap_or(0);
        o.insert(at, (id.clone(), true));
    }
    o
}

/// Các hàng của người xem đang dùng (thứ tự, bật / tắt), kể cả hàng tự thêm.
pub fn row_prefs() -> Vec<(String, bool)> {
    merge_rows(&prefs::home_rows(), Some(&row_defaults(None)))
}

pub fn row_on(id: &str) -> bool {
    row_prefs().into_iter().find(|p| p.0 == id).map(|p| p.1).unwrap_or(true)
}

/// Loại hàng của một hàng lưu từ bản cũ (chưa ghi slot): theo id; hai hàng «Vì … hay xem» theo thứ tự xuất hiện.
fn legacy_slot(id: &str, interest_before: usize) -> String {
    if id.starts_with("interest-") {
        if interest_before == 0 { "interest" } else { "interest2" }.to_string()
    } else if id.starts_with("topic-") {
        "topics".to_string()
    } else {
        id.to_string()
    }
}

/// Xếp các hàng theo cài đặt của người xem: bỏ hàng đã tắt, theo thứ tự đã chọn; hàng chủ đề bản cũ ("topics") đầu, hàng
/// lạ cuối. prefs None = cài đặt hiện tại (row_prefs).
pub fn arrange(rows: &[FeedRow], prefs: Option<&[(String, bool)]>) -> Vec<FeedRow> {
    arrange_index(rows, prefs).into_iter().map(|(i, _)| rows[i].clone()).collect()
}

/// Như arrange, kèm loại hàng (id trong cài đặt) của từng hàng — kể cả hàng lưu từ bản cũ (chưa ghi slot).
pub fn arrange_slots(rows: &[FeedRow], prefs: Option<&[(String, bool)]>) -> Vec<(FeedRow, String)> {
    arrange_index(rows, prefs).into_iter().map(|(i, s)| (rows[i].clone(), s)).collect()
}

/// (chỉ số hàng, loại hàng) theo thứ tự hiện.
fn arrange_index(rows: &[FeedRow], prefs: Option<&[(String, bool)]>) -> Vec<(usize, String)> {
    let owned;
    let prefs = match prefs {
        Some(p) => p,
        None => {
            owned = row_prefs();
            &owned[..]
        }
    };
    let mut pos: HashMap<&str, i64> = HashMap::new();
    for (i, p) in prefs.iter().enumerate() {
        pos.insert(p.0.as_str(), if p.1 { i as i64 } else { -1 });
    }
    let mut interest = 0;
    let mut keyed: Vec<(usize, String)> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let slot = if r.slot.is_empty() { legacy_slot(&r.id, interest) } else { r.slot.clone() };
        if slot.starts_with("interest") {
            interest += 1;
        }
        keyed.push((i, slot));
    }
    let rank = |slot: &str| if slot == "topics" { -1 } else { pos.get(slot).copied().unwrap_or(i64::MAX) };
    keyed.retain(|(_, s)| pos.get(s.as_str()) != Some(&-1));
    keyed.sort_by_key(|(i, s)| (rank(s), *i));
    keyed
}

/// Lưu cài đặt các hàng ("id" bật, "-id" tắt).
pub fn save_rows(prefs: &[(String, bool)]) {
    let l: Vec<String> = prefs.iter().map(|p| if p.1 { p.0.clone() } else { format!("-{}", p.0) }).collect();
    prefs::set_home_rows(&l);
}

/// Tắt một hàng (sửa nhanh trên trang chủ: «Ẩn hàng này»).
pub fn hide(prefs: &[(String, bool)], slot: &str) -> Vec<(String, bool)> {
    prefs.iter().map(|p| if p.0 == slot { (slot.to_string(), false) } else { p.clone() }).collect()
}

/// Sửa nhanh trên trang chủ: chuyển hàng slot lên / xuống qua hàng đang hiện kề nó (shown = loại hàng đang hiện, theo thứ
/// tự trên trang — hàng tắt / không đủ phim nằm giữa bị vượt qua). None: không chuyển được (hàng đầu / cuối, «Có tập
/// mới» / «Xem tiếp», hàng chủ đề bản cũ).
pub fn move_shown(prefs: &[(String, bool)], shown: &[String], slot: &str, up: bool) -> Option<Vec<(String, bool)>> {
    let fixed = |s: &str| HEAD_ROWS.contains(&s) || s == "topics";
    let i = shown.iter().position(|s| s == slot)?;
    if fixed(slot) {
        return None;
    }
    let j = if up { i.checked_sub(1)? } else { i + 1 };
    if j >= shown.len() || fixed(&shown[j]) {
        return None;
    }
    let other = &shown[j];
    let mut output = prefs.to_vec();
    let at = output.iter().position(|p| p.0 == slot)?;
    let me = output.remove(at);
    let k = output.iter().position(|p| p.0 == *other)?;
    output.insert(if up { k } else { k + 1 }, me);
    Some(output)
}

/// Chủ đề cho người xem chưa có lịch sử (id lưu thành hàng tự thêm — home_topics).
pub const TOPICS: &[(&str, &str)] = &[
    ("cn", "Hoạt hình Trung Quốc"),
    ("kh", "Kiếm hiệp"),
    ("kr", "Phim Hàn"),
    ("mv", "Phim lẻ Âu Mỹ"),
    ("hk", "Phim xưa Hồng Kông"),
    ("kid", "Hoạt hình cho bé"),
    ("jp", "Anime Nhật"),
    ("tm", "Thuyết minh / lồng tiếng"),
];

// ---- Hàng tự thêm / chủ đề trong cài đặt (Prefs.HomeCustom / Prefs.HomeTopics của bản Windows) ----

/// Hàng tự thêm của người xem: chủ đề có sẵn (TOPICS) hoặc loại phim + nước + thể loại + ngôn ngữ. Trước 1.6 chủ đề người
/// mới chọn lưu ở "home_topics" — prefs::home_custom_raw đọc lại thành hàng chủ đề.
pub fn home_custom() -> Vec<CustomRow> {
    match prefs::home_custom_raw() {
        Some(Value::Array(a)) => a.iter().filter_map(CustomRow::of).filter(|c| !c.id.is_empty()).collect(),
        _ => Vec::new(),
    }
}

pub fn set_home_custom(rows: &[CustomRow]) {
    prefs::set_home_custom_raw(Value::Array(rows.iter().map(|c| c.json()).collect()));
}

/// Chủ đề người xem đã chọn (id trong TOPICS) — các hàng tự thêm loại «chủ đề».
pub fn home_topics() -> HashSet<String> {
    home_custom().into_iter().filter_map(|c| c.topic).collect()
}

/// Đặt các chủ đề: bỏ hàng chủ đề không còn chọn, thêm hàng cho chủ đề mới (theo thứ tự chữ cái), giữ hàng tự thêm khác.
pub fn set_home_topics(value: &HashSet<String>) {
    let cur = home_custom();
    let mut out: Vec<CustomRow> = cur.iter().filter(|c| c.topic.as_ref().map(|t| value.contains(t)).unwrap_or(true)).cloned().collect();
    let mut added: Vec<&String> = value.iter().filter(|t| cur.iter().all(|c| c.topic.as_ref() != Some(*t))).collect();
    added.sort();
    out.extend(added.into_iter().map(|t| CustomRow::for_topic(t)));
    set_home_custom(&out);
}

// ---- Nguồn ----

/// Nguồn trong danh sách trang (gốc địa chỉ); web: trang web đang dùng + trang hay xem gần đây.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sources {
    pub kk: Option<String>,
    pub nc: Option<String>,
    pub vs: Option<String>,
    pub k20: Option<String>,
    pub web: Vec<String>,
}

impl Sources {
    pub fn count(&self) -> usize {
        [&self.kk, &self.nc, &self.vs, &self.k20].iter().filter(|x| x.is_some()).count() + self.web.len()
    }

    /// id (trong SOURCE_NAMES) của các nguồn có mặt.
    pub fn ids(&self) -> Vec<String> {
        let mut o = Vec::new();
        for (has, id) in [(self.kk.is_some(), "kk"), (self.nc.is_some(), "nc"), (self.vs.is_some(), "vs"), (self.k20.is_some(), "k20"), (!self.web.is_empty(), "web")] {
            if has {
                o.push(id.to_string());
            }
        }
        o
    }
}

/// Nguồn của trang chủ tổng hợp người xem bật / tắt được (Cài đặt → «Nguồn cho trang chủ…», riêng từng người; khoá
/// prefs::home_src_off), theo thứ tự hiện. «web» = trang đang dùng + trang xem gần đây.
pub const SOURCE_NAMES: &[(&str, &str)] = &[
    ("kk", "KKPhim"),
    ("nc", "NguonC"),
    ("vs", "VSMOV"),
    ("k20", "CLB Phim Xưa"),
    ("web", "Trang đang dùng và trang xem gần đây"),
];

/// Bỏ các nguồn người xem đã tắt. Tắt hết các nguồn đang có (vd nguồn còn bật vừa bị xoá khỏi danh sách trang) thì coi như
/// bật hết — trang chủ không bao giờ trống vì cài đặt nguồn.
pub fn without_off(all: &Sources, off: &HashSet<String>) -> Sources {
    if all.ids().iter().all(|i| off.contains(i)) {
        return all.clone();
    }
    let keep = |id: &str, v: &Option<String>| if off.contains(id) { None } else { v.clone() };
    Sources {
        kk: keep("kk", &all.kk),
        nc: keep("nc", &all.nc),
        vs: keep("vs", &all.vs),
        k20: keep("k20", &all.k20),
        web: if off.contains("web") { Vec::new() } else { all.web.clone() },
    }
}

/// Nguồn trang chủ của người xem đang dùng (đã bỏ nguồn tắt).
pub fn get_sources() -> Sources {
    without_off(&all_sources(), &prefs::home_src_off())
}

/// Mọi nguồn có trong danh sách trang (cả nguồn người xem đã tắt).
pub fn all_sources() -> Sources {
    let sites = prefs::sites();
    let base_url = prefs::base_url();
    let find = |f: fn(&str) -> bool| sites.iter().find(|s| f(s)).map(|s| api_site::origin(s));
    let kk = find(api_site::is_kk);
    let nc = find(nguon_c::is_site);
    let vs = find(api_site::is_vs);
    let k20 = find(stremio::is_site);
    let api: HashSet<String> = [&kk, &nc, &vs, &k20].iter().filter_map(|x| x.as_deref()).filter_map(site::host_of).collect();
    let now = now_ms();
    let mut recent: Vec<String> = Vec::new();
    for h in prefs::history().iter().filter(|h| now - h.time < FRESH_WATCHED) {
        if let Some(host) = site::host_of(&h.film_url) {
            if !recent.contains(&host) {
                recent.push(host);
            }
        }
    }
    // Trang web đọc HTML: tối đa 2 trang (trang đang dùng + trang xem gần nhất)
    let candidates = std::iter::once(base_url).chain(recent.iter().filter_map(|h| sites.iter().find(|s| site::host_of(s).as_deref() == Some(h.as_str())).cloned()));
    let mut seen: HashSet<String> = HashSet::new();
    let mut web = Vec::new();
    for s in candidates {
        let Some(h) = site::host_of(&s) else { continue };
        if api.contains(&h) || !seen.insert(h) {
            continue;
        }
        web.push(s);
        if web.len() == 2 {
            break;
        }
    }
    Sources { kk, nc, vs, k20, web }
}

// ---- Sở thích ----

/// Thể loại quá chung (không dùng để gợi ý); chinh-kich / tam-ly tính nửa điểm.
const GENERIC: &[&str] = &["hoat-hinh", "phim-18", "tv-shows", "phim-bo", "phim-le"];
const HALF: &[&str] = &["chinh-kich", "tam-ly"];
/// Không hợp trẻ em (hồ sơ trẻ em bỏ các phim này và mọi phim không phải hoạt hình).
const ADULT: &[&str] = &["phim-18", "kinh-di", "hinh-su", "chien-tranh"];
/// Tên phim ma / kinh dị mà nguồn không gắn thể loại.
static SCARY: Rx = Rx::new(r"(?i)horror|kinh dị|xác sống|zombie|ma quỷ|quỷ dữ|ác quỷ|kyoufu");

fn is_generic(g: &str) -> bool {
    GENERIC.contains(&g)
}

fn is_half(g: &str) -> bool {
    HALF.contains(&g)
}

fn is_adult(g: &str) -> bool {
    ADULT.contains(&g)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Combo {
    pub kind: String,
    pub country: String,
    pub genre: String,
}

impl Combo {
    pub fn new(kind: &str, country: &str, genre: &str) -> Combo {
        Combo { kind: kind.into(), country: country.into(), genre: genre.into() }
    }
}

/// Điểm cộng dồn theo khoá, giữ thứ tự thêm (như Dictionary của bản Windows: bằng điểm thì khoá thêm trước đứng trước).
#[derive(Debug, Clone)]
pub struct Scores<K> {
    keys: Vec<K>,
    map: HashMap<K, f64>,
}

impl<K: Eq + Hash + Clone> Default for Scores<K> {
    fn default() -> Self {
        Scores { keys: Vec::new(), map: HashMap::new() }
    }
}

impl<K: Eq + Hash + Clone> Scores<K> {
    pub fn get<Q: ?Sized + Hash + Eq>(&self, k: &Q) -> f64
    where
        K: Borrow<Q>,
    {
        self.map.get(k).copied().unwrap_or(0.0)
    }

    pub fn contains_key<Q: ?Sized + Hash + Eq>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.map.contains_key(k)
    }

    pub fn add(&mut self, k: K, w: f64) {
        match self.map.get_mut(&k) {
            Some(v) => *v += w,
            None => {
                self.keys.push(k.clone());
                self.map.insert(k, w);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Theo thứ tự thêm.
    pub fn iter(&self) -> impl Iterator<Item = (&K, f64)> + '_ {
        self.keys.iter().map(move |k| (k, self.get(k)))
    }

    /// Điểm cao trước (bằng điểm: thêm trước đứng trước).
    pub fn by_score(&self) -> Vec<(&K, f64)> {
        let mut v: Vec<(&K, f64)> = self.iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        v
    }
}

/// Điểm sở thích của một người xem. pair: (loại phim, nước).
#[derive(Debug, Clone, Default)]
pub struct Taste {
    pub genre: Scores<String>,
    pub combos: Scores<Combo>,
    pub pair: Scores<(String, String)>,
    pub total: f64,
}

impl Taste {
    pub fn affinity(&self, t: Option<&Tags>) -> f64 {
        let Some(t) = t else { return 0.0 };
        let p = self.pair.get(&(t.kind.clone().unwrap_or_default(), t.countries.first().cloned().unwrap_or_default()));
        t.genres.iter().map(|g| self.genre.get(g.as_str())).sum::<f64>() + p
    }
}

/// Điểm của mỗi phim = số tập đã xem (1–20) × ½ mũ (số ngày từ lần xem cuối ÷ 14), cộng vào thể loại, cặp (loại, nước)
/// và bộ ba (loại, nước, thể loại). Phim chưa biết thể loại thì bỏ qua.
pub fn get_taste(history: &[HistoryItem], tags_of: impl Fn(&str) -> Option<Tags>, watched: impl Fn(&str) -> i64, now: i64) -> Taste {
    let mut t = Taste::default();
    for h in history {
        let days = (now - h.time).max(0) as f64 / DAY as f64;
        if days > 120.0 {
            continue;
        }
        let Some(tags) = tags_of(&h.title) else { continue };
        let w = watched(&h.film_url).clamp(1, 20) as f64 * 0.5f64.powf(days / 14.0);
        t.total += w;
        let kind = tags.kind.clone().unwrap_or_default();
        let country = tags.countries.first().cloned().unwrap_or_default();
        t.pair.add((kind.clone(), country.clone()), w);
        for g in tags.genres.iter().filter(|g| !is_generic(g)) {
            let gw = if is_half(g) { w / 2.0 } else { w };
            t.genre.add(g.clone(), gw);
            if !kind.is_empty() && !country.is_empty() {
                t.combos.add(Combo::new(&kind, &country, g), gw);
            }
        }
    }
    t
}

fn type_slug(kind: &str) -> Option<&'static str> {
    match kind {
        "hoathinh" => Some("hoat-hinh"),
        "series" => Some("phim-bo"),
        "single" => Some("phim-le"),
        "tvshows" => Some("tv-shows"),
        _ => None,
    }
}

pub fn type_name(kind: &str) -> &'static str {
    match kind {
        "hoathinh" => "hoạt hình",
        "series" => "phim bộ",
        "single" => "phim lẻ",
        "tvshows" => "TV show",
        _ => "phim",
    }
}

/// Viết hoa chữ đầu ("hoạt hình" → "Hoạt hình").
fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

// ---- Lập danh sách cần tải ----

/// pid: người xem lúc lập kế hoạch; kids: hồ sơ trẻ em.
#[derive(Clone, Default)]
pub struct Plan {
    pub parts: Vec<Part>,
    pub src: Sources,
    pub similar: Option<HistoryItem>,
    pub similar_tags: Option<Tags>,
    pub taste: Option<Taste>,
    pub topics: Option<HashSet<String>>,
    pub pid: String,
    pub kids: bool,
}

impl Plan {
    pub fn new(parts: Vec<Part>, src: Sources) -> Plan {
        Plan { parts, src, ..Default::default() }
    }
}

/// Hàm tải của một yêu cầu (feed::Req) từ một hàm async.
fn loader<F, Fut>(f: F) -> feed::LoadFn
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Loaded>> + Send + 'static,
{
    Arc::new(move || -> feed::LoadFut { Box::pin(f()) })
}

fn loaded(items: Vec<FeedItem>) -> Loaded {
    Loaded { items, categories: None, moved_to: None }
}

async fn get_root(url: &str) -> Result<Obj> {
    let r = http::get(url, &[("Accept", "application/json")]).await?;
    site::json_object(&r.data).ok_or_else(|| Error::app("Nguồn trả dữ liệu lạ"))
}

fn host_or_empty(o: &str) -> String {
    site::host_of(o).unwrap_or_default()
}

/// Lập kế hoạch tải cho người xem đang dùng (đọc cài đặt, không chạm mạng). tags_known_only: không tra thể loại phim đã xem.
pub fn make_plan(tags_known_only: bool) -> Plan {
    let s = get_sources();
    let kids = prefs::kids();
    let mut parts: Vec<Part> = Vec::new();
    let now = now_ms();
    let history = prefs::history();
    // Hàng người xem đã tắt: khỏi tải. «Mới cập nhật» vẫn tải danh sách mới của các nguồn — cần để gộp phim, tìm «Có tập mới»
    let off: HashSet<String> = row_prefs().into_iter().filter(|x| !x.1).map(|x| x.0).collect();

    // Hồ sơ trẻ em: không lấy danh sách «mới cập nhật» chung, chỉ các danh sách hoạt hình
    if !kids {
        if let Some(o) = s.kk.clone() {
            let key = format!("new|{o}");
            let host = host_or_empty(&o);
            parts.push(Part::new(Role::New, Req::new(key, host, loader(move || {
                let o = o.clone();
                async move { Ok(loaded(feed::parse_kk(&api_site::json(&format!("{o}/api/movies/newest?page=1&limit=40")).await?, &o))) }
            })), None, None, "", ""));
        }
        if let Some(o) = s.nc.clone() {
            let key = format!("new|{o}");
            let host = host_or_empty(&o);
            parts.push(Part::new(Role::New, Req::new(key, host, loader(move || {
                let o = o.clone();
                async move { Ok(loaded(feed::parse_nc(&get_root(&nguon_c::newest_url(&o)).await?, &o))) }
            })), None, None, "", ""));
        }
        if let Some(o) = s.vs.clone() {
            let key = format!("new|{o}");
            let host = host_or_empty(&o);
            parts.push(Part::new(Role::New, Req::new(key, host, loader(move || {
                let o = o.clone();
                async move { Ok(loaded(feed::parse_kk(&api_site::json(&format!("{o}/api/danh-sach/phim-moi-cap-nhat?page=1")).await?, &o))) }
            })), None, None, "", ""));
        }
        // Trang web (trang đang dùng trước): phim mới cập nhật + menu mục của trang
        for w in &s.web {
            let key = format!("web|{w}");
            let host = host_or_empty(w);
            let w = w.clone();
            parts.push(Part::new(Role::Web, Req::new(key, host, loader(move || {
                let w = w.clone();
                async move {
                    let h = site::home_of(Some(w.as_str())).await?;
                    let mut items = feed::from_films(&newest_of(&h));
                    items.truncate(ROW_MAX);
                    Ok(Loaded { items, categories: Some(h.categories), moved_to: h.moved_to })
                }
            })), None, None, "", ""));
        }
        if let Some(o) = s.k20.clone() {
            let key = format!("new|{o}");
            let host = host_or_empty(&o);
            parts.push(Part::new(Role::New, Req::new(key, host, loader(move || {
                let o = o.clone();
                async move { Ok(loaded(feed::from_films(&stremio::list(&stremio::newest_url(&o)).await?.films))) }
            })), None, None, "", ""));
        }
    }

    // Thể loại của phim đã xem chưa biết: tra KKPhim (mỗi lượt tối đa 6 phim; không thấy thì 3 ngày sau mới tra lại)
    if !tags_known_only && (s.kk.is_some() || prefs::kk_backup()) {
        let mut seen: HashSet<String> = HashSet::new();
        let mut n = 0;
        for h in history.iter().filter(|h| now - h.time < 120 * DAY) {
            if n == 6 {
                break;
            }
            let key = kk_phim::norm(&h.title);
            if !seen.insert(key.clone()) || !film_tags::needs_lookup(&h.title) {
                continue;
            }
            n += 1;
            let title = h.title.clone();
            let mut req = Req::new(format!("tags|{key}"), "phimapi.com", loader(move || {
                let title = title.clone();
                async move {
                    film_tags::lookup(&title).await?;
                    Ok(Loaded::default())
                }
            }));
            req.keep = false;
            parts.push(Part::new(Role::Tags, req, None, None, "", ""));
        }
    }

    let counts = prefs::watched_counts();
    let taste = if history.is_empty() {
        None
    } else {
        Some(get_taste(&history, film_tags::of, |u| counts.get(u).copied().unwrap_or(0) as i64, now))
    };
    let topics = home_topics();
    let mut similar: Option<HistoryItem> = None;
    let mut similar_tags: Option<Tags> = None;
    let mut top: Option<(String, String)> = None;
    if let Some(taste) = taste.as_ref().filter(|t| t.total > 0.0) {
        // Giống «phim vừa xem»: cùng loại phim, nước và thể loại riêng nhất (trẻ em: chỉ phim hoạt hình)
        similar = history
            .iter()
            .find(|h| {
                film_tags::of(&h.title)
                    .map(|t| t.kind.is_some() && !t.countries.is_empty() && (!kids || t.kind.as_deref() == Some("hoathinh")))
                    .unwrap_or(false)
            })
            .cloned();
        similar_tags = similar.as_ref().and_then(|h| film_tags::of(&h.title));
        let mut used: HashSet<String> = HashSet::new();
        if let (Some(sim), Some(st)) = (&similar, &similar_tags) {
            let g = special_genre(st, kids);
            let country = st.countries.first().map(|c| c.as_str()).unwrap_or("");
            if let Some(url) = filter_url(&s, st.kind.as_deref().unwrap_or(""), country, g.as_deref()) {
                if let Some(g) = &g {
                    used.insert(g.clone());
                }
                if !off.contains("similar") {
                    parts.push(Part::new(Role::Similar, req(&url), Some(format!("Giống «{}»", sim.title)), Some(url), "similar", "similar"));
                }
            }
        }
        // Vì … hay xem <thể loại>: hai bộ ba điểm cao nhất, khác thể loại với hàng trên
        let mut n = 0;
        let profile = prefs::current_profile().name;
        for (cb, _) in taste.combos.by_score() {
            if n == 2 {
                break;
            }
            if used.contains(&cb.genre) || is_half(&cb.genre) {
                continue;
            }
            if kids && (cb.kind != "hoathinh" || is_adult(&cb.genre)) {
                continue;
            }
            let Some(url) = filter_url(&s, &cb.kind, &cb.country, Some(&cb.genre)) else { continue };
            used.insert(cb.genre.clone());
            n += 1;
            let slot = if n == 1 { "interest" } else { "interest2" };
            if off.contains(slot) {
                continue;
            }
            let title = format!(
                "Vì {profile} hay xem {} · {} {}",
                api_site::genre_name(&cb.genre),
                type_name(&cb.kind),
                api_site::country_name(&cb.country)
            );
            parts.push(Part::new(Role::Interest, req(&url), Some(title), Some(url), format!("interest-{}", cb.genre), slot));
        }
        // Loại phim + nước xem nhiều nhất (≥ 40%): hàng mới cập nhật riêng, gộp mọi nguồn
        let best = taste.pair.by_score().into_iter().find(|(k, _)| !k.0.is_empty() && !k.1.is_empty());
        if let Some((k, v)) = best {
            if v >= taste.total * 0.4 {
                top = Some(k.clone());
            }
        }
        if let Some(tp) = top.as_ref().filter(|_| !kids && !off.contains("dominant")) {
            let title = format!("{} {} mới cập nhật · mọi nguồn", cap(type_name(&tp.0)), api_site::country_name(&tp.1));
            if let Some(url) = filter_url(&s, &tp.0, &tp.1, None) {
                parts.push(Part::new(Role::Dominant, req(&url), Some(title), None, "dominant", "dominant"));
            }
        }
    }
    // Hàng tự thêm (chủ đề người mới chọn cũng là hàng tự thêm — vẫn còn khi đã có lịch sử). Hồ sơ trẻ em: chỉ hoạt hình
    for c in home_custom() {
        let key = c.key();
        if off.contains(&key) {
            continue;
        }
        if kids {
            let ok = match &c.topic {
                Some(t) => KIDS_TOPICS.contains(&t.as_str()),
                None => c.kind == "hoathinh",
            };
            if !ok {
                continue;
            }
        }
        let found = match &c.topic {
            Some(t) => topic_req(&s, t),
            None => custom_req(&s, &c),
        };
        let Some((r, more)) = found else { continue };
        parts.push(Part::new(Role::Custom, r, Some(custom_title(&c)), Some(more), key.clone(), key));
    }
    if kids {
        parts.extend(kids_parts(&s).into_iter().filter(|p| !off.contains(&p.slot)));
    } else {
        if !off.contains("dub") {
            if let Some(d) = dub_part(&s, top.as_ref()) {
                parts.push(d);
            }
        }
        // Phim lẻ mới (xếp theo thể loại người xem hay xem)
        if off.contains("movies") {
            // người xem đã tắt hàng này
        } else if let Some(kk) = &s.kk {
            let u = format!("{kk}/api/movies/category/phim-le?page=1&limit=30");
            parts.push(Part::new(Role::Movies, req(&u), Some("Phim lẻ mới".into()), Some(u), "movies", "movies"));
        } else if let Some(vs) = &s.vs {
            let u = format!("{vs}/api/danh-sach/phim-le?page=1");
            parts.push(Part::new(Role::Movies, req(&u), Some("Phim lẻ mới".into()), Some(u), "movies", "movies"));
        }
    }
    Plan { parts, src: s, similar, similar_tags, taste, topics: Some(topics), pid: prefs::profile_id(), kids }
}

/// Thể loại đặc trưng nhất của phim (bỏ thể loại quá chung; trẻ em: bỏ thể loại không hợp; chinh-kich / tam-ly xếp sau).
fn special_genre(t: &Tags, kids: bool) -> Option<String> {
    let mut g: Vec<&String> = t.genres.iter().filter(|x| !is_generic(x) && !(kids && is_adult(x))).collect();
    g.sort_by_key(|x| if is_half(x) { 1 } else { 0 });
    g.first().map(|x| x.to_string())
}

/// Người hay xem bản thuyết minh / lồng tiếng (từ nửa số lần mở tập gần đây, ít nhất 5 lần): hàng riêng.
fn dub_part(s: &Sources, top: Option<&(String, String)>) -> Option<Part> {
    let langs = prefs::watch_langs();
    let tm = langs.iter().filter(|l| *l == "tm").count();
    let lt = langs.iter().filter(|l| *l == "lt").count();
    if langs.len() < 5 || (tm + lt) * 2 < langs.len() {
        return None;
    }
    let slug = if lt > tm { "long-tieng" } else { "thuyet-minh" };
    let name = if lt > tm { "Lồng tiếng" } else { "Thuyết minh" };
    if let Some(o) = &s.kk {
        if let Some(tp) = top {
            if let Some(t) = type_slug(&tp.0) {
                let url = format!("{o}/api/movies/category/{t}?country={}&sort_lang={slug}&page=1&limit=30", tp.1);
                let title = format!("{name} mới · {} {}", type_name(&tp.0), api_site::country_name(&tp.1));
                return Some(Part::new(Role::Dub, req(&url), Some(title), Some(url), "dub", ""));
            }
        }
        let u = format!("{o}/api/movies/category/phim-{slug}?page=1&limit=30");
        return Some(Part::new(Role::Dub, req(&u), Some(format!("Phim {} mới", name.to_lowercase())), Some(u), "dub", ""));
    }
    if let Some(nc) = &s.nc {
        let url = format!("{nc}/api/films/ngon-ngu/{slug}?page=1");
        let (u, o) = (url.clone(), nc.clone());
        let r = Req::new(format!("list|{url}"), host_or_empty(nc), loader(move || {
            let (u, o) = (u.clone(), o.clone());
            async move { Ok(loaded(feed::parse_nc(&get_root(&u).await?, &o))) }
        }));
        return Some(Part::new(Role::Dub, r, Some(format!("Phim {} mới", name.to_lowercase())), Some(url), "dub", ""));
    }
    None
}

/// Hồ sơ trẻ em: hoạt hình thiếu nhi (KKPhim «trẻ em» + VSMOV «thiếu nhi»), gia đình, lồng tiếng.
fn kids_parts(s: &Sources) -> Vec<Part> {
    let mut o = Vec::new();
    let part = |u: String, title: &str, id: &str| Part::new(Role::Kids, req(&u), Some(title.to_string()), Some(u), id, id);
    if let Some(kk) = &s.kk {
        o.push(part(format!("{kk}/api/movies/category/hoat-hinh?category=tre-em&page=1&limit=30"), "Hoạt hình thiếu nhi", "kids-young"));
    }
    if let Some(vs) = &s.vs {
        o.push(part(format!("{vs}/api/the-loai/thieu-nhi?page=1"), "Hoạt hình thiếu nhi", "kids-young"));
    }
    if let Some(kk) = &s.kk {
        o.push(part(format!("{kk}/api/movies/category/hoat-hinh?category=gia-dinh&page=1&limit=30"), "Hoạt hình gia đình", "kids-family"));
        o.push(part(format!("{kk}/api/movies/category/hoat-hinh?sort_lang=long-tieng&page=1&limit=30"), "Hoạt hình lồng tiếng", "kids-dub"));
    }
    o
}

/// Trang phim — hàng «Cùng thể loại» (related): danh sách lọc theo loại + nước + thể loại đặc trưng nhất của phim (như hàng
/// «Giống …»), kèm tên hàng («Hoạt hình Trung Quốc · Tu tiên»). Hồ sơ trẻ em: chỉ hoạt hình, bỏ thể loại không hợp.
/// (địa chỉ, tên hàng).
pub fn related_list(t: &Tags, kids: bool) -> Option<(String, String)> {
    let kind = t.kind.as_deref()?;
    if kids && kind != "hoathinh" {
        return None;
    }
    let c = t.countries.first()?;
    let g = special_genre(t, kids);
    let url = filter_url(&get_sources(), kind, c, g.as_deref())?;
    let mut name = vec![format!("{} {}", cap(type_name(kind)), api_site::country_name(c))];
    if let Some(g) = &g {
        name.push(cap(&api_site::genre_name(g)));
    }
    Some((url, name.join(" · ")))
}

/// Danh sách lọc theo loại phim + nước (+ thể loại): KKPhim lọc được cả ba; không có KKPhim thì VSMOV.
fn filter_url(s: &Sources, kind: &str, country: &str, genre: Option<&str>) -> Option<String> {
    if let (Some(o), Some(t)) = (&s.kk, type_slug(kind)) {
        let g = genre.map(|g| format!("&category={g}")).unwrap_or_default();
        return Some(format!("{o}/api/movies/category/{t}?country={country}{g}&page=1&limit=30"));
    }
    if let Some(v) = &s.vs {
        let p = match genre {
            Some(g) => format!("the-loai/{g}?country={country}"),
            None if kind == "hoathinh" => format!("the-loai/hoat-hinh?country={country}"),
            None => format!("quoc-gia/{country}"),
        };
        return Some(page(&format!("{v}/api/{p}")));
    }
    None
}

fn page(url: &str) -> String {
    format!("{url}{}page=1", if url.contains('?') { "&" } else { "?" })
}

/// Danh sách của một hàng tự thêm: KKPhim lọc được loại + nước + thể loại + ngôn ngữ; không có KKPhim thì VSMOV (thể loại /
/// nước / loại, không lọc ngôn ngữ). (yêu cầu, địa chỉ «Xem thêm»).
fn custom_req(s: &Sources, c: &CustomRow) -> Option<(Req, String)> {
    let mut q: Vec<String> = Vec::new();
    if !c.country.is_empty() {
        q.push(format!("country={}", c.country));
    }
    if !c.genre.is_empty() {
        q.push(format!("category={}", c.genre));
    }
    if !c.lang.is_empty() {
        q.push(format!("sort_lang={}", c.lang));
    }
    if let (Some(o), Some(t)) = (&s.kk, type_slug(&c.kind)) {
        q.push("page=1".into());
        q.push("limit=30".into());
        let url = format!("{o}/api/movies/category/{t}?{}", q.join("&"));
        return Some((req(&url), url));
    }
    if let Some(v) = &s.vs {
        let cq = if c.country.is_empty() { String::new() } else { format!("?country={}", c.country) };
        let p = if !c.genre.is_empty() {
            format!("the-loai/{}{cq}", c.genre)
        } else if c.kind == "hoathinh" {
            format!("the-loai/hoat-hinh{cq}")
        } else if !c.country.is_empty() {
            format!("quoc-gia/{}", c.country)
        } else {
            format!("danh-sach/{}", type_slug(&c.kind)?)
        };
        let url = page(&format!("{v}/api/{p}"));
        return Some((req(&url), url));
    }
    None
}

/// Đọc một danh sách KKPhim / VSMOV theo địa chỉ (định dạng KKPhim).
fn req(url: &str) -> Req {
    let o = api_site::origin(url);
    let u = url.to_string();
    Req::new(format!("list|{url}"), host_or_empty(&o), loader(move || {
        let (u, o) = (u.clone(), o.clone());
        async move { Ok(loaded(feed::parse_kk(&api_site::json(&u).await?, &o))) }
    }))
}

/// Danh sách cho một chủ đề: nguồn đầu tiên có trong danh sách trang. (yêu cầu, địa chỉ «Xem thêm»).
fn topic_req(s: &Sources, id: &str) -> Option<(Req, String)> {
    let kk = |p: &str| s.kk.as_ref().map(|o| page(&format!("{o}/api/movies/{p}")) + "&limit=30");
    let vs = |p: &str| s.vs.as_ref().map(|o| page(&format!("{o}/api/{p}")));
    let url = match id {
        "cn" => kk("category/hoat-hinh?country=trung-quoc").or_else(|| vs("the-loai/hoat-hinh?country=trung-quoc")),
        "kh" => vs("the-loai/kiem-hiep").or_else(|| s.k20.as_ref().map(|o| stremio::catalog(o, Some("Kiếm Hiệp"), None, 0))),
        "kr" => kk("category/phim-bo?country=han-quoc").or_else(|| vs("quoc-gia/han-quoc")),
        "mv" => kk("category/phim-le?country=au-my").or_else(|| vs("danh-sach/phim-le")),
        "hk" => s.k20.as_ref().map(|o| stremio::newest_url(o)).or_else(|| kk("quoc-gia/hong-kong")),
        "kid" => vs("the-loai/thieu-nhi").or_else(|| kk("category/hoat-hinh?category=gia-dinh")),
        "jp" => kk("category/hoat-hinh?country=nhat-ban").or_else(|| vs("the-loai/hoat-hinh?country=nhat-ban")),
        "tm" => kk("category/phim-thuyet-minh").or_else(|| s.nc.as_ref().map(|o| format!("{o}/api/films/ngon-ngu/thuyet-minh?page=1"))),
        _ => None,
    }?;
    let o = api_site::origin(&url);
    let u = url.clone();
    let r = Req::new(format!("list|{url}"), host_or_empty(&url), loader(move || {
        let (u, o) = (u.clone(), o.clone());
        async move {
            if stremio::is_site(&u) {
                return Ok(loaded(feed::from_films(&stremio::list(&u).await?.films)));
            }
            if nguon_c::is_site(&u) {
                return Ok(loaded(feed::parse_nc(&get_root(&u).await?, &o)));
            }
            Ok(loaded(feed::parse_kk(&api_site::json(&u).await?, &o)))
        }
    }));
    Some((r, url))
}

static NEW_RX: Rx = Rx::new(r"(?i)mới|cập nhật|update|new");

/// Khối «mới cập nhật» của trang web (không có thì khối lớn nhất / mọi khối).
fn newest_of(h: &Home) -> Vec<Film> {
    let mut pick: Vec<&FilmSection> = h.sections.iter().filter(|x| NEW_RX.matches(&x.title)).collect();
    if pick.is_empty() {
        pick = h.sections.iter().filter(|x| x.films.len() >= 6).take(1).collect();
    }
    if pick.is_empty() {
        pick = h.sections.iter().collect();
    }
    let mut seen = HashSet::new();
    pick.into_iter().flat_map(|x| x.films.iter()).filter(|f| seen.insert(f.url.clone())).cloned().collect()
}

// ---- Dựng trang ----

/// Một lần dựng trang chủ. note: dòng trạng thái; note_gave_up: khi thôi chờ nguồn chậm; pending: còn nguồn chậm có thể về
/// muộn — late() dựng lại khi chúng về. moved_to: (trang cũ, gốc mới) khi trang đang dùng đã chuyển tên miền.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub rows: Vec<FeedRow>,
    pub fresh: Vec<FreshCard>,
    pub categories: Option<Vec<Category>>,
    pub moved_to: Option<(String, String)>,
    pub note: String,
    pub note_gave_up: String,
    pub empty: bool,
    pub from_cache: bool,
    pub pending: bool,
    pub results: Vec<(Part, LoadResult)>,
    pub plan: Option<Plan>,
    pub started_at: i64,
}

/// Nguồn chậm (quá hạn / chưa tới lượt) — khác nguồn lỗi hẳn: có thể về muộn.
fn slow(r: &LoadResult) -> bool {
    r.state != State::Ok && !r.resting && r.error.as_deref().map(|e| e.starts_with("quá")).unwrap_or(true)
}

fn from_cache_result(req: &Req, state: State, l: Loaded, ms: i64, at: i64) -> LoadResult {
    LoadResult { req: req.clone(), state, items: l.items, categories: l.categories, error: None, resting: false, ms, saved_at: at, moved_to: None }
}

/// Nguồn chậm của lần dựng trước đã về (bản lưu mới hơn lúc bắt đầu tải): dựng lại; chưa có gì mới thì None.
pub async fn late(s: &Snapshot) -> Option<Snapshot> {
    let plan = s.plan.as_ref()?;
    let mut changed = false;
    let res: Vec<(Part, LoadResult)> = s
        .results
        .iter()
        .map(|(p, r)| {
            if p.role == Role::Tags || !slow(r) || !p.req.keep {
                return (p.clone(), r.clone());
            }
            match feed::read_cache(p.key()) {
                Some((l, at)) if at >= s.started_at => {
                    changed = true;
                    (p.clone(), from_cache_result(&p.req, State::Ok, l, r.ms, at))
                }
                _ => (p.clone(), r.clone()),
            }
        })
        .collect();
    if !changed {
        return None;
    }
    let snap = build(plan.clone(), res, false, s.started_at);
    save_last(&plan.pid, &snap);
    Some(snap)
}

/// Bản lưu của mọi phần; None nếu chưa có bản lưu nào.
pub fn cached(plan: &Plan) -> Option<Snapshot> {
    let results: Vec<(Part, LoadResult)> = plan
        .parts
        .iter()
        .map(|p| {
            let r = match feed::read_cache(p.key()) {
                Some((l, at)) => from_cache_result(&p.req, State::Cached, l, 0, at),
                None => LoadResult {
                    req: p.req.clone(),
                    state: State::NoData,
                    items: Vec::new(),
                    categories: None,
                    error: None,
                    resting: false,
                    ms: 0,
                    saved_at: 0,
                    moved_to: None,
                },
            };
            (p.clone(), r)
        })
        .collect();
    if results.iter().any(|x| x.0.role != Role::Tags && !x.1.items.is_empty()) {
        Some(build(plan.clone(), results, true, 0))
    } else {
        None
    }
}

/// Tải mọi phần rồi dựng trang. Lượt đầu chưa biết thể loại phim đã xem: tra xong thì lập lại kế hoạch, tải thêm hàng gợi ý.
pub async fn load(plan: Plan) -> Snapshot {
    let started = now_ms();
    let mut p = plan;
    let got = feed::load(p.parts.iter().map(|x| x.req.clone()).collect()).await;
    let mut results: Vec<(Part, LoadResult)> = p.parts.iter().cloned().zip(got).collect();
    if p.parts.iter().any(|x| x.role == Role::Tags) {
        let again = make_plan(true);
        let have: HashSet<String> = results.iter().map(|x| x.0.key().to_string()).collect();
        let extra: Vec<Part> = again.parts.iter().filter(|x| !have.contains(x.key())).cloned().collect();
        if !extra.is_empty() {
            let more = feed::load(extra.iter().map(|x| x.req.clone()).collect()).await;
            results.extend(extra.into_iter().zip(more));
        }
        results = regroup(&again.parts, &results);
        p = again;
    }
    let pid = p.pid.clone();
    let snap = build(p, results, false, started);
    save_last(&pid, &snap);
    snap
}

/// Kết quả theo kế hoạch mới (thứ tự / tiêu đề mới, phần đã tải thì dùng lại) rồi các phần tra thể loại. Hai hàng có thể
/// cùng một danh sách (vd hàng tự thêm «Hoạt hình Trung Quốc» và hàng «xem nhiều nhất · mọi nguồn»): trùng khoá thì lấy
/// kết quả sau như bản TV / iPad.
pub fn regroup(parts: &[Part], results: &[(Part, LoadResult)]) -> Vec<(Part, LoadResult)> {
    let mut by_key: HashMap<&str, &LoadResult> = HashMap::new();
    for (a, b) in results {
        by_key.insert(a.key(), b);
    }
    parts
        .iter()
        .filter_map(|a| by_key.get(a.key()).map(|b| (a.clone(), (*b).clone())))
        .chain(results.iter().filter(|x| x.0.role == Role::Tags).cloned())
        .collect()
}

/// Dữ liệu người xem dùng khi dựng trang (tách khỏi cài đặt để test). base: tên miền trang đang dùng.
pub struct Ctx {
    pub history: Vec<HistoryItem>,
    pub follows: HashSet<String>,
    pub seen: Box<dyn Fn(&str) -> Option<f64> + Send + Sync>,
    pub base: String,
    pub sites: Vec<String>,
    pub now: i64,
}

pub fn build(plan: Plan, results: Vec<(Part, LoadResult)>, from_cache: bool, started_at: i64) -> Snapshot {
    let now = now_ms();
    let base_url = prefs::base_url();
    let ctx = Ctx {
        history: prefs::history(),
        follows: prefs::follows().into_iter().map(|f| f.film_url).collect(),
        seen: Box::new(prefs::seen_ep),
        base: site::host_of(&base_url).unwrap_or_default(),
        sites: prefs::sites(),
        now,
    };
    let (rows, fresh) = build_rows(&plan, &results, &ctx);
    let web_key = format!("web|{base_url}");
    let web_base = results.iter().find(|x| x.0.role == Role::Web && x.0.key() == web_key).map(|x| &x.1);
    let cats = web_base.and_then(|r| r.categories.clone()).or_else(|| site::menu(&base_url));
    let moved = results
        .iter()
        .find(|x| x.0.role == Role::Web && x.1.state == State::Ok && x.1.moved_to.is_some())
        .map(|x| (x.0.key()["web|".len()..].to_string(), x.1.moved_to.clone().unwrap_or_default()));
    let content: Vec<&(Part, LoadResult)> = if from_cache { Vec::new() } else { results.iter().filter(|x| x.0.role != Role::Tags).collect() };
    let names = |l: &[&&(Part, LoadResult)]| {
        let mut hosts: Vec<&str> = Vec::new();
        for x in l {
            if !hosts.contains(&x.0.req.host.as_str()) {
                hosts.push(&x.0.req.host);
            }
        }
        hosts.iter().map(|h| site::tag(h, &ctx.sites)).collect::<Vec<_>>().join(", ")
    };
    let slow_l: Vec<&&(Part, LoadResult)> = content.iter().filter(|x| slow(&x.1)).collect();
    let failed: Vec<&&(Part, LoadResult)> = content.iter().filter(|x| x.1.state != State::Ok && !slow(&x.1) && !x.1.resting).collect();
    let rest: Vec<&&(Part, LoadResult)> = content.iter().filter(|x| x.1.resting).collect();
    let at = if results.is_empty() { now } else { results.iter().map(|x| x.1.saved_at).max().unwrap_or(0).max(1) };
    let time = local_hm(at);
    let mut tail = String::new();
    if !failed.is_empty() {
        tail.push_str(&format!(" · lỗi: {}", names(&failed)));
    }
    if !rest.is_empty() {
        tail.push_str(&format!(" · tạm nghỉ: {}", names(&rest)));
    }
    let note = if from_cache {
        format!("Bản lưu lúc {time} · đang làm mới…")
    } else if !slow_l.is_empty() {
        format!("Cập nhật lúc {time} · đang chờ {}…{tail}", names(&slow_l))
    } else {
        format!("Cập nhật lúc {time}{tail}")
    };
    let gave_up = format!(
        "Cập nhật lúc {time}{}{tail}",
        if slow_l.is_empty() { String::new() } else { format!(" · chậm, dùng bản lưu: {}", names(&slow_l)) }
    );
    let pending = !slow_l.is_empty();
    let empty = rows.is_empty() && fresh.is_empty();
    Snapshot {
        rows,
        fresh,
        categories: cats,
        moved_to: moved,
        note,
        note_gave_up: gave_up,
        empty,
        from_cache,
        pending,
        results,
        plan: Some(plan),
        started_at,
    }
}

/// Tên đã chuẩn hoá của một tên phim (kk_phim::names với {name}).
fn names_of_title(title: &str) -> HashSet<String> {
    let mut o = Map::new();
    o.insert("name".into(), json!(title));
    kk_phim::names(&o)
}

/// Dựng các hàng (đã xếp theo cài đặt của người xem) và «Có tập mới».
pub fn build_rows(plan: &Plan, results: &[(Part, LoadResult)], ctx: &Ctx) -> (Vec<FeedRow>, Vec<FreshCard>) {
    let content: Vec<&(Part, LoadResult)> = results.iter().filter(|x| x.0.role != Role::Tags).collect();
    // Gộp mọi danh sách (trang web đang dùng trước, rồi các nguồn API) — một phim chỉ thuộc một nhóm
    let mut ordered: Vec<(usize, &(Part, LoadResult))> = content.iter().copied().enumerate().collect();
    ordered.sort_by_key(|(i, x)| (x.0.role, *i));
    let lists: Vec<Vec<FeedItem>> = ordered.iter().map(|(_, x)| x.1.items.clone()).collect();
    let merged = feed::merge(&lists, Some(ctx.now));
    let mut group_of: HashMap<&str, usize> = HashMap::new();
    for (gi, f) in merged.iter().enumerate() {
        for i in &f.items {
            group_of.insert(i.url.as_str(), gi);
        }
    }
    let down: HashSet<&str> = content.iter().filter(|x| x.1.state != State::Ok).map(|x| x.0.req.host.as_str()).collect();

    // Thứ tự nguồn khi mở phim: trang đang dùng, trang hay xem gần đây, rồi KKPhim / NguonC / VSMOV / CLB Phim Xưa
    let mut prefer: Vec<String> = Vec::new();
    let src_hosts = [&plan.src.kk, &plan.src.nc, &plan.src.vs, &plan.src.k20].into_iter().filter_map(|x| x.as_deref()).filter_map(site::host_of);
    for h in std::iter::once(ctx.base.clone()).chain(ctx.history.iter().filter_map(|h| site::host_of(&h.film_url))).chain(src_hosts) {
        if !prefer.contains(&h) {
            prefer.push(h);
        }
    }
    let mut hist_url: HashMap<&str, usize> = HashMap::new();
    let mut hist_name: HashMap<String, usize> = HashMap::new();
    for (i, h) in ctx.history.iter().enumerate() {
        hist_url.entry(h.film_url.as_str()).or_insert(i);
        for n in names_of_title(&h.title) {
            hist_name.entry(n).or_insert(i);
        }
    }
    // Lịch sử xem của từng phim đã gộp (theo địa chỉ, không thì theo tên — lần xem gần nhất)
    let hist_of: Vec<Option<usize>> = merged
        .iter()
        .map(|f| f.items.iter().find_map(|i| hist_url.get(i.url.as_str()).copied()).or_else(|| f.names.iter().filter_map(|n| hist_name.get(n).copied()).min()))
        .collect();
    let watched = |fi: usize| hist_of[fi].is_some();
    let tags: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    let tag = |host: &str| -> String {
        if let Some(t) = tags.borrow().get(host) {
            return t.clone();
        }
        let t = site::tag(host, &ctx.sites);
        tags.borrow_mut().insert(host.to_string(), t.clone());
        t
    };

    let make_card = |fi: usize| -> Option<Card> {
        let f = &merged[fi];
        let own = hist_of[fi].and_then(|h| site::host_of(&ctx.history[h].film_url));
        let pref: Vec<String> = own.into_iter().chain(prefer.iter().cloned()).collect();
        let o = f.ordered(&pref);
        let live: Vec<&FeedItem> = o.items.iter().filter(|i| !down.contains(i.host.as_str())).collect();
        let first = live.first().copied().or(o.items.first())?;
        let links: Vec<FeedItem> = std::iter::once(first.clone()).chain(o.items.iter().filter(|i| i.url != first.url).cloned()).collect();
        let badge = f.latest().and_then(|l| l.ep).map(|ep| format!("Tập {}", num(ep))).or_else(|| first.badge.clone());
        let chip = if live.is_empty() {
            "bản lưu".to_string()
        } else if live.len() > 1 {
            format!("{} nguồn", live.len())
        } else {
            tag(&first.host)
        };
        let film = Film {
            url: first.url.clone(),
            title: first.title.clone(),
            poster: links.iter().find_map(|l| l.poster.clone()),
            badge,
            imdb: links.iter().find_map(|l| l.imdb),
        };
        Some(Card { film, chip: Some(chip), links })
    };

    // Hồ sơ trẻ em: chỉ phim hoạt hình, không có thể loại không hợp trẻ em (phim chưa rõ thể loại — vd danh sách «thiếu
    // nhi» của VSMOV — giữ lại vì đã đến từ danh sách dành cho trẻ em)
    let kids_ok = |fi: usize| -> bool {
        if !plan.kids {
            return true;
        }
        let f = &merged[fi];
        if f.items.iter().any(|i| SCARY.matches(&i.title)) {
            return false;
        }
        match f.tags() {
            None => true,
            Some(t) => (t.kind.is_none() || t.kind.as_deref() == Some("hoathinh")) && !t.genres.iter().any(|g| is_adult(g)),
        }
    };

    let mut rows: Vec<FeedRow> = Vec::new();
    let mut claimed: HashSet<usize> = HashSet::new();
    let films = |r: &LoadResult| -> Vec<usize> {
        let mut seen = HashSet::new();
        r.items.iter().filter_map(|i| group_of.get(i.url.as_str()).copied()).filter(|g| seen.insert(*g)).collect()
    };
    let mut add = |slot: &str, id: &str, title: &str, list: Vec<usize>, more: Option<String>, claim: bool, min: usize| {
        let l: Vec<usize> = list.into_iter().filter(|f| !claimed.contains(f) && kids_ok(*f)).take(ROW_MAX).collect();
        if l.len() < min {
            return;
        }
        if claim {
            claimed.extend(l.iter().take(6).copied());
        }
        rows.push(FeedRow::new(id, title, l.into_iter().filter_map(&make_card).collect(), more, slot));
    };
    let by_role = |r: Role| -> Vec<&(Part, LoadResult)> { content.iter().copied().filter(|x| x.0.role == r).collect() };
    let newest_first = |mut l: Vec<usize>| -> Vec<usize> {
        l.sort_by(|a, b| merged[*b].updated.cmp(&merged[*a].updated));
        l
    };
    let affinity = |fi: usize| plan.taste.as_ref().map(|t| t.affinity(merged[fi].tags())).unwrap_or(0.0);
    let by_affinity = |mut l: Vec<usize>| -> Vec<usize> {
        l.sort_by(|a, b| {
            affinity(*b).partial_cmp(&affinity(*a)).unwrap_or(Ordering::Equal).then(merged[*b].updated.cmp(&merged[*a].updated))
        });
        l
    };
    let unwatched = |l: Vec<usize>| -> Vec<usize> { l.into_iter().filter(|f| !watched(*f)).collect() };

    // Có tập mới: phim đang xem dở (xem trong 30 ngày, đã xem kịp tập mới nhất lúc đó) vừa có tập mới ở bất kỳ nguồn nào
    let mut fresh: Vec<FreshCard> = Vec::new();
    let mut groups: Vec<(String, Vec<&HistoryItem>)> = Vec::new();
    let mut group_at: HashMap<String, usize> = HashMap::new();
    for h in &ctx.history {
        let key = kk_phim::norm(&h.title);
        match group_at.get(&key) {
            Some(&i) => groups[i].1.push(h),
            None => {
                group_at.insert(key.clone(), groups.len());
                groups.push((key, vec![h]));
            }
        }
    }
    for (key, same) in &groups {
        let h = same[0]; // xem gần nhất
        if ctx.now - h.time > FRESH_WATCHED || ctx.follows.contains(&h.film_url) {
            continue;
        }
        let eps: Vec<f64> = same.iter().filter_map(|x| feed::ep_of(Some(&x.ep_title))).collect();
        let Some(watched_ep) = eps.iter().copied().reduce(f64::max) else { continue };
        let Some(f) = merged.iter().find(|g| g.items.iter().any(|i| i.url == h.film_url) || g.names.contains(key)) else { continue };
        let Some(latest) = f.latest() else { continue };
        let Some(ep) = latest.ep else { continue };
        if latest.updated < ctx.now - FRESH_NEW {
            continue;
        }
        let seen = (ctx.seen)(key);
        if let Some(sn) = seen {
            if watched_ep < sn - 1.0 {
                continue; // chưa xem kịp — không phải «tập mới»
            }
        }
        let known = watched_ep.max(seen.unwrap_or(watched_ep));
        if ep <= known {
            continue;
        }
        // Xem từ trước bản này: số tập ra thêm phải hợp với thời gian từ lần xem cuối (≤ 2 tập + 3 tập mỗi tuần)
        if seen.is_none() && ep - watched_ep > (2 + ((ctx.now - h.time) as f64 / DAY as f64 / 7.0 * 3.0) as i64) as f64 {
            continue;
        }
        let own_host = site::host_of(&h.film_url).unwrap_or_default();
        let badge = if ep - known >= 2.0 { format!("+{} · Tập {}", num(ep - known), num(ep)) } else { format!("Tập {} mới", num(ep)) };
        let chip = if down.contains(latest.host.as_str()) {
            Some("lần kiểm trước".to_string())
        } else if latest.host != own_host {
            Some(format!("có ở {}", site::tag(&latest.host, &ctx.sites)))
        } else {
            None
        };
        let open = if latest.host == own_host { f.items.iter().find(|i| i.url == h.film_url).unwrap_or(latest) } else { latest };
        let links: Vec<FeedItem> = std::iter::once(open.clone()).chain(f.items.iter().filter(|i| i.url != open.url).cloned()).collect();
        let film = Film {
            url: open.url.clone(),
            title: h.title.clone(),
            poster: h.poster.clone().or_else(|| open.poster.clone()),
            badge: Some(badge),
            imdb: links.iter().find_map(|l| l.imdb),
        };
        fresh.push(FreshCard { card: Card { film, chip, links }, key: key.clone(), ep });
    }

    // Hàng tự thêm (và chủ đề người mới chọn): phim chưa xem, mới cập nhật trước
    for (p, r) in by_role(Role::Custom) {
        add(&p.slot, p.row_id(), p.title.as_deref().unwrap_or(""), newest_first(unwatched(films(r))), p.more.clone(), true, 3);
    }

    // Giống «phim vừa xem»: xếp theo số thể loại chung
    let sim_genres: HashSet<&str> = plan.similar_tags.as_ref().map(|t| t.genres.iter().map(|g| g.as_str()).collect()).unwrap_or_default();
    let me: HashSet<String> = plan.similar.as_ref().map(|s| names_of_title(&s.title)).unwrap_or_default();
    for (p, r) in by_role(Role::Similar) {
        let mut list: Vec<usize> = films(r).into_iter().filter(|f| !watched(*f) && !merged[*f].matches(&me)).collect();
        let common = |fi: &usize| merged[*fi].tags().map(|t| t.genres.iter().filter(|g| sim_genres.contains(g.as_str())).count()).unwrap_or(0);
        list.sort_by(|a, b| common(b).cmp(&common(a)).then(merged[*b].updated.cmp(&merged[*a].updated)));
        add("similar", p.row_id(), p.title.as_deref().unwrap_or(""), list, p.more.clone(), true, 4);
    }
    let interest = by_role(Role::Interest);
    let interest_row = |slot: &str| interest.iter().find(|x| x.0.slot == slot).copied();
    if let Some((p, r)) = interest_row("interest") {
        add("interest", p.row_id(), p.title.as_deref().unwrap_or(""), by_affinity(unwatched(films(r))), p.more.clone(), true, 3);
    }

    // Loại phim hay xem, mới cập nhật · mọi nguồn
    let dominant = by_role(Role::Dominant);
    if let Some(first) = dominant.first() {
        let mut seen = HashSet::new();
        let set: Vec<usize> = dominant.iter().flat_map(|x| films(&x.1)).filter(|f| seen.insert(*f)).collect();
        add("dominant", "dominant", first.0.title.as_deref().unwrap_or(""), newest_first(unwatched(set)), None, true, 3);
    }
    // Thuyết minh / lồng tiếng mới
    for (p, r) in by_role(Role::Dub) {
        add("dub", p.row_id(), p.title.as_deref().unwrap_or(""), newest_first(unwatched(films(r))), p.more.clone(), true, 3);
    }
    // Hồ sơ trẻ em: hoạt hình thiếu nhi (KKPhim + VSMOV gộp một hàng), gia đình, lồng tiếng
    let mut kid_groups: Vec<(String, Vec<&(Part, LoadResult)>)> = Vec::new();
    for x in by_role(Role::Kids) {
        match kid_groups.iter_mut().find(|g| g.0 == x.0.row_id()) {
            Some(g) => g.1.push(x),
            None => kid_groups.push((x.0.row_id().to_string(), vec![x])),
        }
    }
    for (key, ps) in &kid_groups {
        let mut seen = HashSet::new();
        let set: Vec<usize> = ps.iter().flat_map(|x| films(&x.1)).filter(|f| seen.insert(*f)).collect();
        let more = ps.iter().find_map(|x| x.0.more.clone());
        add(key, key, ps[0].0.title.as_deref().unwrap_or(""), newest_first(unwatched(set)), more, true, 3);
    }
    if let Some((p, r)) = interest_row("interest2") {
        add("interest2", p.row_id(), p.title.as_deref().unwrap_or(""), by_affinity(unwatched(films(r))), p.more.clone(), true, 3);
    }

    // Mới cập nhật · mọi nguồn
    let mut seen = HashSet::new();
    let newest: Vec<usize> = by_role(Role::Web).into_iter().chain(by_role(Role::New)).flat_map(|x| films(&x.1)).filter(|f| seen.insert(*f)).collect();
    add("newest", "newest", "Mới cập nhật · mọi nguồn", newest_first(unwatched(newest)), None, false, 3);

    for (p, r) in by_role(Role::Movies) {
        add("movies", p.row_id(), p.title.as_deref().unwrap_or(""), by_affinity(unwatched(films(r))), p.more.clone(), false, 3);
    }
    // Thứ tự / bật tắt theo cài đặt của người xem (Cài đặt → Trang chủ → Các hàng)
    let order = arrange_index(&rows, None);
    let mut slots: Vec<Option<FeedRow>> = rows.into_iter().map(Some).collect();
    let arranged = order.into_iter().filter_map(|(i, _)| slots[i].take()).collect();
    (arranged, fresh)
}

/// Người xem vừa mở trang phim / một tập: ghi tập mới nhất đã thấy (theo tên — chung mọi nguồn).
pub fn mark_seen(d: &FilmDetail) {
    let eps = d.groups.iter().flat_map(|g| g.episodes.iter()).filter_map(|e| feed::ep_of(Some(&e.title)));
    if let Some(max) = eps.reduce(f64::max) {
        prefs::set_seen_ep(&kk_phim::norm(&d.title), max);
    }
}

/// 108.0 → "108", 10.5 → "10.5".
pub fn num(d: f64) -> String {
    if d == d.floor() && d.abs() < 9.0e18 {
        format!("{}", d as i64)
    } else {
        format!("{d}")
    }
}

// ---- Trang chủ lần trước (mở app là hiện ngay) ----

fn last_key(pid: &str) -> String {
    format!("last|{pid}")
}

pub fn save_last(pid: &str, s: &Snapshot) {
    if s.from_cache || s.empty {
        return;
    }
    let rows: Vec<Value> = s
        .rows
        .iter()
        .map(|r| {
            let cards: Vec<Value> = r.cards.iter().map(card_json).collect();
            json!({ "id": r.id, "t": r.title, "m": r.more.clone().unwrap_or_default(), "s": r.slot, "c": cards })
        })
        .collect();
    let fresh: Vec<Value> = s.fresh.iter().map(|f| json!({ "c": card_json(&f.card), "k": f.key, "e": f.ep })).collect();
    let mut o = Map::new();
    o.insert("at".into(), json!(now_ms()));
    o.insert("rows".into(), Value::Array(rows));
    o.insert("fresh".into(), Value::Array(fresh));
    if let Some(c) = &s.categories {
        o.insert("cats".into(), Value::Array(c.iter().map(|c| json!({"t": c.title, "u": c.url})).collect()));
    }
    feed::write_text(&last_key(pid), &Value::Object(o).to_string());
}

/// Trang chủ lần trước của người xem pid (dưới 24 giờ); None nếu chưa có.
pub fn last(pid: &str) -> Option<Snapshot> {
    let text = feed::read_text(&last_key(pid))?;
    let o = json::parse_object(text.as_bytes())?;
    let at = feed::num64(o.get("at"));
    if now_ms() - at > feed::CACHE_MAX {
        return None;
    }
    let rows: Vec<FeedRow> = json::objects(json::arr(Some(&o), "rows"))
        .into_iter()
        .map(|r| {
            FeedRow::new(
                json::str(Some(r), "id"),
                json::str(Some(r), "t"),
                json::objects(json::arr(Some(r), "c")).into_iter().filter_map(card_of).collect(),
                non_empty(Some(&json::str(Some(r), "m"))),
                json::str(Some(r), "s"),
            )
        })
        .collect();
    let fresh: Vec<FreshCard> = json::objects(json::arr(Some(&o), "fresh"))
        .into_iter()
        .filter_map(|f| {
            let card = card_of(json::obj(Some(f), "c")?)?;
            Some(FreshCard { card, key: json::str(Some(f), "k"), ep: feed::dbl(f.get("e")).unwrap_or(0.0) })
        })
        .collect();
    let cats = json::arr(Some(&o), "cats")
        .map(|cs| json::objects(Some(cs)).into_iter().map(|x| Category::new(json::str(Some(x), "t"), json::str(Some(x), "u"))).collect());
    let note = format!("Bản lưu lúc {} · đang làm mới…", local_hm(at));
    let empty = rows.is_empty() && fresh.is_empty();
    Some(Snapshot { rows, fresh, categories: cats, note: note.clone(), note_gave_up: note, empty, from_cache: true, ..Default::default() })
}

fn card_json(c: &Card) -> Value {
    // nguồn của thẻ: bỏ tên chuẩn hoá / thể loại cho gọn
    let links: Vec<Value> = c
        .links
        .iter()
        .map(|l| {
            let mut l = l.clone();
            l.names = HashSet::new();
            l.tags = None;
            l.json()
        })
        .collect();
    json!({
        "u": c.film.url, "n": c.film.title, "p": c.film.poster.clone().unwrap_or_default(), "b": c.film.badge.clone().unwrap_or_default(),
        "x": c.chip.clone().unwrap_or_default(), "l": links,
    })
}

fn card_of(o: &Obj) -> Option<Card> {
    let url = non_empty(Some(&json::str(Some(o), "u")))?;
    let film = Film::new(url, json::str(Some(o), "n"), non_empty(Some(&json::str(Some(o), "p"))), non_empty(Some(&json::str(Some(o), "b"))));
    let links = json::objects(json::arr(Some(o), "l")).into_iter().filter_map(FeedItem::of).collect();
    Some(Card { film, chip: non_empty(Some(&json::str(Some(o), "x"))), links })
}

/// Thể loại / quốc gia / loại phim của phim đã xem (tra KKPhim theo tên, lưu chung cả máy — prefs::film_tags). Khoá = tên
/// đã chuẩn hoá; {g, c, t, at} hoặc {miss: lúc tra không thấy}.
pub mod film_tags {
    use crate::feed::{self, Tags};
    use crate::json::{self, Obj};
    use crate::site::lock;
    use crate::util::{form_encoded, now_ms, take_chars, Result};
    use crate::{http, kk_phim, prefs};
    use serde_json::{json, Map, Value};
    use std::sync::{LazyLock, Mutex};

    const MAX_COUNT: usize = 400;
    const MISS_RETRY: i64 = 3 * 86_400_000;

    struct Mem {
        of: Option<String>,
        map: Obj,
    }

    static MEM: LazyLock<Mutex<Mem>> = LazyLock::new(|| Mutex::new(Mem { of: None, map: Map::new() }));

    /// Đọc lại từ cài đặt khi chuỗi đã lưu đổi.
    fn store(m: &mut Mem) -> &Obj {
        let raw = prefs::film_tags();
        if m.of.as_deref() != Some(raw.as_str()) {
            m.map = json::parse_object(raw.as_bytes()).unwrap_or_default().into_iter().filter(|(_, v)| v.is_object()).collect();
            m.of = Some(raw);
        }
        &m.map
    }

    pub fn of(title: &str) -> Option<Tags> {
        let mut m = lock(&MEM);
        let e = store(&mut m).get(&kk_phim::norm(title))?.as_object()?;
        if e.contains_key("miss") {
            return None;
        }
        Tags::of(Some(e))
    }

    pub fn needs_lookup(title: &str) -> bool {
        let mut m = lock(&MEM);
        match store(&mut m).get(&kk_phim::norm(title)).and_then(|v| v.as_object()) {
            None => true,
            Some(e) => e.contains_key("miss") && now_ms() - feed::num64(e.get("miss")) > MISS_RETRY,
        }
    }

    pub fn remember(key: &str, tags: Option<&Tags>) {
        if key.is_empty() {
            return;
        }
        let mut m = lock(&MEM);
        let mut s = store(&mut m).clone();
        let v = match tags {
            Some(t) => {
                let mut o = t.json();
                if let Some(o) = o.as_object_mut() {
                    o.insert("at".into(), json!(now_ms()));
                }
                o
            }
            None => json!({ "miss": now_ms() }),
        };
        s.insert(key.to_string(), v);
        if s.len() > MAX_COUNT {
            let at = |o: &Value| {
                let o = o.as_object();
                match o.and_then(|o| o.get("at")) {
                    Some(v) => feed::num64(Some(v)),
                    None => feed::num64(o.and_then(|o| o.get("miss"))),
                }
            };
            let mut keys: Vec<(i64, String)> = s.iter().map(|(k, v)| (at(v), k.clone())).collect();
            // cũ nhất trước (bằng nhau: theo thứ tự lưu)
            keys.sort_by_key(|x| x.0);
            let drop = s.len() - MAX_COUNT;
            for (_, k) in keys.into_iter().take(drop) {
                s.shift_remove(&k);
            }
        }
        let raw = Value::Object(s.clone()).to_string();
        prefs::set_film_tags(&raw);
        m.map = s;
        m.of = Some(raw);
    }

    /// Tra KKPhim theo tên (lỗi mạng → báo lỗi, không ghi; không thấy → ghi «không có»).
    pub async fn lookup(title: &str) -> Result<()> {
        let q = format!("{}/v1/api/tim-kiem?keyword={}&limit=10", kk_phim::API, form_encoded(&take_chars(title, 80)));
        let r = http::get(&q, &[("Accept", "application/json")]).await?;
        let root = crate::site::json_object(&r.data);
        let items = json::objects(json::arr(json::obj(root.as_ref(), "data"), "items"));
        let hit = kk_phim::choose(&items, &super::names_of_title(title));
        let tags = hit.and_then(Tags::from_kk);
        remember(&kk_phim::norm(title), tags.as_ref());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Các hàng trang chủ (HomeRowsTests.cs / HomeRowsTest.kt / HomeRowsTests.swift) + phần không cần cài đặt của FeedTests.cs
    //! (sở thích, ghép lại kết quả). Phần dùng cài đặt: tests/home_feed.rs.
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn defaults() -> Vec<String> {
        s(&["new-eps", "continue", "similar", "interest", "dominant", "newest", "movies"])
    }

    fn ids(l: &[(String, bool)]) -> Vec<&str> {
        l.iter().map(|x| x.0.as_str()).collect()
    }

    fn p(v: &[(&str, bool)]) -> Vec<(String, bool)> {
        v.iter().map(|(a, b)| (a.to_string(), *b)).collect()
    }

    #[test]
    fn empty_means_default_all_on() {
        let d = defaults();
        let m = merge_rows(&[], Some(&d));
        assert_eq!(ids(&m), d.iter().map(|x| x.as_str()).collect::<Vec<_>>());
        assert!(m.iter().all(|x| x.1));
    }

    #[test]
    fn keeps_saved_order_and_off() {
        let m = merge_rows(&s(&["new-eps", "continue", "newest", "-similar", "interest", "dominant", "movies"]), Some(&defaults()));
        assert_eq!(ids(&m), ["new-eps", "continue", "newest", "similar", "interest", "dominant", "movies"]);
        assert!(!m.iter().find(|x| x.0 == "similar").unwrap().1);
    }

    #[test]
    fn drops_unknown_and_duplicates() {
        let m = merge_rows(&s(&["bogus", "movies", "-movies", "similar"]), Some(&defaults()));
        assert_eq!(m.iter().filter(|x| x.0 == "movies").count(), 1);
        assert!(m.iter().find(|x| x.0 == "movies").unwrap().1);
        assert_eq!(m.len(), defaults().len());
    }

    /// Hàng mới ở bản sau (chưa có trong thứ tự đã lưu) chèn ngay sau hàng đứng trước nó trong thứ tự mặc định.
    #[test]
    fn new_rows_inserted_after_their_default_predecessor() {
        let m = merge_rows(&s(&["movies", "newest", "similar", "new-eps", "continue"]), Some(&defaults()));
        assert_eq!(ids(&m), ["movies", "newest", "similar", "interest", "dominant", "new-eps", "continue"]);
    }

    #[test]
    fn custom_rows_default_after_head() {
        let custom = vec![CustomRow::filter("a", "hoathinh", "", "", ""), CustomRow::for_topic("kr")];
        let d = row_defaults(Some(&custom));
        assert_eq!(&d[..5], ["new-eps", "continue", "custom-a", "custom-t-kr", "similar"]);
        let m = merge_rows(&s(&["new-eps", "continue", "custom-a", "movies", "similar"]), Some(&d));
        assert_eq!(&ids(&m)[..6], ["new-eps", "continue", "custom-a", "custom-t-kr", "movies", "similar"]);
    }

    #[test]
    fn custom_titles() {
        assert_eq!(
            custom_title(&CustomRow::filter("x", "hoathinh", "trung-quoc", "phieu-luu", "thuyet-minh")),
            "Hoạt hình Trung Quốc · phiêu lưu · thuyết minh"
        );
        assert_eq!(custom_title(&CustomRow::filter("y", "series", "han-quoc", "", "")), "Phim bộ Hàn Quốc");
        assert_eq!(custom_title(&CustomRow::filter("z", "single", "", "hanh-dong", "")), "Phim lẻ · hành động");
        assert_eq!(custom_title(&CustomRow::for_topic("kr")), "Phim Hàn");
        assert!(CustomRow::filter("c1", "series", "han-quoc", "", "").same(&CustomRow::filter("khac", "series", "han-quoc", "", "")));
        assert!(!CustomRow::filter("c1", "series", "han-quoc", "", "").same(&CustomRow::filter("c1", "series", "", "", "")));
    }

    #[test]
    fn custom_row_json_like_windows() {
        let c = CustomRow::filter("c1", "hoathinh", "trung-quoc", "", "long-tieng");
        assert_eq!(c.json().to_string(), r#"{"id":"c1","topic":null,"type":"hoathinh","country":"trung-quoc","genre":"","lang":"long-tieng"}"#);
        assert_eq!(CustomRow::of(&c.json()), Some(c));
        let t = CustomRow::of(&json!({"id": "t-cn", "topic": "cn"})).unwrap();
        assert_eq!(t, CustomRow::for_topic("cn"));
        assert_eq!(t.key(), "custom-t-cn");
    }

    fn row(id: &str, slot: &str) -> FeedRow {
        FeedRow::new(id, id, Vec::new(), None, slot)
    }

    #[test]
    fn arrange_filters_and_orders() {
        let rows = vec![row("topic-cn", "topics"), row("similar", "similar"), row("interest-a", "interest"), row("newest", "newest"), row("movies", "movies")];
        let prefs = p(&[("new-eps", true), ("continue", true), ("movies", true), ("similar", false), ("newest", true), ("interest", true)]);
        let got: Vec<String> = arrange(&rows, Some(&prefs)).into_iter().map(|r| r.id).collect();
        assert_eq!(got, ["topic-cn", "movies", "newest", "interest-a"]);
    }

    /// Trang chủ lưu từ bản cũ (chưa ghi slot): đoán theo id — hai hàng «Vì … hay xem» theo thứ tự xuất hiện.
    #[test]
    fn arrange_legacy_rows() {
        let rows = vec![row("similar", ""), row("interest-hanh-dong", ""), row("dominant", ""), row("interest-tien-hiep", ""), row("newest", "")];
        let prefs = p(&[("interest2", true), ("similar", true), ("interest", false), ("dominant", true), ("newest", true)]);
        let got: Vec<String> = arrange(&rows, Some(&prefs)).into_iter().map(|r| r.id).collect();
        assert_eq!(got, ["interest-tien-hiep", "similar", "dominant", "newest"]);
    }

    /// Sửa nhanh trên trang chủ: chuyển qua hàng đang hiện kề nó (hàng tắt / không hiện nằm giữa bị vượt qua).
    #[test]
    fn move_shown_skips_hidden_rows() {
        let prefs = p(&[("new-eps", true), ("continue", true), ("similar", true), ("interest", false), ("dominant", true), ("newest", true), ("movies", true)]);
        let shown = s(&["similar", "dominant", "movies"]); // «Mới cập nhật» không đủ phim nên không hiện
        assert_eq!(ids(&move_shown(&prefs, &shown, "dominant", true).unwrap()), ["new-eps", "continue", "dominant", "similar", "interest", "newest", "movies"]);
        let down = move_shown(&prefs, &shown, "dominant", false).unwrap();
        assert_eq!(ids(&down), ["new-eps", "continue", "similar", "interest", "newest", "movies", "dominant"]);
        assert!(!down.iter().find(|x| x.0 == "interest").unwrap().1);
        assert!(move_shown(&prefs, &shown, "similar", true).is_none());
        assert!(move_shown(&prefs, &shown, "movies", false).is_none());
        assert!(move_shown(&prefs, &s(&["continue", "similar"]), "continue", false).is_none());
        assert!(move_shown(&prefs, &shown, "newest", true).is_none());
    }

    #[test]
    fn hide_turns_off_only_that_row() {
        let h = hide(&p(&[("similar", true), ("custom-a", true), ("movies", false)]), "custom-a");
        let got: Vec<String> = h.iter().map(|x| format!("{}={}", x.0, x.1)).collect();
        assert_eq!(got, ["similar=true", "custom-a=false", "movies=false"]);
    }

    #[test]
    fn arrange_slots_keeps_slot_of_legacy_rows() {
        let rows = vec![row("interest-a", ""), row("custom-x", "custom-x"), row("interest-b", "")];
        let prefs = p(&[("custom-x", true), ("interest2", true), ("interest", true)]);
        let got: Vec<String> = arrange_slots(&rows, Some(&prefs)).into_iter().map(|(r, s)| format!("{}={s}", r.id)).collect();
        assert_eq!(got, ["custom-x=custom-x", "interest-b=interest2", "interest-a=interest"]);
    }

    /// Nguồn tắt thì bỏ; tắt hết nguồn đang có thì coi như bật hết (trang chủ không trống).
    #[test]
    fn sources_off() {
        let some = |x: &str| Some(x.to_string());
        let all = Sources { kk: some("https://kk"), nc: some("https://nc"), vs: None, k20: some("https://k20"), web: s(&["https://web1", "https://web2"]) };
        assert_eq!(all.ids(), ["kk", "nc", "k20", "web"]);
        assert_eq!(all.count(), 5);
        let off = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<HashSet<String>>();
        let r = without_off(&all, &off(&["kk", "web", "vs"]));
        assert_eq!(r.ids(), ["nc", "k20"]);
        assert!(r.kk.is_none());
        assert!(r.web.is_empty());
        assert_eq!(without_off(&all, &off(&["kk", "nc", "k20", "web"])).ids(), all.ids());
        let only_nc = Sources { nc: some("https://nc"), ..Default::default() };
        assert_eq!(without_off(&only_nc, &off(&["kk"])).ids(), ["nc"]);
    }

    fn tags(g: &[&str], c: &[&str], t: &str) -> Tags {
        Tags { genres: s(g), countries: s(c), kind: Some(t.to_string()) }
    }

    const NOW: i64 = 1_794_142_800_000; // 2026-11-08T20:00:00+07:00

    #[test]
    fn taste_favours_recent_and_much_watched() {
        assert_eq!(NOW, feed::time(Some("2026-11-08T20:00:00+07:00")));
        let map: HashMap<&str, Tags> = [
            ("Tiên Nghịch", tags(&["vo-thuat", "hoat-hinh", "chinh-kich"], &["trung-quoc"], "hoathinh")),
            ("Hạ Cánh Nơi Anh", tags(&["tinh-cam"], &["han-quoc"], "series")),
        ]
        .into_iter()
        .collect();
        let history = vec![
            HistoryItem { film_url: "https://yanhh3d.ee/tien-nghich".into(), title: "Tiên Nghịch".into(), ep_title: "Tập 166".into(), ep_url: "e1".into(), time: NOW - DAY, poster: None },
            HistoryItem { film_url: "https://www.kkphim.com/phim/ha-canh".into(), title: "Hạ Cánh Nơi Anh".into(), ep_title: "Tập 3".into(), ep_url: "e2".into(), time: NOW - 60 * DAY, poster: None },
        ];
        let t = get_taste(&history, |s| map.get(s).cloned(), |u| if u.contains("tien") { 20 } else { 3 }, NOW);
        assert!(t.genre.get("vo-thuat") > 15.0);
        assert!(!t.genre.contains_key("hoat-hinh"));
        assert!((t.genre.get("vo-thuat") / 2.0 - t.genre.get("chinh-kich")).abs() < 1e-9);
        assert_eq!(t.combos.by_score()[0].0, &Combo::new("hoathinh", "trung-quoc", "vo-thuat"));
        assert!(t.affinity(Some(&tags(&["vo-thuat"], &["trung-quoc"], "hoathinh"))) > t.affinity(Some(&tags(&["tinh-cam"], &["han-quoc"], "series"))));
        assert_eq!(t.affinity(None), 0.0);
    }

    fn req(key: &str) -> Req {
        Req::new(key, "kk.test", Arc::new(|| -> feed::LoadFut { Box::pin(async { Ok(Loaded::default()) }) }))
    }

    fn res(r: &Req) -> LoadResult {
        LoadResult { req: r.clone(), state: State::Ok, items: Vec::new(), categories: None, error: None, resting: false, ms: 0, saved_at: 0, moved_to: None }
    }

    #[test]
    fn regroup_tolerates_two_rows_with_same_list() {
        // Hàng tự thêm «Hoạt hình Trung Quốc» và hàng «xem nhiều nhất · mọi nguồn» cùng một địa chỉ danh sách (lỗi «same key» 11/2026)
        let u = "https://kk.test/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=30";
        let custom = Part::new(Role::Custom, req(&format!("list|{u}")), Some("Hoạt hình Trung Quốc".into()), Some(u.into()), "c1", "c1");
        let dominant = Part::new(Role::Dominant, req(&format!("list|{u}")), Some("Hoạt hình Trung Quốc mới cập nhật · mọi nguồn".into()), None, "dominant", "dominant");
        let tags = Part::new(Role::Tags, req("tags|phim"), None, None, "", "");
        let results = vec![(custom.clone(), res(&custom.req)), (dominant.clone(), res(&dominant.req)), (tags.clone(), res(&tags.req))];
        let got = regroup(&[dominant, custom], &results);
        assert_eq!(got.iter().map(|x| x.0.slot.as_str()).collect::<Vec<_>>(), ["dominant", "c1", ""]);
    }

    #[test]
    fn numbers_and_helpers() {
        assert_eq!(num(108.0), "108");
        assert_eq!(num(10.5), "10.5");
        assert_eq!(cap("hoạt hình"), "Hoạt hình");
        assert_eq!(cap("TV show"), "TV show");
        assert_eq!(type_name("tvshows"), "TV show");
        assert_eq!(row_label("dub"), "Thuyết minh / lồng tiếng mới");
        assert!(adult_genres().contains(&"kinh-di"));
    }

    #[test]
    fn filter_and_topic_urls() {
        let some = |x: &str| Some(x.to_string());
        let kk = Sources { kk: some("https://www.kkphim.com"), vs: some("https://vsmov.com"), ..Default::default() };
        assert_eq!(
            filter_url(&kk, "hoathinh", "trung-quoc", Some("tien-hiep")).as_deref(),
            Some("https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&category=tien-hiep&page=1&limit=30")
        );
        let vs = Sources { vs: some("https://vsmov.com"), ..Default::default() };
        assert_eq!(filter_url(&vs, "hoathinh", "trung-quoc", None).as_deref(), Some("https://vsmov.com/api/the-loai/hoat-hinh?country=trung-quoc&page=1"));
        assert_eq!(filter_url(&vs, "series", "han-quoc", None).as_deref(), Some("https://vsmov.com/api/quoc-gia/han-quoc?page=1"));
        assert!(filter_url(&Sources::default(), "series", "han-quoc", None).is_none());
        let (r, more) = topic_req(&kk, "cn").unwrap();
        assert_eq!(more, "https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=30");
        assert_eq!(r.key, format!("list|{more}"));
        assert_eq!(r.host, "kkphim.com");
        assert_eq!(topic_req(&vs, "kh").unwrap().1, "https://vsmov.com/api/the-loai/kiem-hiep?page=1");
        assert!(topic_req(&Sources::default(), "cn").is_none());
        let c = CustomRow::filter("c", "series", "han-quoc", "", "thuyet-minh");
        assert_eq!(custom_req(&kk, &c).unwrap().1, "https://www.kkphim.com/api/movies/category/phim-bo?country=han-quoc&sort_lang=thuyet-minh&page=1&limit=30");
        assert_eq!(custom_req(&vs, &c).unwrap().1, "https://vsmov.com/api/quoc-gia/han-quoc?page=1");
        assert_eq!(custom_req(&vs, &CustomRow::filter("c", "single", "", "", "")).unwrap().1, "https://vsmov.com/api/danh-sach/phim-le?page=1");
    }

    #[test]
    fn newest_block_of_web_home() {
        let f = |u: &str| Film::new(format!("https://x.test/{u}"), u, None, None);
        let h = Home {
            sections: vec![
                FilmSection { title: "Đề cử".into(), films: (0..7).map(|i| f(&format!("a{i}"))).collect() },
                FilmSection { title: "Phim mới cập nhật".into(), films: vec![f("b"), f("c"), f("b")] },
            ],
            ..Default::default()
        };
        assert_eq!(newest_of(&h).iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), ["b", "c"]);
        let h2 = Home { sections: vec![FilmSection { title: "Ít".into(), films: vec![f("x")] }, FilmSection { title: "Nhiều".into(), films: (0..6).map(|i| f(&format!("n{i}"))).collect() }], ..Default::default() };
        assert_eq!(newest_of(&h2).len(), 6);
    }
}
