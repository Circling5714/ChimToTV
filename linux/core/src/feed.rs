//! Trang chủ tổng hợp — đọc danh sách phim mới của các nguồn, gộp phim trùng, giữ bản lưu từng nguồn. Nguồn lỗi / quá hạn:
//! dùng bản lưu (dưới 24 giờ), lỗi 3 lượt liên tiếp thì tạm nghỉ 30 phút (rồi 2 giờ, 6 giờ). Bản Windows: Feed.cs (Android:
//! Feed.kt, iPad: Feed.swift — giữ cùng tên hàm để sửa song song).

use crate::json::{self, Obj};
use crate::models::{Category, Film, Imdb};
use crate::util::{now_ms, Error, Rx};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// Thể loại / quốc gia / loại phim theo slug của KKPhim (hanh-dong, trung-quoc; loại: hoathinh, series, single, tvshows).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tags {
    pub genres: Vec<String>,
    pub countries: Vec<String>,
    pub kind: Option<String>,
}

fn strings(a: Option<&Vec<Value>>) -> Vec<String> {
    a.map(|a| a.iter().filter_map(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()).unwrap_or_default()
}

fn slugs(a: Option<&Vec<Value>>) -> Vec<String> {
    json::objects(a)
        .into_iter()
        .filter_map(|o| {
            let s = json::str(Some(o), "slug");
            if !s.is_empty() {
                return Some(s);
            }
            let n = slug(&json::str(Some(o), "name"));
            (!n.is_empty()).then_some(n)
        })
        .collect()
}

impl Tags {
    pub fn json(&self) -> Value {
        json!({ "g": self.genres, "c": self.countries, "t": self.kind.clone().unwrap_or_default() })
    }

    pub fn of(o: Option<&Obj>) -> Option<Tags> {
        let o = o?;
        Some(Tags {
            genres: strings(json::arr(Some(o), "g")),
            countries: strings(json::arr(Some(o), "c")),
            kind: crate::util::non_empty(Some(&json::str(Some(o), "t"))),
        })
    }

    /// category / country [{name, slug}] + type của KKPhim / VSMOV.
    pub fn from_kk(m: &Obj) -> Option<Tags> {
        let g = slugs(json::arr(Some(m), "category"));
        let c = slugs(json::arr(Some(m), "country"));
        let t = crate::util::non_empty(Some(&json::str(Some(m), "type")));
        if g.is_empty() && c.is_empty() && t.is_none() {
            None
        } else {
            Some(Tags { genres: g, countries: c, kind: t })
        }
    }
}

/// Một phim trong danh sách của một nguồn. names: tên đã chuẩn hoá (kk_phim::norm) để gộp phim trùng; updated: lúc nguồn cập
/// nhật (ms; 0 = không ghi); ep: số tập mới nhất.
#[derive(Debug, Clone)]
pub struct FeedItem {
    pub url: String,
    pub title: String,
    pub poster: Option<String>,
    pub badge: Option<String>,
    pub names: HashSet<String>,
    pub year: Option<i64>,
    pub updated: i64,
    pub ep: Option<f64>,
    pub tags: Option<Tags>,
    pub imdb: Option<Imdb>,
    /// Tên miền nguồn (tính một lần).
    pub host: String,
}

impl PartialEq for FeedItem {
    fn eq(&self, o: &Self) -> bool {
        self.url == o.url
            && self.title == o.title
            && self.poster == o.poster
            && self.badge == o.badge
            && self.names == o.names
            && self.year == o.year
            && self.updated == o.updated
            && self.ep == o.ep
            && self.tags == o.tags
    }
}

impl FeedItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        url: String,
        title: String,
        poster: Option<String>,
        badge: Option<String>,
        names: HashSet<String>,
        year: Option<i64>,
        updated: i64,
        ep: Option<f64>,
        tags: Option<Tags>,
        imdb: Option<Imdb>,
    ) -> FeedItem {
        let host = crate::site::host_of(&url).unwrap_or_default();
        FeedItem { url, title, poster, badge, names, year, updated, ep, tags, imdb, host }
    }

    pub fn json(&self) -> Value {
        let mut names: Vec<&String> = self.names.iter().collect();
        names.sort();
        let mut o = Map::new();
        o.insert("u".into(), json!(self.url));
        o.insert("n".into(), json!(self.title));
        o.insert("p".into(), json!(self.poster.clone().unwrap_or_default()));
        o.insert("b".into(), json!(self.badge.clone().unwrap_or_default()));
        o.insert("k".into(), json!(names));
        o.insert("y".into(), json!(self.year.unwrap_or(0)));
        o.insert("t".into(), json!(self.updated));
        o.insert("e".into(), json!(self.ep.unwrap_or(-1.0)));
        if let Some(t) = &self.tags {
            o.insert("g".into(), t.json());
        }
        if let Some(i) = &self.imdb {
            o.insert("is".into(), json!(i.score));
            o.insert("iv".into(), json!(i.votes));
        }
        Value::Object(o)
    }

    pub fn of(o: &Obj) -> Option<FeedItem> {
        let url = crate::util::non_empty(Some(&json::str(Some(o), "u")))?;
        let title = crate::util::non_empty(Some(&json::str(Some(o), "n")))?;
        let e = dbl(o.get("e")).unwrap_or(-1.0);
        let y = num64(o.get("y"));
        let imdb = dbl(o.get("is")).filter(|s| *s > 0.0).map(|s| Imdb { score: s, votes: num64(o.get("iv")) });
        Some(FeedItem::new(
            url,
            title,
            crate::util::non_empty(Some(&json::str(Some(o), "p"))),
            crate::util::non_empty(Some(&json::str(Some(o), "b"))),
            strings(json::arr(Some(o), "k")).into_iter().collect(),
            (y > 0).then_some(y),
            num64(o.get("t")),
            (e >= 0.0).then_some(e),
            Tags::of(json::obj(Some(o), "g")),
            imdb,
        ))
    }

    /// Thẻ phim của mục.
    pub fn film(&self) -> Film {
        Film { url: self.url.clone(), title: self.title.clone(), poster: self.poster.clone(), badge: self.badge.clone(), imdb: self.imdb }
    }
}

