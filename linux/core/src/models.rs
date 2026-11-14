//! Dữ liệu đọc từ trang phim — giống hệt bản Android (Site.kt) / iPad (Models.swift) / Windows (Models.cs), để sửa bộ đọc
//! trang song song.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Điểm IMDb (chỉ dữ liệu KKPhim có — imdb_top.rs). Phim dưới `MIN_VOTES` lượt chấm coi như không có điểm.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Imdb {
    pub score: f64,
    pub votes: i64,
}

impl Imdb {
    pub const MIN_VOTES: i64 = 100;

    /// Nhãn trên thẻ phim, vd "IMDb 8.5".
    pub fn label(&self) -> String {
        format!("IMDb {:.1}", self.score)
    }

    /// Dòng thông tin ở trang phim, vd "IMDb 8.5 (378 nghìn lượt chấm)".
    pub fn detail(&self) -> String {
        format!("{} ({} lượt chấm)", self.label(), Imdb::votes_text(self.votes))
    }

    /// Đối tượng "imdb" của KKPhim → Imdb (None = chưa có điểm / quá ít lượt) — như api_site::imdb_of.
    pub fn of(o: Option<&crate::json::Obj>) -> Option<Imdb> {
        crate::api_site::imdb_of(o)
    }

    /// Số lượt chấm: "1,2 triệu", "378 nghìn", "950".
    pub fn votes_text(v: i64) -> String {
        if v >= 1_000_000 {
            // Làm tròn nửa lên như "0.0" của C#
            let x = (v as f64 / 1_000_000.0 * 10.0).round() / 10.0;
            let s = format!("{x:.1}").replace('.', ",");
            return format!("{} triệu", s.strip_suffix(",0").unwrap_or(&s));
        }
        if v >= 1000 {
            format!("{} nghìn", v / 1000)
        } else {
            v.to_string()
        }
    }
}

/// `imdb`: điểm IMDb (None = không có / quá ít lượt chấm).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Film {
    pub url: String,
    pub title: String,
    pub poster: Option<String>,
    pub badge: Option<String>,
    pub imdb: Option<Imdb>,
}

impl Film {
    pub fn new(url: impl Into<String>, title: impl Into<String>, poster: Option<String>, badge: Option<String>) -> Film {
        Film { url: url.into(), title: title.into(), poster, badge, imdb: None }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FilmSection {
    pub title: String,
    pub films: Vec<Film>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub title: String,
    pub url: String,
}

impl Category {
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Category {
        Category { title: title.into(), url: url.into() }
    }
}

/// Đo nhanh một trang: thời gian tải trang chủ (ms) và số phim đọc được (0 = có thể là trang chặn của nhà mạng).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteCheck {
    pub ms: i64,
    pub films: usize,
}

/// `moved_to`: trang đã chuyển sang tên miền khác (theo chuyển hướng) → gốc mới.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Home {
    pub sections: Vec<FilmSection>,
    pub categories: Vec<Category>,
    pub moved_to: Option<String>,
}

/// `sections`: trang chia nhóm (vd lịch chiếu theo ngày) — khi có thì hiển thị thay cho `films`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FilmPage {
    pub films: Vec<Film>,
    pub next_url: Option<String>,
    pub sections: Vec<FilmSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub title: String,
    pub url: String,
}

impl Episode {
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Episode {
        Episode { title: title.into(), url: url.into() }
    }
}

/// Một danh sách tập, vd "Việt Sub" / "Thuyết Minh".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeGroup {
    pub name: String,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FilmDetail {
    pub url: String,
    pub title: String,
    pub other_title: Option<String>,
    pub poster: Option<String>,
    pub description: Option<String>,
    pub info: Option<String>,
    pub groups: Vec<EpisodeGroup>,
}

impl FilmDetail {
    pub fn episode_count(&self) -> usize {
        self.groups.iter().map(|g| g.episodes.len()).max().unwrap_or(0)
    }

