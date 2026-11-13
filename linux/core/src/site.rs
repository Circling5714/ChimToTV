//! Đọc dữ liệu từ trang phim (bản Android: Site.kt, iPad: Site.swift, Windows: Site.cs — giữ cùng tên hàm, cùng bộ chọn CSS
//! để sửa song song). Không viết riêng cho từng trang: nhận dạng theo mẫu giao diện ("flw-item" kiểu yanhh3d, "halim-item"
//! của theme HaLim). Trang dựng bằng Next.js (kiểu PhimMoi) thì chuyển cho next_site; trang ứng dụng JS có API JSON (mephim)
//! cho api_site.
//!
//! Riêng bản Rust: `Doc` (scraper) không chuyển được sang luồng khác, mà việc nền chạy trên tokio nhiều luồng → hàm async
//! tải trang xong thì đọc HTML ngay trong hàm thường (parse_… / read_…), không giữ `Doc` / `El` qua `.await`.

use crate::html::{Doc, El};
use crate::json::{self, Obj};
use crate::models::*;
use crate::util::{after_last, form_encoded, non_empty, now_ms, with_timeout, Error, Result, Rx};
use crate::{api_site, http, kk_phim, next_site, prefs, stremio};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{LazyLock, Mutex, MutexGuard};

/// Thẻ phim ở lưới / trượt / bảng xếp hạng.
pub const CARD: &str = ".flw-item, li.item-top, .halim-item, a.halim-trending-link, a.t10-card";

/// Tiêu đề khối trên trang chủ.
pub const HEADING: &str = ".cat-heading, .section-title";

// ---- Tiện ích chung của các bộ đọc trang (bản Rust) ----

/// None khi rỗng (sau khi bỏ khoảng trắng) — `NonEmpty()` của bản Windows.
pub(crate) fn ne(s: &str) -> Option<String> {
    non_empty(Some(s))
}

/// Chuỗi của khoá (J.Str).
pub(crate) fn jstr(o: &Obj, key: &str) -> String {
    json::str(Some(o), key)
}

/// Bỏ phần tử trùng khoá, giữ phần tử đầu (DistinctBy).
pub(crate) fn distinct_by<T, K: Eq + Hash>(items: impl IntoIterator<Item = T>, key: impl Fn(&T) -> K) -> Vec<T> {
    let mut seen = HashSet::new();
    items.into_iter().filter(|x| seen.insert(key(x))).collect()
}

/// Khoá bộ nhớ dùng chung (luồng khác hỏng giữa chừng thì vẫn dùng tiếp dữ liệu).
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// JSON của API → đối tượng (bỏ BOM đầu tệp như JsonDocument của bản Windows); hỏng / không phải đối tượng → None.
pub(crate) fn json_object(data: &[u8]) -> Option<Obj> {
    json::parse_object(data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data))
}

/// Nhận diện phần tử (thay cho so sánh tham chiếu của bản Windows): địa chỉ dữ liệu phần tử trong cây — không đổi khi tài
/// liệu còn sống.
fn nid(e: &El) -> usize {
    e.e.value() as *const _ as usize
}

// ---- Bộ nhớ chi tiết phim (30 phim gần nhất) ----

struct DetailCache {
    map: HashMap<String, FilmDetail>,
    order: Vec<String>,
}

static DETAIL_CACHE: LazyLock<Mutex<DetailCache>> =
    LazyLock::new(|| Mutex::new(DetailCache { map: HashMap::new(), order: Vec::new() }));

pub fn cached_detail(url: &str) -> Option<FilmDetail> {
    lock(&DETAIL_CACHE).map.get(url).cloned()
}

fn cache(d: &FilmDetail, url: &str) {
    let mut c = lock(&DETAIL_CACHE);
    c.map.insert(url.to_string(), d.clone());
    c.order.retain(|u| u != url);
    c.order.push(url.to_string());
    if c.order.len() > 30 {
        let old = c.order.remove(0);
        c.map.remove(&old);
    }
}

/// Tải trang rồi đọc HTML. `Doc` không gửi được sang luồng khác: dùng xong trước lần `.await` kế tiếp.
pub async fn fetch(url: &str, headers: &[(&str, &str)]) -> Result<Doc> {
    let r = http::get(url, headers).await?;
    Ok(Doc::parse(&r.body(), &r.url))
}

// ---- Trang chủ ----

pub async fn home_of(base_url: Option<&str>) -> Result<Home> {
    let b = base_url.map(str::to_string).unwrap_or_else(prefs::base_url);
    if api_site::is_known(&b) {
        return api_site::home(&b).await;
    }
    let r = http::get(&format!("{b}/"), &[]).await?;
    let (moved, home, next_origin, shell) = read_home(&r.body(), &r.url, &b);
    let mut home = match next_origin {
        Some(o) => next_site::home(home, &o).await,
        None => home,
    };
    // Trang trắng của ứng dụng JS (React…): thử API kiểu mephim
    if home.sections.is_empty() && shell && api_site::probe(&b).await {
        return api_site::home(&b).await;
    }
    home.moved_to = moved;
    Ok(home)
}

/// Phần đọc HTML của home_of: (đã chuyển sang tên miền, trang chủ, gốc trang nếu là Next.js, trang trắng của ứng dụng JS).
fn read_home(body: &str, location: &str, b: &str) -> (Option<String>, Home, Option<String>, bool) {
    let doc = Doc::parse(body, location);
    let o = origin_of(&doc.location);
    let moved = o.filter(|o| host_of(o) != host_of(b));
    let (home, next) = match next_site::data(&doc) {
        Some(data) => (next_site::parse_home(&doc, &data), Some(next_site::origin_of(&doc))),
        None => (parse_home_html(&doc), None),
    };
    (moved, home, next, is_app_shell(&doc))
}

/// Trang chỉ có khung rỗng cho ứng dụng JS (vd &lt;div id="root"&gt;&lt;/div&gt;) — dữ liệu không nằm trong HTML.
pub fn is_app_shell(doc: &Doc) -> bool {
    match doc.q1("#root, #app, #__nuxt") {
        Some(root) => root.children().is_empty() && doc.q("a[href]").len() < 5,
        None => false,
    }
}