/// Phim đã gộp từ các nguồn (items: mỗi nguồn một mục).
#[derive(Debug, Clone)]
pub struct FeedFilm {
    pub items: Vec<FeedItem>,
    pub names: HashSet<String>,
    pub updated: i64,
}

impl FeedFilm {
    pub fn new(items: Vec<FeedItem>) -> FeedFilm {
        let names = items.iter().flat_map(|i| i.names.iter().cloned()).collect();
        let updated = items.iter().map(|i| i.updated).max().unwrap_or(0);
        FeedFilm { items, names, updated }
    }

    pub fn tags(&self) -> Option<&Tags> {
        self.items.iter().find_map(|i| i.tags.as_ref())
    }

    pub fn year(&self) -> Option<i64> {
        self.items.iter().find_map(|i| i.year)
    }

    /// Nguồn có tập mới nhất (None = không nguồn nào ghi số tập).
    pub fn latest(&self) -> Option<&FeedItem> {
        let mut best: Option<&FeedItem> = None;
        for i in &self.items {
            if let Some(e) = i.ep {
                if best.map(|b| e > b.ep.unwrap_or(f64::MIN)).unwrap_or(true) {
                    best = Some(i);
                }
            }
        }
        best
    }

    /// Xếp lại các nguồn theo thứ tự ưu tiên mở (giữ thứ tự cũ khi bằng nhau).
    pub fn ordered(&self, prefer: &[String]) -> FeedFilm {
        let rank = |i: &FeedItem| prefer.iter().position(|p| *p == i.host).unwrap_or(prefer.len());
        let mut v: Vec<(usize, &FeedItem)> = self.items.iter().enumerate().collect();
        v.sort_by_key(|(n, i)| (rank(i), *n));
        FeedFilm::new(v.into_iter().map(|(_, i)| i.clone()).collect())
    }

    pub fn matches(&self, other: &HashSet<String>) -> bool {
        !other.is_disjoint(&self.names)
    }
}

