//! Trang phim dựng bằng React (trang trắng + JS, vd mephim.ddns.net) có API JSON riêng ở /api/movies/… (nguồn KKPhim, định
//! dạng của phimapi.com) — bản Android: ApiSite.kt, iPad: ApiSite.swift, Windows: ApiSite.cs (giữ cùng tên hàm để sửa song
//! song).
//!
//! Địa chỉ trong app theo đường dẫn của chính trang: phim = /phim/&lt;slug&gt;, tập = /phim/&lt;slug&gt;/&lt;tập&gt;?sv=&lt;số thứ tự bản&gt;
//! (Vietsub / Thuyết Minh cùng tên tập nên phải ghi bản). Danh sách = địa chỉ API.

use crate::html::Doc;
use crate::json::{self, Obj};
use crate::models::*;
use crate::site::{distinct_by, jstr, json_object, lock, ne};
use crate::util::{after_last, collapse_spaces, form_encoded, Error, Result, Rx};
use crate::{http, next_site, nguon_c, site, stremio};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{LazyLock, Mutex};

/// Trang KKPhim (www.kkphim.com, từ 1.3): không có API /api/movies riêng — mọi dữ liệu gọi thẳng API công khai
/// phimapi.com (cùng định dạng, qua fallback). Địa chỉ phim / tập trong app vẫn theo tên miền kkphim.com.
pub const KK_HOST: &str = "kkphim.com";

/// VSMOV (vsmov.com, từ 1.4): API riêng cùng định dạng KKPhim nhưng khác đường dẫn (/api/danh-sach/…, /api/phim/…,
/// /api/tim-kiem). Tập chỉ có trình phát nhúng; app đọc luồng HLS trong trang trình phát (link có chữ ký) để tự phát.
pub const VS_HOST: &str = "vsmov.com";

struct State {
    known: HashSet<String>,
    not_api: HashSet<String>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| {
    Mutex::new(State {
        known: ["mephim.ddns.net", KK_HOST, VS_HOST, nguon_c::HOST, stremio::HOST].iter().map(|s| s.to_string()).collect(),
        not_api: HashSet::new(),
    })
});
const CDN: &str = "https://phimimg.com";
const LIMIT: i64 = 24;

/// Menu cố định (API không có danh sách mục): hoạt hình Trung Quốc lên đầu như các trang khác của app.
const MENU: &[(&str, &str)] = &[
    ("Hoạt hình Trung Quốc", "category/hoat-hinh?country=trung-quoc"),
    ("Hoạt hình", "category/hoat-hinh"),
    ("Phim bộ", "category/phim-bo"),
    ("Phim lẻ", "category/phim-le"),
    ("TV Shows", "category/tv-shows"),
    ("Phim thuyết minh", "category/phim-thuyet-minh"),
    ("Phim lồng tiếng", "category/phim-long-tieng"),
    ("Phim Trung Quốc", "quoc-gia/trung-quoc"),
    ("Phim Hàn Quốc", "quoc-gia/han-quoc"),
    ("Phim Âu Mỹ", "quoc-gia/au-my"),
    ("Phim Nhật Bản", "quoc-gia/nhat-ban"),
    ("Phim Thái Lan", "quoc-gia/thai-lan"),
];

const GENRES: &[(&str, &str)] = &[
    ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Viễn Tưởng", "vien-tuong"), ("Kinh Dị", "kinh-di"),
    ("Tình Cảm", "tinh-cam"), ("Hài Hước", "hai-huoc"), ("Võ Thuật", "vo-thuat"), ("Tâm Lý", "tam-ly"),
    ("Phiêu Lưu", "phieu-luu"), ("Hình Sự", "hinh-su"), ("Chiến Tranh", "chien-tranh"), ("Thần Thoại", "than-thoai"),
    ("Bí Ẩn", "bi-an"), ("Học Đường", "hoc-duong"), ("Gia Đình", "gia-dinh"), ("Thể Thao", "the-thao"),
    ("Âm Nhạc", "am-nhac"), ("Chính Kịch", "chinh-kich"), ("Khoa Học", "khoa-hoc"), ("Tài Liệu", "tai-lieu"),
    ("Lịch Sử", "lich-su"), ("Kinh Điển", "kinh-dien"), ("Phim 18+", "phim-18"),
];

const COUNTRIES: &[(&str, &str)] = &[
    ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Âu Mỹ", "au-my"),
    ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
    ("Việt Nam", "viet-nam"), ("Anh", "anh"), ("Pháp", "phap"), ("Canada", "canada"), ("Philippines", "philippines"),
    ("Indonesia", "indonesia"), ("Malaysia", "malaysia"), ("Nga", "nga"), ("Đức", "duc"), ("Tây Ban Nha", "tay-ban-nha"),
    ("Thổ Nhĩ Kỳ", "tho-nhi-ky"), ("Úc", "uc"), ("Quốc Gia Khác", "quoc-gia-khac"),
];

/// Trang KKPhim (gọi thẳng phimapi.com) thêm vài mục theo các danh sách riêng của KKPhim.
fn kk_menu() -> Vec<(&'static str, &'static str)> {
    let mut m = vec![MENU[0], ("Hoạt hình Trung Quốc thuyết minh", "category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh")];
    m.extend_from_slice(&MENU[1..]);
    m.push(("Phim chiếu rạp", "category/phim-chieu-rap"));
    m
}

pub fn is_kk(url: &str) -> bool {
    site::host_of(url).as_deref() == Some(KK_HOST)
}

pub fn is_vs(url: &str) -> bool {
    site::host_of(url).as_deref() == Some(VS_HOST)
}

