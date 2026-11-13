//! Server dự phòng KKPhim (API công khai phimapi.com) cho các trang tự có trình phát — bản Android: KkPhim.kt, iPad:
//! KkPhim.swift, Windows: KkPhim.cs (giữ cùng tên hàm để sửa song song). yanhh3d, hoathinh3d.ee, hhkungfu nhúng chung
//! streamfree.vip — hỏng là hỏng cả ba. Mở một tập thì tìm đúng phim đó trên KKPhim (slug của trang, không khớp thì tìm theo
//! tên; tên phim hoặc tên gốc phải trùng), lấy tập cùng số rồi thêm server "KKPhim Vietsub / Thuyết Minh" (luồng HLS — app tự
//! phát, bỏ quảng cáo) vào bảng tuỳ chọn. Trang chia bản (Việt Sub / Thuyết Minh) thì chỉ thêm đúng ngôn ngữ của bản đang
//! xem. Trang lỗi hẳn mà KKPhim có tập này thì vẫn xem được bằng KKPhim. Trang đã có sẵn KKPhim (mephim / kkphim — api_site,
//! phimmoi — next_site) không thêm.

use crate::json::{self, Obj};
use crate::models::*;
use crate::site::{jstr, json_object, lock, ne};
use crate::util::{after_last, form_encoded, now_ms, take_chars, Error, Result, Rx};
use crate::{api_site, http, next_site, nguon_c, prefs, site};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;

pub const API: &str = "https://phimapi.com";

/// Đầu tên loại server KKPhim, vd "kk-vs", "kk-tm" (server quen lưu theo tên này).
pub const PREFIX: &str = "kk-";

/// Trang xong thì chờ nguồn dự phòng thêm tới mốc này (ms, tính từ lúc bắt đầu) — combine / combine_all.
pub const WAIT_MS: i64 = 6_000;
/// Trang lỗi thì chờ nguồn dự phòng tới mốc này (ms) — lúc đó dự phòng là đường duy nhất.
pub const FAIL_WAIT_MS: i64 = 15_000;

const TTL: i64 = 10 * 60_000; // chi tiết phim (danh sách tập) giữ 10 phút
const MISS_TTL: i64 = 30 * 60_000; // phim không có trên KKPhim: 30 phút sau mới hỏi lại

/// Một nguồn dự phòng đang chờ: danh sách (server, link) — chạy ở nền (tokio::spawn) nên phải 'static + Send.
pub type Backup = Pin<Box<dyn Future<Output = Result<Vec<(Server, Video)>>> + Send>>;

struct State {
    cache: HashMap<String, (i64, Option<Obj>)>,
    /// Phim của trang → slug trên KKPhim đã tìm thấy (khỏi tìm lại theo tên).
    slugs: HashMap<String, String>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State { cache: HashMap::new(), slugs: HashMap::new() }));

/// Trang dùng nguồn dự phòng: bật trong Cài đặt và không phải trang đã có sẵn KKPhim.
pub fn applies_to(url: &str) -> bool {
    if !prefs::kk_backup() || api_site::is_known(url) || next_site::is_known(url) {
        return false;
    }
    match site::host_of(url) {
        Some(h) => !h.ends_with("phimapi.com") && !h.ends_with("kkphim.com"),
        None => false,
    }
}

/// Server dự phòng (KKPhim "kk-…", NguonC "nc-…"): link có sẵn trong EpisodeSource.links.
pub fn is_backup(kind: &str) -> bool {
    kind.starts_with(PREFIX) || kind.starts_with(nguon_c::PREFIX)
}

