//! NguonC (phim.nguonc.com, từ 1.4) — bản Android: NguonC.kt, iPad: NguonC.swift, Windows: NguonC.cs (giữ cùng tên hàm).
//! API công khai /api/films/… (định dạng riêng, không phải KKPhim). Dùng hai cách:
//! - Trang riêng: trang chủ / danh sách / tìm kiếm / chi tiết / tập đọc từ API. Địa chỉ phim = /phim/&lt;slug&gt;, tập =
//!   /phim/&lt;slug&gt;/&lt;tập&gt;?sv=&lt;thứ tự server&gt;; danh sách = địa chỉ API.
//! - Server dự phòng thứ hai (cạnh KKPhim) cho yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw.
//! Video chỉ có trình phát nhúng (embedN.streamc.xyz) → phát qua iframe, agent tự bỏ quảng cáo của JW.

use crate::json::{self, Obj};
use crate::models::*;
use crate::site::{distinct_by, jstr, json_object, lock, ne};
use crate::util::{after_last, form_encoded, now_ms, take_chars, Error, Result};
use crate::{api_site, http, kk_phim, next_site, prefs, site};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

pub const HOST: &str = "phim.nguonc.com";
pub const API_BASE: &str = "https://phim.nguonc.com/api";

/// Đầu tên loại server dự phòng NguonC, vd "nc-vs", "nc-tm".
pub const PREFIX: &str = "nc-";

const TTL: i64 = 10 * 60_000;
const MISS_TTL: i64 = 30 * 60_000;

const MENU: &[(&str, &str)] = &[
    ("Hoạt hình", "films/danh-sach/hoat-hinh"),
    ("Phim bộ", "films/danh-sach/phim-bo"),
    ("Phim lẻ", "films/danh-sach/phim-le"),
    ("TV Shows", "films/danh-sach/tv-shows"),
    ("Phim thuyết minh", "films/ngon-ngu/thuyet-minh"),
    ("Phim lồng tiếng", "films/ngon-ngu/long-tieng"),
    ("Phim Trung Quốc", "films/quoc-gia/trung-quoc"),
    ("Phim Hàn Quốc", "films/quoc-gia/han-quoc"),
    ("Phim Mỹ", "films/quoc-gia/my"),
    ("Phim Nhật Bản", "films/quoc-gia/nhat-ban"),
    ("Phim Thái Lan", "films/quoc-gia/thai-lan"),
];

const GENRES: &[(&str, &str)] = &[
    ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Hoạt Hình", "hoat-hinh"), ("Giả Tưởng", "gia-tuong"),
    ("Khoa Học Viễn Tưởng", "khoa-hoc-vien-tuong"), ("Kinh Dị", "kinh-di"), ("Tình Cảm", "tinh-cam"), ("Lãng Mạn", "lang-man"),
    ("Hài", "phim-hai"), ("Tâm Lý", "tam-ly"), ("Phiêu Lưu", "phieu-luu"), ("Hình Sự", "hinh-su"), ("Gây Cấn", "gay-can"),
    ("Bí Ẩn", "bi-an"), ("Chiến Tranh", "chien-tranh"), ("Chính Kịch", "chinh-kich"), ("Gia Đình", "gia-dinh"),
    ("Lịch Sử", "lich-su"), ("Tài Liệu", "tai-lieu"), ("Nhạc", "phim-nhac"), ("Miền Tây", "mien-tay"), ("Phim 18+", "phim-18"),
];

const COUNTRIES: &[(&str, &str)] = &[
    ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Mỹ", "my"), ("Anh", "anh"),
    ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
    ("Việt Nam", "viet-nam"), ("Pháp", "phap"), ("Đức", "duc"), ("Tây Ban Nha", "tay-ban-nha"), ("Nga", "nga"),
    ("Canada", "canada"), ("Úc", "uc"), ("Italy", "italy"), ("Philippines", "philippines"), ("Brazil", "brazil"), ("Bỉ", "bi"),
];

pub fn is_site(url: &str) -> bool {
    site::host_of(url).as_deref() == Some(HOST)
}

// ---- Trang riêng ----

