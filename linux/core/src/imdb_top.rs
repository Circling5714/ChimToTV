//! Danh mục «Điểm IMDb cao» (như bản TV — ImdbTop.kt; Windows: ImdbTop.cs, iPad: ImdbTop.swift): vài trang đầu (phim mới cập
//! nhật) của các danh sách KKPhim, xếp theo điểm IMDb có trọng số (như Top 250 của IMDb: WR = v/(v+m)·R + m/(v+m)·C; dưới số
//! lượt tối thiểu thì bỏ, m = 5 lần số đó). API KKPhim không sắp theo điểm, kho quá lớn để máy tự quét hết → chỉ xếp trong vài
//! trăm phim mới nhất mỗi nhóm. Lưu kết quả 12 giờ.
//!
//! Điểm IMDb của một phim (models::Imdb) lấy thẳng từ dữ liệu KKPhim (imdb.vote_average / vote_count) — không gọi IMDb.

use crate::api_site::KK_HOST;
use crate::json;
use crate::models::{Film, FilmPage, FilmSection, Imdb};
use crate::site::{self, lock};
use crate::util::{non_empty, now_ms, Error, Result, Rx};
use futures::StreamExt;
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

/// Một nhóm của trang «Điểm IMDb cao»: đường dẫn API kiểu KKPhim, số trang đọc, số lượt chấm tối thiểu; kids: có ở hồ sơ trẻ em.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Group {
    pub id: &'static str,
    pub title: &'static str,
    pub path: &'static str,
    pub pages: i64,
    pub min_votes: i64,
    pub kids: bool,
}

const fn group(id: &'static str, title: &'static str, path: &'static str, pages: i64, min_votes: i64, kids: bool) -> Group {
    Group { id, title, path, pages, min_votes, kids }
}

/// Đường dẫn API kiểu KKPhim (www.kkphim.com/api/movies/… — api_site gọi thẳng phimapi.com).
pub const GROUPS: &[Group] = &[
    group("le", "Phim lẻ", "category/phim-le", 8, 10_000, false),
    group("bo", "Phim bộ", "category/phim-bo", 8, 2_000, false),
    group("han", "Phim Hàn Quốc", "quoc-gia/han-quoc", 5, 1_000, false),
    group("aumy", "Phim Âu Mỹ", "quoc-gia/au-my", 5, 5_000, false),
    group("tq", "Phim Trung Quốc", "quoc-gia/trung-quoc", 5, 500, false),
    group("hh", "Hoạt hình", "category/hoat-hinh", 6, 1_000, true),
];

const ORIGIN: &str = "https://www.kkphim.com";
const PAGE_SIZE: i64 = 64;
const TOP: usize = 60;
const TTL: i64 = 12 * 3_600_000;

/// Danh mục (của trang) xếp được theo IMDb: danh sách KKPhim (dữ liệu có điểm IMDb).
pub fn can_rank(url: &str) -> bool {
    match site::host_of(url) {
        Some(h) => (h.ends_with(KK_HOST) || h.ends_with("phimapi.com")) && url.contains("/api/"),
        None => false,
    }
}

/// Trang «Điểm IMDb cao»: mỗi nhóm một hàng (hồ sơ trẻ em: chỉ hoạt hình).
pub async fn page(kids: bool) -> Result<FilmPage> {
    let gs: Vec<&'static Group> = GROUPS.iter().filter(|g| !kids || g.kids).collect();
    let lists = futures::future::join_all(gs.iter().map(|g| async move { group_films(g).await.ok() })).await;
    let sections: Vec<FilmSection> = gs
        .iter()
        .zip(lists)
        .filter_map(|(g, l)| l.filter(|l| !l.is_empty()).map(|films| FilmSection { title: g.title.to_string(), films }))
        .collect();
    if sections.is_empty() {
        return Err(Error::app("Không tải được điểm IMDb (danh sách KKPhim)"));
    }
    Ok(FilmPage { sections, ..Default::default() })
}

static PAGE_PARAM: Rx = Rx::new(r"[?&]page=\d+");