/// Trang tập + server dự phòng KKPhim và NguonC, tải song song. Nguồn dự phòng chậm / lỗi thì thôi; trang tập lỗi mà
/// nguồn dự phòng có tập này → chỉ phát bằng nguồn dự phòng.
pub async fn source(d: &FilmDetail, group: Option<&EpisodeGroup>, ep: &Episode) -> Result<EpisodeSource> {
    let mut backups: Vec<Backup> = Vec::new();
    if applies_to(&d.url) {
        let (d, g, e) = (d.clone(), group.cloned(), ep.clone());
        backups.push(Box::pin(async move { servers(&d, g.as_ref(), &e).await }));
    }
    if nguon_c::applies_to(&d.url) {
        let (d, g, e) = (d.clone(), group.cloned(), ep.clone());
        backups.push(Box::pin(async move { nguon_c::servers(&d, g.as_ref(), &e).await }));
    }
    if backups.is_empty() {
        return site::source(&ep.url).await;
    }
    combine_all(&ep.url, site::source(&ep.url), backups, WAIT_MS, FAIL_WAIT_MS).await
}

/// Một nguồn dự phòng (combine_all với một nguồn).
pub async fn combine(
    ep_url: &str,
    site: impl Future<Output = Result<EpisodeSource>>,
    kk: Backup,
    wait_ms: i64,
    fail_wait_ms: i64,
) -> Result<EpisodeSource> {
    combine_all(ep_url, site, vec![kk], wait_ms, fail_wait_ms).await
}

/// Chạy song song trang tập (site) và các nguồn dự phòng (mỗi nguồn một việc nền). Trang xong thì chờ nguồn dự phòng thêm
/// tới wait_ms tính từ lúc bắt đầu; trang lỗi thì chờ tới fail_wait_ms (lúc đó dự phòng là đường duy nhất). Nguồn nào chưa
/// xong / lỗi thì bỏ, nguồn đã xong vẫn dùng. Cần chạy trong tokio (app: `cbtv::rt()`).
pub async fn combine_all(
    ep_url: &str,
    site: impl Future<Output = Result<EpisodeSource>>,
    backups: Vec<Backup>,
    wait_ms: i64,
    fail_wait_ms: i64,
) -> Result<EpisodeSource> {
    let start = now_ms();
    let tasks: Vec<_> = backups.into_iter().map(|b| tokio::spawn(async move { b.await.unwrap_or_default() })).collect();
    let src = site.await;
    let deadline = start + if src.is_ok() { wait_ms } else { fail_wait_ms };
    let mut extra = Vec::new();
    for mut t in tasks {
        let left = (deadline - now_ms()).max(1) as u64;
        // Quá hạn thì bỏ (việc nền vẫn chạy nốt, như bản Windows)
        if let Ok(Ok(list)) = tokio::time::timeout(Duration::from_millis(left), &mut t).await {
            extra.extend(list);
        }
    }
    match src {
        Ok(s) => Ok(attach(s, extra)),
        Err(e) => {
            if extra.is_empty() {
                return Err(e);
            }
            let links = extra.iter().map(|(s, v)| (s.kind.clone(), v.clone())).collect();
            Ok(EpisodeSource {
                ep_url: ep_url.to_string(),
                servers: extra.into_iter().map(|x| x.0).collect(),
                links,
                ..Default::default()
            })
        }
    }
}

/// Thêm server KKPhim sau các server của trang. Trang mở thẳng trang tập (page_mode, vd hoathinh3d.pw) thì trình phát
/// của trang thành một server ("Trình phát của trang") để chọn qua lại được.
pub fn attach(src: EpisodeSource, extra: Vec<(Server, Video)>) -> EpisodeSource {
    if extra.is_empty() || src.next {
        return src;
    }
    let mut src = src;
    let own = if src.page_mode {
        vec![Server::new("Trình phát của trang", EpisodeSource::PAGE_SERVER)]
    } else {
        std::mem::take(&mut src.servers)
    };
    for (s, v) in &extra {
        src.links.insert(s.kind.clone(), v.clone());
    }
    src.servers = own.into_iter().chain(extra.into_iter().map(|x| x.0)).collect();
    src
}

