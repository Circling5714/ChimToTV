//! CLB Phim Xưa qua add-on Stremio «K20 Phim Tổng Hợp» (sc.k-20.xyz, từ 1.4) — bản Android: Stremio.kt, iPad: Stremio.swift,
//! Windows: Stremio.cs (giữ cùng tên hàm). Giao thức Stremio: /catalog/movie/clbpx-movie[/genre=…&amp;search=…&amp;skip=N].json
//! (30 phim mỗi lần), /meta/movie/clbpx:&lt;slug&gt;.json (videos = các tập; phim lẻ không có), /stream/movie/&lt;mã tập&gt;.json
//! (link MP4 qua máy chủ trung gian của k-20: mở chậm, tua xa có khi không được). Chỉ trả dữ liệu cho User-Agent kiểu trình
//! duyệt.
//!
//! Địa chỉ trong app: danh sách = địa chỉ catalog; phim = https://sc.k-20.xyz/clbpx/&lt;slug&gt;; tập = …/clbpx/&lt;slug&gt;/&lt;mã tập&gt;
//! (phim lẻ: …/clbpx/&lt;slug&gt;/full).

use crate::json;
use crate::models::*;
use crate::site::{self, distinct_by, jstr, json_object, ne};
use crate::util::{after_last, Error, Result, Rx};
use crate::{api_site, http};
use std::collections::HashMap;

pub const HOST: &str = "sc.k-20.xyz";

/// Tên hiện trong app (tên miền của add-on không nói lên gì).
pub const NAME: &str = "CLB Phim Xưa";

const TYPE: &str = "movie";
const CATALOG_ID: &str = "clbpx-movie";
const ID_PREFIX: &str = "clbpx";
const FULL: &str = "full";

/// Thể loại của catalog (manifest: extra genre). Mục đầu = mới cập nhật.
const GENRE_LIST: &[&str] = &[
    "Mới Cập Nhật", "Kiếm Hiệp", "Tiên Hiệp", "Tâm Lý", "Ma Kinh Dị", "Điện Ảnh Châu Á", "Điện Ảnh Âu Mỹ", "Hàn Quốc", "Anime",
    "TV Series", "Thập Niên 60", "Thập Niên 70", "Thập Niên 80", "Thập Niên 90", "Thập Niên 2000",
];

pub fn is_site(url: &str) -> bool {
    site::host_of(url).as_deref() == Some(HOST)
}

pub async fn home(base_url: &str) -> Result<Home> {
    let o = api_site::origin(base_url);
    let blocks: Vec<(String, String)> = ["Mới cập nhật", "Kiếm Hiệp", "TV Series", "Thập Niên 90", "Thập Niên 80"]
        .iter()
        .map(|t| (t.to_string(), catalog(&o, if *t == "Mới cập nhật" { None } else { Some(*t) }, None, 0)))
        .collect();
    let sections = api_site::sections(&blocks, |u| async move { Ok::<_, Error>(parse_list(&get_data(&u).await?, &u).films) }).await?;
    Ok(Home { sections, categories: menu_of(base_url), moved_to: None })
}

pub fn menu_of(base_url: &str) -> Vec<Category> {
    let o = api_site::origin(base_url);
    GENRE_LIST[1..].iter().map(|g| Category::new(*g, catalog(&o, Some(*g), None, 0))).collect()
}

pub fn newest_url(base_url: &str) -> String {
    catalog(&api_site::origin(base_url), None, None, 0)
}

pub fn search_url(base_url: &str, query: &str) -> String {
    catalog(&api_site::origin(base_url), None, Some(query.trim()), 0)
}

/// (thể loại, quốc gia — không có).
pub fn filters(base_url: &str) -> (Vec<Category>, Vec<Category>) {
    let o = api_site::origin(base_url);
    (GENRE_LIST.iter().map(|g| Category::new(*g, catalog(&o, Some(*g), None, 0))).collect(), Vec::new())
}

/// Địa chỉ catalog; tham số kiểu Stremio: /catalog/movie/clbpx-movie/genre=…&amp;skip=30.json.
pub fn catalog(origin: &str, genre: Option<&str>, search: Option<&str>, skip: i64) -> String {
    let mut extra = Vec::new();
    if let Some(g) = genre {
        extra.push(format!("genre={}", enc(&format!("Thể loại: {g}"))));
    }
    if let Some(s) = search {
        extra.push(format!("search={}", enc(s)));
    }
    if skip > 0 {
        extra.push(format!("skip={skip}"));
    }
    let tail = if extra.is_empty() { String::new() } else { format!("/{}", extra.join("&")) };
    format!("{origin}/catalog/{TYPE}/{CATALOG_ID}{tail}.json")
}

pub async fn list(url: &str) -> Result<FilmPage> {
    Ok(parse_list(&get_data(url).await?, url))
}

static TAP_COUNT: Rx = Rx::new(r"(?i)(\d+)\s*Tập\s*$");