/// Menu VSMOV: lọc hoạt hình theo nước có trên /api/the-loai/…?country=.
const VS_MENU: &[(&str, &str)] = &[
    ("Hoạt hình Trung Quốc", "the-loai/hoat-hinh?country=trung-quoc"),
    ("Hoạt hình", "the-loai/hoat-hinh"),
    ("Phim bộ", "danh-sach/phim-bo"),
    ("Phim lẻ", "danh-sach/phim-le"),
    ("Phim nhóm sub VSMOV", "danh-sach/subteam"),
    ("Phim Trung Quốc", "quoc-gia/trung-quoc"),
    ("Phim Hàn Quốc", "quoc-gia/han-quoc"),
    ("Phim Âu Mỹ", "quoc-gia/au-my"),
    ("Phim Nhật Bản", "quoc-gia/nhat-ban"),
    ("Phim Thái Lan", "quoc-gia/thai-lan"),
];

/// Thể loại (tên, slug KKPhim) — chọn cho hàng tự thêm của trang chủ tổng hợp (GenreList của bản Windows).
pub fn genre_list() -> Vec<(&'static str, &'static str)> {
    GENRES.iter().copied().filter(|g| g.1 != "phim-18").collect()
}

/// Quốc gia (tên, slug KKPhim) — chọn cho hàng tự thêm của trang chủ tổng hợp (CountryList).
pub fn country_list() -> Vec<(&'static str, &'static str)> {
    COUNTRIES.to_vec()
}

const VS_GENRES: &[(&str, &str)] = &[
    ("Hành Động", "hanh-dong"), ("Cổ Trang", "co-trang"), ("Kiếm Hiệp", "kiem-hiep"), ("Tiên Hiệp", "tien-hiep"),
    ("Võ Thuật", "vo-thuat"), ("Viễn Tưởng", "vien-tuong"), ("Giả Tưởng", "gia-tuong"), ("Kinh Dị", "kinh-di"),
    ("Lãng Mạn", "lang-man"), ("Hài", "hai"), ("Chính Kịch", "chinh-kich"), ("Phiêu Lưu", "phieu-luu"),
    ("Hình Sự", "hinh-su"), ("Tội Phạm", "toi-pham"), ("Chiến Tranh", "chien-tranh"), ("Bí Ẩn", "bi-an"),
    ("Học Đường", "hoc-duong"), ("Gia Đình", "gia-dinh"), ("Thiếu Nhi", "thieu-nhi"), ("Thanh Xuân", "thanh-xuan"),
    ("Thương Trường", "thuong-truong"), ("Xã Hội Đen", "xa-hoi-den"), ("Truyền Hình Thực Tế", "truyen-hinh-thuc-te"),
    ("Phim Nhạc", "phim-nhac"),
];

const VS_COUNTRIES: &[(&str, &str)] = &[
    ("Trung Quốc", "trung-quoc"), ("Hàn Quốc", "han-quoc"), ("Nhật Bản", "nhat-ban"), ("Âu Mỹ", "au-my"), ("Mỹ", "my"),
    ("Thái Lan", "thai-lan"), ("Đài Loan", "dai-loan"), ("Hồng Kông", "hong-kong"), ("Ấn Độ", "an-do"),
    ("Việt Nam", "viet-nam"), ("Anh", "anh"), ("Pháp", "phap"), ("Canada", "canada"), ("Úc", "uc"), ("Đức", "duc"),
    ("Tây Ban Nha", "tay-ban-nha"), ("Thổ Nhĩ Kỳ", "tho-nhi-ky"), ("Philippines", "philippines"), ("Indonesia", "indonesia"),
];

/// Địa chỉ API của VSMOV: "danh-sach/phim-bo" → https://vsmov.com/api/danh-sach/phim-bo?page=1.
fn vs_api(origin: &str, path: &str, page: i64) -> String {
    match path.split_once('?') {
        Some((p, q)) => format!("{origin}/api/{p}?{q}&page={page}"),
        None => format!("{origin}/api/{path}?page={page}"),
    }
}

pub fn is_known(url: &str) -> bool {
    match site::host_of(url) {
        Some(h) => lock(&STATE).known.contains(&h),
        None => false,
    }
}

/// Trang chưa biết mà HTML là trang trắng của ứng dụng JS: thử API (một lần mỗi trang).
pub async fn probe(base_url: &str) -> bool {
    let Some(host) = site::host_of(base_url) else { return false };
    {
        let st = lock(&STATE);
        if st.known.contains(&host) {
            return true;
        }
        if st.not_api.contains(&host) {
            return false;
        }
    }
    let ok = match get_json(&format!("{}/api/movies/newest?page=1&limit=1", origin(base_url)), false).await {
        Ok(o) => json::arr(Some(&o), "items").is_some(),
        Err(_) => false,
    };
    let mut st = lock(&STATE);
    if ok {
        st.known.insert(host);
    } else {
        st.not_api.insert(host);
    }
    ok
}

// ---- Trang chủ / danh sách ----

/// Trang chủ: các khối tải song song; khối nào lỗi thì bỏ.
pub async fn home(base_url: &str) -> Result<Home> {
    if nguon_c::is_site(base_url) {
        return nguon_c::home(base_url).await;
    }
    if stremio::is_site(base_url) {
        return stremio::home(base_url).await;
    }
    let o = origin(base_url);
    let kk = is_kk(base_url);
    let vs = is_vs(base_url);
    // KKPhim không có khối thịnh hành / song ngữ: thêm hoạt hình thuyết minh, phim bộ, phim lẻ mới
    let blocks: Vec<(String, String)> = if vs {
        vec![
            ("Hoạt hình Trung Quốc".into(), vs_api(&o, "the-loai/hoat-hinh?country=trung-quoc", 1)),
            ("Phim mới cập nhật".into(), newest_url(&o)),
            ("Phim bộ mới".into(), vs_api(&o, "danh-sach/phim-bo", 1)),
            ("Phim lẻ mới".into(), vs_api(&o, "danh-sach/phim-le", 1)),
            ("Phim nhóm sub VSMOV".into(), vs_api(&o, "danh-sach/subteam", 1)),
        ]
    } else if kk {
        vec![
            ("Hoạt hình Trung Quốc mới cập nhật".into(), api(&o, "category/hoat-hinh?country=trung-quoc", 1)),
            ("Hoạt hình Trung Quốc thuyết minh".into(), api(&o, "category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh", 1)),
            ("Phim mới cập nhật".into(), newest_url(&o)),
            ("Phim bộ mới".into(), api(&o, "category/phim-bo", 1)),
            ("Phim lẻ mới".into(), api(&o, "category/phim-le", 1)),
        ]
    } else {
        vec![
            ("Hoạt hình Trung Quốc mới cập nhật".into(), api(&o, "category/hoat-hinh?country=trung-quoc", 1)),
            ("Phim mới cập nhật".into(), newest_url(&o)),
            ("Thịnh hành".into(), format!("{o}/api/movies/trending")),
            ("Song ngữ / Thuyết minh".into(), format!("{o}/api/movies/song-ngu")),
        ]
    };
    let org = o.as_str();
    let sections = sections(&blocks, |u| async move { Ok(parse_items(&get_json(&u, true).await?, org)) }).await?;
    Ok(Home { sections, categories: menu_of(base_url), moved_to: None })
}