/// Server KKPhim cho một tập (rỗng nếu KKPhim không có phim / tập này). Lỗi mạng → báo lỗi.
pub async fn servers(d: &FilmDetail, group: Option<&EpisodeGroup>, ep: &Episode) -> Result<Vec<(Server, Video)>> {
    let num = site::ep_number(&ep.title).or_else(|| site::ep_number(after_last(ep.url.trim_end_matches('/'), '/')));
    let Some(num) = num else { return Ok(Vec::new()) };
    let want = group.and_then(|g| lang(&g.name));
    Ok(match detail(d).await? {
        Some(detail) => pick(&detail, num, want),
        None => Vec::new(),
    })
}

/// Server có tập số num trong chi tiết phim KKPhim ({movie, episodes:[{server_name, is_ai, server_data:[{name, slug,
/// link_m3u8}]}]}). lang: ngôn ngữ của bản đang xem ("vs" / "tm" / "lt"; None = trang không chia bản → mọi ngôn ngữ).
pub fn pick(root: &Obj, num: f64, lang: Option<&str>) -> Vec<(Server, Video)> {
    let mut out = Vec::new();
    let mut used = HashSet::new();
    for s in json::objects(json::arr(Some(root), "episodes")) {
        let l = self::lang(&jstr(s, "server_name")).unwrap_or("vs");
        if lang.is_some_and(|w| w != l) {
            continue;
        }
        let e = json::objects(json::arr(Some(s), "server_data"))
            .into_iter()
            .find(|x| site::ep_number(&jstr(x, "name")).or_else(|| site::ep_number(&jstr(x, "slug"))) == Some(num));
        let Some(e) = e else { continue };
        let m3u8 = jstr(e, "link_m3u8");
        if !m3u8.starts_with("http") {
            continue;
        }
        let ai = json::bool(Some(s), "is_ai", false);
        let b = format!("{l}{}", if ai { "-ai" } else { "" });
        let mut kind = format!("{PREFIX}{b}");
        let mut n = 1;
        while !used.insert(kind.clone()) {
            n += 1;
            kind = format!("{PREFIX}{b}-{n}");
        }
        let name = match l {
            "tm" => "Thuyết Minh",
            "lt" => "Lồng Tiếng",
            _ => "Vietsub",
        };
        let label = format!("KKPhim {name}{}{}", if ai { " (AI)" } else { "" }, if n > 1 { format!(" {n}") } else { String::new() });
        out.push((Server::new(label, kind), Video::new(m3u8)));
    }
    out
}

static TM_WORD: Rx = Rx::new(r"\btm\b");

/// Ngôn ngữ ghi trong tên server / tên bản ("vs" / "tm" / "lt"); None = không rõ — như PlayStats.lang của bản TV.
pub fn lang(label: &str) -> Option<&'static str> {
    let s = plain(label);
    if s.contains("thuyet minh") || TM_WORD.matches(&s) {
        return Some("tm");
    }
    if s.contains("long tieng") {
        return Some("lt");
    }
    if s.contains("sub") {
        return Some("vs");
    }
    None
}

/// Bỏ dấu, chữ thường: "Thuyết minh 1080 V1" → "thuyet minh 1080 v1".
fn plain(s: &str) -> String {
    let d: String = s.replace('đ', "d").replace('Đ', "D").nfd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).collect();
    d.nfc().collect::<String>().to_lowercase()
}

// ---- Tìm phim trên KKPhim ----