/// Mỗi nguồn chờ tối đa 6 giây; quá hạn thì dùng bản lưu, về muộn vẫn lưu cho lần sau.
pub const TIMEOUT: i64 = 6_000;
const TOTAL: i64 = 15_000;
pub const CACHE_MAX: i64 = 24 * 3_600_000;
const PSEUDO_GAP: i64 = 15 * 60_000;
const FAILS_TO_REST: i64 = 3;
const REST: [i64; 3] = [30 * 60_000, 2 * 3_600_000, 6 * 3_600_000];

fn dir() -> std::path::PathBuf {
    crate::prefs::data_dir().join("feed")
}

// ---- Đọc danh sách của từng nguồn ----

/// KKPhim (phimapi.com v1) / VSMOV: có thể loại, quốc gia, loại phim, giờ cập nhật (VSMOV không có thể loại).
pub fn parse_kk(root: &Obj, origin: &str) -> Vec<FeedItem> {
    let data = json::obj(Some(root), "data").unwrap_or(root);
    let cdn = crate::util::non_empty(Some(&json::str(Some(data), "APP_DOMAIN_CDN_IMAGE")))
        .or_else(|| crate::util::non_empty(Some(&json::str(Some(root), "pathImage"))))
        .unwrap_or_else(|| "https://phimimg.com".into());
    let mut seen = HashSet::new();
    json::objects(json::arr(Some(data), "items"))
        .into_iter()
        .filter_map(|o| {
            let slug = json::str(Some(o), "slug");
            let name = json::str(Some(o), "name");
            if slug.is_empty() || name.is_empty() {
                return None;
            }
            let ep = json::str(Some(o), "episode_current");
            let poster_raw = crate::util::non_empty(Some(&json::str(Some(o), "poster_url"))).unwrap_or_else(|| json::str(Some(o), "thumb_url"));
            Some(FeedItem::new(
                format!("{origin}/phim/{slug}"),
                name,
                crate::api_site::image(&poster_raw, &cdn),
                crate::next_site::badge(&ep),
                crate::kk_phim::names(o),
                year(o),
                time(Some(&json::str(json::obj(Some(o), "modified"), "time"))),
                ep_of(Some(&ep)),
                Tags::from_kk(o),
                crate::api_site::imdb_of(json::obj(Some(o), "imdb")),
            ))
        })
        .filter(|i| seen.insert(i.url.clone()))
        .collect()
}

/// NguonC /api/films/…: tên gốc (có thể nhiều tên), năm, tập hiện tại, giờ cập nhật; không có thể loại.
pub fn parse_nc(root: &Obj, origin: &str) -> Vec<FeedItem> {
    let mut seen = HashSet::new();
    json::objects(json::arr(Some(root), "items"))
        .into_iter()
        .filter_map(|o| {
            let slug = json::str(Some(o), "slug");
            let name = json::str(Some(o), "name");
            if slug.is_empty() || name.is_empty() {
                return None;
            }
            let ep = json::str(Some(o), "current_episode");
            let poster = crate::util::non_empty(Some(&json::str(Some(o), "thumb_url"))).unwrap_or_else(|| json::str(Some(o), "poster_url"));
            Some(FeedItem::new(
                format!("{origin}/phim/{slug}"),
                name,
                poster.starts_with("http").then_some(poster),
                crate::next_site::badge(&ep),
                crate::kk_phim::names(o),
                year(o),
                time(Some(&json::str(Some(o), "modified"))),
                ep_of(Some(&ep)),
                None,
                None,
            ))
        })
        .filter(|i| seen.insert(i.url.clone()))
        .collect()
}

/// Phim từ trang web / CLB Phim Xưa (chỉ có tên, ảnh, nhãn tập).
pub fn from_films(films: &[Film]) -> Vec<FeedItem> {
    let mut seen = HashSet::new();
    films
        .iter()
        .map(|f| {
            let mut o = Map::new();
            o.insert("name".into(), json!(f.title));
            FeedItem::new(
                f.url.clone(),
                f.title.clone(),
                f.poster.clone(),
                f.badge.clone(),
                crate::kk_phim::names(&o),
                None,
                0,
                ep_of(f.badge.as_deref()),
                None,
                f.imdb,
            )
        })
        .filter(|i| seen.insert(i.url.clone()))
        .collect()
}