/// {metas:[{id:"clbpx:&lt;slug&gt;", name, poster}]}; trang sau = skip + số phim (hết khi rỗng; tìm kiếm chỉ một trang).
pub fn parse_list(data: &[u8], url: &str) -> FilmPage {
    let o = api_site::origin(url);
    let root = json_object(data).unwrap_or_default();
    let prefix = format!("{ID_PREFIX}:");
    let films = json::objects(json::arr(Some(&root), "metas")).into_iter().filter_map(|m| {
        let id = jstr(m, "id");
        let name = ne(&jstr(m, "name"))?;
        let slug = id.strip_prefix(&prefix)?;
        if slug.is_empty() {
            return None;
        }
        let poster = jstr(m, "poster");
        let poster = poster.starts_with("http").then_some(poster);
        let count = TAP_COUNT.group(1, &name);
        Some(Film::new(format!("{o}/{ID_PREFIX}/{slug}"), name, poster, count.map(|c| format!("{c} tập"))))
    });
    let films = distinct_by(films, |f| f.url.clone());
    let extra = extra_of(url);
    let mut next = None;
    if !films.is_empty() && !extra.contains_key("search") {
        let skip = extra.get("skip").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0) + films.len() as i64;
        let g = extra.get("genre").map(|g| g.strip_prefix("Thể loại: ").unwrap_or(g).to_string());
        next = Some(catalog(&o, g.as_deref(), None, skip));
    }
    FilmPage { films, next_url: next, sections: Vec::new() }
}

/// Tham số trong đoạn cuối địa chỉ catalog: "genre=Th%E1…&amp;skip=30.json" → {genre: "Thể loại: …", skip: "30"}.
fn extra_of(url: &str) -> HashMap<String, String> {
    let mut last = url::Url::parse(url).map(|u| after_last(u.path(), '/').to_string()).unwrap_or_default();
    if let Some(t) = last.strip_suffix(".json") {
        last = t.to_string();
    }
    let mut out = HashMap::new();
    if !last.contains('=') {
        return out;
    }
    for p in last.split('&') {
        if let Some((k, v)) = p.split_once('=') {
            out.insert(k.to_string(), unescape(v));
        }
    }
    out
}

/// Giải mã %XX (Uri.UnescapeDataString: "+" giữ nguyên).
fn unescape(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().into_owned()
}

pub async fn detail(url: &str) -> Result<FilmDetail> {
    let film = film_url(url)?;
    parse_detail(&get_data(&meta_url(&film)).await?, &film)
}

static LEADING_ZERO: Rx = Rx::new(r"(?i)^(Tập\s*)0+(\d)");

/// {meta:{name, poster, description, releaseInfo, genres, videos:[{id, title, season, episode}]}}.
pub fn parse_detail(data: &[u8], url: &str) -> Result<FilmDetail> {
    let root = json_object(data);
    let m = json::obj(root.as_ref(), "meta").ok_or_else(|| Error::app("Không đọc được thông tin phim"))?;
    let prefix = format!("{}:", jstr(m, "id"));
    let mut videos = json::objects(json::arr(Some(m), "videos"));
    videos.sort_by_key(|v| (json::int(Some(*v), "season", 1), json::int(Some(*v), "episode", 0)));
    let eps = if videos.is_empty() {
        vec![Episode::new("Xem phim", format!("{url}/{FULL}"))]
    } else {
        let all = videos.into_iter().filter_map(|v| {
            let id = jstr(v, "id");
            let rest = id.strip_prefix(&prefix)?;
            if rest.is_empty() {
                return None;
            }
            let t = ne(&jstr(v, "title")).unwrap_or_else(|| format!("Tập {}", json::int(Some(v), "episode", 0)));
            Some(Episode::new(LEADING_ZERO.replace(&t, "${1}${2}"), format!("{url}/{}", enc(rest))))
        });
        distinct_by(all, |e| e.url.clone())
    };
    let genres = json::arr(Some(m), "genres").map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    let info = [jstr(m, "releaseInfo"), genres].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("  •  ");
    let poster = jstr(m, "poster");
    Ok(FilmDetail {
        url: url.to_string(),
        title: ne(&jstr(m, "name")).unwrap_or_else(|| url.to_string()),
        other_title: None,
        poster: poster.starts_with("http").then_some(poster),
        description: ne(&jstr(m, "description")),
        info: ne(&info),
        groups: vec![EpisodeGroup { name: "Tập phim".into(), episodes: eps }],
    })
}

