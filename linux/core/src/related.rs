//! Phim liên quan ở trang phim (từ 1.9; bản Windows: Related.cs, TV: Related.kt, iPad: Related.swift):
//! - «Cùng bộ»: các phần / mùa khác của cùng bộ — tìm theo tên gốc (bỏ «Phần N / Season N / Mùa N», năm, «– 20 Tập», phần sau
//!   dấu «:») trên trang của phim + KKPhim, xếp theo số phần.
//! - «Cùng thể loại»: KKPhim / VSMOV lọc theo loại + nước + thể loại đặc trưng nhất (như hàng «Giống …» của trang chủ); bỏ phim
//!   đang xem và các phần cùng bộ, phim chưa xem lên trước.
//!
//! Không nguồn nào (KKPhim, VSMOV, NguonC, ophim) tìm được theo diễn viên / đạo diễn nên chưa có hàng đó.

use crate::home_feed::{self, film_tags};
use crate::kk_phim::norm;
use crate::models::Film;
use crate::util::{collapse_spaces, Rx};
use crate::{prefs, site};
use std::collections::HashSet;

pub const MAX: usize = 12;

/// Một hàng: id "series" (cùng bộ) / "similar" (cùng thể loại).
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub films: Vec<Film>,
}

static PART: Rx = Rx::new(r"(?i)(?:phần|phan|season|mùa|mua|part|ss)\s*0*(\d{1,3})");
static EPISODES: Rx = Rx::new(r"(?i)\s[–\-]\s*\d+\s*tập.*$");
static YEAR: Rx = Rx::new(r"\(\s*(?:19|20)\d{2}\s*\)|\b(?:19|20)\d{2}\b");
static BRACKETS: Rx = Rx::new(r"\([^)]*\)|\[[^\]]*\]");
static TRAILING_NUMBER: Rx = Rx::new(r"\s(\d{1,2})$");

/// Tên để tìm (còn dấu): «Tiên Nghịch (Phần 2) 2024 – 20 Tập» → «Tiên Nghịch».
pub fn search_name(title: &str) -> String {
    let t = EPISODES.replace(title, " ");
    let t = PART.replace(&t, " ");
    let t = YEAR.replace(&t, " ");
    let t = BRACKETS.replace(&t, " ");
    let mut t = t.split(':').next().unwrap_or("");
    if let Some(dash) = t.find(" - ") {
        t = &t[..dash];
    }
    let collapsed = collapse_spaces(t);
    let o = collapsed.trim_end_matches(['-', '–', ' ']);
    if o.is_empty() {
        title.trim().to_string()
    } else {
        o.to_string()
    }
}

/// Tên gốc để so cùng bộ (chuẩn hoá, bỏ số phần đứng cuối): «Đấu Phá Thương Khung 5» → «dau pha thuong khung».
pub fn base(title: &str) -> String {
    let n = TRAILING_NUMBER.replace(&norm(&search_name(title)), "").trim().to_string();
    if n.is_empty() {
        norm(title)
    } else {
        n
    }
}

/// Số phần: «Phần 5», «Season 2», «(Phần 01)», số cuối phần tên trước «:» («Đấu Phá Thương Khung 4: Đào Vong»); không có → None.
pub fn part_of(title: &str) -> Option<i64> {
    if let Some(p) = PART.group(1, title).and_then(|g| g.parse::<i64>().ok()) {
        return Some(p);
    }
    let head = title.split(':').next().unwrap_or("");
    let n = norm(&EPISODES.replace(&YEAR.replace(head, " "), " "));
    TRAILING_NUMBER.group(1, &n).and_then(|g| g.parse().ok())
}

/// Cùng bộ với phim đang xem: tên gốc trùng (đủ dài để khỏi trùng nhầm), không phải chính phim đó (cùng tên đầy đủ).
pub fn same_series(mine: &str, other: &str) -> bool {
    let b = base(mine);
    b.chars().count() >= 4 && b == base(other) && norm(mine) != norm(other)
}