static TAP_RX: Rx = Rx::new(r"(?i)t[aậ]p\s*(\d+(?:\.\d+)?)");
static OF_RX: Rx = Rx::new(r"(\d+)\s*/\s*(\d+|\?+)");
static NUM_RX: Rx = Rx::new(r"^(\d+(?:\.\d+)?)$");

/// Số tập trong nhãn: "Tập 108", "Hoàn Tất (16/16)", "108/120", "108"; "Full", "Top 1", "HD" → None.
pub fn ep_of(label: Option<&str>) -> Option<f64> {
    let s = label.unwrap_or("").trim();
    if s.is_empty() {
        return None;
    }
    for rx in [&TAP_RX, &OF_RX, &NUM_RX] {
        if let Some(g) = rx.group(1, s) {
            return g.parse().ok();
        }
    }
    None
}

static ISO_RX: Rx = Rx::new(r"(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2}))?(?:\.\d+)?\s*(Z|[+-]\d{2}:?\d{2})?");

/// Ngày → số ngày kể từ 1970-01-01 (lịch Gregory).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// "2026-11-08T18:55:29.000Z", "2026-11-08T21:12:36+07:00" → ms; không ghi múi giờ thì coi là giờ Việt Nam.
pub fn time(s: Option<&str>) -> i64 {
    let Some(s) = s else { return 0 };
    let Some(g) = ISO_RX.groups(s) else { return 0 };
    let n = |i: usize| g[i].parse::<i64>().unwrap_or(0);
    let (y, mo, d, h, mi, se) = (n(1), n(2), n(3), n(4), n(5), n(6));
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 59 {
        return 0;
    }
    let t = ((days_from_civil(y, mo, d) * 24 + h) * 60 + mi) * 60_000 + se * 1000;
    let zone = &g[7];
    let offset = if zone.is_empty() {
        7 * 60
    } else if zone == "Z" {
        0
    } else {
        let digits = zone[1..].replace(':', "");
        let min = digits[..2].parse::<i64>().unwrap_or(0) * 60 + digits[2..].parse::<i64>().unwrap_or(0);
        if zone.starts_with('-') {
            -min
        } else {
            min
        }
    };
    t - offset * 60_000
}

fn year(o: &Obj) -> Option<i64> {
    let y = json::int(Some(o), "year", 0);
    (1900..=2100).contains(&y).then_some(y)
}

/// "Hành Động" → "hanh-dong" (NguonC ghi tên thể loại, không có slug).
pub fn slug(name: &str) -> String {
    crate::kk_phim::norm(name).replace(' ', "-")
}

/// Số trong dữ liệu JSON (số nguyên, số thực, chuỗi số).
pub fn num64(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|d| d as i64)).unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

pub fn dbl(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
}

// ---- Gộp phim trùng ----

/// Gộp các danh sách (thứ tự = ưu tiên): có tên chung, năm lệch ≤ 1, khác nguồn. Cùng địa chỉ = cùng phim. Mục không ghi giờ
/// cập nhật tính giờ giả theo thứ tự trên trang. Kết quả xếp mới cập nhật trước.
pub fn merge(lists: &[Vec<FeedItem>], now: Option<i64>) -> Vec<FeedFilm> {
    let t0 = now.unwrap_or_else(now_ms);
    let mut groups: Vec<Vec<FeedItem>> = Vec::new();
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    let mut by_url: HashSet<String> = HashSet::new();
    for list in lists {
        for (rank, raw) in list.iter().enumerate() {
            if !by_url.insert(raw.url.clone()) {
                continue;
            }
            let mut item = raw.clone();
            if item.updated <= 0 {
                item.updated = t0 - (rank as i64 + 1) * PSEUDO_GAP;
            }
            let mut hit: Option<usize> = None;
            // thứ tự tên cố định (HashSet không có thứ tự — C# duyệt theo thứ tự chèn)
            let mut names: Vec<&String> = item.names.iter().collect();
            names.sort();
            'outer: for n in &names {
                if let Some(gis) = by_name.get(*n) {
                    for &gi in gis {
                        let g = &groups[gi];
                        let y = g.iter().find_map(|i| i.year);
                        let year_ok = match (y, item.year) {
                            (Some(a), Some(b)) => (a - b).abs() <= 1,
                            _ => true,
                        };
                        if g.iter().all(|i| i.host != item.host) && year_ok {
                            hit = Some(gi);
                            break 'outer;
                        }
                    }
                }
            }
            let idx = match hit {
                Some(i) => i,
                None => {
                    groups.push(Vec::new());
                    groups.len() - 1
                }
            };
            for n in &item.names {
                let l = by_name.entry(n.clone()).or_default();
                if !l.contains(&idx) {
                    l.push(idx);
                }
            }
            groups[idx].push(item);
        }
    }
    let mut films: Vec<(FeedFilm, usize)> = groups.into_iter().enumerate().map(|(i, g)| (FeedFilm::new(g), i)).collect();
    films.sort_by(|a, b| b.0.updated.cmp(&a.0.updated).then(a.1.cmp(&b.1)));
    films.into_iter().map(|x| x.0).collect()
}