/// Xếp một danh mục KKPhim theo IMDb: 5 trang đầu (≈ 320 phim mới cập nhật), phim từ 1000 lượt chấm.
pub async fn for_list(url: &str) -> Result<FilmPage> {
    let key = format!("list|{}", PAGE_PARAM.replace(url, ""));
    if let Some(c) = cached(&key) {
        return Ok(FilmPage { films: c, ..Default::default() });
    }
    let films = fetch((1..=5).map(|p| with_page(url, p)).collect()).await?;
    let ranked = rank(&films, 1000, 120);
    if !ranked.is_empty() {
        store(&key, &ranked);
    }
    Ok(FilmPage { films: ranked, ..Default::default() })
}

async fn group_films(g: &Group) -> Result<Vec<Film>> {
    let key = format!("group|{}", g.id);
    if let Some(c) = cached(&key) {
        return Ok(c);
    }
    let films = fetch((1..=g.pages).map(|p| with_page(&format!("{ORIGIN}/api/movies/{}", g.path), p)).collect()).await?;
    let ranked = rank(&films, g.min_votes, TOP);
    if !ranked.is_empty() {
        store(&key, &ranked);
    }
    Ok(ranked)
}

/// Tải các trang song song (tối đa 4 cùng lúc); trang lỗi thì bỏ, lỗi hết thì báo.
async fn fetch(urls: Vec<String>) -> Result<Vec<Film>> {
    let pages: Vec<Option<Vec<Film>>> =
        futures::stream::iter(urls).map(|u| async move { site::list(&u).await.ok().map(|p| p.films) }).buffered(4).collect().await;
    if pages.iter().all(|p| p.is_none()) {
        return Err(Error::app("Không tải được danh sách KKPhim"));
    }
    let mut seen = HashSet::new();
    Ok(pages.into_iter().flatten().flatten().filter(|f| seen.insert(f.url.clone())).collect())
}

static PAGE_SET: Rx = Rx::new(r"([?&])page=\d+");
static LIMIT_SET: Rx = Rx::new(r"([?&])limit=\d+");
static HAS_PAGE: Rx = Rx::new(r"[?&]page=");
static HAS_LIMIT: Rx = Rx::new(r"[?&]limit=");

/// Trang thứ n, mỗi trang PAGE_SIZE phim (thay page= / limit= có sẵn).
pub fn with_page(url: &str, page: i64) -> String {
    let mut u = PAGE_SET.replace(url, &format!("${{1}}page={page}"));
    u = LIMIT_SET.replace(&u, &format!("${{1}}limit={PAGE_SIZE}"));
    if !HAS_PAGE.matches(&u) {
        u.push_str(if u.contains('?') { "&" } else { "?" });
        u.push_str(&format!("page={page}"));
    }
    if !HAS_LIMIT.matches(&u) {
        u.push_str(&format!("&limit={PAGE_SIZE}"));
    }
    u
}

/// Xếp theo điểm có trọng số: chỉ phim từ min_votes lượt chấm; C = điểm trung bình của các phim đó; m = 5 lần min_votes (phim
/// 9,4 điểm từ 1.200 lượt không vượt phim 8,6 điểm từ 2 triệu lượt). Bằng điểm thì phim nhiều lượt hơn trước.
pub fn rank(films: &[Film], min_votes: i64, top: usize) -> Vec<Film> {
    let ok: Vec<(&Film, Imdb)> =
        films.iter().filter_map(|f| f.imdb.map(|i| (f, i))).filter(|(_, i)| i.votes >= min_votes).collect();
    if ok.is_empty() {
        return Vec::new();
    }
    let c = ok.iter().map(|(_, i)| i.score).sum::<f64>() / ok.len() as f64;
    let m = min_votes as f64 * 5.0;
    let wr = |i: &Imdb| {
        let v = i.votes as f64;
        v / (v + m) * i.score + m / (v + m) * c
    };
    let mut ranked: Vec<(&Film, Imdb, f64)> = ok.into_iter().map(|(f, i)| (f, i, wr(&i))).collect();
    ranked.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal).then(b.1.votes.cmp(&a.1.votes)));
    ranked.into_iter().take(top).map(|(f, _, _)| f.clone()).collect()
}