pub async fn home(base_url: &str) -> Result<Home> {
    let o = api_site::origin(base_url);
    let blocks: Vec<(String, String)> = vec![
        ("Hoạt hình mới cập nhật".into(), api_url(&o, "films/danh-sach/hoat-hinh")),
        ("Phim mới cập nhật".into(), newest_url(&o)),
        ("Phim bộ mới".into(), api_url(&o, "films/danh-sach/phim-bo")),
        ("Phim lẻ mới".into(), api_url(&o, "films/danh-sach/phim-le")),
        ("Phim thuyết minh".into(), api_url(&o, "films/ngon-ngu/thuyet-minh")),
    ];
    let sections = api_site::sections(&blocks, |u| async move { Ok::<_, Error>(parse_list(&get_json(&u).await?, &u).films) }).await?;
    Ok(Home { sections, categories: menu_of(base_url), moved_to: None })
}

pub fn menu_of(base_url: &str) -> Vec<Category> {
    let o = api_site::origin(base_url);
    MENU.iter().map(|(t, p)| Category::new(*t, api_url(&o, p))).collect()
}

pub fn newest_url(base_url: &str) -> String {
    api_url(&api_site::origin(base_url), "films/phim-moi-cap-nhat")
}

pub fn search_url(base_url: &str, query: &str) -> String {
    format!("{}/api/films/search?keyword={}&page=1", api_site::origin(base_url), form_encoded(query.trim()))
}

/// (thể loại, quốc gia).
pub fn filters(base_url: &str) -> (Vec<Category>, Vec<Category>) {
    let o = api_site::origin(base_url);
    (
        GENRES.iter().map(|(n, s)| Category::new(*n, api_url(&o, &format!("films/the-loai/{s}")))).collect(),
        COUNTRIES.iter().map(|(n, s)| Category::new(*n, api_url(&o, &format!("films/quoc-gia/{s}")))).collect(),
    )
}

fn api_url(origin: &str, path: &str) -> String {
    format!("{origin}/api/{path}?page=1")
}

pub async fn list(url: &str) -> Result<FilmPage> {
    Ok(parse_list(&get_json(url).await?, url))
}

/// {status, paginate:{current_page, total_page}, items:[{name, slug, thumb_url (dọc), poster_url (ngang), current_episode}]}.
pub fn parse_list(root: &Obj, url: &str) -> FilmPage {
    let o = api_site::origin(url);
    let films = json::objects(json::arr(Some(root), "items")).into_iter().filter_map(|m| {
        let slug = ne(&jstr(m, "slug"))?;
        let name = ne(&jstr(m, "name"))?;
        let p = ne(&jstr(m, "thumb_url")).unwrap_or_else(|| jstr(m, "poster_url"));
        let poster = p.starts_with("http").then_some(p);
        Some(Film::new(format!("{o}/phim/{slug}"), name, poster, next_site::badge(&jstr(m, "current_episode"))))
    });
    let films = distinct_by(films, |f| f.url.clone());
    let pg = json::obj(Some(root), "paginate");
    let (page, pages) = (json::int(pg, "current_page", 0), json::int(pg, "total_page", 0));
    let next = (!films.is_empty() && page >= 1 && page < pages).then(|| api_site::with_page(url, page + 1));
    FilmPage { films, next_url: next, sections: Vec::new() }
}

pub async fn detail(url: &str) -> Result<FilmDetail> {
    let film = film_url(url)?;
    parse_detail(&get_json(&detail_api(&film)).await?, &film)
}

pub fn parse_detail(root: &Obj, url: &str) -> Result<FilmDetail> {
    let m = json::obj(Some(root), "movie").ok_or_else(|| Error::app("Không đọc được thông tin phim"))?;
    let description = ne(&jstr(m, "description")).and_then(|c| api_site::body_text(&c, url));
    let cats = categories(m);
    let mut year = json::any(Some(m), "year");
    if year == "0" {
        year.clear();
    }
    let cat = |k: &str| cats.get(k).cloned().unwrap_or_default();
    let info = [jstr(m, "current_episode"), jstr(m, "time"), jstr(m, "language"), cat("Quốc gia"), year, cat("Thể loại")]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("  •  ");
    let mut groups = Vec::new();
    for (i, s) in json::objects(json::arr(Some(m), "episodes")).into_iter().enumerate() {
        let eps = json::objects(json::arr(Some(s), "items")).into_iter().filter_map(|e| {
            let slug = ne(&jstr(e, "slug"))?;
            if !jstr(e, "embed").starts_with("http") && !jstr(e, "m3u8").starts_with("http") {
                return None;
            }
            let name = ne(&jstr(e, "name")).unwrap_or_else(|| slug.clone());
            Some(Episode::new(api_site::title(&name), format!("{url}/{slug}?sv={i}")))
        });
        let eps = distinct_by(eps, |e| e.url.clone());
        if !eps.is_empty() {
            groups.push(EpisodeGroup { name: ne(&jstr(s, "server_name")).unwrap_or_else(|| format!("Server {}", i + 1)), episodes: eps });
        }
    }
    let poster = ne(&jstr(m, "thumb_url")).unwrap_or_else(|| jstr(m, "poster_url"));
    Ok(FilmDetail {
        url: url.to_string(),
        title: ne(&jstr(m, "name")).unwrap_or_else(|| url.to_string()),
        other_title: ne(&jstr(m, "original_name")),
        poster: poster.starts_with("http").then_some(poster),
        description,
        info: ne(&info),
        groups,
    })
}