/// Các phần khác của cùng bộ (đã gộp các nguồn, nguồn trước được ưu tiên): bỏ phim trùng tên, xếp theo số phần (không ghi
/// số = phần 1; cùng số giữ thứ tự tìm được).
pub fn pick_series(title: &str, url: &str, found: &[Film]) -> Vec<Film> {
    let mut seen = HashSet::new();
    let mut l: Vec<&Film> = found.iter().filter(|f| f.url != url && same_series(title, &f.title) && seen.insert(norm(&f.title))).collect();
    l.sort_by_key(|f| part_of(&f.title).unwrap_or(1));
    l.into_iter().take(MAX).cloned().collect()
}

/// Hàng «Cùng thể loại»: bỏ phim đang xem và các phần cùng bộ; phim chưa xem lên trước (giữ thứ tự của nguồn).
pub fn pick_similar(title: &str, url: &str, found: &[Film], watched: &HashSet<String>) -> Vec<Film> {
    let mut seen = HashSet::new();
    let mine = norm(title);
    let films: Vec<&Film> = found
        .iter()
        .filter(|f| {
            let n = norm(&f.title);
            f.url != url && n != mine && !same_series(title, &f.title) && seen.insert(n)
        })
        .collect();
    let (fresh, seen_before): (Vec<&Film>, Vec<&Film>) = films.into_iter().partition(|f| !watched.contains(&norm(&f.title)));
    fresh.into_iter().chain(seen_before).take(MAX).cloned().collect()
}