async fn detail(d: &FilmDetail) -> Result<Option<Obj>> {
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

/// Chi tiết phim KKPhim khớp với phim của trang; None = KKPhim không có.
async fn find(d: &FilmDetail) -> Result<Option<Obj>> {
    let mine: HashSet<String> = [Some(&d.title), d.other_title.as_ref()].into_iter().flatten().map(|s| norm(s)).filter(|s| !s.is_empty()).collect();
    if mine.is_empty() {
        return Ok(None);
    }
    let known = lock(&STATE).slugs.get(&d.url).cloned();
    if let Some(k) = known {
        if let Some(m) = movie(&k).await? {
            return Ok(Some(m));
        }
    }
    // Slug của trang thường trùng KKPhim (tien-nghich, dau-pha-thuong-khung-phan-5…); chỉ nhận phim hoạt hình ở bước này
    if let Some(own) = slug_of(&d.url) {
        if let Some(m) = movie(&own).await? {
            let ok = json::obj(Some(&m), "movie").is_some_and(|mv| jstr(mv, "type") == "hoathinh" && matches(mv, &mine));
            if ok {
                remember(&d.url, &own);
                return Ok(Some(m));
            }
        }
    }
    let q = format!("{API}/v1/api/tim-kiem?keyword={}&limit=20", form_encoded(&take_chars(&d.title, 80)));
    let root = get_json(&q).await?;
    let items = json::objects(json::arr(json::obj(Some(&root), "data"), "items"));
    let slug = choose(&items, &mine).and_then(|hit| ne(&jstr(hit, "slug")));
    let Some(slug) = slug else { return Ok(None) };
    let r = movie(&slug).await?;
    if r.is_some() {
        remember(&d.url, &slug);
    }
    Ok(r)
}

/// Kết quả tìm khớp tên (phim hoạt hình trước — tìm kiếm của KKPhim có thể ra phim người đóng trùng tên).
pub fn choose<'a>(items: &[&'a Obj], mine: &HashSet<String>) -> Option<&'a Obj> {
    let hits: Vec<&Obj> = items.iter().copied().filter(|i| matches(i, mine)).collect();
    hits.iter().copied().find(|i| jstr(i, "type") == "hoathinh").or_else(|| hits.first().copied())
}

fn remember(film: &str, slug: &str) {
    lock(&STATE).slugs.insert(film.to_string(), slug.to_string());
}

/// Chi tiết phim trên KKPhim; None nếu không có slug này (KKPhim trả 404).
async fn movie(slug: &str) -> Result<Option<Obj>> {
    match get_json(&format!("{API}/phim/{}", form_encoded(slug))).await {
        Ok(o) => Ok((json::bool(Some(&o), "status", false) && json::obj(Some(&o), "movie").is_some()).then_some(o)),
        Err(Error::Http(404)) => Ok(None),
        Err(e) => Err(e),
    }
}

async fn get_json(url: &str) -> Result<Obj> {
    let r = http::get(url, &[("Accept", "application/json")]).await?;
    json_object(&r.data).ok_or_else(|| Error::app("KKPhim trả dữ liệu lạ"))
}

static SLUG_RX: Rx = Rx::new(r"^[a-z0-9-]+$");

/// Slug của phim trong địa chỉ trang: ".../tu-tien/tien-nghich" → "tien-nghich".
pub fn slug_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let mut s = after_last(u.path().trim_end_matches('/'), '/');
    if let Some(t) = s.strip_suffix(".html") {
        s = t;
    }
    SLUG_RX.matches(s).then(|| s.to_string())
}

pub fn matches(m: &Obj, mine: &HashSet<String>) -> bool {
    !names(m).is_disjoint(mine)
}

static PAREN: Rx = Rx::new(r"^(.+?)\s*\((.+)\)\s*$");
static DIGITS: Rx = Rx::new(r"^\d+$");

/// Các tên của một phim KKPhim (đã chuẩn hoá): tên, tên gốc; tên có phần trong ngoặc thì thêm phần ngoài / trong ngoặc
/// ("Thần Y Cổ Đại Ở Đô Thị (Đô Thị Cổ Tiên Y)") — trừ khi trong ngoặc chỉ là số phần ("Đấu Phá Thương Khung (Phần 5)").
pub fn names(m: &Obj) -> HashSet<String> {
    let mut out = HashSet::new();
    // NguonC: original_name có thể gồm nhiều tên cách nhau dấu phẩy ("Mu Shen Ji, Tales of Herding Gods")
    let original = jstr(m, "original_name");
    let others = original.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    for raw in [jstr(m, "name"), jstr(m, "origin_name")].into_iter().chain(others) {
        out.insert(norm(&raw));
        if let Some(g) = PAREN.groups(&raw) {
            let inner = norm(&g[2]);
            if !DIGITS.matches(&inner) {
                out.insert(norm(&g[1]));
                out.insert(inner);
            }
        }
    }
    out.remove("");
    out
}