// ---- Tải các nguồn: song song, hạn 6 giây, bản lưu, tạm nghỉ ----

pub type LoadFut = Pin<Box<dyn Future<Output = crate::Result<Loaded>> + Send>>;
pub type LoadFn = Arc<dyn Fn() -> LoadFut + Send + Sync>;

/// Một lần đọc: key = tên bản lưu, host = nguồn (tính lỗi / tạm nghỉ); keep = false: không lưu bản.
#[derive(Clone)]
pub struct Req {
    pub key: String,
    pub host: String,
    pub load: LoadFn,
    pub keep: bool,
}

impl Req {
    pub fn new(key: impl Into<String>, host: impl Into<String>, load: LoadFn) -> Req {
        Req { key: key.into(), host: host.into(), load, keep: true }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Loaded {
    pub items: Vec<FeedItem>,
    pub categories: Option<Vec<Category>>,
    pub moved_to: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ok,
    Cached,
    NoData,
}

#[derive(Clone)]
pub struct LoadResult {
    pub req: Req,
    pub state: State,
    pub items: Vec<FeedItem>,
    pub categories: Option<Vec<Category>>,
    pub error: Option<String>,
    pub resting: bool,
    pub ms: i64,
    pub saved_at: i64,
    pub moved_to: Option<String>,
}

#[derive(Default)]
struct Job {
    started_at: i64,
    finished_at: i64,
    done: Option<Loaded>,
    failed: Option<Error>,
    counted: bool,
}

/// Tải các nguồn. Trả về khi mọi nguồn xong / quá hạn — nguồn quá hạn vẫn chạy tiếp để lưu bản mới.
pub async fn load(reqs: Vec<Req>) -> Vec<LoadResult> {
    let t0 = now_ms();
    let mut results: Vec<Option<LoadResult>> = vec![None; reqs.len()];
    let mut jobs: Vec<Option<Arc<Mutex<Job>>>> = vec![None; reqs.len()];
    // Một lượt hỏi một nguồn nhiều lần (KKPhim: mới cập nhật + các hàng gợi ý) — chỉ tính một lần lỗi mỗi lượt
    let failed_hosts: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let failed_once = {
        let fh = failed_hosts.clone();
        move |host: &str| {
            if fh.lock().unwrap().insert(host.to_string()) {
                failed(host);
            }
        }
    };
    for (i, r) in reqs.iter().enumerate() {
        if resting(&r.host) {
            results[i] = Some(cached(r, None, true, 0));
            continue;
        }
        let job = Arc::new(Mutex::new(Job::default()));
        jobs[i] = Some(job.clone());
        let r = r.clone();
        let fo = failed_once.clone();
        tokio::spawn(async move {
            job.lock().unwrap().started_at = now_ms();
            // Bản thử: tệp <thư mục dữ liệu>/feed_fail_<tên miền> (hoặc feed_fail_all) → nguồn đó giả lỗi
            let fake_fail = std::env::var_os("CONBANG_DEBUG").is_some()
                && (crate::prefs::data_dir().join(format!("feed_fail_{}", r.host)).exists() || crate::prefs::data_dir().join("feed_fail_all").exists());
            let res = if fake_fail {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                Err(Error::App("máy chủ trả lỗi 502 (giả lỗi — bản thử)".into()))
            } else {
                (r.load)().await
            };
            match res {
                Ok(got) => {
                    if r.keep {
                        save(&r.key, &got);
                    }
                    job.lock().unwrap().done = Some(got);
                    healthy(&r.host);
                }
                Err(e) => {
                    let counted = {
                        let mut j = job.lock().unwrap();
                        j.failed = Some(e.clone());
                        j.counted
                    };
                    crate::log::d("feed", &format!("{} lỗi: {e}", r.key));
                    if !counted {
                        fo(&r.host);
                    }
                }
            }
            job.lock().unwrap().finished_at = now_ms();
        });
    }
    loop {
        let now = now_ms();
        let mut waiting = false;
        for i in 0..reqs.len() {
            let Some(j) = &jobs[i] else { continue };
            if results[i].is_some() {
                continue;
            }
            let (started, finished, done, err) = {
                let g = j.lock().unwrap();
                (g.started_at, g.finished_at, g.done.clone(), g.failed.clone())
            };
            let ms = (if finished > 0 { finished } else { now }) - (if started > 0 { started } else { now });
            if let Some(d) = done {
                results[i] = Some(LoadResult {
                    req: reqs[i].clone(),
                    state: State::Ok,
                    items: d.items,
                    categories: d.categories,
                    error: None,
                    resting: false,
                    ms,
                    saved_at: now,
                    moved_to: d.moved_to,
                });
            } else if let Some(e) = err {
                results[i] = Some(cached(&reqs[i], Some(short(&e)), false, ms));
            } else if started > 0 && now - started > TIMEOUT {
                j.lock().unwrap().counted = true;
                failed_once(&reqs[i].host);
                results[i] = Some(cached(&reqs[i], Some(format!("quá {} giây", TIMEOUT / 1000)), false, ms));
            } else if now - t0 > TOTAL {
                results[i] = Some(cached(&reqs[i], None, false, ms));
            } else {
                waiting = true;
            }
        }
        if !waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    }
    let out: Vec<LoadResult> = results.into_iter().map(|r| r.expect("đủ kết quả")).collect();
    for r in &out {
        crate::log::d("feed", &format!("{} {:?} {} ms {} phim {}", r.req.key, r.state, r.ms, r.items.len(), r.error.clone().unwrap_or_default()));
    }
    out
}

fn short(e: &Error) -> String {
    match e {
        Error::Http(c) => format!("lỗi {c}"),
        Error::Timeout => "không trả lời".into(),
        Error::Net(_) => "không kết nối được".into(),
        other => crate::util::take_chars(&other.to_string(), 60),
    }
}

fn cached(r: &Req, error: Option<String>, resting: bool, ms: i64) -> LoadResult {
    match r.keep.then(|| read_cache(&r.key)).flatten() {
        Some((l, at)) => LoadResult {
            req: r.clone(),
            state: State::Cached,
            items: l.items,
            categories: l.categories,
            error,
            resting,
            ms,
            saved_at: at,
            moved_to: None,
        },
        None => LoadResult { req: r.clone(), state: State::NoData, items: Vec::new(), categories: None, error, resting, ms, saved_at: 0, moved_to: None },
    }
}

/// Bản lưu của một nguồn (dưới 24 giờ).
pub fn read_cache(key: &str) -> Option<(Loaded, i64)> {
    let text = read_text(key)?;
    let o = json::parse_object(text.as_bytes())?;
    let at = num64(o.get("at"));
    if now_ms() - at > CACHE_MAX {
        return None;
    }
    let items = json::objects(json::arr(Some(&o), "items")).into_iter().filter_map(FeedItem::of).collect();
    let cats = json::arr(Some(&o), "cats").map(|c| {
        json::objects(Some(c)).into_iter().map(|x| Category::new(json::str(Some(x), "t"), json::str(Some(x), "u"))).collect()
    });
    Some((Loaded { items, categories: cats, moved_to: None }, at))
}

fn save(key: &str, l: &Loaded) {
    let mut o = Map::new();
    o.insert("at".into(), json!(now_ms()));
    o.insert("items".into(), Value::Array(l.items.iter().map(|i| i.json()).collect()));
    if let Some(c) = &l.categories {
        o.insert("cats".into(), Value::Array(c.iter().map(|c| json!({"t": c.title, "u": c.url})).collect()));
    }
    write_text(key, &Value::Object(o).to_string());
}

/// Tệp lưu tự do theo khoá (vd trang chủ lần trước của từng người xem).
pub fn write_text(key: &str, text: &str) {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    let f = file_of(key);
    let tmp = f.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    }
}

pub fn read_text(key: &str) -> Option<String> {
    std::fs::read_to_string(file_of(key)).ok()
}

fn file_of(key: &str) -> std::path::PathBuf {
    let h: String = Sha256::digest(key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    dir().join(format!("{}.json", &h[..32]))
}

// ---- Tình trạng nguồn (host → {f: lỗi liên tiếp, r: tạm nghỉ tới, l: số lần đã tạm nghỉ}) ----

static HEALTH_LOCK: Mutex<()> = Mutex::new(());

fn health() -> Map<String, Value> {
    match serde_json::from_str::<Value>(&crate::prefs::feed_health()) {
        Ok(Value::Object(o)) => o.into_iter().filter(|(_, v)| v.is_object()).collect(),
        _ => Map::new(),
    }
}

fn save_health(h: &Map<String, Value>) {
    crate::prefs::set_feed_health(&Value::Object(h.clone()).to_string());
}

pub fn resting(host: &str) -> bool {
    let _g = HEALTH_LOCK.lock().unwrap();
    health().get(host).map(|o| num64(o.get("r")) > now_ms()).unwrap_or(false)
}

/// Nguồn đang tạm nghỉ: host → tạm nghỉ tới (ms).
pub fn resting_hosts() -> HashMap<String, i64> {
    let _g = HEALTH_LOCK.lock().unwrap();
    let now = now_ms();
    health().into_iter().map(|(k, v)| (k, num64(v.get("r")))).filter(|(_, r)| *r > now).collect()
}

fn healthy(host: &str) {
    let _g = HEALTH_LOCK.lock().unwrap();
    let mut h = health();
    if h.remove(host).is_some() {
        save_health(&h);
    }
}

fn failed(host: &str) {
    let _g = HEALTH_LOCK.lock().unwrap();
    let mut h = health();
    let mut o = h.get(host).and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let f = num64(o.get("f")) + 1;
    o.insert("f".into(), json!(f));
    if f >= FAILS_TO_REST {
        let level = num64(o.get("l"));
        o.insert("r".into(), json!(now_ms() + REST[(level as usize).min(REST.len() - 1)]));
        o.insert("l".into(), json!(level + 1));
        o.insert("f".into(), json!(0));
    }
    h.insert(host.to_string(), Value::Object(o));
    save_health(&h);
}

/// «Thử lại ngay» trong Cài đặt: bỏ tạm nghỉ của mọi nguồn.
pub fn clear_rest() {
    let _g = HEALTH_LOCK.lock().unwrap();
    crate::prefs::set_feed_health("{}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ep_and_time() {
        assert_eq!(ep_of(Some("Tập 108")), Some(108.0));
        assert_eq!(ep_of(Some("Hoàn Tất (16/16)")), Some(16.0));
        assert_eq!(ep_of(Some("108/120")), Some(108.0));
        assert_eq!(ep_of(Some("108")), Some(108.0));
        assert_eq!(ep_of(Some("Full")), None);
        assert_eq!(ep_of(Some("HD")), None);
        assert_eq!(time(Some("1970-01-01T00:00:00.000Z")), 0);
        assert_eq!(time(Some("2026-11-08T21:12:36+07:00")), time(Some("2026-11-08T14:12:36Z")));
        assert_eq!(time(Some("2026-11-08 21:12:36")), time(Some("2026-11-08T14:12:36Z")));
        assert_eq!(time(Some("2026-11-08T14:12:36Z")), 1_794_147_156_000);
    }
}