    /// Tập mới nhất của bản nhiều tập nhất (bản Vietsub thường ra trước Thuyết Minh).
    pub fn latest_episode(&self) -> Option<&Episode> {
        // MaxBy của C# lấy phần tử đầu khi bằng nhau
        let mut best: Option<&EpisodeGroup> = None;
        for g in &self.groups {
            if best.map(|b| g.episodes.len() > b.episodes.len()).unwrap_or(true) {
                best = Some(g);
            }
        }
        best.and_then(|g| g.episodes.last())
    }

    pub fn group_of(&self, ep_url: &str) -> Option<&EpisodeGroup> {
        self.groups.iter().find(|g| g.episodes.iter().any(|e| e.url == ep_url))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub label: String,
    pub kind: String,
}

impl Server {
    pub fn new(label: impl Into<String>, kind: impl Into<String>) -> Server {
        Server { label: label.into(), kind: kind.into() }
    }
}

/// Link phát của một server: iframe trình phát / trang tập, hoặc luồng .m3u8 (kèm phụ đề rời nếu có).
/// `file`: tệp video phát thẳng bằng <video> (MP4 của CLB Phim Xưa), không phải trang trình phát.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Video {
    pub url: String,
    pub subtitle: Option<String>,
    pub file: bool,
}

impl Video {
    pub fn new(url: impl Into<String>) -> Video {
        Video { url: url.into(), subtitle: None, file: false }
    }
}

/// Cách phát một tập: player.php → iframe; `page_mode` → mở thẳng trang tập; `next` → trang Next.js / trang API.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EpisodeSource {
    pub ep_url: String,
    pub servers: Vec<Server>,
    pub post_id: String,
    pub ep: String,
    pub sv: String,
    pub player_url: Option<String>,
    pub direct_embed: Option<String>,
    pub page_mode: bool,
    pub links: HashMap<String, Video>,
    pub next: bool,
}

impl EpisodeSource {
    /// Loại server "Trình phát của trang" (mở thẳng trang tập) khi trang PageMode có thêm server dự phòng KKPhim.
    pub const PAGE_SERVER: &'static str = "page";

    pub fn new(ep_url: impl Into<String>) -> EpisodeSource {
        EpisodeSource { ep_url: ep_url.into(), ..Default::default() }
    }

    /// Chỉ có trình phát của trang tập (không có server nào khác để chọn, kể cả server dự phòng KKPhim).
    pub fn page_only(&self) -> bool {
        self.page_mode && self.servers.iter().all(|s| s.kind != Self::PAGE_SERVER)
    }
}

/// Lịch sử xem (khoá JSON camelCase như bản Android / iPad / Windows).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryItem {
    pub film_url: String,
    pub title: String,
    pub poster: Option<String>,
    pub ep_title: String,
    pub ep_url: String,
    /// ms kể từ 1970
    pub time: i64,
}

/// Phim theo dõi: count = số tập lần kiểm gần nhất, seen = số tập lúc người xem mở phim lần cuối.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Follow {
    pub film_url: String,
    pub title: String,
    pub poster: Option<String>,
    pub count: i64,
    pub latest: String,
    pub seen: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<i64>,
}

impl Follow {
    pub fn new_count(&self) -> i64 {
        (self.count - self.seen).max(0)
    }
}

/// Người xem (hồ sơ) — cùng bản TV / iPad / Windows. id "" = người xem đầu tiên (khoá cũ); color = chỉ số trong
/// `prefs::PROFILE_COLORS`; pin = mã băm của mã PIN (None = không khoá).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub color: i64,
    pub pin: Option<String>,
}

impl Profile {
    pub fn locked(&self) -> bool {
        self.pin.is_some()
    }

    /// Chữ cái đầu tên (trên vòng tròn màu).
    pub fn initial(&self) -> String {
        self.name.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into())
    }
}

/// Vị trí đang xem một tập (giây; 0 = đã xem hết).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Position {
    pub ep: String,
    pub sec: i64,
    pub film: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<i64>,
}
