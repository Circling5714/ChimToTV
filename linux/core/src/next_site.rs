//! Trang phim dựng bằng Next.js, giao diện kiểu "PhimMoi" (vd phimmois.fun) — bản Android: NextSite.kt, iPad: NextSite.swift,
//! Windows: NextSite.cs. Không đọc HTML: mọi dữ liệu (phim, đủ danh sách tập, server, link video) nằm sẵn trong các đoạn push
//! của trang.
//!
//! Bản Rust: vị trí trong dữ liệu là vị trí byte (UTF-8) — regex, tìm chuỗi và `json::scan` cùng dùng một kiểu vị trí.

use crate::html::Doc;
use crate::json::{self, Obj};
use crate::models::*;
use crate::site::{distinct_by, jstr, lock, ne};
use crate::util::{after_last, form_encoded, Error, Result, Rx};
use crate::{http, prefs, site};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

/// Dữ liệu Next.js của một trang (các chuỗi self.__next_f.push nối lại).
#[derive(Debug, Clone)]
pub struct NextData {
    pub text: String,
}

impl NextData {
    pub fn new(text: String) -> NextData {
        NextData { text }
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Vị trí (byte) của lần xuất hiện đầu từ `from`; không có → None.
    pub fn index_of(&self, needle: &str, from: usize) -> Option<usize> {
        if from >= self.text.len() {
            return None;
        }
        let mut f = from;
        while !self.text.is_char_boundary(f) {
            f += 1;
        }
        self.text[f..].find(needle).map(|i| i + f)
    }

    /// Giá trị JSON tại vị trí; không đọc được → None.
    pub fn value_at(&self, i: usize) -> Option<Value> {
        json::scan(&self.text, i)
    }
}

const PUSH: &str = "self.__next_f.push([1,";
const EPISODE_KEY: &str = "\"episode\":{\"episodes\"";

/// Khối phim trang chủ: ["$","$L26","Trung Quốc Hot Nhất",{"movies":[…]}]
static SECTION_RX: Rx = Rx::new(r#"\["\$","\$L\w+","([^"]+)",\{"movies":\["#);
static MOVIE_RX: Rx = Rx::new(r#"\{"id":\d+,"name":""#);
static PAGING_RX: Rx = Rx::new(r#""total":(\d+),"page":(\d+),"limit":(\d+)"#);

/// Menu của trang chỉ có 10 thể loại → thêm mục cố định (đường dẫn bộ lọc của chính trang, mới cập nhật lên đầu).
pub const MENU: &[(&str, &str)] = &[
    ("Hoạt hình Trung Quốc", "/loc-phim?genre=hoat-hinh&country=trung-quoc&sort=updated_at&order=desc"),
    ("Hoạt hình", "/loc-phim?genre=hoat-hinh&sort=updated_at&order=desc"),
    ("Phim bộ mới", "/loc-phim?type=phim-bo&sort=updated_at&order=desc"),
    ("Phim lẻ mới", "/loc-phim?type=phim-le&sort=updated_at&order=desc"),
    ("Phim Hàn Quốc", "/loc-phim?country=han-quoc&sort=updated_at&order=desc"),
    ("Phim Trung Quốc", "/loc-phim?country=trung-quoc&sort=updated_at&order=desc"),
    ("Phim Âu Mỹ", "/loc-phim?country=au-my&sort=updated_at&order=desc"),
    ("Phim Nhật Bản", "/loc-phim?country=nhat-ban&sort=updated_at&order=desc"),
    ("Phim Thái Lan", "/loc-phim?country=thai-lan&sort=updated_at&order=desc"),
];

/// Trang đã gặp là Next.js (để tìm kiếm đi thẳng /tim-kiem thay cho ?s= của WordPress).
static HOSTS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub fn is_known(base_url: &str) -> bool {
    match site::host_of(base_url) {
        Some(h) => lock(&HOSTS).contains(&h),
        None => false,
    }
}

/// Dữ liệu Next.js của trang; None nếu không phải trang Next.js.
pub fn data(doc: &Doc) -> Option<NextData> {
    let mut sb = String::new();
    for s in doc.q("script") {
        let js = s.script_text();
        let Some(i) = js.find(PUSH) else { continue };
        if let Some(Value::String(part)) = json::scan(&js, i + PUSH.len()) {
            sb.push_str(&part);
        }
    }
    if sb.is_empty() {
        return None;
    }
    if let Some(h) = site::host_of(&doc.location) {
        lock(&HOSTS).insert(h);
    }
    Some(NextData::new(sb))
}

// ---- Trang chủ / danh sách ----

/// Trang chủ, thêm khối hoạt hình Trung Quốc mới cập nhật lên đầu. Bản Windows: Home(doc, data) — ở đây nhận trang chủ đã
/// đọc (`parse_home`) và gốc trang (`origin_of(doc)`), vì `Doc` không giữ qua lần chờ mạng.
pub async fn home(parsed: Home, origin: &str) -> Home {
    let mut h = parsed;
    if let Ok(p) = list_of(&format!("{origin}{}", MENU[0].1)).await {
        if !p.films.is_empty() {
            h.sections.insert(0, FilmSection { title: "Hoạt hình Trung Quốc mới cập nhật".into(), films: p.films });
        }
    }
    h
}

pub fn parse_home(doc: &Doc, data: &NextData) -> Home {
    let origin = origin_of(doc);
    let mut sections = Vec::new();
    let matches: Vec<(usize, usize, String)> = SECTION_RX
        .re()
        .captures_iter(&data.text)
        .filter_map(|c| {
            let m = c.get(0)?;
            Some((m.start(), m.end(), c.get(1).map(|g| g.as_str().to_string()).unwrap_or_default()))
        })
        .collect();
    let first = matches.first().map(|m| m.0).unwrap_or(data.len());
    let top = movies(data, &origin, 0, Some(first));
    if !top.is_empty() {
        sections.push(FilmSection { title: "Nổi bật".into(), films: top });
    }
    for (_, end, title) in matches {
        // Vị trí dấu "[" cuối mẫu khớp
        let Some(Value::Array(arr)) = data.value_at(end - 1) else { continue };
        let objs: Vec<&Obj> = arr.iter().filter_map(|v| v.as_object()).collect();
        let films = distinct_by(objs.into_iter().filter_map(|o| film_of(o, &origin)), |f| f.url.clone());
        if !films.is_empty() {
            sections.push(FilmSection { title, films });
        }
    }
    Home { sections, categories: categories(doc, &origin), moved_to: None }
}

fn categories(doc: &Doc, origin: &str) -> Vec<Category> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (title, path) in MENU {
        if seen.insert(format!("{origin}{path}")) {
            out.push(Category::new(*title, format!("{origin}{path}")));
        }
    }
    for a in doc.q("a[href^=\"/the-loai/\"]") {
        let url = a.abs("href").trim_end_matches('/').to_string();
        let title = a.txt();
        if !title.is_empty() && seen.insert(url.clone()) {
            out.push(Category::new(title, url));
        }
    }
    out
}

/// Bộ lọc của trang (/loc-phim): đủ thể loại và quốc gia. Bỏ mục rác (năm, mã hai chữ) và tên trùng.
pub async fn filters(base_url: &str) -> Result<(Vec<Category>, Vec<Category>)> {
    let r = http::get(&format!("{base_url}/loc-phim"), &[]).await?;
    let doc = Doc::parse(&r.body(), &r.url);
    let d = data(&doc).ok_or_else(|| Error::app("Trang không còn dạng Next.js"))?;
    Ok(parse_filters(&doc, &d))
}

pub fn parse_filters(doc: &Doc, d: &NextData) -> (Vec<Category>, Vec<Category>) {
    let origin = origin_of(doc);
    let read = |key: &str, param: &str| -> Vec<Category> {
        // Phim trong trang cũng có "genres":[…] của riêng nó → lấy danh sách dài nhất (bộ lọc của trang)
        let tag = format!("\"{key}\":[{{");
        let mut best: Vec<Value> = Vec::new();
        let mut i = d.index_of(&tag, 0);
        while let Some(at) = i {
            if let Some(Value::Array(a)) = d.value_at(at + key.len() + 3) {
                if a.len() > best.len() {
                    best = a;
                }
            }
            i = d.index_of(&tag, at + tag.len());
        }
        let items = best
            .iter()
            .filter_map(|v| v.as_object())
            .filter(|o| json::bool(Some(*o), "is_active", true))
            .map(|o| (jstr(o, "name"), jstr(o, "slug")))
            .filter(|(name, slug)| {
                let short_caps = name.chars().count() <= 2 && name.chars().all(char::is_uppercase);
                !slug.is_empty() && name.chars().any(char::is_alphabetic) && !short_caps
            });
        distinct_by(items, |(name, _)| name.to_lowercase())
            .into_iter()
            .map(|(name, slug)| Category::new(name, format!("{origin}/loc-phim?{param}={}&sort=updated_at&order=desc", form_encoded(&slug))))
            .collect()
    };
    (read("genres", "genre"), read("countries", "country"))
}

async fn list_of(url: &str) -> Result<FilmPage> {
    let r = http::get(url, &[]).await?;
    let doc = Doc::parse(&r.body(), &r.url);
    let d = data(&doc).ok_or_else(|| Error::app("Trang không còn dạng Next.js"))?;
    Ok(parse_list(&doc, &d, url))
}

/// Danh sách / tìm kiếm: 21 phim một trang, trang sau = tham số page tiếp theo (dữ liệu có total/page/limit).
pub fn parse_list(doc: &Doc, data: &NextData, url: &str) -> FilmPage {
    let films = movies(data, &origin_of(doc), 0, None);
    let mut next = None;
    if let Some(g) = PAGING_RX.groups(&data.text) {
        if let (Ok(total), Ok(page), Ok(limit)) = (g[1].parse::<i64>(), g[2].parse::<i64>(), g[3].parse::<i64>()) {
            if !films.is_empty() && page * limit < total {
                next = Some(with_page(url, page + 1));
            }
        }
    }
    FilmPage { films, next_url: next, sections: Vec::new() }
}

pub fn search_url(base_url: &str, query: &str) -> String {
    format!("{base_url}/tim-kiem?keyword={}", form_encoded(query.trim()))
}

static PAGE_PARAM: Rx = Rx::new(r"([?&]page=)\d+");

fn with_page(url: &str, page: i64) -> String {
    if PAGE_PARAM.matches(url) {
        return PAGE_PARAM.replace(url, &format!("${{1}}{page}"));
    }
    format!("{url}{}page={page}", if url.contains('?') { "&" } else { "?" })
}

// ---- Chi tiết phim ----

/// Chi tiết + danh sách tập của các ngôn ngữ khác (tải song song). Bản Windows: Detail(doc, data, url) — ở đây nhận chi tiết
/// đã đọc (`parse_detail`), dữ liệu và gốc trang (`origin_of(doc)`), vì `Doc` không giữ qua lần chờ mạng.
pub async fn detail(parsed: FilmDetail, data: &NextData, origin: &str, url: &str) -> FilmDetail {
    let mut d = parsed;
    let Some(ep) = episode_data(data) else { return d };
    let slug = movie_of(data, url).map(|m| jstr(&m, "slug")).unwrap_or_default();
    if slug.is_empty() {
        return d;
    }
    // Trang chỉ trả danh sách tập của một ngôn ngữ khi tập được hỏi có ở ngôn ngữ đó → hỏi tập đầu, rồi tập cuối
    let main_eps: &[Episode] = d.groups.first().map(|g| g.episodes.as_slice()).unwrap_or(&[]);
    let found0 = distinct_by(
        [main_eps.first(), main_eps.last()].into_iter().flatten().map(|e| ep_slug(&e.url)).filter(|s| !s.is_empty()),
        |s| s.clone(),
    );
    let probes = if found0.is_empty() { vec!["tap-1".to_string()] } else { found0 };
    let extra: Vec<(String, String)> = languages(&ep).into_iter().filter(|(_, name)| d.groups.iter().all(|g| g.name != *name)).collect();
    let tasks = extra.iter().map(|(lang_slug, lang_name)| {
        let server = best_server(&ep, lang_slug);
        let (probes, slug) = (&probes, &slug);
        async move {
            let server = server?;
            for probe in probes {
                let Ok(r) = http::get(&format!("{origin}/xem-phim/{slug}/{probe}/{lang_slug}?server={server}"), &[]).await else {
                    continue;
                };
                let eps = language_episodes(&r.body(), &r.url, origin, slug, lang_slug);
                if !eps.is_empty() {
                    return Some(EpisodeGroup { name: lang_name.clone(), episodes: eps });
                }
            }
            None
        }
    });
    for g in futures::future::join_all(tasks).await.into_iter().flatten() {
        d.groups.push(g);
    }
    d
}

/// Danh sách tập của một ngôn ngữ trong trang tập vừa tải (rỗng nếu trang mở ngôn ngữ khác).
fn language_episodes(body: &str, location: &str, origin: &str, slug: &str, lang: &str) -> Vec<Episode> {
    let doc = Doc::parse(body, location);
    let e = data(&doc).and_then(|dd| episode_data(&dd));
    match e {
        Some(e) if jstr(&e, "current_language") == lang => episodes(&e, origin, slug, lang),
        _ => Vec::new(),
    }
}

pub fn parse_detail(doc: &Doc, data: &NextData, url: &str) -> Result<FilmDetail> {
    let origin = origin_of(doc);
    let m = movie_of(data, url).ok_or_else(|| Error::app("Không đọc được thông tin phim"))?;
    let slug = jstr(&m, "slug");
    let raw_desc = jstr(&m, "description");
    let description = if raw_desc.chars().count() > 20 && !raw_desc.starts_with('$') {
        Some(raw_desc)
    } else {
        doc.q1("meta[property=\"og:description\"], meta[name=\"description\"]").and_then(|e| ne(&e.at("content")))
    };
    let year = match m.get("year") {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|d| d as i64)).map(|y| y.to_string()).unwrap_or_default(),
        _ => String::new(),
    };
    let info = [
        ne(&jstr(&m, "status_text")).unwrap_or_else(|| jstr(&m, "display_status")),
        json::str(json::obj(Some(&m), "country"), "name"),
        year,
        names(json::arr(Some(&m), "genres")),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join("  •  ");

    let mut groups = Vec::new();
    if let Some(ep) = episode_data(data) {
        let lang = jstr(&ep, "current_language");
        let eps = episodes(&ep, &origin, &slug, &lang);
        if !eps.is_empty() {
            let name = languages(&ep).into_iter().find(|(s, _)| *s == lang).map(|(_, n)| n).unwrap_or_else(|| "Tập phim".into());
            groups.push(EpisodeGroup { name, episodes: eps });
        }
    }
    let poster_ref = ne(&jstr(&m, "poster_url")).or_else(|| ne(&jstr(&m, "thumb_url")));
    Ok(FilmDetail {
        url: url.to_string(),
        title: ne(&jstr(&m, "name")).unwrap_or_else(|| url.to_string()),
        other_title: ne(&jstr(&m, "origin_name")),
        poster: poster_ref.and_then(|p| abs(&origin, &p)),
        description,
        info: ne(&info),
        groups,
    })
}

/// Đối tượng phim của trang (trang có thể nhắc tới phim khác → ưu tiên phim trùng slug trong địa chỉ).
fn movie_of(data: &NextData, url: &str) -> Option<Obj> {
    let slug = after_last(url.trim_end_matches('/'), '/');
    let mut first: Option<Obj> = None;
    const TAG: &str = "\"movie\":{";
    let mut i = data.index_of(TAG, 0);
    while let Some(at) = i {
        if let Some(Value::Object(o)) = data.value_at(at + "\"movie\":".len()) {
            if jstr(&o, "slug") == slug {
                return Some(o);
            }
            if first.is_none() {
                first = Some(o);
            }
        }
        i = data.index_of(TAG, at + 1);
    }
    first
}

fn episode_data(data: &NextData) -> Option<Obj> {
    let i = data.index_of(EPISODE_KEY, 0)?;
    match data.value_at(i + "\"episode\":".len()) {
        Some(Value::Object(o)) => Some(o),
        _ => None,
    }
}

/// Ngôn ngữ có tập (theo thứ tự trang đưa): (slug, tên hiển thị), vd ("thuyetminh", "Thuyết Minh").
fn languages(ep: &Obj) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for s in json::objects(json::arr(Some(ep), "available_sources")) {
        let slug = jstr(s, "language_slug");
        if !slug.is_empty() && out.iter().all(|l| l.0 != slug) {
            let name = ne(&jstr(s, "language_name")).unwrap_or_else(|| slug.clone());
            out.push((slug, name));
        }
    }
    out
}

/// Server có nhiều tập nhất của một ngôn ngữ (lấy danh sách tập đầy đủ nhất).
fn best_server(ep: &Obj, lang: &str) -> Option<String> {
    // MaxBy của C#: bằng nhau thì giữ server đầu
    let mut best: Option<&Obj> = None;
    for s in json::objects(json::arr(Some(ep), "available_sources")) {
        if jstr(s, "language_slug") != lang {
            continue;
        }
        if best.map(|b| json::int(Some(s), "total_episodes", 0) > json::int(Some(b), "total_episodes", 0)).unwrap_or(true) {
            best = Some(s);
        }
    }
    best.and_then(|b| ne(&jstr(b, "server_slug")))
}

fn episodes(ep: &Obj, origin: &str, slug: &str, lang: &str) -> Vec<Episode> {
    let all = json::objects(json::arr(Some(ep), "episodes")).into_iter().filter_map(|e| {
        let s = ne(&jstr(e, "slug"))?;
        let l = ne(&jstr(e, "language")).unwrap_or_else(|| lang.to_string());
        let title = ne(&jstr(e, "episode_label")).or_else(|| ne(&jstr(e, "name"))).unwrap_or_else(|| s.clone());
        Some(Episode::new(title, format!("{origin}/xem-phim/{slug}/{s}/{l}")))
    });
    distinct_by(all, |e| e.url.clone())
}

// ---- Trang tập phim ----

/// Server của tập theo ngôn ngữ đang xem; link video của server trang mở sẵn đọc luôn (khỏi tải lại).
pub fn parse_source(doc: &Doc, data: &NextData, ep_url: &str) -> Result<EpisodeSource> {
    let ep = episode_data(data).ok_or_else(|| Error::app("Không đọc được trang tập phim"))?;
    let lang = jstr(&ep, "current_language");
    let mut servers: Vec<Server> = json::objects(json::arr(Some(&ep), "episode_sources"))
        .into_iter()
        .filter_map(|s| {
            let sv = json::obj(Some(s), "server")?;
            let l = json::str(json::obj(Some(s), "language"), "slug");
            if !lang.is_empty() && !l.is_empty() && l != lang {
                return None;
            }
            Some(Server::new(jstr(sv, "name"), jstr(sv, "slug")))
        })
        .collect();
    if servers.is_empty() {
        servers = json::objects(json::arr(Some(&ep), "available_sources"))
            .into_iter()
            .filter(|s| lang.is_empty() || jstr(s, "language_slug") == lang)
            .map(|s| Server::new(jstr(s, "server_name"), jstr(s, "server_slug")))
            .collect();
    }
    let servers: Vec<Server> = distinct_by(servers.into_iter().filter(|s| !s.kind.is_empty()), |s| s.kind.clone())
        .into_iter()
        .map(|s| if s.label.is_empty() { Server::new(s.kind.clone(), s.kind) } else { s })
        .collect();
    let cur = current_server(&ep);
    let mut links = HashMap::new();
    if !cur.is_empty() {
        if let Some(v) = video_of_ep(&ep, &origin_of(doc)) {
            links.insert(cur, v);
        }
    }
    Ok(EpisodeSource { ep_url: ep_url.to_string(), servers, ep: ep_slug(ep_url), links, next: true, ..Default::default() })
}

/// ".../xem-phim/tien-nghich/tap-165/vietsub" → "tap-165".
pub fn ep_slug(ep_url: &str) -> String {
    let parts: Vec<&str> = ep_url.trim_end_matches('/').split('/').collect();
    if parts.len() >= 2 {
        parts[parts.len() - 2].to_string()
    } else {
        String::new()
    }
}

/// Link video của một server: đã có sẵn thì dùng, không thì mở trang tập với ?server=… để lấy.
pub async fn video_of(src: &EpisodeSource, server: &Server) -> Result<Video> {
    if let Some(v) = src.links.get(&server.kind) {
        return Ok(v.clone());
    }
    let r = http::get(&format!("{}?server={}", src.ep_url, form_encoded(&server.kind)), &[]).await?;
    let doc = Doc::parse(&r.body(), &r.url);
    let ep = data(&doc).and_then(|d| episode_data(&d)).ok_or_else(|| Error::app("Không đọc được trang tập phim"))?;
    // Tập chưa có trên server này thì trang tự mở server khác → coi như chưa có
    match video_of_ep(&ep, &origin_of(&doc)) {
        Some(video) if current_server(&ep) == server.kind => Ok(video),
        _ => Err(Error::App(format!("Server \"{}\" chưa có video cho tập này", server.label))),
    }
}

fn current_server(ep: &Obj) -> String {
    match ep.get("current_server") {
        Some(Value::Object(o)) => jstr(o, "slug"),
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Server riêng của trang ("HD") cho link dạng /public/m3u8/… (cùng tên miền) và phụ đề rời.
fn video_of_ep(ep: &Obj, origin: &str) -> Option<Video> {
    let v = json::obj(Some(ep), "video")?;
    let link = ne(&jstr(v, "link_m3u8")).or_else(|| ne(&jstr(v, "link_embed")))?;
    let url = abs(origin, &link)?;
    Some(Video { url, subtitle: subtitle(json::arr(Some(v), "subtitles"), origin), file: false })
}

/// Phụ đề rời: bản mặc định, không có thì tiếng Việt.
fn subtitle(a: Option<&Vec<Value>>, origin: &str) -> Option<String> {
    let subs: Vec<&Obj> = json::objects(a).into_iter().filter(|s| !jstr(s, "url").is_empty()).collect();
    let s = subs
        .iter()
        .find(|x| json::bool(Some(**x), "isDefault", false))
        .or_else(|| subs.iter().find(|x| jstr(x, "lang") == "vie"))
        .or_else(|| subs.first())?;
    abs(origin, &jstr(s, "url"))
}

// ---- Tiện ích ----

fn movies(data: &NextData, origin: &str, from: usize, to: Option<usize>) -> Vec<Film> {
    let end = to.unwrap_or(data.len());
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    if from > data.len() || !data.text.is_char_boundary(from) {
        return out;
    }
    for m in MOVIE_RX.re().find_iter(&data.text[from..]) {
        let at = from + m.start();
        if at >= end {
            break;
        }
        let Some(Value::Object(o)) = data.value_at(at) else { continue };
        let Some(f) = film_of(&o, origin) else { continue };
        if seen.insert(f.url.clone()) {
            out.push(f);
        }
    }
    out
}

/// Phim trong danh sách (đối tượng thể loại / quốc gia cũng có id + name + slug nhưng không có poster).
fn film_of(o: &Obj, origin: &str) -> Option<Film> {
    if !o.contains_key("poster_url") {
        return None;
    }
    let slug = ne(&jstr(o, "slug"))?;
    let name = ne(&jstr(o, "name"))?;
    let p = ne(&jstr(o, "poster_url")).or_else(|| ne(&jstr(o, "thumb_url")));
    Some(Film::new(format!("{origin}/phim/{slug}"), name, p.and_then(|p| abs(origin, &p)), badge(&jstr(o, "display_status"))))
}

static NUMBER_RX: Rx = Rx::new(r"\d+");

/// Nhãn trên thẻ phim (một dòng): "Thuyết Minh 107, Vietsub 165" → "Tập 165", "Vietsub Full" → "Full".
pub fn badge(status: &str) -> Option<String> {
    let max = NUMBER_RX.re().find_iter(status).filter_map(|m| m.as_str().parse::<i32>().ok()).max();
    if let Some(n) = max {
        return Some(format!("Tập {n}"));
    }
    if status.to_lowercase().contains("full") {
        return Some("Full".into());
    }
    ne(status)
}

fn names(a: Option<&Vec<Value>>) -> String {
    json::objects(a).into_iter().map(|o| jstr(o, "name")).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ")
}

/// Gốc của trang đang đọc (sai thì trang đang dùng trong Cài đặt).
pub fn origin_of(doc: &Doc) -> String {
    site::origin_of(&doc.location).unwrap_or_else(prefs::base_url)
}

/// Đường dẫn tuyệt đối; dạng lạ (vd "data:…") thì None — bỏ một ảnh / link chứ không hỏng cả trang.
fn abs(origin: &str, reference: &str) -> Option<String> {
    let b = url::Url::parse(&format!("{origin}/")).ok()?;
    let s = b.join(reference).ok()?.to_string();
    s.starts_with("http").then_some(s)
}

#[cfg(test)]
mod tests {
    //! Trang Next.js kiểu PhimMoi — cùng ca với ApiAndNextTests.cs (NextSiteTests) / NextSiteTests.swift.
    use super::*;

    const BASE: &str = "https://phim.test";

    /// Trang có dữ liệu cắt thành nhiều đoạn push (trang thật cắt ngay giữa đối tượng JSON).
    fn page(url: &str, data: &str, parts: usize, extra: &str) -> Doc {
        let chars: Vec<char> = data.chars().collect();
        let size = chars.len().div_ceil(parts);
        let mut scripts = String::new();
        for chunk in chars.chunks(size.max(1)) {
            let s: String = chunk.iter().collect();
            let quoted = serde_json::to_string(&s).expect("chuỗi JSON");
            scripts.push_str(&format!("<script>self.__next_f.push([1,{quoted}])</script>"));
        }
        Doc::parse(
            &format!("<html><head><script>self.__next_f=self.__next_f||[]</script></head><body>{scripts}{extra}</body></html>"),
            url,
        )
    }

    fn page3(url: &str, data: &str) -> Doc {
        page(url, data, 3, "")
    }

    fn movie(id: i32, slug: &str, name: &str, status: &str) -> String {
        format!(
            r#"{{"id":{id},"name":"{name}","origin_name":"","slug":"{slug}","description":"$27","poster_url":"/public/images/posters/{slug}.webp","display_status":"{status}","country":{{"name":"Trung Quốc","slug":"trung-quoc"}},"genres":[{{"id":11,"name":"Hoạt Hình","slug":"hoat-hinh","is_active":true}}]}}"#
        )
    }

    fn eps(n: i32, lang: &str) -> String {
        let items: Vec<String> = (1..=n)
            .map(|i| format!(r#"{{"id":{i},"episode_label":"Tập {i}","slug":"tap-{i}","is_embed":false,"language":"{lang}","name":"Tập {i}"}}"#))
            .collect();
        format!("[{}]", items.join(","))
    }

    const SOURCES: &str = concat!(
        r#"[{"server_name":"KK Phim","server_slug":"kk","language_name":"Vietsub","language_slug":"vietsub","total_episodes":3,"priority_order":2},"#,
        r#"{"server_name":"NguonC","server_slug":"nc","language_name":"Vietsub","language_slug":"vietsub","total_episodes":3,"priority_order":3},"#,
        r#"{"server_name":"KK Phim","server_slug":"kk","language_name":"Thuyết Minh","language_slug":"thuyetminh","total_episodes":2,"priority_order":2}]"#
    );

    fn titles(c: &[Category]) -> Vec<&str> {
        c.iter().map(|x| x.title.as_str()).collect()
    }

    #[test]
    fn filter_lists() {
        fn item(id: i32, name: &str, slug: &str, active: bool) -> String {
            format!(r#"{{"id":{id},"name":"{name}","slug":"{slug}","is_active":{active}}}"#)
        }
        let data = format!(
            r#"1:{{"results":[{}],"genres":[{}],"countries":[{}]}}"#,
            movie(1, "a", "A", "Tập 1"),
            [item(1, "Âm Nhạc", "am-nhac", true), item(2, "Cổ Trang", "co-trang", true), item(3, "Ẩn", "an", false)].join(","),
            [
                item(54, "1983", "1983", true),
                item(10, "Anh", "anh", true),
                item(60, "KW", "kw", true),
                item(61, "Hàn Quốc", "han-quoc", true),
                item(62, "hàn quốc", "han-quoc-2", true)
            ]
            .join(",")
        ) + "\n";
        let doc = page3(&format!("{BASE}/loc-phim"), &data);
        let d = data_of(&doc);
        let (genres, countries) = parse_filters(&doc, &d);
        assert_eq!(titles(&genres), ["Âm Nhạc", "Cổ Trang"]);
        assert_eq!(genres[1].url, format!("{BASE}/loc-phim?genre=co-trang&sort=updated_at&order=desc"));
        assert_eq!(titles(&countries), ["Anh", "Hàn Quốc"]);
        assert_eq!(countries[1].url, format!("{BASE}/loc-phim?country=han-quoc&sort=updated_at&order=desc"));
    }

    fn data_of(doc: &Doc) -> NextData {
        data(doc).expect("trang Next.js")
    }

    #[test]
    fn not_next_js() {
        assert!(data(&Doc::parse("<html><body><script>var a=1</script><div class=flw-item></div></body></html>", &format!("{BASE}/"))).is_none());
    }

    #[test]
    fn home_sections() {
        let data = format!(r#"0:["$","div",null,{{"children":[{}]}}]"#, movie(1, "slide-a", "Phim Nổi Bật", "Vietsub 4"))
            + "\n"
            + &format!(
                r#"5:["$","$L26","Trung Quốc Hot Nhất",{{"movies":[{},{}]}}]"#,
                movie(2, "tien-nghich", "Tiên Nghịch", "Thuyết Minh 107, Vietsub 165"),
                movie(3, "phim-le", "Phim Lẻ", "Vietsub Full")
            );
        let doc = page(&format!("{BASE}/"), &data, 3, r#"<a href="/the-loai/hanh-dong">Hành Động</a>"#);
        let h = site::parse_home(&doc);
        assert_eq!(h.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["Nổi bật", "Trung Quốc Hot Nhất"]);
        let f = &h.sections[1].films;
        assert_eq!(f.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), ["Tiên Nghịch", "Phim Lẻ"]);
        assert_eq!(f[0].url, format!("{BASE}/phim/tien-nghich"));
        assert_eq!(f[0].poster.as_deref(), Some(format!("{BASE}/public/images/posters/tien-nghich.webp").as_str()));
        assert_eq!(f.iter().map(|x| x.badge.as_deref()).collect::<Vec<_>>(), [Some("Tập 165"), Some("Full")]);
        assert_eq!(h.categories.first().map(|c| c.title.as_str()), Some("Hoạt hình Trung Quốc"));
        assert_eq!(h.categories.last().map(|c| c.url.clone()), Some(format!("{BASE}/the-loai/hanh-dong")));
    }

    #[test]
    fn list_paging() {
        let movies = (1..=21).map(|i| movie(i, &format!("phim-{i}"), &format!("Phim {i}"), &format!("Vietsub {i}"))).collect::<Vec<_>>().join(",");
        let url = format!("{BASE}/loc-phim?genre=hoat-hinh&sort=updated_at&order=desc");
        let p = site::parse_list(&page3(&url, &format!(r#"{{"movies":[{movies}],"total":50,"page":1,"limit":21,"filters":{{}}}}"#)), &url);
        assert_eq!(p.films.len(), 21);
        assert_eq!(p.next_url, Some(format!("{url}&page=2")));
        let url3 = format!("{url}&page=3");
        let last = site::parse_list(&page3(&url3, &format!(r#"{{"movies":[{}],"total":43,"page":3,"limit":21}}"#, movie(1, "a", "A", ""))), &url3);
        assert!(last.next_url.is_none());
    }

    #[test]
    fn detail_of_next() {
        let data = String::from(r#"{"movie":{"id":283,"name":"Tiên Nghịch","origin_name":"Renegade Immortal","slug":"tien-nghich","#)
            + r#""description":"Cuộc hành trình của Vương Lâm, một thiếu niên bình thường.","poster_url":"/public/images/posters/p.webp","#
            + r#""status_text":"Thuyết Minh 2, Vietsub 3","year":2023,"country":{"name":"Trung Quốc"},"genres":[{"name":"Hoạt Hình"}]},"#
            + r#""episode":{"episodes":"#
            + &eps(3, "vietsub")
            + r#","available_languages":[],"available_sources":"#
            + SOURCES
            + r#","current_language":"vietsub","current_server":"kk"}}"#;
        let url = format!("{BASE}/phim/tien-nghich");
        let d = site::parse_detail(&page3(&url, &data), &url);
        assert_eq!(d.title, "Tiên Nghịch");
        assert_eq!(d.other_title.as_deref(), Some("Renegade Immortal"));
        assert!(d.info.as_deref().unwrap_or("").starts_with("Thuyết Minh 2, Vietsub 3  •  Trung Quốc  •  2023"));
        assert_eq!(d.groups.len(), 1);
        assert_eq!(d.groups[0].name, "Vietsub");
        assert_eq!(d.groups[0].episodes.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), ["Tập 1", "Tập 2", "Tập 3"]);
        assert_eq!(d.groups[0].episodes.last().map(|e| e.url.clone()), Some(format!("{BASE}/xem-phim/tien-nghich/tap-3/vietsub")));
        assert_eq!(d.poster, Some(format!("{BASE}/public/images/posters/p.webp")));
    }

    #[test]
    fn source_of_next() {
        let ep_url = format!("{BASE}/xem-phim/tien-nghich/tap-3/vietsub");
        let data = String::from(r#"{"episode":{"episodes":"#)
            + &eps(3, "vietsub")
            + r#","current_language":"vietsub","available_sources":"#
            + SOURCES
            + ","
            + r#""episode_sources":[{"server":{"id":3,"name":"KK Phim","slug":"kk"},"language":{"name":"Vietsub","slug":"vietsub"},"is_lock":false},"#
            + r#"{"server":{"id":2,"name":"NguonC","slug":"nc"},"language":{"name":"Vietsub","slug":"vietsub"},"is_lock":false},"#
            + r#"{"server":{"id":3,"name":"KK Phim","slug":"kk"},"language":{"name":"Thuyết Minh","slug":"thuyetminh"},"is_lock":false}],"#
            + r#""current_server":{"id":3,"name":"KK Phim","slug":"kk"},"current_source":{"server":"$1f:props:data:episode:current_server"},"#
            + r#""video":{"id":1,"link_embed":"https://player.test/?url=x","link_m3u8":"https://cdn.test/a/index.m3u8","is_embed":false}}}"#;
        let s = site::parse_source(&page3(&ep_url, &data), &ep_url);
        assert!(s.next);
        assert!(!s.page_mode);
        assert_eq!(s.servers.iter().map(|x| format!("{}={}", x.kind, x.label)).collect::<Vec<_>>(), ["kk=KK Phim", "nc=NguonC"]);
        assert_eq!(s.links.keys().map(|k| k.as_str()).collect::<Vec<_>>(), ["kk"]);
        assert_eq!(s.links["kk"].url, "https://cdn.test/a/index.m3u8");
        assert!(s.links["kk"].subtitle.is_none());
        assert_eq!(s.ep, "tap-3");
        let embed = site::parse_source(&page3(&ep_url, &data.replace("https://cdn.test/a/index.m3u8", "")), &ep_url);
        assert_eq!(embed.links["kk"].url, "https://player.test/?url=x");
    }

    #[test]
    fn own_server_with_subtitles() {
        let ep_url = format!("{BASE}/xem-phim/bac-si-x/tap-1/song-ngu");
        let data = String::from(r#"{"episode":{"episodes":"#)
            + &eps(1, "song-ngu")
            + r#","current_language":"song-ngu","#
            + r#""episode_sources":[{"server":{"id":9,"name":"HD","slug":"hd"},"language":{"name":"Song Ngữ","slug":"song-ngu"},"is_lock":false}],"#
            + r#""current_server":{"id":9,"name":"HD","slug":"hd"},"video":{"id":1,"link_embed":"/embed/v2/abc","link_m3u8":"/public/m3u8/job_1.m3u8","#
            + r#""is_embed":false,"subtitles":[{"url":"https://sub.test/zh","lang":"chi","label":"Chinese","isDefault":false},"#
            + r#"{"url":"https://sub.test/vi","lang":"vie","label":"Vietnamese","isDefault":true}]}}}"#;
        let s = site::parse_source(&page3(&ep_url, &data), &ep_url);
        assert_eq!(s.links["hd"].url, format!("{BASE}/public/m3u8/job_1.m3u8"));
        assert_eq!(s.links["hd"].subtitle.as_deref(), Some("https://sub.test/vi"));
    }

    #[test]
    fn odd_poster_does_not_break_list() {
        let odd = movie(1, "phim-a", "Phim A", "Vietsub 3").replace("/public/images/posters/phim-a.webp", "data:image/png;base64,AAAA");
        let url = format!("{BASE}/loc-phim?type=phim-bo");
        let data = format!(r#"{{"movies":[{odd},{}],"total":2,"page":1,"limit":21}}"#, movie(2, "phim-b", "Phim B", "Full"));
        let p = site::parse_list(&page3(&url, &data), &url);
        assert_eq!(p.films.iter().map(|f| f.title.as_str()).collect::<Vec<_>>(), ["Phim A", "Phim B"]);
        assert!(p.films[0].poster.is_none());
        assert_eq!(p.films[1].poster, Some(format!("{BASE}/public/images/posters/phim-b.webp")));
    }

    #[test]
    fn badge_of() {
        assert_eq!(badge("Song Ngữ 4, Thuyết Minh 8, Vietsub 8").as_deref(), Some("Tập 8"));
        assert_eq!(badge("Thuyết Minh Full, Vietsub Full").as_deref(), Some("Full"));
        assert_eq!(badge(""), None);
    }

    #[test]
    fn futures_are_send() {
        fn send<T: Send>(_: T) {}
        let d = NextData::new(String::new());
        let src = EpisodeSource::default();
        let sv = Server::new("a", "b");
        send(home(Home::default(), ""));
        send(filters(""));
        send(detail(FilmDetail::default(), &d, "", ""));
        send(video_of(&src, &sv));
    }
}