/// category: {"1": {group:{name:"Định dạng"}, list:[{name}]}, …} → tên nhóm → "a, b".
fn categories(m: &Obj) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for v in json::obj(Some(m), "category").map(|c| c.values().collect::<Vec<_>>()).unwrap_or_default() {
        let Some(g) = v.as_object() else { continue };
        let Some(name) = ne(&json::str(json::obj(Some(g), "group"), "name")) else { continue };
        let list = json::objects(json::arr(Some(g), "list")).into_iter().map(|x| jstr(x, "name")).filter(|s| !s.is_empty());
        out.insert(name, list.collect::<Vec<_>>().join(", "));
    }
    out
}

pub async fn source(ep_url: &str) -> Result<EpisodeSource> {
    let root = get_json(&detail_api(&film_url(ep_url)?)).await?;
    let movie = json::obj(Some(&root), "movie").ok_or_else(|| Error::app("Không đọc được thông tin phim"))?;
    parse_source(movie, ep_url)
}

/// Phần đọc của source(): phim NguonC (movie) → server của tập (HLS nếu có, trình phát nhúng).
pub fn parse_source(movie: &Obj, ep_url: &str) -> Result<EpisodeSource> {
    let (sv, tap) = api_site::sv_and_tap(ep_url)?;
    let servers = json::objects(json::arr(Some(movie), "episodes"));
    let e = api_site::find_episode(&servers, sv, "items", &tap).ok_or_else(|| Error::app("Không tìm thấy tập này trên trang"))?;
    let mut links = HashMap::new();
    let mut list = Vec::new();
    let m3u8 = jstr(e, "m3u8");
    if m3u8.starts_with("http") {
        links.insert("m3u8".to_string(), Video::new(m3u8));
        list.push(Server::new("HLS", "m3u8"));
    }
    let embed = jstr(e, "embed");
    if embed.starts_with("http") {
        links.insert("embed".to_string(), Video::new(embed));
        list.push(Server::new("Trình phát NguonC", "embed"));
    }
    if links.is_empty() {
        return Err(Error::app("Tập này chưa có video"));
    }
    Ok(EpisodeSource { ep_url: ep_url.to_string(), servers: list, ep: tap, sv: sv.to_string(), links, next: true, ..Default::default() })
}