// ---- Lưu kết quả 12 giờ ----

static MEMORY: LazyLock<Mutex<HashMap<String, (i64, Vec<Film>)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Tệp lưu: None = chưa đặt (dùng &lt;thư mục dữ liệu&gt;/imdb_top.json); Some(None) = chỉ lưu trong bộ nhớ.
static CACHE_FILE: Mutex<Option<Option<PathBuf>>> = Mutex::new(None);

/// Đặt tệp lưu (None = chỉ lưu trong bộ nhớ). Không đặt thì dùng &lt;thư mục dữ liệu của app&gt;/imdb_top.json như bản Windows.
pub fn set_cache_file(path: Option<PathBuf>) {
    *lock(&CACHE_FILE) = Some(path);
}

pub fn cache_file() -> Option<PathBuf> {
    lock(&CACHE_FILE).clone().unwrap_or_else(|| Some(crate::prefs::data_dir().join("imdb_top.json")))
}

fn cached(key: &str) -> Option<Vec<Film>> {
    let mut mem = lock(&MEMORY);
    if let Some((t, films)) = mem.get(key) {
        if now_ms() - t < TTL {
            return Some(films.clone());
        }
    }
    let text = std::fs::read_to_string(cache_file()?).ok()?;
    let root = json::parse_object(text.as_bytes())?;
    let o = json::obj(Some(&root), key)?;
    let t = o.get("t").and_then(|v| v.as_i64())?;
    if now_ms() - t >= TTL {
        return None;
    }
    let films: Vec<Film> = json::objects(json::arr(Some(o), "f"))
        .into_iter()
        .filter_map(|x| {
            let url = non_empty(Some(&json::str(Some(x), "u")))?;
            let s = json::num(Some(x), "s").unwrap_or(0.0);
            let mut f = Film::new(url, json::str(Some(x), "n"), non_empty(Some(&json::str(Some(x), "p"))), non_empty(Some(&json::str(Some(x), "b"))));
            f.imdb = (s > 0.0).then(|| Imdb { score: s, votes: json::int(Some(x), "v", 0) });
            Some(f)
        })
        .collect();
    mem.insert(key.to_string(), (t, films.clone()));
    Some(films)
}

fn store(key: &str, films: &[Film]) {
    let mut mem = lock(&MEMORY);
    let t = now_ms();
    mem.insert(key.to_string(), (t, films.to_vec()));
    let Some(file) = cache_file() else { return };
    let mut root: Map<String, Value> = std::fs::read(&file).ok().and_then(|b| json::parse_object(&b)).unwrap_or_default();
    let f: Vec<Value> = films
        .iter()
        .map(|f| {
            json!({
                "u": f.url, "n": f.title, "p": f.poster.clone().unwrap_or_default(), "b": f.badge.clone().unwrap_or_default(),
                "s": f.imdb.map(|i| i.score).unwrap_or(0.0), "v": f.imdb.map(|i| i.votes).unwrap_or(0),
            })
        })
        .collect();
    root.insert(key.to_string(), json!({ "t": t, "f": f }));
    if let Some(d) = file.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&file, Value::Object(root).to_string());
}

#[cfg(test)]
mod tests {
    //! Điểm IMDb từ dữ liệu KKPhim + xếp hạng «Điểm IMDb cao» — cùng ca với ImdbTopTests.cs / ImdbTopTest.kt / ImdbTopTests.swift.
    use super::*;
    use crate::feed::FeedItem;