/// Mục menu của trang (cố định — không cần tải trang chủ; trang chủ tổng hợp dùng).
pub fn menu_of(base_url: &str) -> Vec<Category> {
    if nguon_c::is_site(base_url) {
        return nguon_c::menu_of(base_url);
    }
    if stremio::is_site(base_url) {
        return stremio::menu_of(base_url);
    }
    let o = origin(base_url);
    if is_vs(base_url) {
        return VS_MENU.iter().map(|(t, p)| Category::new(*t, vs_api(&o, p, 1))).collect();
    }
    let menu = if is_kk(base_url) { kk_menu() } else { MENU.to_vec() };
    menu.iter().map(|(t, p)| Category::new(*t, api(&o, p, 1))).collect()
}

/// Tên thể loại theo slug ("vo-thuat" → "võ thuật") — tiêu đề hàng gợi ý của trang chủ tổng hợp.
pub fn genre_name(slug: &str) -> String {
    GENRES.iter().chain(VS_GENRES).find(|g| g.1 == slug).map(|g| g.0.to_lowercase()).unwrap_or_else(|| slug.replace('-', " "))
}

/// Tên nước theo slug ("trung-quoc" → "Trung Quốc").
pub fn country_name(slug: &str) -> String {
    COUNTRIES.iter().chain(VS_COUNTRIES).find(|c| c.1 == slug).map(|c| c.0.to_string()).unwrap_or_else(|| slug.replace('-', " "))
}

/// JSON của một API (KKPhim: gọi thẳng phimapi.com; trang lỗi thì phimapi.com) — trang chủ tổng hợp dùng.
pub async fn json(url: &str) -> Result<Obj> {
    get_json(url, true).await
}