/// ".../phim/muc-than-ky/tap-1?sv=0" → ".../phim/muc-than-ky".
pub fn film_url(url: &str) -> Result<String> {
    let parts: Vec<String> = url::Url::parse(url)
        .map(|u| u.path().split('/').filter(|p| !p.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    match parts.iter().position(|p| p == "phim") {
        Some(i) if i + 1 < parts.len() => Ok(format!("{}/phim/{}", api_site::origin(url), parts[i + 1])),
        _ => Err(Error::App(format!("Địa chỉ phim lạ: {url}"))),
    }
}

fn detail_api(film: &str) -> String {
    format!("{}/api/film/{}", api_site::origin(film), after_last(film, '/'))
}

// ---- Server dự phòng ----

struct State {
    cache: HashMap<String, (i64, Option<Obj>)>,
    slugs: HashMap<String, String>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State { cache: HashMap::new(), slugs: HashMap::new() }));

/// Trang dùng server dự phòng NguonC: bật trong Cài đặt, không phải trang đã có NguonC (NguonC, phimmoi) / trang API.
pub fn applies_to(url: &str) -> bool {
    prefs::nc_backup() && !api_site::is_known(url) && !next_site::is_known(url)
}

/// Server NguonC cho một tập (rỗng nếu NguonC không có phim / tập này). Lỗi mạng → báo lỗi.
pub async fn servers(d: &FilmDetail, group: Option<&EpisodeGroup>, ep: &Episode) -> Result<Vec<(Server, Video)>> {
    let num = site::ep_number(&ep.title).or_else(|| site::ep_number(after_last(ep.url.trim_end_matches('/'), '/')));
    let Some(num) = num else { return Ok(Vec::new()) };
    let want = group.and_then(|g| kk_phim::lang(&g.name));
    Ok(match movie_for(d).await? {
        Some(movie) => pick(&movie, num, want),
        None => Vec::new(),
    })
}

/// Server có tập số num trong phim NguonC ({name, episodes:[{server_name, items:[{name, slug, embed}]}]}).
/// lang: ngôn ngữ của bản đang xem ("vs" / "tm" / "lt"; None = trang không chia bản → mọi ngôn ngữ).
pub fn pick(movie: &Obj, num: f64, lang: Option<&str>) -> Vec<(Server, Video)> {
    let mut out = Vec::new();
    let mut used = std::collections::HashSet::new();
    for s in json::objects(json::arr(Some(movie), "episodes")) {
        let l = kk_phim::lang(&jstr(s, "server_name")).unwrap_or("vs");
        if lang.is_some_and(|w| w != l) {
            continue;
        }
        let e = json::objects(json::arr(Some(s), "items"))
            .into_iter()
            .find(|x| site::ep_number(&jstr(x, "name")).or_else(|| site::ep_number(&jstr(x, "slug"))) == Some(num));
        let Some(e) = e else { continue };
        let embed = jstr(e, "embed");
        if !embed.starts_with("http") {
            continue;
        }
        let mut kind = format!("{PREFIX}{l}");
        let mut n = 1;
        while !used.insert(kind.clone()) {
            n += 1;
            kind = format!("{PREFIX}{l}-{n}");
        }
        let name = match l {
            "tm" => "Thuyết Minh",
            "lt" => "Lồng Tiếng",
            _ => "Vietsub",
        };
        let label = format!("NguonC {name}{}", if n > 1 { format!(" {n}") } else { String::new() });
        out.push((Server::new(label, kind), Video::new(embed)));
    }
    out
}

async fn movie_for(d: &FilmDetail) -> Result<Option<Obj>> {
    let now = now_ms();
    {
        let st = lock(&STATE);
        if let Some((at, hit)) = st.cache.get(&d.url) {
            if now - at < if hit.is_none() { MISS_TTL } else { TTL } {
                return Ok(hit.clone());
            }
        }
    }
    let found = find(d).await?;
    let mut st = lock(&STATE);
    st.cache.insert(d.url.clone(), (now, found.clone()));
    if st.cache.len() > 40 {
        if let Some(k) = st.cache.iter().min_by_key(|x| x.1 .0).map(|x| x.0.clone()) {
            st.cache.remove(&k);
        }
    }
    Ok(found)
}

/// Phim NguonC khớp tên với phim của trang (slug của trang trước, rồi tìm theo tên); None = NguonC không có.
async fn find(d: &FilmDetail) -> Result<Option<Obj>> {
    let mine: std::collections::HashSet<String> =
        [Some(&d.title), d.other_title.as_ref()].into_iter().flatten().map(|s| kk_phim::norm(s)).filter(|s| !s.is_empty()).collect();
    if mine.is_empty() {
        return Ok(None);
    }
    let known = lock(&STATE).slugs.get(&d.url).cloned();
    if let Some(k) = known {
        if let Some(m) = movie(&k).await? {
            return Ok(Some(m));
        }
    }
    if let Some(own) = kk_phim::slug_of(&d.url) {
        if let Some(m) = movie(&own).await? {
            if kk_phim::matches(&m, &mine) {
                remember(&d.url, &own);
                return Ok(Some(m));
            }
        }
    }
    let q = format!("{API_BASE}/films/search?keyword={}", form_encoded(&take_chars(&d.title, 80)));
    let root = get_json(&q).await?;
    let slug = json::objects(json::arr(Some(&root), "items"))
        .into_iter()
        .find(|i| kk_phim::matches(i, &mine))
        .and_then(|hit| ne(&jstr(hit, "slug")));
    let Some(slug) = slug else { return Ok(None) };
    let r = movie(&slug).await?;
    if r.is_some() {
        remember(&d.url, &slug);
    }
    Ok(r)
}

fn remember(film: &str, slug: &str) {
    lock(&STATE).slugs.insert(film.to_string(), slug.to_string());
}

/// Phim trên NguonC (movie); None nếu không có slug này (404).
async fn movie(slug: &str) -> Result<Option<Obj>> {
    match get_json(&format!("{API_BASE}/film/{}", form_encoded(slug))).await {
        Ok(o) => Ok(json::obj(Some(&o), "movie").cloned()),
        Err(Error::Http(404)) => Ok(None),
        Err(e) => Err(e),
    }
}

async fn get_json(url: &str) -> Result<Obj> {
    let r = http::get(url, &[("Accept", "application/json")]).await?;
    json_object(&r.data).ok_or_else(|| Error::app("NguonC trả dữ liệu lạ"))
}

#[cfg(test)]
mod tests {
    //! NguonC + VSMOV + CLB Phim Xưa (k-20) (11/2026) — cùng ca với NguonCTests.cs / NguonCTest.kt / StremioTest.kt /
    //! NguonCTests.swift.
    use super::*;
    use crate::kk_phim::Backup;
    use crate::stremio;

    const O: &str = "https://phim.nguonc.com";
    const K: &str = "https://sc.k-20.xyz";

    fn obj(s: &str) -> Obj {
        json::parse_object(s.as_bytes()).expect("JSON đối tượng")
    }

    fn detail_json() -> Obj {
        obj(r##"
        {"status":"success","movie":{"name":"Mục Thần Ký","original_name":"Mu Shen Ji, Tales of Herding Gods",
        "slug":"muc-than-ky","description":"<p>Tần Mục lớn lên ở <b>Tàn Lão Thôn</b>.</p>","total_episodes":120,
        "current_episode":"Tập 108","time":"21 phút/tập","quality":"HD","language":"Vietsub","year":2023,
        "thumb_url":"https://img.nguonc.com/images/a.jpg","poster_url":"https://img.nguonc.com/images/b.jpg",
        "category":{"1":{"group":{"name":"Định dạng"},"list":[{"name":"Phim bộ"}]},
                    "2":{"group":{"name":"Thể loại"},"list":[{"name":"Hành động"},{"name":"Hoạt hình"}]},
                    "4":{"group":{"name":"Quốc gia"},"list":[{"name":"Trung Quốc"}]}},
        "episodes":[
          {"server_name":"Vietsub #1","items":[
            {"name":"1","slug":"tap-1","embed":"https://embed1.streamc.xyz/embed.php?hash=a1"},
            {"name":"107","slug":"tap-107","embed":"https://embed14.streamc.xyz/embed.php?hash=a107"},
            {"name":"108","slug":"tap-108","embed":"https://embed10.streamc.xyz/embed.php?hash=a108"},
            {"name":"109","slug":"tap-109","embed":""}]},
          {"server_name":"Thuyết Minh #1","items":[
            {"name":"108","slug":"tap-108","embed":"https://embed10.streamc.xyz/embed.php?hash=t108"}]}]}}
        "##)
    }

    fn yan_site(ep: &str) -> EpisodeSource {
        EpisodeSource {
            ep_url: ep.into(),
            servers: vec![Server::new("VIETSUB 1080 V1", "tiktik")],
            post_id: "1".into(),
            ep: "tap-108".into(),
            sv: "1".into(),
            player_url: Some("https://yanhh3d.ee/player/player.php".into()),
            ..Default::default()
        }
    }

    fn kinds(s: &EpisodeSource) -> Vec<&str> {
        s.servers.iter().map(|x| x.kind.as_str()).collect()
    }

    #[test]
    fn parses_list_with_portrait_posters_and_paging() {
        let root = obj(r#"
            {"status":"success","paginate":{"current_page":2,"total_page":3365},"items":[
             {"name":"Mục Thần Ký","slug":"muc-than-ky","current_episode":"Tập 108",
              "thumb_url":"https://img.nguonc.com/images/thumb.jpg","poster_url":"https://img.nguonc.com/images/wide.jpg"},
             {"name":"Doraemon","slug":"doraemon","current_episode":"Hoàn tất (26/26)","poster_url":"https://img.nguonc.com/images/d.jpg"},
             {"name":"","slug":"rong"}]}
            "#);
        let p = parse_list(&root, &format!("{O}/api/films/danh-sach/hoat-hinh?page=2"));
        assert_eq!(p.films.iter().map(|f| f.title.as_str()).collect::<Vec<_>>(), ["Mục Thần Ký", "Doraemon"]);
        assert_eq!(p.films[0].url, format!("{O}/phim/muc-than-ky"));
        assert_eq!(p.films[0].poster.as_deref(), Some("https://img.nguonc.com/images/thumb.jpg"));
        assert_eq!(p.films.iter().map(|f| f.badge.as_deref()).collect::<Vec<_>>(), [Some("Tập 108"), Some("Tập 26")]);
        assert_eq!(p.next_url, Some(format!("{O}/api/films/danh-sach/hoat-hinh?page=3")));
    }

    #[test]
    fn parses_detail_groups_and_info() {
        let d = parse_detail(&detail_json(), &format!("{O}/phim/muc-than-ky")).expect("đọc được");
        assert_eq!(d.other_title.as_deref(), Some("Mu Shen Ji, Tales of Herding Gods"));
        assert_eq!(d.description.as_deref(), Some("Tần Mục lớn lên ở Tàn Lão Thôn."));
        assert_eq!(d.info.as_deref(), Some("Tập 108  •  21 phút/tập  •  Vietsub  •  Trung Quốc  •  2023  •  Hành động, Hoạt hình"));
        assert_eq!(d.groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["Vietsub #1", "Thuyết Minh #1"]);
        assert_eq!(d.groups[0].episodes.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), ["Tập 1", "Tập 107", "Tập 108"]);
        assert_eq!(d.groups[1].episodes[0].url, format!("{O}/phim/muc-than-ky/tap-108?sv=1"));
        assert_eq!(film_url(&format!("{O}/phim/muc-than-ky/tap-108?sv=1")).ok(), Some(format!("{O}/phim/muc-than-ky")));
    }

    #[test]
    fn source_from_movie() {
        let root = detail_json();
        let m = json::obj(Some(&root), "movie").expect("movie");
        let s = parse_source(m, &format!("{O}/phim/muc-than-ky/tap-108?sv=1")).expect("có tập");
        assert_eq!(kinds(&s), ["embed"]);
        assert_eq!(s.links["embed"].url, "https://embed10.streamc.xyz/embed.php?hash=t108");
        assert_eq!((s.ep.as_str(), s.sv.as_str()), ("tap-108", "1"));
        assert!(matches!(parse_source(m, &format!("{O}/phim/muc-than-ky/tap-109?sv=0")), Err(Error::App(e)) if e == "Tập này chưa có video"));
    }

    #[tokio::test]
    async fn backup_picks_same_episode_and_plays_from_links() {
        let root = detail_json();
        let m = json::obj(Some(&root), "movie").expect("movie");
        let all = pick(m, 108.0, None);
        assert_eq!(all.iter().map(|x| x.0.kind.as_str()).collect::<Vec<_>>(), ["nc-vs", "nc-tm"]);
        assert_eq!(all.iter().map(|x| x.0.label.as_str()).collect::<Vec<_>>(), ["NguonC Vietsub", "NguonC Thuyết Minh"]);
        assert_eq!(pick(m, 108.0, Some("tm")).iter().map(|x| x.0.kind.as_str()).collect::<Vec<_>>(), ["nc-tm"]);
        assert!(pick(m, 109.0, None).is_empty());
        let mine = ["Tales of Herding Gods"].iter().map(|s| kk_phim::norm(s)).collect();
        assert!(kk_phim::matches(m, &mine));
        let s = kk_phim::attach(yan_site("https://yanhh3d.ee/tu-tien/muc-than-ky/tap-108.html"), all);
        assert_eq!(kinds(&s), ["tiktik", "nc-vs", "nc-tm"]);
        let v = site::video_of(&s, Some(&s.servers[2])).await.expect("có link");
        assert_eq!(v.url, "https://embed10.streamc.xyz/embed.php?hash=t108");
    }

    fn ready(list: Vec<(Server, Video)>) -> Backup {
        Box::pin(async move { Ok(list) })
    }

    fn failing(msg: &'static str) -> Backup {
        Box::pin(async move { Err(Error::app(msg)) })
    }

    #[tokio::test]
    async fn slow_backup_does_not_hide_the_fast_one() {
        let ep = "https://yanhh3d.ee/tu-tien/muc-than-ky/tap-108.html";
        let nc = vec![(Server::new("NguonC Vietsub", "nc-vs"), Video::new("https://embed1.streamc.xyz/embed.php?hash=x"))];
        let kk = vec![(Server::new("KKPhim Vietsub", "kk-vs"), Video::new("https://a.kvp726.com/108/index.m3u8"))];
        let t0 = now_ms();
        let kk_slow = kk.clone();
        let slow: Backup = Box::pin(async move {
            tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
            Ok(kk_slow)
        });
        let s = kk_phim::combine_all(ep, async { Ok(yan_site(ep)) }, vec![slow, ready(nc.clone())], 400, kk_phim::FAIL_WAIT_MS)
            .await
            .expect("phát được");
        assert_eq!(kinds(&s), ["tiktik", "nc-vs"]);
        assert!(now_ms() - t0 < 2000);
        let both = kk_phim::combine_all(ep, async { Ok(yan_site(ep)) }, vec![ready(kk), ready(nc.clone())], kk_phim::WAIT_MS, kk_phim::FAIL_WAIT_MS)
            .await
            .expect("phát được");
        assert_eq!(kinds(&both), ["tiktik", "kk-vs", "nc-vs"]);
        let down = kk_phim::combine_all(
            ep,
            async { Err(Error::app("HTTP 522")) },
            vec![failing("KK lỗi"), ready(nc)],
            kk_phim::WAIT_MS,
            kk_phim::FAIL_WAIT_MS,
        )
        .await
        .expect("phát được bằng NguonC");
        assert_eq!(kinds(&down), ["nc-vs"]);
    }

    #[test]
    fn vsmov_lists_and_urls() {
        let vs = "https://vsmov.com";
        assert!(api_site::is_known(&format!("{vs}/")) && api_site::is_vs(&format!("{vs}/phim/tien-nghich")));
        assert_eq!(api_site::newest_url(vs), format!("{vs}/api/danh-sach/phim-moi-cap-nhat?page=1"));
        let (genres, _) = api_site::filters(vs);
        assert_eq!(genres.iter().find(|g| g.title == "Kiếm Hiệp").map(|g| g.url.clone()), Some(format!("{vs}/api/the-loai/kiem-hiep?page=1")));
        let p = api_site::parse_list(
            &obj(r#"
            {"status":true,"items":[{"name":"Túy Linh Lung","slug":"tuy-linh-lung","poster_url":"https://vsmov.com/storage/images/e6.jpg"}],
             "pagination":{"totalItems":2804,"totalItemsPerPage":20,"currentPage":1,"totalPages":141}}
            "#),
            &format!("{vs}/api/the-loai/hoat-hinh?country=trung-quoc&page=1"),
        );
        assert_eq!(p.films.len(), 1);
        assert_eq!(p.films[0].url, format!("{vs}/phim/tuy-linh-lung"));
        assert_eq!(p.next_url, Some(format!("{vs}/api/the-loai/hoat-hinh?country=trung-quoc&page=2")));
        let d = api_site::parse_detail(
            &obj(r##"
            {"status":true,"movie":{"name":"Túy Linh Lung"},"episodes":[{"server_name":"Lồng tiếng\r\n      #1","server_data":[
             {"name":"1","slug":"tap-1","link_embed":"https://v15.streamvsmov.com/video/fc1d03d9"}]}]}
            "##),
            &format!("{vs}/phim/tuy-linh-lung"),
        )
        .expect("đọc được");
        assert_eq!(d.groups.len(), 1);
        assert_eq!(d.groups[0].name, "Lồng tiếng #1");
        assert_eq!(d.groups[0].episodes.len(), 1);
        assert_eq!(d.groups[0].episodes[0].title, "Tập 1");
        assert_eq!(d.groups[0].episodes[0].url, format!("{vs}/phim/tuy-linh-lung/tap-1?sv=0"));
    }

    #[test]
    fn vsmov_hls_from_player_page() {
        let signed = r#"enableSignedUrl: true, signedMasterUrl: "https://v2.streamvsmov.com/stream/55010e78/master.m3u8?expires=1794131570&signature=ca8c", volume: 100 }; const baseUrl = "https://v2.streamvsmov.com"; const videoHash = "55010e78";"#;
        assert_eq!(
            api_site::vs_hls_from(signed).as_deref(),
            Some("https://v2.streamvsmov.com/stream/55010e78/master.m3u8?expires=1794131570&signature=ca8c")
        );
        let plain = r#"enableSignedUrl: false, signedMasterUrl: "", const baseUrl = "https://v15.streamvsmov.com"; const videoHash = "fc1d03d9-0667";"#;
        assert_eq!(api_site::vs_hls_from(plain).as_deref(), Some("https://v15.streamvsmov.com/stream/fc1d03d9-0667/master.m3u8"));
        assert!(api_site::vs_hls_from("<html>lỗi</html>").is_none());
    }

    #[test]
    fn stremio_catalog_urls_and_paging() {
        assert_eq!(stremio::catalog(K, None, None, 0), format!("{K}/catalog/movie/clbpx-movie.json"));
        assert_eq!(
            stremio::catalog(K, Some("Kiếm Hiệp"), None, 30),
            format!("{K}/catalog/movie/clbpx-movie/genre=Th%E1%BB%83%20lo%E1%BA%A1i%3A%20Ki%E1%BA%BFm%20Hi%E1%BB%87p&skip=30.json")
        );
        assert_eq!(stremio::search_url(K, " thiên long "), format!("{K}/catalog/movie/clbpx-movie/search=thi%C3%AAn%20long.json"));
        let data = r#"
            {"metas":[
              {"id":"clbpx:nch1993","name":"Nguyên Chấn Hiệp 1993 – 20 Tập","poster":"https://clbphimxua.com/a.jpg"},
              {"id":"clbpx:harry-5","name":"Harry Bẩn Thỉu 5: Mồ Chôn Tội Ác 1988","poster":"https://clbphimxua.com/b.jpg"},
              {"id":"nguonc:khac","name":"Không phải CLBPX"}]}
            "#
        .as_bytes();
        let p = stremio::parse_list(data, &stremio::catalog(K, Some("Kiếm Hiệp"), None, 30));
        assert_eq!(p.films.iter().map(|f| f.url.clone()).collect::<Vec<_>>(), [format!("{K}/clbpx/nch1993"), format!("{K}/clbpx/harry-5")]);
        assert_eq!(p.films.iter().map(|f| f.badge.as_deref()).collect::<Vec<_>>(), [Some("20 tập"), None]);
        assert_eq!(p.next_url, Some(stremio::catalog(K, Some("Kiếm Hiệp"), None, 32)));
        assert_eq!(stremio::parse_list(data, &stremio::catalog(K, None, None, 0)).next_url, Some(stremio::catalog(K, None, None, 2)));
        assert!(stremio::parse_list(data, &stremio::search_url(K, "x")).next_url.is_none());
    }

    #[tokio::test]
    async fn stremio_detail_and_mp4_servers() {
        let d = stremio::parse_detail(
            r#"
            {"meta":{"id":"clbpx:nch1993","name":"Nguyên Chấn Hiệp 1993 – 20 Tập","poster":"https://clbphimxua.com/p.jpg","releaseInfo":"1993",
             "videos":[{"id":"clbpx:nch1993:HFvsf0tTx","title":"Tập 02","season":1,"episode":2},
                       {"id":"clbpx:nch1993:LM4bPc-3a","title":"Tập 01","season":1,"episode":1}]}}
            "#
            .as_bytes(),
            &format!("{K}/clbpx/nch1993"),
        )
        .expect("đọc được");
        assert_eq!(d.info.as_deref(), Some("1993"));
        assert_eq!(d.groups.len(), 1);
        assert_eq!(d.groups[0].episodes.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), ["Tập 1", "Tập 2"]);
        assert_eq!(d.groups[0].episodes[0].url, format!("{K}/clbpx/nch1993/LM4bPc-3a"));
        assert_eq!(stremio::film_url(&format!("{K}/clbpx/nch1993/LM4bPc-3a")).ok(), Some(format!("{K}/clbpx/nch1993")));
        let movie = stremio::parse_detail(br#"{"meta":{"id":"clbpx:harry-5","name":"Harry 5"}}"#, &format!("{K}/clbpx/harry-5")).expect("đọc được");
        assert_eq!(movie.groups.len(), 1);
        assert_eq!(movie.groups[0].episodes.iter().map(|e| e.url.clone()).collect::<Vec<_>>(), [format!("{K}/clbpx/harry-5/full")]);
        let s = stremio::parse_source(
            r#"
            {"streams":[
              {"title":"1080p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=5&size=3069863340"},
              {"title":"1080p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=5&size=1938835536"},
              {"title":"360p\n🎬 MP4 Proxy (Abyss)","url":"https://sc.k-20.xyz/hx-mp4?embed=x&res=2&size=395465856"},
              {"title":"720p","url":""}]}
            "#
            .as_bytes(),
            &format!("{K}/clbpx/harry-5/full"),
        )
        .expect("có video");
        assert_eq!(s.servers.iter().map(|x| x.label.as_str()).collect::<Vec<_>>(), ["1080p · 3,1 GB", "1080p · 1,9 GB", "360p"]);
        let v = site::video_of(&s, Some(&s.servers[2])).await.expect("có link");
        assert_eq!(v.url, "https://sc.k-20.xyz/hx-mp4?embed=x&res=2&size=395465856");
        assert!(v.file);
        assert_eq!(site::display_name("sc.k-20.xyz"), "CLB Phim Xưa (sc.k-20.xyz)");
    }

    #[test]
    fn futures_are_send() {
        fn send<T: Send>(_: T) {}
        let d = FilmDetail::default();
        let ep = Episode::new("", "");
        send(home(""));
        send(list(""));
        send(detail(""));
        send(source(""));
        send(servers(&d, None, &ep));
    }
}