    fn imdb_obj(s: f64, v: i64) -> json::Obj {
        json!({ "id": "tt1", "vote_average": s, "vote_count": v }).as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn parses_score_and_ignores_tiny_vote_counts() {
        assert_eq!(Imdb::of(Some(&imdb_obj(8.5, 377_702))), Some(Imdb { score: 8.5, votes: 377_702 }));
        assert!(Imdb::of(Some(&imdb_obj(10.0, 1))).is_none());
        assert!(Imdb::of(Some(&imdb_obj(0.0, 0))).is_none());
        assert!(Imdb::of(None).is_none());
        assert_eq!(Imdb { score: 8.5, votes: 1000 }.label(), "IMDb 8.5");
        assert_eq!(Imdb { score: 7.0, votes: 1000 }.label(), "IMDb 7.0");
        assert_eq!(Imdb { score: 8.5, votes: 377_702 }.detail(), "IMDb 8.5 (377 nghìn lượt chấm)");
        assert_eq!(Imdb::votes_text(2_400_000), "2,4 triệu");
        assert_eq!(Imdb::votes_text(1_000_000), "1 triệu");
        assert_eq!(Imdb::votes_text(850), "850");
    }

    #[test]
    fn feed_item_keeps_imdb_in_cache() {
        let names: HashSet<String> = ["a".to_string()].into_iter().collect();
        let f = FeedItem::new("https://www.kkphim.com/phim/a".into(), "A".into(), None, None, names, Some(2026), 0, None, None, Some(Imdb { score: 7.9, votes: 5000 }));
        let back = |i: &FeedItem| i.json().as_object().and_then(FeedItem::of).and_then(|x| x.imdb);
        assert_eq!(back(&f), Some(Imdb { score: 7.9, votes: 5000 }));
        let mut g = f.clone();
        g.imdb = None;
        assert_eq!(back(&g), None);
    }

    #[test]
    fn weighted_rank_pulls_low_vote_films_toward_the_mean() {
        let f = |n: &str, s: f64, v: i64| {
            let mut x = Film::new(format!("https://www.kkphim.com/phim/{n}"), n, None, None);
            x.imdb = Some(Imdb { score: s, votes: v });
            x
        };
        let films = vec![
            f("nhieu-luot", 8.6, 2_000_000),
            f("it-luot", 9.4, 1_200),
            f("trung-binh", 7.0, 300_000),
            f("qua-it", 9.9, 300),
            Film::new("https://www.kkphim.com/phim/khong", "không điểm", None, None),
        ];
        let titles: Vec<String> = rank(&films, 1000, 10).into_iter().map(|f| f.title).collect();
        assert_eq!(titles, ["nhieu-luot", "it-luot", "trung-binh"]);
        assert!(rank(&films, 5_000_000, 10).is_empty());
        assert_eq!(rank(&films, 1000, 1).len(), 1);
    }

    #[test]
    fn paging_urls() {
        assert_eq!(
            with_page("https://www.kkphim.com/api/movies/the-loai/hanh-dong?page=1&limit=24", 3),
            "https://www.kkphim.com/api/movies/the-loai/hanh-dong?page=3&limit=64"
        );
        assert_eq!(with_page("https://www.kkphim.com/api/movies/category/phim-le", 2), "https://www.kkphim.com/api/movies/category/phim-le?page=2&limit=64");
        assert_eq!(
            with_page("https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24", 4),
            "https://www.kkphim.com/api/movies/category/hoat-hinh?country=trung-quoc&page=4&limit=64"
        );
        assert!(can_rank("https://www.kkphim.com/api/movies/category/phim-le?page=1&limit=24"));
        assert!(!can_rank("https://vsmov.com/api/the-loai/hoat-hinh?page=1"));
        assert!(!can_rank("không phải địa chỉ"));
    }

    #[test]
    fn cache_file_round_trip() {
        let dir = std::env::temp_dir().join(format!("cbtv-imdb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        set_cache_file(Some(dir.join("imdb_top.json")));
        let mut a = Film::new("https://www.kkphim.com/phim/a", "A", Some("https://img/a.jpg".into()), None);
        a.imdb = Some(Imdb { score: 8.1, votes: 12_345 });
        store("group|thu", &[a.clone()]);
        lock(&MEMORY).clear();
        assert_eq!(cached("group|thu"), Some(vec![a]));
        assert_eq!(cached("group|khac"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