static NON_ALNUM: Rx = Rx::new(r"[^a-z0-9]+");
static SEASON: Rx = Rx::new(r"\b(phan|season|mua|part|ss)\s*(\d+)");
static SPACES: Rx = Rx::new(r"\s+");

/// Tên để so: bỏ dấu, chữ thường, chỉ chữ + số; "Phần 5" / "Season 5" / "SS5" → "5".
pub fn norm(s: &str) -> String {
    let t = NON_ALNUM.replace(&plain(s), " ");
    let t = SEASON.replace(&t, "${2}");
    SPACES.replace(&t, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    //! Server dự phòng KKPhim + trang KKPhim (11/2026) — cùng ca với KkPhimTests.cs / KkPhimTest.kt / KkPhimTests.swift.
    use super::*;

    fn obj(s: &str) -> Obj {
        json::parse_object(s.as_bytes()).expect("JSON đối tượng")
    }

    fn movie_obj(name: &str, origin: &str, kind: &str, slug: &str) -> Obj {
        let v = serde_json::json!({ "name": name, "origin_name": origin, "type": kind, "slug": slug });
        v.as_object().cloned().unwrap_or_default()
    }

    fn mv(name: &str) -> Obj {
        movie_obj(name, "", "hoathinh", "x")
    }

    fn mine(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| norm(s)).collect()
    }

    fn kinds(list: &[(Server, Video)]) -> Vec<&str> {
        list.iter().map(|x| x.0.kind.as_str()).collect()
    }

    fn src_kinds(s: &EpisodeSource) -> Vec<&str> {
        s.servers.iter().map(|x| x.kind.as_str()).collect()
    }

    #[test]
    fn normalizes_seasons_and_accents() {
        assert_eq!(norm("Đấu Phá Thương Khung Phần 5"), norm("Đấu Phá Thương Khung (Phần 5)"));
        assert_eq!(norm("Đấu Phá Thương Khung (Phần 5)"), "dau pha thuong khung 5");
        assert_eq!(norm("Sword of Coming SS2"), norm("Sword of Coming (Season 2)"));
        assert_eq!(norm("Fights Break Sphere 5"), norm("Fights Break Sphere (Season 5)"));
        assert_eq!(norm("Đấu La Đại Lục 2: Tuyệt Thế Đường Môn"), norm("Đấu La Đại Lục 2 (Tuyệt Thế Đường Môn)"));
        assert_eq!(norm("Spare Me, Great Lord! Season 3"), norm("Spare Me, Great Lord! (Season 3)"));
    }

    #[test]
    fn matches_by_name_or_original_name() {
        let m = mine(&["Già Thiên", "Zhe Tian"]);
        assert!(matches(&movie_obj("Già Thiên", "Shrouding The Heavens", "hoathinh", "x"), &m));
        assert!(matches(&movie_obj("Che Trời", "Zhe Tian", "hoathinh", "x"), &m));
        assert!(!matches(&movie_obj("Già Thiên Movie", "Shrouding The Heavens Movie", "hoathinh", "x"), &m));
        let doctor = movie_obj("Thần Y Cổ Đại Ở Đô Thị (Đô Thị Cổ Tiên Y)", "Urban Miracle Doctor", "hoathinh", "x");
        assert!(matches(&doctor, &mine(&["Thần Y Cổ Đại Ở Đô Thị"])));
        assert!(matches(&doctor, &mine(&["Đô Thị Cổ Tiên Y"])));
        // Trong ngoặc chỉ là số phần: phần 5 không được ghép với phim "Đấu Phá Thương Khung" (phần 1)
        let s5 = movie_obj("Đấu Phá Thương Khung (Phần 5)", "Fights Break Sphere (Season 5)", "hoathinh", "x");
        assert!(!matches(&s5, &mine(&["Đấu Phá Thương Khung"])));
        assert!(matches(&s5, &mine(&["Đấu Phá Thương Khung Phần 5"])));
    }

    #[test]
    fn prefers_animation_among_same_name_results() {
        let m = mine(&["Già Thiên"]);
        let live = movie_obj("Già Thiên", "", "series", "gia-thien-live");
        let anim = movie_obj("Già Thiên", "", "hoathinh", "gia-thien");
        assert_eq!(choose(&[&live, &anim], &m).map(|o| jstr(o, "slug")).as_deref(), Some("gia-thien"));
        assert_eq!(choose(&[&live], &m).map(|o| jstr(o, "slug")).as_deref(), Some("gia-thien-live"));
        assert!(choose(&[&mv("Tiên Nghịch")], &m).is_none());
    }

    #[test]
    fn reads_slug_from_film_url() {
        assert_eq!(slug_of("https://yanhh3d.ee/tu-tien/tien-nghich").as_deref(), Some("tien-nghich"));
        assert_eq!(slug_of("https://hoathinh3d.ee/dau-pha-thuong-khung-phan-5/").as_deref(), Some("dau-pha-thuong-khung-phan-5"));
        assert!(slug_of("https://example.com/").is_none());
    }

    fn detail_json() -> Obj {
        obj(r##"
        {"status":true,"movie":{"name":"Tiên Nghịch","slug":"tien-nghich","type":"hoathinh"},"episodes":[
        {"server_name":"Vietsub #Hà Nội","is_ai":false,"server_data":[
          {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://a.kvp726.com/164/index.m3u8"},
          {"name":"Tập 165","slug":"tap-165","link_m3u8":"https://a.kvp726.com/165/index.m3u8"}]},
        {"server_name":"Thuyết Minh","is_ai":false,"server_data":[
          {"name":"Tập 08","slug":"tap-08","link_m3u8":"https://s6.kkphimplayer6.com/8/index.m3u8"},
          {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://s6.kkphimplayer6.com/164/index.m3u8"}]},
        {"server_name":"Thuyết Minh","is_ai":true,"server_data":[
          {"name":"Tập 164","slug":"tap-164","link_m3u8":"https://ai.example/164/index.m3u8"}]},
        {"server_name":"Lồng Tiếng","is_ai":false,"server_data":[
          {"name":"Tập 164","slug":"tap-164","link_m3u8":"","link_embed":"https://player.phimapi.com/player/?url=x"}]}
        ]}
        "##)
    }

    #[test]
    fn picks_same_episode_number_in_every_language() {
        let d = detail_json();
        let all = pick(&d, 164.0, None);
        assert_eq!(kinds(&all), ["kk-vs", "kk-tm", "kk-tm-ai"]);
        assert_eq!(all.iter().map(|x| x.0.label.as_str()).collect::<Vec<_>>(), ["KKPhim Vietsub", "KKPhim Thuyết Minh", "KKPhim Thuyết Minh (AI)"]);
        assert_eq!(all[0].1.url, "https://a.kvp726.com/164/index.m3u8");
        assert_eq!(kinds(&pick(&d, 8.0, None)), ["kk-tm"]);
        assert_eq!(kinds(&pick(&d, 165.0, None)), ["kk-vs"]);
        assert!(pick(&d, 166.0, None).is_empty());
    }

    #[test]
    fn only_language_of_the_version_being_watched() {
        let d = detail_json();
        assert_eq!(kinds(&pick(&d, 164.0, lang("Việt Sub"))), ["kk-vs"]);
        assert_eq!(kinds(&pick(&d, 164.0, lang("Thuyết Minh"))), ["kk-tm", "kk-tm-ai"]);
        assert_eq!(lang("4K Việt Sub"), Some("vs"));
        assert_eq!(lang("Thuyết minh 1080 V1"), Some("tm"));
        assert_eq!(lang("Tập phim"), None);
    }

    fn yan(ep: &str, post_id: &str) -> EpisodeSource {
        EpisodeSource {
            ep_url: ep.into(),
            servers: vec![Server::new("VIETSUB 1080 V1", "tiktik")],
            post_id: post_id.into(),
            ep: "tap-164".into(),
            sv: "1".into(),
            player_url: Some("https://yanhh3d.ee/player/player.php".into()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn attaches_after_site_servers_and_plays_from_links() {
        let kk = pick(&detail_json(), 164.0, None);
        let y = yan("https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html", "1010");
        let s = attach(y.clone(), kk.clone());
        assert_eq!(src_kinds(&s), ["tiktik", "kk-vs", "kk-tm", "kk-tm-ai"]);
        assert!(!s.page_only());
        let v = site::video_of(&s, Some(&s.servers[2])).await.expect("có link");
        assert_eq!(v.url, "https://s6.kkphimplayer6.com/164/index.m3u8");
        // Không có server dự phòng → giữ nguyên
        let same = attach(y.clone(), Vec::new());
        assert_eq!(src_kinds(&same), src_kinds(&y));
        assert!(same.links.is_empty());
        // Trang mở thẳng trang tập (hoathinh3d.pw): trình phát của trang thành server đầu tiên
        let pw = EpisodeSource { ep_url: "https://hoathinh3d.pw/xem-phim-gia-thien/tap-2-sv1.html".into(), page_mode: true, ..Default::default() };
        assert!(pw.page_only());
        let p = attach(pw, kk);
        assert_eq!(p.servers[0].kind, EpisodeSource::PAGE_SERVER);
        assert!(!p.page_only());
        assert!(p.page_mode);
    }

    fn ready(list: Vec<(Server, Video)>) -> Backup {
        Box::pin(async move { Ok(list) })
    }

    #[tokio::test]
    async fn site_down_still_plays_from_kk_and_slow_kk_is_skipped() {
        let ep = "https://yanhh3d.ee/tu-tien/tien-nghich/tap-164.html";
        let kk = pick(&detail_json(), 164.0, None);
        let site_src = yan(ep, "1");
        // Trang lỗi → chỉ còn server KKPhim, phát được
        let only = combine(ep, async { Err(Error::app("Trang trả về lỗi HTTP 522")) }, ready(kk.clone()), WAIT_MS, FAIL_WAIT_MS)
            .await
            .expect("phát được bằng KKPhim");
        assert_eq!(src_kinds(&only), ["kk-vs", "kk-tm", "kk-tm-ai"]);
        assert_eq!(site::video_of(&only, Some(&only.servers[0])).await.map(|v| v.url).ok().as_deref(), Some("https://a.kvp726.com/164/index.m3u8"));
        // Trang lỗi và KKPhim cũng không có → báo lỗi của trang
        let err = combine(ep, async { Err(Error::app("HTTP 522")) }, ready(Vec::new()), WAIT_MS, FAIL_WAIT_MS).await;
        assert!(matches!(err, Err(Error::App(m)) if m == "HTTP 522"));
        // KKPhim lỗi / quá chậm → phát bằng server của trang như cũ, không chờ mãi
        let failing: Backup = Box::pin(async { Err(Error::app("KK lỗi")) });
        let same = combine(ep, async { Ok(site_src.clone()) }, failing, WAIT_MS, FAIL_WAIT_MS).await.expect("phát được");
        assert_eq!(src_kinds(&same), ["tiktik"]);
        assert!(same.links.is_empty());
        let t0 = now_ms();
        let kk_slow = kk.clone();
        let slow_backup: Backup = Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(3_000)).await;
            Ok(kk_slow)
        });
        let slow = combine(ep, async { Ok(site_src.clone()) }, slow_backup, 300, FAIL_WAIT_MS).await.expect("phát được");
        assert_eq!(src_kinds(&slow), ["tiktik"]);
        assert!(now_ms() - t0 < 2_000);
        let all = combine(ep, async { Ok(site_src.clone()) }, ready(kk), WAIT_MS, FAIL_WAIT_MS).await.expect("phát được");
        assert_eq!(all.servers.len(), 4);
    }

    /// Trang KKPhim (1.3): địa chỉ trong app theo www.kkphim.com, dữ liệu gọi thẳng phimapi.com.
    #[test]
    fn kk_phim_site_uses_public_api() {
        let kk = prefs::KK_SITE;
        assert!(api_site::is_known(&format!("{kk}/")));
        assert!(api_site::is_kk(&format!("{kk}/phim/tien-nghich/tap-165?sv=0")));
        assert!(api_site::is_kk("https://kkphim.com"));
        assert!(!api_site::is_kk("https://mephim.ddns.net/phim/tien-nghich"));
        assert_eq!(
            api_site::fallback(&api_site::newest_url(kk)).as_deref(),
            Some("https://phimapi.com/v1/api/danh-sach/phim-moi-cap-nhat?page=1&limit=24")
        );
        assert_eq!(
            api_site::fallback(&api_site::search_url(kk, "già thiên")).as_deref(),
            Some("https://phimapi.com/v1/api/tim-kiem?keyword=gi%C3%A0+thi%C3%AAn&page=1&limit=24")
        );
        assert_eq!(
            api_site::fallback(&format!("{kk}/api/movies/category/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh&page=1&limit=24")).as_deref(),
            Some("https://phimapi.com/v1/api/danh-sach/hoat-hinh?country=trung-quoc&sort_lang=thuyet-minh&page=1&limit=24")
        );
        assert_eq!(api_site::film_url(&format!("{kk}/phim/tien-nghich/tap-165?sv=0")).ok(), Some(format!("{kk}/phim/tien-nghich")));
        let body = r#"
        {"status":true,"data":{"items":[{"name":"Đấu Phá Thương Khung (Phần 5)","slug":"dau-pha-thuong-khung-phan-5",
        "poster_url":"upload/vod/20240822-1/58a4.jpg","episode_current":"Tập 218"}],
        "params":{"pagination":{"totalItems":462,"totalItemsPerPage":24,"currentPage":1,"totalPages":20}},
        "APP_DOMAIN_CDN_IMAGE":"https://phimimg.com"}}
        "#;
        let p = api_site::parse_list(&obj(body), &format!("{kk}/api/movies/category/hoat-hinh?country=trung-quoc&page=1&limit=24"));
        assert_eq!(p.films.len(), 1);
        assert_eq!(p.films[0].url, "https://www.kkphim.com/phim/dau-pha-thuong-khung-phan-5");
        assert_eq!(p.films[0].poster.as_deref(), Some("https://phimimg.com/upload/vod/20240822-1/58a4.jpg"));
        assert_eq!(p.next_url, Some(format!("{kk}/api/movies/category/hoat-hinh?country=trung-quoc&page=2&limit=24")));
    }

    #[test]
    fn bare_kk_domain_becomes_the_www_site() {
        // kkphim.com trần bị nhà mạng chặn, và trùng với trang mẫu www.kkphim.com trong danh sách
        assert_eq!(prefs::normalize_url("kkphim.com").as_deref(), Some(prefs::KK_SITE));
        assert_eq!(prefs::normalize_url("https://KKPhim.com/").as_deref(), Some(prefs::KK_SITE));
        assert_eq!(prefs::normalize_url("www.kkphim.com").as_deref(), Some(prefs::KK_SITE));
        assert_eq!(prefs::normalize_url("yanhh3d.ee/").as_deref(), Some("https://yanhh3d.ee"));
    }

    #[test]
    fn backup_kinds() {
        assert!(is_backup("kk-vs") && is_backup("nc-tm-2") && !is_backup("tiktik"));
    }

    #[test]
    fn futures_are_send() {
        fn send<T: Send>(_: T) {}
        let d = FilmDetail::default();
        let ep = Episode::new("", "");
        send(servers(&d, None, &ep));
        send(source(&d, None, &ep));
        send(combine_all("", async { Err(Error::app("x")) }, Vec::new(), WAIT_MS, FAIL_WAIT_MS));
    }
}