/// Thời gian tính phần tải qua mạng, không tính lúc đọc HTML.
pub async fn check(base_url: &str) -> Result<SiteCheck> {
    let t0 = now_ms();
    if api_site::is_known(base_url) {
        let n = api_site::list(&api_site::newest_url(base_url)).await?.films.len();
        return Ok(SiteCheck { ms: now_ms() - t0, films: n });
    }
    let r = http::get(&format!("{base_url}/"), &[]).await?;
    let ms = now_ms() - t0;
    let doc = Doc::parse(&r.body(), &r.url);
    Ok(SiteCheck { ms, films: parse_home(&doc).sections.iter().map(|s| s.films.len()).sum() })
}

/// Phần tử có tổ tiên (không tính chính nó, trừ khi `include_self`) nằm trong tập.
fn has_ancestor(el: &El, set: &HashSet<usize>, include_self: bool) -> bool {
    let mut e = if include_self { Some(*el) } else { el.parent() };
    while let Some(x) = e {
        if set.contains(&nid(&x)) {
            return true;
        }
        e = x.parent();
    }
    false
}

fn id_set(root: &El, css: &str) -> HashSet<usize> {
    root.q(css).iter().map(nid).collect()
}

/// Gom thẻ phim theo tiêu đề khối đứng trước nó (theo thứ tự trong trang).
pub fn parse_home(doc: &Doc) -> Home {
    match next_site::data(doc) {
        Some(data) => next_site::parse_home(doc, &data),
        None => parse_home_html(doc),
    }
}

fn parse_home_html(doc: &Doc) -> Home {
    let root = doc.root();
    let cards = id_set(&root, CARD);
    let realtime = id_set(&root, ".block_area-realtime");
    let schedule = id_set(&root, ".block_area-schedule");
    // Khối theo thứ tự gặp; None = thẻ đứng trước mọi tiêu đề
    let mut order: Vec<Option<El>> = Vec::new();
    let mut groups: HashMap<Option<usize>, Vec<El>> = HashMap::new();
    let mut heading: Option<El> = None;
    for el in doc.q(&format!("{HEADING}, {CARD}")) {
        if has_ancestor(&el, &cards, false) {
            continue;
        }
        if cards.contains(&nid(&el)) {
            let k = heading.as_ref().map(nid);
            groups
                .entry(k)
                .or_insert_with(|| {
                    order.push(heading);
                    Vec::new()
                })
                .push(el);
        } else {
            heading = Some(el);
        }
    }
    let mut sections = Vec::new();
    for h in order {
        let els = groups.remove(&h.as_ref().map(nid)).unwrap_or_default();
        let films = distinct_by(els.iter().filter_map(parse_card), |f| f.url.clone());
        if films.is_empty() {
            continue;
        }
        let title = match h {
            None => "Nổi bật".to_string(),
            Some(h) => {
                let text = h.txt();
                if text.is_empty() {
                    if has_ancestor(&h, &realtime, true) { "Xem nhiều" } else { "Phim" }.to_string()
                } else if has_ancestor(&h, &schedule, true) {
                    format!("{text} hôm nay")
                } else {
                    text
                }
            }
        };
        sections.push(FilmSection { title, films });
    }
    Home { sections, categories: parse_categories(doc), moved_to: None }
}

static SKIP_CATEGORY: Rx = Rx::new("account|login|dang-nhap|lich-su|history|bookmark|theo-doi|sitemap|feed");

fn parse_categories(doc: &Doc) -> Vec<Category> {
    let home = origin_of(&doc.location).and_then(|o| host_of(&o));
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut links = doc.q("a.nav-link[href], li.menu-item > a[href], #header nav a[href], .navbar a[href], #menu a[href]");
    if links.is_empty() {
        links = doc.q(".block_area-genres a[href]");
    }
    for a in links {
        // Bỏ mục cha của menu thả xuống (href="#")
        let href = a.at("href");
        let href = href.trim();
        if href.starts_with('#') || href.to_lowercase().starts_with("javascript") {
            continue;
        }
        let url = a.abs("href").trim_end_matches('/').to_string();
        let text = a.txt();
        let title = if text.is_empty() { a.at("title").trim().to_string() } else { text };
        if url.is_empty() || title.is_empty() || seen.contains(&url) {
            continue;
        }
        if host_of(&url) != home {
            continue;
        }
        match url::Url::parse(&url) {
            Ok(u) if !u.path().trim_matches('/').is_empty() => {}
            _ => continue,
        }
        if SKIP_CATEGORY.matches(&url) {
            continue;
        }
        seen.insert(url.clone());
        out.push(Category::new(title, url));
    }
    out
}

/// Thẻ phim → Film. Hỗ trợ flw-item (yanhh3d), halim-item (HaLim), item-top (bảng xếp hạng).
pub fn parse_card(el: &El) -> Option<Film> {
    let a = el
        .q1("a.film-poster-ahref[href], a.halim-thumb[href], .film-name a[href]")
        .or_else(|| el.q1("a[href]"))
        .or_else(|| (el.tag() == "a" && el.has("href")).then_some(*el))?;
    let url = a.abs("href");
    if url.is_empty() {
        return None;
    }
    let img = el.q1("img");
    let candidates = [
        el.q1(".film-name a, .entry-title, .halim-trending-title-text, h4, .t10-movie-title").map(|e| e.txt()),
        Some(a.at("title")),
        img.map(|i| i.at("alt")),
    ];
    let title = candidates.iter().flatten().find_map(|c| ne(c))?;
    let mut badge = el.q1(".tick-rate, .fdi-item, .episode").and_then(|e| ne(&e.txt()));
    if badge.is_none() {
        if let Some(rank) = el.q1(".t10-rank-num").and_then(|e| ne(&e.txt())) {
            badge = Some(format!("Top {rank}"));
        }
    }
    Some(Film::new(url, title, img.and_then(|i| image_url(&i)), badge))
}

pub fn parse_cards(root: &El) -> Vec<Film> {
    let all = root.q(CARD);
    let set: HashSet<usize> = all.iter().map(nid).collect();
    distinct_by(all.iter().filter(|e| !has_ancestor(e, &set, false)).filter_map(parse_card), |f| f.url.clone())
}

fn image_url(img: &El) -> Option<String> {
    ["data-src", "data-lazy-src", "data-original", "src"].iter().map(|k| img.abs(k)).find(|u| u.starts_with("http"))
}