/// Tải các hàng (lỗi một nguồn thì bỏ nguồn đó).
pub async fn load(title: &str, url: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    let name = search_name(title);
    // Cùng bộ: trang của phim trước (cùng nguồn), rồi KKPhim
    let mut origins: Vec<String> = Vec::new();
    for o in [site::origin_of(url), home_feed::get_sources().kk].into_iter().flatten() {
        if !origins.contains(&o) {
            origins.push(o);
        }
    }
    let mut found: Vec<Film> = Vec::new();
    for o in &origins {
        if let Ok(p) = site::search(&name, Some(o.as_str())).await {
            found.extend(p.films);
        }
    }
    let series = pick_series(title, url, &found);
    if !series.is_empty() {
        rows.push(Row { id: "series".into(), title: format!("Cùng bộ «{name}»"), films: series.clone() });
    }
    // Cùng thể loại (tra thể loại qua KKPhim theo tên, nhớ chung cả nhà)
    if film_tags::needs_lookup(title) {
        let _ = film_tags::lookup(title).await;
    }
    let sim = film_tags::of(title).and_then(|tags| home_feed::related_list(&tags, prefs::kids()));
    if let Some((sim_url, sim_title)) = sim {
        let list = site::list(&sim_url).await.map(|p| p.films).unwrap_or_default();
        let watched: HashSet<String> = prefs::history().iter().map(|h| norm(&h.title)).collect();
        let films: Vec<Film> = pick_similar(title, url, &list, &watched).into_iter().filter(|f| series.iter().all(|x| x.url != f.url)).collect();
        if !films.is_empty() {
            rows.push(Row { id: "similar".into(), title: format!("Cùng thể loại · {sim_title}"), films });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    //! Tên gốc, số phần, ghép cùng bộ, hàng cùng thể loại — cùng ca với RelatedTests.cs / RelatedTest.kt / RelatedTests.swift.
    use super::*;

    fn f(title: &str) -> Film {
        Film::new(format!("https://x.test/{}", norm(title).replace(' ', "-")), title, None, None)
    }

    fn fu(title: &str, url: &str) -> Film {
        Film::new(url, title, None, None)
    }

    fn titles(l: &[Film]) -> Vec<&str> {
        l.iter().map(|x| x.title.as_str()).collect()
    }

    #[test]
    fn search_name_drops_part_year_and_episodes() {
        assert_eq!(search_name("Tiên Nghịch (Phần 2) 2024 – 20 Tập"), "Tiên Nghịch");
        assert_eq!(search_name("Đấu Phá Thương Khung Phần 5"), "Đấu Phá Thương Khung");
        assert_eq!(search_name("Nhất Nguyệt Thần Kiếm 1990 (Phần 01) – 20 Tập"), "Nhất Nguyệt Thần Kiếm");
        assert_eq!(search_name("Avatar: Dòng Chảy Của Nước"), "Avatar");
        assert_eq!(search_name("1917"), "1917");
    }

    #[test]
    fn part_numbers() {
        assert_eq!(part_of("Đấu Phá Thương Khung Phần 5"), Some(5));
        assert_eq!(part_of("Nhất Nguyệt Thần Kiếm 1990 (Phần 01) – 20 Tập"), Some(1));
        assert_eq!(part_of("Squid Game Season 2"), Some(2));
        assert_eq!(part_of("Thế Giới Hoàn Mỹ 3"), Some(3));
        assert_eq!(part_of("Đấu Phá Thương Khung 4: Đào Vong"), Some(4));
        assert_eq!(part_of("Tiên Nghịch"), None);
        assert_eq!(part_of("Trái Tim Quái Thú 2025"), None);
    }

    #[test]
    fn same_series_matches_other_parts_only() {
        assert!(same_series("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung Phần 1"));
        assert!(same_series("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung"));
        assert!(same_series("Avatar", "Avatar: Dòng Chảy Của Nước"));
        assert!(!same_series("Đấu Phá Thương Khung Phần 5", "Đấu Phá Thương Khung Phần 5"));
        assert!(!same_series("Tiên Nghịch", "Tiên Nghịch Truyện Kỳ"));
        assert!(!same_series("Yêu", "Yêu 2"));
    }

    #[test]
    fn pick_series_orders_by_part() {
        let found = vec![
            f("Đấu Phá Thương Khung Phần 3"),
            fu("Đấu Phá Thương Khung Phần 5", "https://x.test/me"),
            f("Đấu Phá Thương Khung"),
            fu("Đấu Phá Thương Khung Phần 3", "https://kk.test/dpk3"),
            f("Kiếm Lai"),
            f("Đấu Phá Thương Khung Phần 2"),
        ];
        assert_eq!(
            titles(&pick_series("Đấu Phá Thương Khung Phần 5", "https://x.test/me", &found)),
            ["Đấu Phá Thương Khung", "Đấu Phá Thương Khung Phần 2", "Đấu Phá Thương Khung Phần 3"]
        );
        let more = pick_series(
            "Đấu Phá Thương Khung (Phần 5)",
            "https://x.test/me",
            &[f("Đấu Phá Thương Khung: Thức Tỉnh"), f("Đấu Phá Thương Khung 2: Ngừng Chiến"), f("Đấu Phá Thương Khung (Phần 2)")],
        );
        assert_eq!(titles(&more), ["Đấu Phá Thương Khung: Thức Tỉnh", "Đấu Phá Thương Khung 2: Ngừng Chiến", "Đấu Phá Thương Khung (Phần 2)"]);
    }

    #[test]
    fn pick_similar_skips_self_series_and_puts_watched_last() {
        let found = vec![f("Tiên Nghịch"), f("Phàm Nhân Tu Tiên"), f("Tiên Nghịch Phần 2"), f("Thế Giới Hoàn Mỹ"), f("Phàm Nhân Tu Tiên")];
        let watched: HashSet<String> = [norm("Phàm Nhân Tu Tiên")].into_iter().collect();
        let got = pick_similar("Tiên Nghịch", "https://x.test/me", &found, &watched);
        assert_eq!(titles(&got), ["Thế Giới Hoàn Mỹ", "Phàm Nhân Tu Tiên"]);
    }
}