/// Mỗi link MP4 là một server: "720p", "1080p · 3,1 GB"… (cùng chất lượng có nhiều bản thì ghi dung lượng).
pub async fn source(ep_url: &str) -> Result<EpisodeSource> {
    let film = film_url(ep_url)?;
    let path = url::Url::parse(ep_url).map(|u| u.path().to_string()).unwrap_or_default();
    let code = unescape(after_last(path.trim_end_matches('/'), '/'));
    let slug = after_last(&film, '/');
    let id = if code == FULL { format!("{ID_PREFIX}:{slug}") } else { format!("{ID_PREFIX}:{slug}:{code}") };
    let data = get_data(&format!("{}/stream/{TYPE}/{}.json", api_site::origin(&film), enc(&id))).await?;
    parse_source(&data, ep_url)
}

static SIZE_RX: Rx = Rx::new(r"[?&]size=(\d+)");

pub fn parse_source(data: &[u8], ep_url: &str) -> Result<EpisodeSource> {
    let root = json_object(data);
    let streams: Vec<_> = json::objects(json::arr(root.as_ref(), "streams")).into_iter().filter(|s| jstr(s, "url").starts_with("http")).collect();
    if streams.is_empty() {
        return Err(Error::app("Tập này chưa có video"));
    }
    let quality: Vec<String> = streams
        .iter()
        .map(|s| ne(jstr(s, "title").split('\n').next().unwrap_or("").trim()).unwrap_or_else(|| "MP4".into()))
        .collect();
    let mut servers = Vec::new();
    let mut links = HashMap::new();
    for (i, s) in streams.iter().enumerate() {
        let url = jstr(s, "url");
        let size = SIZE_RX.group(1, &url).and_then(|b| b.parse::<f64>().ok());
        let dup = quality.iter().filter(|q| **q == quality[i]).count() > 1;
        let label = match size {
            Some(b) if dup => format!("{} · {} GB", quality[i], gb(b)),
            _ => quality[i].clone(),
        };
        let kind = format!("mp4-{i}");
        servers.push(Server::new(label, kind.clone()));
        links.insert(kind, Video { url, subtitle: None, file: true });
    }
    Ok(EpisodeSource { ep_url: ep_url.to_string(), servers, links, next: true, ..Default::default() })
}

/// Dung lượng GB một chữ số lẻ, dấu phẩy thập phân ("3,1"); làm tròn nửa lên như "0.0" của C#.
fn gb(bytes: f64) -> String {
    let x = (bytes / 1e9 * 10.0).round() / 10.0;
    format!("{x:.1}").replace('.', ",")
}

/// ".../clbpx/nch1993/LM4bPc-3a" → ".../clbpx/nch1993".
pub fn film_url(url: &str) -> Result<String> {
    let parts: Vec<String> = url::Url::parse(url)
        .map(|u| u.path().split('/').filter(|p| !p.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    match parts.iter().position(|p| p == ID_PREFIX) {
        Some(i) if i + 1 < parts.len() => Ok(format!("{}/{ID_PREFIX}/{}", api_site::origin(url), parts[i + 1])),
        _ => Err(Error::App(format!("Địa chỉ phim lạ: {url}"))),
    }
}

fn meta_url(film: &str) -> String {
    format!("{}/meta/{TYPE}/{}.json", api_site::origin(film), enc(&format!("{ID_PREFIX}:{}", after_last(film, '/'))))
}

async fn get_data(url: &str) -> Result<Vec<u8>> {
    match http::get(url, &[("Accept", "application/json")]).await {
        Ok(r) => Ok(r.data),
        Err(Error::Http(404)) => Err(Error::app("CLB Phim Xưa không có phim / tập này")),
        Err(e) => Err(e),
    }
}

/// Mã hoá chặt (chỉ giữ chữ / số ASCII và "-._*"), dấu cách = %20 — như URLEncoder của bản Android + "%20".
pub fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        let c = b as char;
        if b.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '*') {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    //! Các ca của Stremio nằm cùng NguonCTests.cs → xem nguon_c.rs (tests); ở đây thêm vài ca nhỏ.
    use super::*;

    #[test]
    fn encodes_and_reads_extra() {
        assert_eq!(enc("Thể loại: a b"), "Th%E1%BB%83%20lo%E1%BA%A1i%3A%20a%20b");
        let e = extra_of("https://sc.k-20.xyz/catalog/movie/clbpx-movie/genre=Th%E1%BB%83%20lo%E1%BA%A1i%3A%20Anime&skip=30.json");
        assert_eq!(e.get("genre").map(String::as_str), Some("Thể loại: Anime"));
        assert_eq!(e.get("skip").map(String::as_str), Some("30"));
        assert!(extra_of("https://sc.k-20.xyz/catalog/movie/clbpx-movie.json").is_empty());
        assert_eq!(gb(3_069_863_340.0), "3,1");
        assert!(matches!(film_url("https://sc.k-20.xyz/khac"), Err(Error::App(_))));
        assert_eq!(menu_of("https://sc.k-20.xyz").len(), GENRE_LIST.len() - 1);
    }

    #[test]
    fn futures_are_send() {
        fn send<T: Send>(_: T) {}
        send(home(""));
        send(list(""));
        send(detail(""));
        send(source(""));
    }
}