/// Trang danh sách kiểu mới của hoathinh3d.pw (11/2026): HTML chỉ có khung trống, phim nạp bằng API JSON —
/// /wp-json/halim/v1/{movies|ongoing|completed|top-rated}?page=N&amp;per_page=20 và lịch chiếu
/// /wp-json/movie-schedule/v1/daily?day=thu-hai… Đọc địa chỉ API ngay trong mã JS của trang (API_URL).
pub mod halim_api {
    use super::{distinct_by, jstr, json_object, ne};
    use crate::http;
    use crate::json::{self, Obj};
    use crate::models::*;
    use crate::util::{now_ms, Rx};

    pub const PATH: &str = "/wp-json/halim/v1/";
    pub const PER_PAGE: i64 = 20;
    static API: Rx = Rx::new(r#"API_URL\s*=\s*["']([^"']*wp-json\\?/halim\\?/v1\\?/[a-z0-9-]+)["']"#);
    static SORT: Rx = Rx::new(r#"SORTBY\s*=\s*["']([a-z0-9_-]+)["']"#);
    static SCHEDULE_RX: Rx = Rx::new(r#"API_URL\s*=\s*["']([^"']*wp-json\\?/movie-schedule\\?/v1\\?/daily)["']"#);
    static PAGE_RX: Rx = Rx::new(r"[?&]page=(\d+)");
    static PAGE_PARAM: Rx = Rx::new(r"([?&]page=)\d+");

    /// Trang 1 của API trong HTML (None = không phải trang kiểu này).
    pub fn first_page(html: &str) -> Option<String> {
        let a = API.group(1, html)?.replace("\\/", "/");
        let s = SORT.group(1, html).map(|so| format!("sortby={so}&")).unwrap_or_default();
        Some(format!("{a}?{s}page=1&per_page={PER_PAGE}"))
    }

    pub fn schedule_api(html: &str) -> Option<String> {
        SCHEDULE_RX.group(1, html).map(|a| a.replace("\\/", "/"))
    }

    /// {success, has_more, items:[{title, permalink, poster_url, latest_episode?, rank?}]}
    pub fn parse(data: &[u8], url: &str) -> FilmPage {
        let Some(o) = json_object(data) else { return FilmPage::default() };
        let items = json::objects(json::arr(Some(&o), "items"));
        let films = distinct_by(items.into_iter().filter_map(film_of), |f| f.url.clone());
        let page = PAGE_RX.group(1, url).and_then(|p| p.parse::<i64>().ok()).unwrap_or(1);
        let mut next = None;
        if json::bool(Some(&o), "has_more", false) && !films.is_empty() {
            if let Some(c) = PAGE_PARAM.re().captures(url) {
                if let (Some(m), Some(g1)) = (c.get(0), c.get(1)) {
                    next = Some(format!("{}{}{}{}", &url[..m.start()], g1.as_str(), page + 1, &url[m.end()..]));
                }
            }
        }
        FilmPage { films, next_url: next, sections: Vec::new() }
    }

    fn film_of(o: &Obj) -> Option<Film> {
        let url = ne(&jstr(o, "permalink"))?;
        let title = ne(&jstr(o, "title"))?;
        let poster = ne(&jstr(o, "poster_url")).or_else(|| ne(&jstr(o, "thumbnail")));
        let badge = if let Some(ep) = ne(&jstr(o, "latest_episode")) {
            Some(if ep.chars().all(|c| c.is_ascii_digit()) { format!("Tập {ep}") } else { ep })
        } else {
            let rank = json::int(Some(o), "rank", 0);
            (rank > 0).then(|| format!("Top {rank}"))
        };
        let early = if json::bool(Some(o), "is_early_screening", false) { ne(&jstr(o, "early_screening_time")) } else { None };
        let b = [early, badge].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        Some(Film::new(url, title, poster, if b.is_empty() { None } else { Some(b) }))
    }

    const DAYS: [(&str, &str); 7] = [
        ("chu-nhat", "Chủ nhật"), ("thu-hai", "Thứ 2"), ("thu-ba", "Thứ 3"), ("thu-tu", "Thứ 4"),
        ("thu-nam", "Thứ 5"), ("thu-sau", "Thứ 6"), ("thu-bay", "Thứ 7"),
    ];

    /// Một ngày của lịch chiếu: phim chiếu sớm (kèm giờ) trước, rồi phim thường.
    pub fn parse_day(data: &[u8]) -> Vec<Film> {
        let Some(o) = json_object(data) else { return Vec::new() };
        let all = ["early_movies", "regular_movies"].iter().flat_map(|k| json::objects(json::arr(Some(&o), k))).filter_map(film_of);
        distinct_by(all, |f| f.url.clone())
    }

    /// Cả tuần, bắt đầu từ hôm nay (giờ Việt Nam).
    pub async fn schedule(api: &str) -> Vec<FilmSection> {
        // Giờ Việt Nam = UTC+7 (không đổi giờ mùa hè); 1/1/1970 là thứ Năm (0 = Chủ nhật)
        let days = (now_ms().div_euclid(1000) + 7 * 3600).div_euclid(86_400);
        let today = (days + 4).rem_euclid(7) as usize;
        let mut out = Vec::new();
        for i in 0..7 {
            let (slug, name) = DAYS[(today + i) % 7];
            if let Ok(r) = http::get(&format!("{api}?day={slug}"), &[]).await {
                let films = parse_day(&r.data);
                if !films.is_empty() {
                    out.push(FilmSection { title: format!("{name}{}", if i == 0 { " (hôm nay)" } else { "" }), films });
                }
            }
        }
        out
    }
}

// ---- Danh sách / tìm kiếm ----

/// Bước tiếp theo sau khi tải một trang danh sách (bản Windows gọi lại List / nạp lịch chiếu ngay trong hàm).
enum ListStep {
    /// Trang hoathinh3d.pw nạp phim bằng API: đọc API này
    Api(String),
    /// Lịch chiếu cả tuần qua API movie-schedule
    Schedule(String),
    /// Lịch chiếu yanhh3d: (admin-ajax, địa chỉ trang, các ngày (data-id, tên) bắt đầu từ hôm nay)
    Days(String, String, Vec<(String, String)>),
    Page(FilmPage),
}

pub async fn list(url: &str) -> Result<FilmPage> {
    let mut url = url.to_string();
    // Trang danh sách của hoathinh3d.pw dẫn tới địa chỉ API — chỉ đi tiếp vài lần (tránh vòng lặp với trang lạ)
    for _ in 0..4 {
        if api_site::is_known(&url) {
            return api_site::list(&url).await;
        }
        // hoathinh3d.pw (11/2026): Phim Lẻ / Đang Chiếu / Hoàn Thành / Đánh Giá Cao nạp phim bằng API JSON
        if url.contains(halim_api::PATH) {
            let r = http::get(&url, &[("Accept", "application/json")]).await?;
            return Ok(halim_api::parse(&r.data, &url));
        }
        let r = http::get(&url, &[]).await?;
        let step = list_step(&r.body(), &r.url, &url);
        match step {
            ListStep::Api(first) => url = first,
            ListStep::Schedule(api) => return Ok(FilmPage { sections: halim_api::schedule(&api).await, ..Default::default() }),
            ListStep::Days(ajax, location, days) => {
                return Ok(FilmPage { sections: schedule_of(&ajax, &location, &days).await, ..Default::default() })
            }
            ListStep::Page(p) => return Ok(p),
        }
    }
    Err(Error::app("Không tải được danh sách phim"))
}

fn list_step(body: &str, location: &str, url: &str) -> ListStep {
    // Mã JS của trang (bản Windows: HtmlText của tài liệu — phần script giữ nguyên như mã gốc)
    if let Some(first) = halim_api::first_page(body) {
        return ListStep::Api(first);
    }
    if let Some(api) = halim_api::schedule_api(body) {
        return ListStep::Schedule(api);
    }
    let doc = Doc::parse(body, location);
    let page = parse_list(&doc, url);
    if page.films.is_empty() {
        let days = doc.q(".day-tabs .day-tab[data-id]");
        if !days.is_empty() {
            let scripts = doc.q("script").iter().map(|s| s.script_text()).collect::<Vec<_>>().join("\n");
            let ajax = AJAX_URL.group(1, &scripts).unwrap_or_else(|| resolve(&doc.location, "/wp-admin/admin-ajax.php"));
            let today = days.iter().position(|d| d.class_has("active")).unwrap_or(0);
            let ordered = days[today..].iter().chain(days[..today].iter()).map(|d| (d.at("data-id"), d.txt())).collect();
            return ListStep::Days(ajax, doc.location.clone(), ordered);
        }
    }
    ListStep::Page(page)
}

pub fn parse_list(doc: &Doc, url: &str) -> FilmPage {
    if let Some(data) = next_site::data(doc) {
        return next_site::parse_list(doc, &data, url);
    }
    let main = doc.q("#main-content, #main-contents, main").into_iter().find(|m| !parse_cards(m).is_empty());
    // Trang lịch chiếu yanhh3d: khung chính trống, chỉ có bảng xếp hạng ở cột phụ → để list() nạp theo ngày
    let films = match main {
        Some(m) => parse_cards(&m),
        None if !doc.q(".day-tabs .day-tab[data-id]").is_empty() => Vec::new(),
        None => parse_cards(&doc.root()),
    };
    let next = doc.q1(".pagination a.next[href], a.next.page-numbers[href], a[rel=next][href]").map(|a| a.abs("href"));
    FilmPage { films, next_url: next.filter(|n| !n.is_empty() && n != url), sections: Vec::new() }
}

static AJAX_URL: Rx = Rx::new(r#"ajax_url\s*=\s*['"]([^'"]+)['"]"#);

/// Trang lịch chiếu yanhh3d: mỗi ngày nạp riêng qua admin-ajax (action yan_ajax_schedule), bắt đầu từ hôm nay.
async fn schedule_of(ajax: &str, location: &str, days: &[(String, String)]) -> Vec<FilmSection> {
    let mut out = Vec::new();
    for (i, (id, name)) in days.iter().enumerate() {
        let form = [("action", "yan_ajax_schedule"), ("day", id.as_str())];
        let Ok(r) = http::post(ajax, &form, &[("Referer", location), ("X-Requested-With", "XMLHttpRequest")]).await else {
            continue;
        };
        let html = json::str(json_object(&r.data).as_ref(), "html");
        if html.is_empty() {
            continue;
        }
        let films = cards_of(&html, location);
        if !films.is_empty() {
            out.push(FilmSection { title: format!("{name}{}", if i == 0 { " (hôm nay)" } else { "" }), films });
        }
    }
    out
}

fn cards_of(html: &str, location: &str) -> Vec<Film> {
    let doc = Doc::parse(html, location);
    let root = doc.root();
    parse_cards(&root)
}

/// Trang WordPress nhận ?s=; trang Next.js tìm ở /tim-kiem (chưa biết loại trang thì thử ?s= trước).
pub async fn search(query: &str, base_url: Option<&str>) -> Result<FilmPage> {
    let b = base_url.map(str::to_string).unwrap_or_else(prefs::base_url);
    if api_site::is_known(&b) {
        return api_site::list(&api_site::search_url(&b, query)).await;
    }
    if !next_site::is_known(&b) {
        let page = list(&format!("{b}/?s={}", form_encoded(query.trim()))).await?;
        if !next_site::is_known(&b) {
            return Ok(page);
        }
    }
    list(&next_site::search_url(&b, query)).await
}

/// Tìm cùng lúc trên mọi trang trong danh sách: mỗi trang một nhóm, trang đang dùng đứng đầu; lỗi / quá 20 giây thì bỏ.
pub async fn search_all(query: &str) -> Result<FilmPage> {
    let mut sites = vec![prefs::base_url()];
    for s in prefs::sites() {
        if !sites.contains(&s) {
            sites.push(s);
        }
    }
    let results = futures::future::join_all(sites.iter().map(|b| with_timeout(20.0, search(query, Some(b.as_str()))))).await;
    let mut sections = Vec::new();
    let mut first_error: Option<Error> = None;
    let mut any_ok = false;
    for (site, r) in sites.iter().zip(results) {
        match r {
            Ok(p) => {
                any_ok = true;
                let all = p.films.into_iter().chain(p.sections.into_iter().flat_map(|s| s.films));
                let films = distinct_by(all, |f| f.url.clone());
                if !films.is_empty() {
                    let name = host_of(site).unwrap_or_else(|| site.clone());
                    sections.push(FilmSection { title: format!("{name} — {} phim", films.len()), films });
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
    }
    if let Some(e) = first_error {
        if !any_ok {
            return Err(e);
        }
    }
    Ok(FilmPage { sections, ..Default::default() })
}

// ---- Chi tiết phim ----

/// Phần đọc HTML của detail(): trang Next.js (kết quả đọc, dữ liệu, gốc trang) hoặc trang thường.
enum DetailRead {
    Next(Result<FilmDetail>, next_site::NextData, String),
    Html(FilmDetail),
}

fn read_detail(body: &str, location: &str, url: &str) -> (bool, DetailRead) {
    let doc = Doc::parse(body, location);
    let read = match next_site::data(&doc) {
        Some(data) => {
            let parsed = next_site::parse_detail(&doc, &data, url);
            DetailRead::Next(parsed, data, next_site::origin_of(&doc))
        }
        None => DetailRead::Html(parse_detail_html(&doc, url)),
    };
    (is_app_shell(&doc), read)
}

pub async fn detail(url: &str) -> Result<FilmDetail> {
    if !api_site::is_known(url) {
        let r = http::get(url, &[]).await?;
        let (shell, read) = read_detail(&r.body(), &r.url, url);
        // Trang trắng của ứng dụng JS mà có API → đọc bằng API (bên dưới, như bản Windows gọi lại Detail)
        if !(shell && api_site::probe(url).await) {
            let d = match read {
                DetailRead::Next(parsed, data, origin) => next_site::detail(parsed?, &data, &origin, url).await,
                DetailRead::Html(d) => d,
            };
            cache(&d, url);
            return Ok(d);
        }
    }
    let ad = api_site::detail(url).await?;
    cache(&ad, url);
    Ok(ad)
}

pub fn parse_detail(doc: &Doc, url: &str) -> FilmDetail {
    if let Some(data) = next_site::data(doc) {
        if let Ok(d) = next_site::parse_detail(doc, &data, url) {
            return d;
        }
    }
    parse_detail_html(doc, url)
}

fn parse_detail_html(doc: &Doc, url: &str) -> FilmDetail {
    let title = doc
        .q1(".anisc-detail .film-name, h1.film-name, h1")
        .and_then(|e| ne(&e.txt()))
        .or_else(|| meta(doc, "og:title"))
        .unwrap_or_else(|| url.to_string());
    let other = doc
        .q1(".film-name-real, .info-hero__alt, .movie-detail .original_title, h2.org_title")
        .and_then(|e| ne(&e.txt()))
        .filter(|o| *o != title);
    let poster = doc
        .q1(".anisc-poster img, .info-hero__poster img, .movie-thumb, .halim-movie-wrapper .movie-poster img")
        .and_then(|e| image_url(&e))
        .or_else(|| meta(doc, "og:image"));
    let desc_el = doc.q1(
        ".film-description .text, .film-description, .short-content, .info-hero__desc, #film-content, .film-content, .entry-content .video-item",
    );
    let description = match desc_el.map(|e| e.txt()) {
        Some(t) if t.chars().count() > 20 => Some(t),
        _ => meta(doc, "og:description"),
    };
    let mut info_parts = Vec::new();
    if let Some(st) = doc.q1(".movie-status, .info-hero__status, .status-badge") {
        info_parts.push(st.txt());
    }
    let dur = doc.q(".anisc-info .item").into_iter().find(|e| e.txt().starts_with("Thời lượng")).and_then(|e| e.q1(".name"));
    if let Some(dur) = dur {
        info_parts.push(dur.txt());
    }
    let cats = doc.q(".item-cate a, .info-hero__tags a, .category a[rel~=tag]");
    info_parts.push(cats.iter().map(|e| e.txt()).collect::<Vec<_>>().join(", "));
    let info = ne(&distinct_by(info_parts.into_iter().filter(|p| !p.is_empty()), |p| p.clone()).join("  •  "));

    let mut groups = parse_episode_groups(doc);
    if groups.is_empty() {
        if let Some(play) = doc.q1("a.btn-play[href], a.watch-btn[href], a.watch-movie[href]").and_then(|a| ne(&a.abs("href"))) {
            groups = vec![EpisodeGroup { name: "Tập phim".into(), episodes: vec![Episode::new("Xem phim", play)] }];
        }
    }
    FilmDetail { url: url.to_string(), title, other_title: other, poster, description, info, groups }
}

/// Tên server HaLim: bỏ "#", ":" và dấu cách ở hai đầu.
fn clean_label(e: Option<El>) -> Option<String> {
    e.and_then(|e| ne(e.txt().trim_matches(|c| c == '#' || c == ':' || c == ' ')))
}

/// Danh sách tập theo nhóm, mỗi nhóm sắp tăng dần (trang web thường xếp tập mới nhất lên đầu).
fn parse_episode_groups(doc: &Doc) -> Vec<EpisodeGroup> {
    // HaLim: mỗi <ul class="halim-list-eps"> là danh sách tập của một server (Việt Sub, Thuyết Minh…)
    let lists = doc.q("ul.halim-list-eps");
    if !lists.is_empty() {
        let tab_labels = doc.q(".hh3d-server-tab__label, .halim-server-tab .label");
        let mut groups = Vec::new();
        for (i, ul) in lists.iter().enumerate() {
            let eps = episodes_from(&ul.q("a[href]"));
            if eps.is_empty() {
                continue;
            }
            let name = clean_label(ul.closest(".halim-server").and_then(|s| s.q1(".halim-server-name")))
                .or_else(|| clean_label(tab_labels.get(i).copied()))
                .unwrap_or_else(|| format!("Server {}", i + 1));
            groups.push(EpisodeGroup { name, episodes: eps });
        }
        return distinct_by(groups, |g| g.episodes.iter().map(|e| e.url.as_str()).collect::<Vec<_>>().join("\n"));
    }
    let mut links = doc.q("a.ep-item[href]");
    if links.is_empty() {
        links = doc.q(".ss-list a[href], .episodes a[href]");
    }
    if links.is_empty() {
        links = doc.q("a[href*='/tap-']");
    }
    let all = episodes_from(&links);
    if all.is_empty() {
        Vec::new()
    } else {
        vec![EpisodeGroup { name: "Tập phim".into(), episodes: all }]
    }
}

static PLAIN_NUMBER: Rx = Rx::new(r"^\d+(\.\d+)?$");

fn episodes_from(links: &[El]) -> Vec<Episode> {
    let mut eps = Vec::new();
    let mut seen = HashSet::new();
    for a in links {
        let u = a.abs("href");
        if u.is_empty() || !seen.insert(u.clone()) {
            continue;
        }
        let text = a.txt();
        let mut title = if text.is_empty() { a.at("title").trim().to_string() } else { text };
        if PLAIN_NUMBER.matches(&title) {
            title = format!("Tập {title}");
        }
        eps.push(Episode::new(title, u));
    }
    let nums: Vec<Option<f64>> = eps.iter().map(|e| ep_number(&e.title).or_else(|| ep_number(after_last(&e.url, '/')))).collect();
    if nums.iter().filter(|n| n.is_some()).count() as f64 >= eps.len() as f64 * 0.8 {
        // Sắp theo số tập (tập không có số xuống cuối), giữ thứ tự gốc khi bằng nhau
        let mut idx: Vec<usize> = (0..eps.len()).collect();
        idx.sort_by(|&i, &j| {
            let a = nums[i].unwrap_or(0.0);
            let b = nums[j].unwrap_or(0.0);
            nums[i].is_none().cmp(&nums[j].is_none()).then(a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal))
        });
        return idx.into_iter().map(|i| eps[i].clone()).collect();
    }
    eps.reverse();
    eps
}

static EP_WORD: Rx = Rx::new(r"(?i)t[aậ]p[\s\-_]*(\d+(?:\.\d+)?)");
static ANY_NUMBER: Rx = Rx::new(r"(\d+(?:\.\d+)?)");

pub fn ep_number(s: &str) -> Option<f64> {
    let g = EP_WORD.group(1, s).or_else(|| ANY_NUMBER.group(1, s))?;
    g.parse::<f64>().ok()
}

// ---- Trang tập phim ----

pub async fn source(ep_url: &str) -> Result<EpisodeSource> {
    if api_site::is_known(ep_url) {
        return api_site::source(ep_url).await;
    }
    let r = http::get(ep_url, &[]).await?;
    let (shell, parsed) = read_source(&r.body(), &r.url, ep_url);
    if shell && api_site::probe(ep_url).await {
        return api_site::source(ep_url).await;
    }
    Ok(parsed)
}

fn read_source(body: &str, location: &str, ep_url: &str) -> (bool, EpisodeSource) {
    let doc = Doc::parse(body, location);
    (is_app_shell(&doc), parse_source(&doc, ep_url))
}

static MOVIE_INFO: Rx = Rx::new(r"MovieInfo\s*=\s*\{\s*id:\s*(\d+)");
static PLAYER_PHP: Rx = Rx::new(r#"url:\s*['"]([^'"]*player\.php)['"]"#);

pub fn parse_source(doc: &Doc, ep_url: &str) -> EpisodeSource {
    if let Some(data) = next_site::data(doc) {
        if let Ok(s) = next_site::parse_source(doc, &data, ep_url) {
            return s;
        }
    }
    let scripts = doc.q("script").iter().map(|s| s.script_text()).collect::<Vec<_>>().join("\n");
    let server_els = doc.q("#list_sv [data-type], .get-eps[data-type], .btn3dsv[data-type]");
    let servers = server_els.iter().map(|e| Server::new(e.txt(), e.at("data-type"))).filter(|s| !s.kind.is_empty());
    let servers: Vec<Server> = distinct_by(servers, |s| s.kind.clone())
        .into_iter()
        .map(|s| if s.label.is_empty() { Server::new(s.kind.clone(), s.kind) } else { s })
        .collect();

    let active = doc
        .q1("a.ep-item.active[data-post-id], .halim-episode.active a[data-post-id], .active > a[data-post-id]")
        .or_else(|| doc.q1("[data-post-id][data-ep]"));
    let post_id = active.and_then(|a| ne(&a.at("data-post-id"))).or_else(|| MOVIE_INFO.group(1, &scripts)).unwrap_or_default();
    let mut tail = after_last(ep_url.trim_end_matches('/'), '/');
    if let Some(t) = tail.strip_suffix(".html") {
        tail = t;
    }
    let ep = active.and_then(|a| ne(&a.at("data-ep"))).unwrap_or_else(|| tail.to_string());
    let sv = active.and_then(|a| ne(&a.at("data-sv"))).unwrap_or_else(|| "1".into());

    // player.php kiểu "dox_ajax_player" (yanhh3d và các trang cùng nhóm)
    let player_url = scripts.contains("dox_ajax_player").then(|| match PLAYER_PHP.group(1, &scripts) {
        Some(p) => resolve(ep_url, &p),
        None => resolve(ep_url, "/player/player.php"),
    });
    // Bỏ qua iframe giữ chỗ src=""
    let direct = doc
        .q("#video-player iframe, .player-frame iframe, #halim-player-wrapper iframe")
        .into_iter()
        .find(|f| f.at("src").starts_with("http") || f.at("src").starts_with("//"))
        .and_then(|f| ne(&f.abs("src")));
    let page_mode = (player_url.is_none() || servers.is_empty() || post_id.is_empty()) && direct.is_none();
    EpisodeSource {
        ep_url: ep_url.to_string(),
        servers,
        post_id,
        ep,
        sv,
        player_url,
        direct_embed: direct,
        page_mode,
        ..Default::default()
    }
}

/// Hỏi player.php địa chỉ trình phát nhúng (iframe) của một server (trang Next.js / API: link .m3u8 / iframe).
pub async fn video_of(src: &EpisodeSource, server: Option<&Server>) -> Result<Video> {
    // Server dự phòng KKPhim / NguonC: link có sẵn (kk_phim::attach)
    if let Some(s) = server {
        if kk_phim::is_backup(&s.kind) {
            if let Some(v) = src.links.get(&s.kind) {
                return Ok(v.clone());
            }
        }
    }
    if src.next {
        let Some(s) = server else { return Err(Error::app("Không có server")) };
        return next_site::video_of(src, s).await;
    }
    let (Some(server), Some(player)) = (server, src.player_url.as_ref()) else {
        return match &src.direct_embed {
            Some(d) => Ok(Video::new(d.clone())),
            None => Err(Error::app("Không có server")),
        };
    };
    let url = format!(
        "{player}?action=dox_ajax_player&post_id={}&chapter_st={}&type={}&sv={}",
        form_encoded(&src.post_id),
        form_encoded(&src.ep),
        form_encoded(&server.kind),
        form_encoded(&src.sv)
    );
    let r = http::get(&url, &[("Referer", src.ep_url.as_str()), ("X-Requested-With", "XMLHttpRequest")]).await?;
    match iframe_of(&r.body(), &src.ep_url) {
        Some(f) if f.starts_with("http") => Ok(Video::new(f)),
        _ => Err(Error::App(format!("Server \"{}\" chưa có video cho tập này", server.label))),
    }
}

fn iframe_of(html: &str, location: &str) -> Option<String> {
    let doc = Doc::parse(html, location);
    let f = doc.q1("iframe[src]")?.abs("src");
    Some(f)
}

pub fn meta(doc: &Doc, property: &str) -> Option<String> {
    doc.q1(&format!("meta[property=\"{property}\"], meta[name=\"{property}\"]")).and_then(|m| ne(&m.at("content")))
}

pub fn resolve(base_url: &str, reference: &str) -> String {
    url::Url::parse(base_url)
        .ok()
        .and_then(|b| b.join(reference).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| reference.to_string())
}

/// Tên trang hiện trong app: tên miền, riêng add-on k-20 ghi «CLB Phim Xưa (sc.k-20.xyz)».
pub fn display_name(host: &str) -> String {
    if host == stremio::HOST {
        format!("{} ({host})", stremio::NAME)
    } else {
        host.to_string()
    }
}

/// Tên ngắn của trang: "yanhh3d", "nguonc" (bỏ nhãn chung "phim." / "www."), "CLB Phim Xưa".
pub fn short_name(host: &str) -> String {
    if host == stremio::HOST {
        return stremio::NAME.to_string();
    }
    let p: Vec<&str> = host.split('.').collect();
    if p.len() > 2 && matches!(p[0], "phim" | "www" | "m") {
        p[1].to_string()
    } else {
        p[0].to_string()
    }
}

/// Tên ngắn của trang trong danh sách; hai trang cùng tên ngắn (hoathinh3d.ee / hoathinh3d.pw) thì ghi đủ tên miền.
pub fn tag(host: &str, sites: &[String]) -> String {
    let s = short_name(host);
    let clash = sites.iter().any(|x| matches!(host_of(x), Some(o) if o != host && short_name(&o) == s));
    if clash {
        host.to_string()
    } else {
        s
    }
}

/// Menu cố định của trang API (không cần tải trang chủ); trang web thì None (menu đọc từ trang chủ).
pub fn menu(base_url: &str) -> Option<Vec<Category>> {
    api_site::is_known(base_url).then(|| api_site::menu_of(base_url))
}

/// Tên miền (chữ thường, bỏ "www.") của một địa chỉ; sai thì None.
pub fn host_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let h = u.host_str()?.to_lowercase();
    if h.is_empty() {
        return None;
    }
    Some(h.strip_prefix("www.").map(|s| s.to_string()).unwrap_or(h))
}

/// "https://yanhh3d.ee" (kèm cổng nếu không phải cổng mặc định); sai thì None.
pub fn origin_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let h = u.host_str()?;
    if h.is_empty() {
        return None;
    }
    Some(match u.port() {
        Some(p) => format!("{}://{h}:{p}", u.scheme()),
        None => format!("{}://{h}", u.scheme()),
    })
}

#[cfg(test)]
mod tests {
    //! Bộ đọc trang HTML (yanhh3d / HaLim) trên HTML dựng tay — cùng ca với SiteTests.cs / SiteTests.swift / SiteTest.kt.
    use super::*;

    const BASE: &str = "https://hh.test";

    #[test]
    fn home_cards_under_headings() {
        let html = r##"
        <html><body>
        <div class="block_area"><h2 class="cat-heading">Mới cập nhật</h2>
          <div class="flw-item"><div class="film-poster"><img data-src="/img/a.jpg" alt="Ảnh A">
            <a class="film-poster-ahref" href="/phim/a"></a></div><div class="tick-rate">Tập 5</div>
            <h3 class="film-name"><a href="/phim/a">Phim A</a></h3></div>
          <div class="flw-item"><a href="/phim/b" title="Phim B"><img src="https://cdn.test/b.jpg"></a></div>
        </div>
        <div class="block_area-realtime"><h2 class="cat-heading"></h2>
          <li class="item-top"><a href="/phim/c"><h4>Phim C</h4></a></li>
        </div>
        <nav><ul><li class="menu-item"><a href="/the-loai/tien-hiep">Tiên Hiệp</a></li>
          <li class="menu-item"><a href="#">Thể loại</a></li><li class="menu-item"><a href="/dang-nhap">Đăng nhập</a></li></ul></nav>
        </body></html>
        "##;
        let h = parse_home(&Doc::parse(html, &format!("{BASE}/")));
        assert_eq!(h.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["Mới cập nhật", "Xem nhiều"]);
        let a = &h.sections[0].films[0];
        assert_eq!(a.title, "Phim A");
        assert_eq!(a.url, format!("{BASE}/phim/a"));
        assert_eq!(a.poster.as_deref(), Some(format!("{BASE}/img/a.jpg").as_str()));
        assert_eq!(a.badge.as_deref(), Some("Tập 5"));
        assert_eq!(h.sections[0].films[1].title, "Phim B");
        assert_eq!(h.sections[1].films.iter().map(|f| f.title.as_str()).collect::<Vec<_>>(), ["Phim C"]);
        assert_eq!(h.categories.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Tiên Hiệp"]);
    }

    #[test]
    fn detail_episodes_sorted_and_grouped() {
        let html = r##"
        <html><head><meta property="og:image" content="https://cdn.test/p.jpg"></head><body>
        <h1 class="film-name">Tiên Nghịch</h1>
        <div class="film-description"><div class="text">Cuộc hành trình của Vương Lâm, một thiếu niên bình thường.</div></div>
        <div class="halim-server"><span class="halim-server-name">#Thuyết Minh:</span><ul class="halim-list-eps">
          <li><a href="/xem/tn/tap-2-tm">2</a></li><li><a href="/xem/tn/tap-1-tm">1</a></li><li><a href="/xem/tn/tap-10-tm">10</a></li></ul></div>
        <div class="halim-server"><span class="halim-server-name">Vietsub</span><ul class="halim-list-eps">
          <li><a href="/xem/tn/tap-1">Tập 1</a></li></ul></div>
        <div class="category"><a rel="category tag" href="/the-loai/a">Tiên Hiệp</a></div>
        </body></html>
        "##;
        let url = format!("{BASE}/phim/tn");
        let d = parse_detail(&Doc::parse(html, &url), &url);
        assert_eq!(d.title, "Tiên Nghịch");
        assert_eq!(d.poster.as_deref(), Some("https://cdn.test/p.jpg"));
        assert_eq!(d.groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["Thuyết Minh", "Vietsub"]);
        assert_eq!(d.groups[0].episodes.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), ["Tập 1", "Tập 2", "Tập 10"]);
        assert_eq!(d.info.as_deref(), Some("Tiên Hiệp"));
        assert_eq!(d.latest_episode().map(|e| e.title.as_str()), Some("Tập 10"));
    }

    #[test]
    fn source_with_player_php() {
        let html = r#"
        <html><body>
        <div id="list_sv"><a data-type="tiktik">VIETSUB 1080 V1</a><a data-type="vip2">VIETSUB 1080 V2</a><a data-type="">x</a></div>
        <ul><li class="active"><a class="ep-item active" data-post-id="123" data-ep="165" data-sv="1" href="/xem/tn/tap-165">165</a></li></ul>
        <script>jQuery.ajax({ url: '/player/player.php', data: { action: 'dox_ajax_player' } });</script>
        </body></html>
        "#;
        let ep_url = format!("{BASE}/xem/tn/tap-165");
        let s = parse_source(&Doc::parse(html, &ep_url), &ep_url);
        assert!(!s.page_mode);
        assert_eq!(s.servers.iter().map(|x| x.kind.as_str()).collect::<Vec<_>>(), ["tiktik", "vip2"]);
        assert_eq!(s.post_id, "123");
        assert_eq!(s.ep, "165");
        assert_eq!(s.player_url.as_deref(), Some(format!("{BASE}/player/player.php").as_str()));
    }

    #[test]
    fn page_mode_without_player() {
        let ep_url = format!("{BASE}/xem/tn/tap-1.html");
        let s = parse_source(&Doc::parse("<html><body><div id='player'></div></body></html>", &ep_url), &ep_url);
        assert!(s.page_mode);
        assert_eq!(s.ep, "tap-1");
    }

    #[test]
    fn direct_iframe_ignores_empty_placeholder() {
        let ep_url = format!("{BASE}/xem/tn/tap-2");
        let html = r#"<div id="halim-player-wrapper"><iframe src=""></iframe><iframe src="//player.test/e/1"></iframe></div>"#;
        let s = parse_source(&Doc::parse(html, &ep_url), &ep_url);
        assert_eq!(s.direct_embed.as_deref(), Some("https://player.test/e/1"));
        assert!(!s.page_mode);
    }

    /// SiteTests.JsonScanReadsInPlace (json::scan của lõi nền)
    #[test]
    fn json_scan_reads_in_place() {
        let s = r#"xx{"a":"b\"cạ😀","n":[1,2.5,-3e2,true,null],"o":{"k":"v"}} trailing"#;
        let v = json::scan(s, 2).expect("đọc được");
        assert_eq!(v["a"], "b\"cạ😀");
        assert_eq!(v["n"][0].as_i64(), Some(1));
        assert_eq!(v["n"][1].as_f64(), Some(2.5));
        assert_eq!(v["n"][2].as_f64(), Some(-300.0));
        assert_eq!(v["n"][3], true);
        assert!(v["n"][4].is_null());
        assert_eq!(v["o"]["k"], "v");
        assert!(json::scan(r#"{"a":"#, 0).is_none());
    }

    /// SiteTests.ApiNumbersAreIntegers
    #[test]
    fn api_numbers_are_integers() {
        let a = json::parse(br#"[2,2.5,{"n":3}]"#).expect("JSON");
        assert!(a[0].is_i64());
        assert!(a[1].is_f64());
        assert_eq!(json::int(a[2].as_object(), "n", 0), 3);
    }

    /// SiteTests.FormEncodedLikeJava
    #[test]
    fn form_encoded_like_java() {
        assert_eq!(form_encoded("tiên nghịch"), "ti%C3%AAn+ngh%E1%BB%8Bch");
        assert_eq!(form_encoded("a-b_c.d*e"), "a-b_c.d*e");
    }

    #[test]
    fn names_hosts_and_numbers() {
        assert_eq!(host_of("https://WWW.Yanhh3d.ee/phim"), Some("yanhh3d.ee".into()));
        assert_eq!(host_of("yanhh3d.ee"), None);
        assert_eq!(origin_of("https://yanhh3d.ee:8443/a/b?c"), Some("https://yanhh3d.ee:8443".into()));
        assert_eq!(origin_of("https://yanhh3d.ee:443/a"), Some("https://yanhh3d.ee".into()));
        assert_eq!(short_name("phim.nguonc.com"), "nguonc");
        assert_eq!(short_name("yanhh3d.ee"), "yanhh3d");
        assert_eq!(short_name("sc.k-20.xyz"), "CLB Phim Xưa");
        let sites = vec!["https://hoathinh3d.ee".to_string(), "https://hoathinh3d.pw".to_string()];
        assert_eq!(tag("hoathinh3d.pw", &sites), "hoathinh3d.pw");
        assert_eq!(tag("yanhh3d.ee", &sites), "yanhh3d");
        assert_eq!(ep_number("Tập 12.5"), Some(12.5));
        assert_eq!(ep_number("tap-07"), Some(7.0));
        assert_eq!(ep_number("Full"), None);
        assert_eq!(resolve("https://hh.test/xem/a", "/player/player.php"), "https://hh.test/player/player.php");
        assert!(menu("https://yanhh3d.ee").is_none());
        assert!(menu("https://phim.nguonc.com").is_some());
    }

    /// Các hàm async chạy trên tokio nhiều luồng: future phải gửi được sang luồng khác (kiểm lúc dịch, không chạy mạng).
    #[test]
    fn futures_are_send() {
        fn send<T: Send>(_: T) {}
        let src = EpisodeSource::default();
        send(home_of(None));
        send(check(""));
        send(list(""));
        send(search("", None));
        send(search_all(""));
        send(detail(""));
        send(source(""));
        send(video_of(&src, None));
        send(halim_api::schedule(""));
    }
}