/// Các khối trang chủ (tên, địa chỉ) tải song song; khối nào lỗi / rỗng thì bỏ, lỗi hết thì báo lỗi.
pub async fn sections<F, Fut>(blocks: &[(String, String)], load: F) -> Result<Vec<FilmSection>>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<Film>>>,
{
    let results = futures::future::join_all(blocks.iter().map(|b| load(b.1.clone()))).await;
    let mut out = Vec::new();
    let mut first_error: Option<Error> = None;
    for (b, r) in blocks.iter().zip(results) {
        match r {
            Ok(films) => {
                if !films.is_empty() {
                    out.push(FilmSection { title: b.0.clone(), films });
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
    }
    if out.is_empty() {
        return Err(first_error.unwrap_or_else(|| Error::app("Không tải được danh sách phim")));
    }
    Ok(out)
}

pub async fn list(url: &str) -> Result<FilmPage> {
    if nguon_c::is_site(url) {
        return nguon_c::list(url).await;
    }
    if stremio::is_site(url) {
        return stremio::list(url).await;
    }
    Ok(parse_list(&get_json(url, true).await?, url))
}

/// Danh sách có phân trang (data.params.pagination; VSMOV: pagination ở gốc) → trang sau = page + 1.
pub fn parse_list(root: &Obj, url: &str) -> FilmPage {
    let films = parse_items(root, &origin(url));
    let data = json::obj(Some(root), "data").unwrap_or(root);
    let p = json::obj(json::obj(Some(data), "params"), "pagination").or_else(|| json::obj(Some(root), "pagination"));
    let (page, pages) = (json::int(p, "currentPage", 0), json::int(p, "totalPages", 0));
    let next = (!films.is_empty() && page >= 1 && page < pages).then(|| with_page(url, page + 1));
    FilmPage { films, next_url: next, sections: Vec::new() }
}

/// Phim mới cập nhật (đo nhanh trang trong Cài đặt).
pub fn newest_url(base_url: &str) -> String {
    if nguon_c::is_site(base_url) {
        nguon_c::newest_url(base_url)
    } else if stremio::is_site(base_url) {
        stremio::newest_url(base_url)
    } else if is_vs(base_url) {
        format!("{}/api/danh-sach/phim-moi-cap-nhat?page=1", origin(base_url))
    } else {
        format!("{}/api/movies/newest?page=1&limit={LIMIT}", origin(base_url))
    }
}

pub fn search_url(base_url: &str, query: &str) -> String {
    if nguon_c::is_site(base_url) {
        nguon_c::search_url(base_url, query)
    } else if stremio::is_site(base_url) {
        stremio::search_url(base_url, query)
    } else if is_vs(base_url) {
        format!("{}/api/tim-kiem?keyword={}&page=1&limit={LIMIT}", origin(base_url), form_encoded(query.trim()))
    } else {
        format!("{}/api/movies/search?keyword={}&page=1&limit={LIMIT}", origin(base_url), form_encoded(query.trim()))
    }
}

/// (thể loại, quốc gia) của trang.
pub fn filters(base_url: &str) -> (Vec<Category>, Vec<Category>) {
    if nguon_c::is_site(base_url) {
        return nguon_c::filters(base_url);
    }
    if stremio::is_site(base_url) {
        return stremio::filters(base_url);
    }
    let o = origin(base_url);
    if is_vs(base_url) {
        return (
            VS_GENRES.iter().map(|(n, s)| Category::new(*n, vs_api(&o, &format!("the-loai/{s}"), 1))).collect(),
            VS_COUNTRIES.iter().map(|(n, s)| Category::new(*n, vs_api(&o, &format!("quoc-gia/{s}"), 1))).collect(),
        );
    }
    (
        GENRES.iter().map(|(n, s)| Category::new(*n, api(&o, &format!("the-loai/{s}"), 1))).collect(),
        COUNTRIES.iter().map(|(n, s)| Category::new(*n, api(&o, &format!("quoc-gia/{s}"), 1))).collect(),
    )
}

/// Phim trong một phản hồi danh sách (items ở gốc hoặc trong data).
pub fn parse_items(root: &Obj, origin: &str) -> Vec<Film> {
    let data = json::obj(Some(root), "data").unwrap_or(root);
    let b = ne(&jstr(data, "APP_DOMAIN_CDN_IMAGE")).or_else(|| ne(&jstr(root, "cdnDomain"))).unwrap_or_else(|| CDN.to_string());
    let films = json::objects(json::arr(Some(data), "items")).into_iter().filter_map(|o| film_of(o, origin, &b));
    distinct_by(films, |f| f.url.clone())
}

fn film_of(o: &Obj, origin: &str, b: &str) -> Option<Film> {
    let slug = ne(&jstr(o, "slug"))?;
    let name = ne(&jstr(o, "name"))?;
    let reference = ne(&jstr(o, "poster_url")).unwrap_or_else(|| jstr(o, "thumb_url"));
    let poster = image(&reference, &ne(&jstr(o, "cdnDomain")).unwrap_or_else(|| b.to_string()));
    Some(Film {
        url: format!("{origin}/phim/{slug}"),
        title: name,
        poster,
        badge: next_site::badge(&jstr(o, "episode_current")),
        imdb: imdb_of(json::obj(Some(o), "imdb")),
    })
}

// ---- Chi tiết phim ----

pub async fn detail(url: &str) -> Result<FilmDetail> {
    if nguon_c::is_site(url) {
        return nguon_c::detail(url).await;
    }
    if stremio::is_site(url) {
        return stremio::detail(url).await;
    }
    let film = film_url(url)?;
    let root = get_json(&detail_api(&film)?, true).await?;
    parse_detail(&root, &film)
}

static SPACES: Rx = Rx::new(r"\s+");

pub fn parse_detail(root: &Obj, url: &str) -> Result<FilmDetail> {
    let m = json::obj(Some(root), "movie").ok_or_else(|| Error::app("Không đọc được thông tin phim"))?;
    let description = ne(&jstr(m, "content")).and_then(|c| body_text(&c, url));
    let mut year = json::any(Some(m), "year");
    if year == "0" {
        year.clear();
    }
    let info = [
        jstr(m, "episode_current"),
        jstr(m, "time"),
        jstr(m, "lang"),
        names(json::arr(Some(m), "country")),
        year,
        imdb_of(json::obj(Some(m), "imdb")).map(|i| i.detail()).unwrap_or_default(),
        names(json::arr(Some(m), "category")),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join("  •  ");
    let mut groups = Vec::new();
    for (i, s) in json::objects(json::arr(Some(root), "episodes")).into_iter().enumerate() {
        let eps = json::objects(json::arr(Some(s), "server_data")).into_iter().filter_map(|e| {
            let slug = ne(&jstr(e, "slug"))?;
            if jstr(e, "link_m3u8").is_empty() && jstr(e, "link_embed").is_empty() {
                return None;
            }
            let name = ne(&jstr(e, "name")).unwrap_or_else(|| slug.clone());
            Some(Episode::new(title(&name), format!("{url}/{slug}?sv={i}")))
        });
        let eps = distinct_by(eps, |e| e.url.clone());
        // VSMOV: "Vietsub\r\n      #1" → gộp khoảng trắng
        let name = ne(&SPACES.replace(&jstr(s, "server_name"), " "));
        if !eps.is_empty() {
            groups.push(EpisodeGroup { name: name.unwrap_or_else(|| format!("Bản {}", i + 1)), episodes: eps });
        }
    }
    Ok(FilmDetail {
        url: url.to_string(),
        title: ne(&jstr(m, "name")).unwrap_or_else(|| url.to_string()),
        other_title: ne(&jstr(m, "origin_name")),
        poster: image(&ne(&jstr(m, "poster_url")).unwrap_or_else(|| jstr(m, "thumb_url")), CDN),
        description,
        info: ne(&info),
        groups,
    })
}

/// Chữ của đoạn HTML (mô tả phim trong API): toàn bộ chữ trong body, khoảng trắng gộp lại; rỗng → None.
pub(crate) fn body_text(html: &str, base: &str) -> Option<String> {
    let doc = Doc::parse(html, base);
    let body = doc.q1("body")?;
    ne(&collapse_spaces(&body.script_text()))
}

static LEADING_ZERO: Rx = Rx::new(r"(?i)^(Tập\s*)0+(\d)");

static BARE_NUMBER: Rx = Rx::new(r"^\d+(\.\d+)?$");

/// "Tập 01" → "Tập 1", "164" → "Tập 164" (VSMOV ghi số trần); "Full" giữ nguyên.
pub fn title(name: &str) -> String {
    let s = name.trim();
    if BARE_NUMBER.matches(s) {
        let n = s.trim_start_matches('0');
        return if n.is_empty() || n.starts_with('.') { format!("Tập 0{n}") } else { format!("Tập {n}") };
    }
    LEADING_ZERO.replace(s, "${1}${2}")
}

// ---- Tập phim ----

static SV_RX: Rx = Rx::new(r"[?&]sv=(\d+)");

/// Thứ tự bản (?sv=N) và tên tập (đoạn cuối đường dẫn) của địa chỉ tập; dùng chung với nguon_c.
pub(crate) fn sv_and_tap(ep_url: &str) -> Result<(usize, String)> {
    let u = url::Url::parse(ep_url).map_err(|_| Error::App(format!("Địa chỉ phim lạ: {ep_url}")))?;
    let query = u.query().map(|q| format!("?{q}")).unwrap_or_default();
    let sv = SV_RX.group(1, &query).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    Ok((sv, after_last(u.path().trim_end_matches('/'), '/').to_string()))
}

/// Tập có tên `tap` trong danh sách server (bản đang xem — `sv` — trước, rồi mọi bản); `key` = khoá danh sách tập.
pub(crate) fn find_episode<'a>(servers: &[&'a Obj], sv: usize, key: &str, tap: &str) -> Option<&'a Obj> {
    let first = servers.get(sv).copied();
    first
        .into_iter()
        .chain(servers.iter().copied())
        .find_map(|s| json::objects(json::arr(Some(s), key)).into_iter().find(|x| jstr(x, "slug") == tap))
}

/// Hai server cho mỗi tập: luồng HLS (app tự phát, bỏ quảng cáo) và trình phát nhúng của KKPhim (dự phòng).
pub async fn source(ep_url: &str) -> Result<EpisodeSource> {
    if nguon_c::is_site(ep_url) {
        return nguon_c::source(ep_url).await;
    }
    if stremio::is_site(ep_url) {
        return stremio::source(ep_url).await;
    }
    let root = get_json(&detail_api(&film_url(ep_url)?)?, true).await?;
    let (sv, tap) = sv_and_tap(ep_url)?;
    let (embed, mut hls) = {
        let servers = json::objects(json::arr(Some(&root), "episodes"));
        let e = find_episode(&servers, sv, "server_data", &tap).ok_or_else(|| Error::app("Không tìm thấy tập này trên trang"))?;
        let (le, lm) = (jstr(e, "link_embed"), jstr(e, "link_m3u8"));
        (le.starts_with("http").then_some(le), lm.starts_with("http").then_some(lm))
    };
    let vs = is_vs(ep_url);
    if hls.is_none() && vs {
        if let Some(em) = &embed {
            hls = vs_hls(em).await;
        }
    }
    let mut links = HashMap::new();
    let mut list = Vec::new();
    if let Some(h) = hls {
        links.insert("m3u8".to_string(), Video::new(h));
        list.push(Server::new(if vs { "HLS" } else { "HLS (bỏ quảng cáo)" }, "m3u8"));
    }
    if let Some(em) = embed {
        links.insert("embed".to_string(), Video::new(em));
        list.push(Server::new(if vs { "Trình phát VSMOV" } else { "Trình phát KKPhim" }, "embed"));
    }
    if links.is_empty() {
        return Err(Error::app("Tập này chưa có video"));
    }
    Ok(EpisodeSource { ep_url: ep_url.to_string(), servers: list, ep: tap, sv: sv.to_string(), links, next: true, ..Default::default() })
}

/// Luồng HLS trong trang trình phát VSMOV (vN.streamvsmov.com/video/&lt;mã&gt;): link có chữ ký (signedMasterUrl, hết hạn sau
/// khoảng một giờ) hoặc /stream/&lt;mã&gt;/master.m3u8 khi máy chủ đó không ký. Đoạn video ngụy trang ảnh PNG — hls.html cắt
/// phần ảnh. Không đọc được thì None (vẫn còn trình phát nhúng).
async fn vs_hls(embed: &str) -> Option<String> {
    let r = http::get(embed, &[("Referer", "https://vsmov.com/")]).await.ok()?;
    vs_hls_from(&r.body())
}

static SIGNED_RX: Rx = Rx::new(r#"signedMasterUrl\s*:\s*"(https?://[^"]+)""#);
static BASE_RX: Rx = Rx::new(r#"baseUrl\s*=\s*"(https?://[^"]+)""#);
static HASH_RX: Rx = Rx::new(r#"videoHash\s*=\s*"([0-9a-zA-Z-]+)""#);

pub fn vs_hls_from(page: &str) -> Option<String> {
    if let Some(signed) = SIGNED_RX.group(1, page) {
        return Some(signed);
    }
    let b = BASE_RX.group(1, page)?;
    let h = HASH_RX.group(1, page)?;
    Some(format!("{b}/stream/{h}/master.m3u8"))
}

// ---- Tiện ích ----

/// Địa chỉ phim từ địa chỉ phim / tập: ".../phim/tien-nghich/tap-01?sv=1" → ".../phim/tien-nghich".
pub fn film_url(url: &str) -> Result<String> {
    let parts: Vec<String> = url::Url::parse(url)
        .map(|u| u.path().split('/').filter(|p| !p.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    match parts.iter().position(|p| p == "phim") {
        Some(i) if i + 1 < parts.len() => Ok(format!("{}/phim/{}", origin(url), parts[i + 1])),
        _ => Err(Error::App(format!("Địa chỉ phim lạ: {url}"))),
    }
}

fn detail_api(url: &str) -> Result<String> {
    let film = film_url(url)?;
    let slug = after_last(&film, '/');
    Ok(if is_vs(url) { format!("{}/api/phim/{slug}", origin(url)) } else { format!("{}/api/movies/detail/{slug}", origin(url)) })
}

fn api(origin: &str, path: &str, page: i64) -> String {
    match path.split_once('?') {
        Some((p, q)) => format!("{origin}/api/movies/{p}?{q}&page={page}&limit={LIMIT}"),
        None => format!("{origin}/api/movies/{path}?page={page}&limit={LIMIT}"),
    }
}

static PAGE_RX: Rx = Rx::new(r"([?&]page=)\d+");

pub fn with_page(url: &str, page: i64) -> String {
    if PAGE_RX.matches(url) {
        return PAGE_RX.re().replacen(url, 1, format!("${{1}}{page}").as_str()).into_owned();
    }
    format!("{url}{}page={page}", if url.contains('?') { "&" } else { "?" })
}

/// Gọi API của trang; trang lỗi (máy chủ của trang hay báo 500) thì gọi thẳng nguồn KKPhim (phimapi.com — cùng định
/// dạng), như chính trang web làm.
async fn get_json(url: &str, use_fallback: bool) -> Result<Obj> {
    // Trang KKPhim: gọi thẳng phimapi.com (kkphim.com không có /api/movies)
    if is_kk(url) {
        return fetch_json(&fallback(url).unwrap_or_else(|| url.to_string())).await;
    }
    match fetch_json(url).await {
        Ok(o) => Ok(o),
        Err(e) => {
            let Some(alt) = (if use_fallback { fallback(url) } else { None }) else { return Err(e) };
            match fetch_json(&alt).await {
                Ok(o) => Ok(o),
                Err(_) => Err(e),
            }
        }
    }
}

async fn fetch_json(url: &str) -> Result<Obj> {
    let r = http::get(url, &[("Accept", "application/json")]).await?;
    let o = json_object(&r.data).ok_or_else(|| Error::app("Trang trả dữ liệu lạ"))?;
    if let Some(Value::String(es)) = o.get("error") {
        if !o.contains_key("items") && !o.contains_key("data") && !o.contains_key("movie") {
            return Err(Error::App(es.clone()));
        }
    }
    Ok(o)
}

/// Địa chỉ tương ứng trên phimapi.com của một API của trang (không có thì None, vd thịnh hành / song ngữ).
pub fn fallback(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let full = u.path();
    let i = full.find("/api/movies/")?;
    let path = &full[i + "/api/movies/".len()..];
    let q = url.find('?').map(|i| format!("?{}", &url[i + 1..])).unwrap_or_default();
    const KK: &str = "https://phimapi.com";
    if path == "newest" {
        return Some(format!("{KK}/v1/api/danh-sach/phim-moi-cap-nhat{q}"));
    }
    if let Some(rest) = path.strip_prefix("category/") {
        return Some(format!("{KK}/v1/api/danh-sach/{rest}{q}"));
    }
    if path.starts_with("the-loai/") || path.starts_with("quoc-gia/") {
        return Some(format!("{KK}/v1/api/{path}{q}"));
    }
    if path == "search" {
        return Some(format!("{KK}/v1/api/tim-kiem{q}"));
    }
    if let Some(rest) = path.strip_prefix("detail/") {
        return Some(format!("{KK}/phim/{rest}"));
    }
    None
}

pub fn origin(url: &str) -> String {
    site::origin_of(url).unwrap_or_else(|| url.trim_end_matches('/').to_string())
}

/// Ảnh: đường dẫn đầy đủ giữ nguyên; "uploads/movies/…" → nối vào máy chủ ảnh.
pub fn image(reference: &str, b: &str) -> Option<String> {
    if reference.is_empty() {
        return None;
    }
    if reference.starts_with("http") {
        return Some(reference.to_string());
    }
    let host = site::origin_of(b).unwrap_or_else(|| CDN.to_string());
    Some(format!("{host}/{}", reference.trim_start_matches('/')))
}

fn names(a: Option<&Vec<Value>>) -> String {
    json::objects(a).into_iter().map(|o| jstr(o, "name")).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ")
}

// ---- Điểm IMDb (Imdb.Of của bản Windows — ImdbTop.cs) ----

/// Số thực của giá trị JSON bất kỳ kiểu (Feed.Dbl).
fn dbl(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Số nguyên của giá trị JSON bất kỳ kiểu (Feed.Num64); không đọc được → 0.
fn num64(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|d| d as i64)).unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// Đối tượng "imdb" của KKPhim → Imdb (None = chưa có điểm / quá ít lượt).
pub fn imdb_of(o: Option<&Obj>) -> Option<Imdb> {
    let o = o?;
    let s = dbl(o.get("vote_average")).unwrap_or(0.0);
    let v = num64(o.get("vote_count"));
    (s > 0.0 && s <= 10.0 && v >= Imdb::MIN_VOTES).then_some(Imdb { score: s, votes: v })
}


#[cfg(test)]
mod tests {
    //! mephim.ddns.net + hoathinh3d.pw — cùng ca với ApiAndNextTests.cs (ApiSiteTests, HalimApiTests) / ApiSiteTests.swift.
    use super::*;
    use crate::site::halim_api;

    const O: &str = "https://mephim.ddns.net";

    fn obj(s: &str) -> Obj {
        json::parse_object(s.as_bytes()).expect("JSON đối tượng")
    }

    fn titles(f: &[Film]) -> Vec<&str> {
        f.iter().map(|x| x.title.as_str()).collect()
    }

    const LIST_BODY: &str = r#"
        {"status":true,"msg":"done","data":{"items":[
        {"name":"Tiên Nghịch","slug":"tien-nghich","poster_url":"uploads/movies/20260720/tien-nghich-poster.webp",
         "thumb_url":"uploads/movies/20260720/tien-nghich-thumb.webp","episode_current":"Tập 165"},
        {"name":"Avatar","slug":"avatar","poster_url":"https://phimimg.com/upload/vod/a.jpg","episode_current":"Full"},
        {"name":"Hết","slug":"het","thumb_url":"uploads/movies/x-thumb.webp","episode_current":"Hoàn Tất (12/12)"}],
        "params":{"pagination":{"totalItems":3629,"totalItemsPerPage":24,"currentPage":2,"totalPages":152}},
        "APP_DOMAIN_CDN_IMAGE":"https://phimimg.com"}}
        "#;

    #[test]
    fn parses_list_with_relative_posters_and_paging() {
        let url = format!("{O}/api/movies/category/hoat-hinh?country=trung-quoc&page=2&limit=24");
        let p = parse_list(&obj(LIST_BODY), &url);
        assert_eq!(titles(&p.films), ["Tiên Nghịch", "Avatar", "Hết"]);
        assert_eq!(p.films[0].url, format!("{O}/phim/tien-nghich"));
        assert_eq!(p.films[0].poster.as_deref(), Some("https://phimimg.com/uploads/movies/20260720/tien-nghich-poster.webp"));
        assert_eq!(p.films[1].poster.as_deref(), Some("https://phimimg.com/upload/vod/a.jpg"));
        assert_eq!(p.films[2].poster.as_deref(), Some("https://phimimg.com/uploads/movies/x-thumb.webp"));
        assert_eq!(p.films.iter().map(|f| f.badge.as_deref()).collect::<Vec<_>>(), [Some("Tập 165"), Some("Full"), Some("Tập 12")]);
        assert_eq!(p.next_url, Some(format!("{O}/api/movies/category/hoat-hinh?country=trung-quoc&page=3&limit=24")));
        let last = LIST_BODY.replace("\"currentPage\":2", "\"currentPage\":152");
        assert!(parse_list(&obj(&last), &url).next_url.is_none());
    }

    #[test]
    fn parses_trending_items_at_root() {
        let body = r#"
        {"status":true,"cdnDomain":"https://phimimg.com/uploads/movies","items":[
        {"name":"VisionQuest","slug":"vision","poster_url":"https://image.tmdb.org/t/p/w500/x.jpg","episode_current":"Tập 6"}]}
        "#;
        let films = parse_items(&obj(body), O);
        assert_eq!(films.iter().map(|f| f.url.clone()).collect::<Vec<_>>(), [format!("{O}/phim/vision")]);
        assert_eq!(films[0].poster.as_deref(), Some("https://image.tmdb.org/t/p/w500/x.jpg"));
    }

    #[test]
    fn parses_detail_groups_per_language() {
        let body = r#"
        {"status":true,"movie":{"name":"Tiên Nghịch","origin_name":"Renegade Immortal",
        "content":"<p>Phim kể về <b>Vương Lâm</b>, một thiếu niên bình thường.</p>",
        "poster_url":"https://phimimg.com/uploads/movies/20260720/tien-nghich-poster.webp","episode_current":"Tập 165",
        "time":"23 phút/tập","lang":"Vietsub + Thuyết Minh","year":2023,
        "country":[{"name":"Trung Quốc"}],"category":[{"name":"Cổ Trang"},{"name":"Hoạt Hình"}]},
        "episodes":[
         {"server_name":"Vietsub","server_data":[
           {"name":"Tập 01","slug":"tap-01","link_embed":"https://player.phimapi.com/player/?url=a","link_m3u8":"https://s4.phim1280.tv/a/index.m3u8"},
           {"name":"Tập 02","slug":"tap-02","link_embed":"","link_m3u8":"https://s4.phim1280.tv/b/index.m3u8"},
           {"name":"Tập 03","slug":"tap-03","link_embed":"","link_m3u8":""}]},
         {"server_name":"Thuyết Minh","server_data":[
           {"name":"Tập 01","slug":"tap-01","link_embed":"","link_m3u8":"https://s5.phim1280.tv/c/index.m3u8"}]}]}
        "#;
        let d = parse_detail(&obj(body), &format!("{O}/phim/tien-nghich")).expect("đọc được");
        assert_eq!(d.title, "Tiên Nghịch");
        assert_eq!(d.other_title.as_deref(), Some("Renegade Immortal"));
        assert_eq!(d.description.as_deref(), Some("Phim kể về Vương Lâm, một thiếu niên bình thường."));
        assert_eq!(
            d.info.as_deref(),
            Some("Tập 165  •  23 phút/tập  •  Vietsub + Thuyết Minh  •  Trung Quốc  •  2023  •  Cổ Trang, Hoạt Hình")
        );
        assert_eq!(d.groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["Vietsub", "Thuyết Minh"]);
        assert_eq!(d.groups[0].episodes.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), ["Tập 1", "Tập 2"]);
        assert_eq!(d.groups[0].episodes[0].url, format!("{O}/phim/tien-nghich/tap-01?sv=0"));
        assert_eq!(d.groups[1].episodes[0].url, format!("{O}/phim/tien-nghich/tap-01?sv=1"));
    }

    #[test]
    fn falls_back_to_kk_phim_api() {
        assert_eq!(
            fallback(&format!("{O}/api/movies/newest?page=2&limit=24")).as_deref(),
            Some("https://phimapi.com/v1/api/danh-sach/phim-moi-cap-nhat?page=2&limit=24")
        );
        assert_eq!(
            fallback(&format!("{O}/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24")).as_deref(),
            Some("https://phimapi.com/v1/api/danh-sach/hoat-hinh?country=trung-quoc&page=1&limit=24")
        );
        assert_eq!(
            fallback(&format!("{O}/api/movies/the-loai/co-trang?page=1&limit=24")).as_deref(),
            Some("https://phimapi.com/v1/api/the-loai/co-trang?page=1&limit=24")
        );
        assert_eq!(
            fallback(&format!("{O}/api/movies/search?keyword=ti%C3%AAn&page=1&limit=24")).as_deref(),
            Some("https://phimapi.com/v1/api/tim-kiem?keyword=ti%C3%AAn&page=1&limit=24")
        );
        assert_eq!(fallback(&format!("{O}/api/movies/detail/tien-nghich")).as_deref(), Some("https://phimapi.com/phim/tien-nghich"));
        assert!(fallback(&format!("{O}/api/movies/trending")).is_none());
    }

    #[test]
    fn film_url_from_episode() {
        assert_eq!(film_url(&format!("{O}/phim/tien-nghich/tap-01?sv=1")).ok(), Some(format!("{O}/phim/tien-nghich")));
        assert_eq!(film_url(&format!("{O}/phim/tien-nghich")).ok(), Some(format!("{O}/phim/tien-nghich")));
        assert!(is_known(&format!("{O}/")));
        assert_eq!(search_url(O, " tiên nghịch "), format!("{O}/api/movies/search?keyword=ti%C3%AAn+ngh%E1%BB%8Bch&page=1&limit=24"));
        assert!(matches!(film_url("https://x.test/khac"), Err(Error::App(m)) if m == "Địa chỉ phim lạ: https://x.test/khac"));
    }

    #[test]
    fn titles_and_imdb() {
        assert_eq!(title("Tập 01"), "Tập 1");
        assert_eq!(title("164"), "Tập 164");
        assert_eq!(title("000"), "Tập 0");
        assert_eq!(title("Full"), "Full");
        let i = imdb_of(Some(&obj(r#"{"vote_average":8.5,"vote_count":"378000"}"#))).expect("có điểm");
        assert_eq!(i.detail(), "IMDb 8.5 (378 nghìn lượt chấm)");
        assert!(imdb_of(Some(&obj(r#"{"vote_average":10,"vote_count":3}"#))).is_none());
        assert_eq!(Imdb::votes_text(1_250_000), "1,3 triệu");
        assert_eq!(Imdb::votes_text(2_000_000), "2 triệu");
        assert_eq!(Imdb::votes_text(950), "950");
        assert_eq!(genre_name("vo-thuat"), "võ thuật");
        assert_eq!(country_name("trung-quoc"), "Trung Quốc");
        assert_eq!(genre_name("la-la"), "la la");
        assert!(!genre_list().iter().any(|g| g.1 == "phim-18"));
    }

    // ---- hoathinh3d.pw: danh sách nạp bằng API JSON — cùng ca với HalimApiTests (ApiAndNextTests.cs) / HalimApiTests.swift ----

    const PAGE_HTML: &str = r#"
    <main id="main-contents"><div class="mv-grid" id="mvGrid"></div></main>
    <script>(function(){ const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/halim\/v1\/movies"; const PER_PAGE = 20;
    const SORTBY = "default"; })();</script>
    "#;

    #[test]
    fn halim_finds_api_in_page() {
        assert_eq!(
            halim_api::first_page(PAGE_HTML).as_deref(),
            Some("https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=1&per_page=20")
        );
        assert_eq!(
            halim_api::first_page(r#"const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/halim\/v1\/ongoing";"#).as_deref(),
            Some("https://hoathinh3d.pw/wp-json/halim/v1/ongoing?page=1&per_page=20")
        );
        assert!(halim_api::first_page(r#"<div class="halim-item"></div>"#).is_none());
        assert_eq!(
            halim_api::schedule_api(r#"const API_URL = "https:\/\/hoathinh3d.pw\/wp-json\/movie-schedule\/v1\/daily";"#).as_deref(),
            Some("https://hoathinh3d.pw/wp-json/movie-schedule/v1/daily")
        );
    }

    #[test]
    fn halim_parses_items_and_next_page() {
        let json = r#"
        {"success":true,"page":1,"per_page":20,"total":33,"has_more":true,"items":[
        {"id":1,"title":"Tiên Nghịch","permalink":"https:\/\/hoathinh3d.pw\/tien-nghich","poster_url":"https:\/\/x\/a.jpg","average_rating":4.6,"latest_episode":"Tập 165"},
        {"id":2,"rank":2,"title":"Mục Thần Ký","permalink":"https:\/\/hoathinh3d.pw\/muc-than-ky","poster_url":"https:\/\/x\/b.jpg"},
        {"id":3,"title":"Phim","permalink":"https:\/\/hoathinh3d.pw\/phim","poster_url":"","latest_episode":"12"}]}
        "#;
        let url = "https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=1&per_page=20";
        let p = halim_api::parse(json.as_bytes(), url);
        assert_eq!(titles(&p.films), ["Tiên Nghịch", "Mục Thần Ký", "Phim"]);
        assert_eq!(p.films.iter().map(|f| f.badge.as_deref()).collect::<Vec<_>>(), [Some("Tập 165"), Some("Top 2"), Some("Tập 12")]);
        assert!(p.films[2].poster.is_none());
        assert_eq!(p.next_url.as_deref(), Some("https://hoathinh3d.pw/wp-json/halim/v1/movies?sortby=default&page=2&per_page=20"));
        let last = json.replace("\"has_more\":true", "\"has_more\":false");
        assert!(halim_api::parse(last.as_bytes(), url).next_url.is_none());
    }

    #[test]
    fn halim_parses_schedule_day() {
        let json = r#"
        {"success":true,"day":"thu-nam","early_movies":[{"title":"Tru Tiên","permalink":"https:\/\/h\/tru-tien",
        "thumbnail":"https:\/\/x\/t.jpg","latest_episode":"Tập 91","is_early_screening":true,"early_screening_time":"18:00"}],
        "regular_movies":[{"title":"A","permalink":"https:\/\/h\/a","thumbnail":"","latest_episode":"Tập 17"}]}
        "#;
        let films = halim_api::parse_day(json.as_bytes());
        assert_eq!(titles(&films), ["Tru Tiên", "A"]);
        assert_eq!(films[0].badge.as_deref(), Some("18:00 · Tập 91"));
        assert_eq!(films[0].poster.as_deref(), Some("https://x/t.jpg"));
    }

    #[test]
    fn futures_are_send() {
        fn send<T: Send>(_: T) {}
        send(probe(""));
        send(home(""));
        send(json(""));
        send(list(""));
        send(detail(""));
        send(source(""));
        let blocks: Vec<(String, String)> = Vec::new();
        send(sections(&blocks, |u| async move { list(&u).await.map(|p| p.films) }));
    }
}
