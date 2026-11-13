//! Trang chủ tổng hợp — cùng ca với FeedTests.cs (Windows) / FeedTest.kt / FeedTests.swift: đọc danh sách, gộp phim trùng,
//! dựng hàng, tập mới, hồ sơ trẻ em, cài đặt của trang chủ; thêm: hàng tự thêm lưu như bản Windows, trang chủ lần trước, bản
//! lưu / nguồn về muộn, cả đường tải (nguồn giả, không mạng). Kho cài đặt là một cho cả tiến trình → test dùng cài đặt chạy
//! lần lượt (khoá LOCK), mỗi test một thư mục dữ liệu riêng.

use cbtv::feed::{self, FeedItem, LoadResult, Loaded, Req, State, Tags};
use cbtv::home_feed::{self, film_tags, Card, Ctx, CustomRow, FeedRow, FreshCard, Part, Plan, Role, Snapshot, Sources};
use cbtv::{json, kk_phim, prefs, Category, Film, HistoryItem};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

const KK: &str = "https://www.kkphim.com";
const NC: &str = "https://phim.nguonc.com";
/// 2026-11-08T20:00:00+07:00
const NOW: i64 = 1_794_142_800_000;
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

static LOCK: Mutex<()> = Mutex::new(());
static N: AtomicUsize = AtomicUsize::new(0);

struct Fresh {
    dir: PathBuf,
    _g: MutexGuard<'static, ()>,
}

impl Drop for Fresh {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Kho cài đặt mới tinh (thư mục riêng).
fn fresh() -> Fresh {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("cbtv-home-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    prefs::load(Some(dir.clone()));
    Fresh { dir, _g: g }
}

fn obj(s: &str) -> json::Obj {
    json::parse_object(s.as_bytes()).expect("JSON đối tượng")
}

fn kk_items() -> Vec<FeedItem> {
    feed::parse_kk(
        &obj(r#"
        {"status":"success","data":{"APP_DOMAIN_CDN_IMAGE":"https://phimimg.com","items":[
          {"name":"Mục Thần Ký","slug":"muc-than-ky","origin_name":"Tales Of Herding Gods","type":"hoathinh",
           "poster_url":"upload/vod/a.jpg","episode_current":"Tập 107","year":2024,"modified":{"time":"2026-11-08T11:55:29.000Z"},
           "category":[{"name":"Phiêu Lưu","slug":"phieu-luu"},{"name":"Viễn Tưởng","slug":"vien-tuong"}],
           "country":[{"name":"Trung Quốc","slug":"trung-quoc"}]},
          {"name":"Đấu Phá Thương Khung (Phần 5)","slug":"dau-pha-thuong-khung-phan-5","type":"hoathinh",
           "poster_url":"https://img.example/dp5.jpg","episode_current":"Tập 150","year":2022,"modified":{"time":"2026-11-08T10:00:00.000Z"}},
          {"name":"Thôn Phệ Tinh Không","slug":"thon-phe-tinh-khong","type":"hoathinh","episode_current":"Tập 248","year":2020,
           "modified":{"time":"2026-11-08T09:00:00.000Z"}},
          {"name":"Thiên Long Bát Bộ","slug":"thien-long-bat-bo-2021","type":"series","episode_current":"Hoàn Tất (50/50)","year":2021,
           "modified":{"time":"2026-11-01T09:00:00.000Z"}}
        ]}}
        "#),
        KK,
    )
}

fn nc_items() -> Vec<FeedItem> {
    feed::parse_nc(
        &obj(r#"
        {"status":"success","items":[
          {"name":"Mục Thần Ký","slug":"muc-than-ky","original_name":"Mu Shen Ji, Tales of Herding Gods","year":"2024",
           "current_episode":"Tập 108","thumb_url":"https://phim.nguonc.com/t.jpg","modified":"2026-11-08T12:30:00.000000Z"},
          {"name":"Đấu Phá Thương Khung 5","slug":"dau-pha-5","year":"2022","current_episode":"Tập 150","modified":"2026-11-08T08:00:00.000000Z"},
          {"name":"Thôn Phệ Tinh Không 2","slug":"thon-phe-2","year":"2024","current_episode":"Tập 222","modified":"2026-11-08T12:00:00.000000Z"}
        ]}
        "#),
        NC,
    )
}

fn web(titles: &[(&str, Option<&str>)]) -> Vec<FeedItem> {
    let films: Vec<Film> = titles
        .iter()
        .map(|(t, b)| Film::new(format!("https://yanhh3d.ee/{}", kk_phim::norm(t).replace(' ', "-")), *t, None, b.map(String::from)))
        .collect();
    feed::from_films(&films)
}

fn find<'a>(films: &'a [feed::FeedFilm], n: &str) -> Vec<&'a feed::FeedFilm> {
    let k = kk_phim::norm(n);
    films.iter().filter(|f| f.names.contains(&k)).collect()
}

#[test]
fn episode_numbers_and_times() {
    assert_eq!(feed::ep_of(Some("Tập 108")), Some(108.0));
    assert_eq!(feed::ep_of(Some("Hoàn Tất (16/16)")), Some(16.0));
    assert_eq!(feed::ep_of(Some("7/??")), Some(7.0));
    assert_eq!(feed::ep_of(Some("Full")), None);
    assert_eq!(feed::ep_of(Some("Top 1")), None);
    let utc = feed::time(Some("2026-11-08T14:12:36Z"));
    assert_eq!(utc, feed::time(Some("2026-11-08T21:12:36+07:00")));
    assert_eq!(utc, feed::time(Some("2026-11-08T14:12:36.000000Z")));
    assert_eq!(utc, feed::time(Some("2026-11-08 21:12:36")));
    assert_eq!(feed::time(Some("không phải giờ")), 0);
    assert_eq!(feed::time(Some("2026-02-30T10:00:00Z")), 0);
    assert_eq!(feed::time(Some("2026-11-08T20:00:00+07:00")), NOW);
}

#[test]
fn parses_kk_and_nguonc() {
    let kk = kk_items();
    let m = &kk[0];
    assert_eq!(m.url, format!("{KK}/phim/muc-than-ky"));
    assert_eq!(m.poster.as_deref(), Some("https://phimimg.com/upload/vod/a.jpg"));
    assert_eq!(m.ep, Some(107.0));
    let t = m.tags.as_ref().expect("có thể loại");
    assert_eq!(t.genres, ["phieu-luu", "vien-tuong"]);
    assert_eq!(t.kind.as_deref(), Some("hoathinh"));
    assert_eq!(m.host, "kkphim.com");
    let nc = nc_items();
    let n = &nc[0];
    assert_eq!(n.ep, Some(108.0));
    assert!(n.names.contains(&kk_phim::norm("Mu Shen Ji")));
    assert_eq!(m.json().as_object().and_then(FeedItem::of).as_ref(), Some(m));
}

#[test]
fn merges_same_film_across_sources() {
    let y = web(&[("Đấu Phá Thương Khung Phần 5", Some("Tập 150")), ("Mục Thần Ký", Some("Tập 107")), ("Thôn Phệ Tinh Không", Some("Tập 248"))]);
    let films = feed::merge(&[y, kk_items(), nc_items(), kk_items().into_iter().take(1).collect()], Some(NOW));
    let dp = find(&films, "Đấu Phá Thương Khung Phần 5");
    assert_eq!(dp.len(), 1);
    assert_eq!(dp[0].items.len(), 3);
    let mtk = find(&films, "Mục Thần Ký");
    assert_eq!(mtk.len(), 1);
    assert_eq!(mtk[0].items.len(), 3); // cùng phim trong hai danh sách KKPhim vẫn một nhóm
    assert_eq!(mtk[0].latest().map(|l| l.host.as_str()), Some("phim.nguonc.com"));
    assert_eq!(find(&films, "Thôn Phệ Tinh Không").len(), 1);
    assert_eq!(find(&films, "Thôn Phệ Tinh Không 2").len(), 1);
    let two = feed::from_films(&[Film::new("https://yanhh3d.ee/a", "Tiên Nghịch", None, None), Film::new("https://yanhh3d.ee/b", "Tiên Nghịch", None, None)]);
    assert_eq!(feed::merge(&[two], Some(NOW)).len(), 2);
}

fn part(role: Role, key: &str, host: &str, items: Vec<FeedItem>, title: Option<&str>, id: &str, state: State) -> (Part, LoadResult) {
    let it = items.clone();
    let r = Req::new(
        key,
        host,
        Arc::new(move || -> feed::LoadFut {
            let it = it.clone();
            Box::pin(async move { Ok(Loaded { items: it, ..Default::default() }) })
        }),
    );
    let error = if state == State::Ok { None } else { Some("lỗi 502".to_string()) };
    let res = LoadResult { req: r.clone(), state, items, categories: None, error, resting: false, ms: 0, saved_at: NOW, moved_to: None };
    (Part::new(role, r, title.map(String::from), None, id, ""), res)
}

fn ok(role: Role, key: &str, host: &str, items: Vec<FeedItem>) -> (Part, LoadResult) {
    part(role, key, host, items, None, "", State::Ok)
}

fn plan(parts: &[(Part, LoadResult)], kids: bool) -> Plan {
    let src = Sources { kk: Some(KK.into()), nc: Some(NC.into()), vs: Some("https://vsmov.com".into()), k20: None, web: vec!["https://yanhh3d.ee".into()] };
    Plan { kids, ..Plan::new(parts.iter().map(|p| p.0.clone()).collect(), src) }
}

fn ctx(history: Vec<HistoryItem>, seen: HashMap<String, f64>) -> Ctx {
    Ctx {
        history,
        follows: HashSet::new(),
        seen: Box::new(move |k| seen.get(k).copied()),
        base: "yanhh3d.ee".into(),
        sites: vec!["https://yanhh3d.ee".into(), KK.into(), NC.into()],
        now: NOW,
    }
}

fn h(slug: &str, title: &str, ep: &str, ago: i64) -> HistoryItem {
    HistoryItem {
        film_url: format!("https://yanhh3d.ee/{slug}"),
        title: title.into(),
        poster: None,
        ep_title: ep.into(),
        ep_url: format!("e-{slug}"),
        time: NOW - ago,
    }
}

fn seen_of(v: &[(&str, f64)]) -> HashMap<String, f64> {
    v.iter().map(|(k, e)| (kk_phim::norm(k), *e)).collect()
}

#[test]
fn new_episodes_of_films_being_watched() {
    let _f = fresh();
    let parts = vec![
        ok(Role::Web, "web|y", "yanhh3d.ee", web(&[("Mục Thần Ký", Some("Tập 107")), ("Tiên Nghịch", Some("Tập 168")), ("Thiên Long Bát Bộ", Some("Tập 45"))])),
        ok(Role::New, "new|kk", "kkphim.com", kk_items()),
        ok(Role::New, "new|nc", "phim.nguonc.com", nc_items()),
    ];
    let history = vec![
        h("muc-than-ky", "Mục Thần Ký", "Tập 107", DAY),
        h("tien-nghich", "Tiên Nghịch", "Tập 166", 2 * DAY),
        h("thien-long-bat-bo", "Thiên Long Bát Bộ", "Tập 30", DAY),
    ];
    let p = plan(&parts, false);
    let (_, fresh_cards) = home_feed::build_rows(&p, &parts, &ctx(history.clone(), seen_of(&[("Thiên Long Bát Bộ", 45.0)])));
    let by: HashMap<String, &FreshCard> = fresh_cards.iter().map(|f| (f.card.film.title.clone(), f)).collect();
    assert_eq!(by.keys().cloned().collect::<HashSet<_>>(), ["Mục Thần Ký", "Tiên Nghịch"].iter().map(|s| s.to_string()).collect::<HashSet<_>>());
    let mtk = &by["Mục Thần Ký"].card;
    assert_eq!(mtk.film.badge.as_deref(), Some("Tập 108 mới"));
    assert_eq!(mtk.chip.as_deref(), Some("có ở nguonc"));
    assert_eq!(mtk.film.url, format!("{NC}/phim/muc-than-ky"));
    assert_eq!(by["Tiên Nghịch"].card.film.badge.as_deref(), Some("+2 · Tập 168"));
    // Đang xem dở từ trước (tập 31 / 108): không phải «tập mới»
    assert!(home_feed::build_rows(&p, &parts, &ctx(vec![h("muc-than-ky", "Mục Thần Ký", "Tập 31", 3 * DAY)], HashMap::new())).1.is_empty());
    // Đã thấy tập 108 → thôi báo
    let again = home_feed::build_rows(&p, &parts, &ctx(history, seen_of(&[("Mục Thần Ký", 108.0)]))).1;
    assert!(!again.iter().any(|f| f.card.film.title == "Mục Thần Ký"));
}

#[test]
fn newest_row_merges_sources_and_marks_broken_ones() {
    let _f = fresh();
    let parts = vec![
        ok(Role::Web, "web|y", "yanhh3d.ee", web(&[("Đấu Phá Thương Khung Phần 5", Some("Tập 150"))])),
        part(Role::New, "new|kk", "kkphim.com", kk_items(), None, "", State::Cached), // KKPhim lỗi, dùng bản lưu
        ok(Role::New, "new|nc", "phim.nguonc.com", nc_items()),
    ];
    let (rows, _) = home_feed::build_rows(&plan(&parts, false), &parts, &ctx(Vec::new(), HashMap::new()));
    let newest: Vec<&FeedRow> = rows.iter().filter(|r| r.id == "newest").collect();
    assert_eq!(newest.len(), 1);
    let card = |pred: &dyn Fn(&Card) -> bool| newest[0].cards.iter().filter(|c| pred(c)).cloned().collect::<Vec<Card>>();
    let tl = card(&|c: &Card| c.film.title == "Thiên Long Bát Bộ");
    assert_eq!(tl.len(), 1);
    assert_eq!(tl[0].chip.as_deref(), Some("bản lưu"));
    let dp = card(&|c: &Card| kk_phim::norm(&c.film.title) == kk_phim::norm("Đấu Phá Thương Khung Phần 5"));
    assert_eq!(dp.len(), 1);
    assert_eq!(dp[0].chip.as_deref(), Some("2 nguồn"));
    assert_eq!(dp[0].film.url, "https://yanhh3d.ee/dau-pha-thuong-khung-5"); // trang đang dùng mở trước
    assert_eq!(dp[0].links.len(), 3);
}

#[test]
fn kids_profile_keeps_only_child_friendly_animation() {
    let _f = fresh();
    let item = |slug: &str, kind: &str, genres: &[&str]| {
        let g: Vec<String> = genres.iter().map(|g| format!("{{\"slug\":\"{g}\"}}")).collect();
        format!(
            "{{\"name\":\"{slug}\",\"slug\":\"{slug}\",\"type\":\"{kind}\",\"modified\":{{\"time\":\"2026-11-08T10:00:00Z\"}},\"category\":[{}]}}",
            g.join(",")
        )
    };
    let all = [
        item("doraemon", "hoathinh", &["gia-dinh"]),
        item("chiikawa", "hoathinh", &["tre-em"]),
        item("conan", "hoathinh", &["bi-an"]),
        item("edgerunners", "hoathinh", &["hinh-su"]),
        item("phim-ma", "hoathinh", &["kinh-di"]),
        item("nguoi-dong", "series", &["gia-dinh"]),
        item("Horror Collector", "hoathinh", &["tre-em"]),
    ];
    let list = feed::parse_kk(&obj(&format!("{{\"data\":{{\"items\":[{}]}}}}", all.join(","))), KK);
    let vs = feed::from_films(&[Film::new("https://vsmov.com/phim/na-tra", "Na Tra Truyền Kỳ", None, None)]);
    let parts = vec![
        part(Role::Kids, "list|kk", "kkphim.com", list, Some("Hoạt hình thiếu nhi"), "kids-young", State::Ok),
        part(Role::Kids, "list|vs", "vsmov.com", vs, Some("Hoạt hình thiếu nhi"), "kids-young", State::Ok),
    ];
    let rows = home_feed::build_rows(&plan(&parts, true), &parts, &ctx(Vec::new(), HashMap::new())).0;
    assert_eq!(rows.len(), 1);
    let titles: HashSet<String> = rows[0].cards.iter().map(|c| c.film.title.clone()).collect();
    assert_eq!(titles, ["doraemon", "chiikawa", "conan", "Na Tra Truyền Kỳ"].iter().map(|s| s.to_string()).collect::<HashSet<_>>());
    assert_eq!(rows[0].slot, "kids-young");
    assert_eq!(home_feed::build_rows(&plan(&parts, false), &parts, &ctx(Vec::new(), HashMap::new())).0[0].cards.len(), 8);
}

#[test]
fn prefs_for_the_feed() {
    let _f = fresh();
    assert!(film_tags::needs_lookup("Tiên Nghịch"));
    let t = Tags { genres: vec!["vo-thuat".into()], countries: vec!["trung-quoc".into()], kind: Some("hoathinh".into()) };
    film_tags::remember(&kk_phim::norm("Tiên Nghịch"), Some(&t));
    assert_eq!(film_tags::of("Tiên nghịch").and_then(|t| t.kind).as_deref(), Some("hoathinh"));
    film_tags::remember(&kk_phim::norm("Phim Lạ"), None);
    assert!(film_tags::of("Phim Lạ").is_none());
    assert!(!film_tags::needs_lookup("Phim Lạ"));

    prefs::set_seen_ep("muc than ky", 107.0);
    prefs::set_seen_ep("muc than ky", 105.0);
    assert_eq!(prefs::seen_ep("muc than ky"), Some(107.0));
    prefs::save_position("https://yanhh3d.ee/muc-than-ky/tap-1", 0, "https://yanhh3d.ee/muc-than-ky");
    prefs::save_position("https://yanhh3d.ee/muc-than-ky/tap-2", 30, "https://yanhh3d.ee/muc-than-ky");
    assert_eq!(prefs::watched_counts().get("https://yanhh3d.ee/muc-than-ky"), Some(&2));
    for i in 0..45 {
        prefs::record_watch_lang(if i % 3 == 0 { "vs" } else { "tm" });
    }
    assert_eq!(prefs::watch_langs().len(), 40);
    assert!(prefs::home_mix_on());
    prefs::set_home_mix(false);
    assert!(!prefs::home_mix_on());
    prefs::set_kids(true);
    assert!(prefs::home_mix_on());
    home_feed::set_home_topics(&["cn", "kh"].iter().map(|s| s.to_string()).collect());
    assert_eq!(home_feed::home_topics(), ["cn", "kh"].iter().map(|s| s.to_string()).collect::<HashSet<_>>());
}

/// Hàng tự thêm lưu cùng dạng JSON với bản Windows (camelCase); chủ đề lưu ở "home_topics" (trước 1.6) đọc lại thành hàng chủ đề.
#[test]
fn custom_rows_and_topics_in_prefs() {
    let _f = fresh();
    prefs::set("home_topics", Some(vec!["kr", "cn"]));
    assert_eq!(home_feed::home_custom(), vec![CustomRow::for_topic("kr"), CustomRow::for_topic("cn")]);
    let row = CustomRow::filter("c1", "hoathinh", "trung-quoc", "tien-hiep", "");
    let mut list = home_feed::home_custom();
    list.push(row.clone());
    home_feed::set_home_custom(&list);
    let raw = prefs::home_custom_raw().expect("đã lưu").to_string();
    assert!(raw.contains(r#"{"id":"c1","topic":null,"type":"hoathinh","country":"trung-quoc","genre":"tien-hiep","lang":""}"#), "{raw}");
    assert!(raw.contains(r#"{"id":"t-kr","topic":"kr","type":"","country":"","genre":"","lang":""}"#), "{raw}");
    // Đổi chủ đề: bỏ «cn», thêm «jp»; hàng lọc giữ nguyên
    home_feed::set_home_topics(&["kr", "jp"].iter().map(|s| s.to_string()).collect());
    assert_eq!(home_feed::home_custom(), vec![CustomRow::for_topic("kr"), row.clone(), CustomRow::for_topic("jp")]);
    // Thứ tự mặc định: hai hàng đầu, các hàng tự thêm, rồi các hàng app tự chọn
    let d = home_feed::row_defaults(None);
    assert_eq!(&d[..5], ["new-eps", "continue", "custom-t-kr", "custom-c1", "custom-t-jp"]);
    // Ẩn một hàng / lưu
    home_feed::save_rows(&home_feed::hide(&home_feed::row_prefs(), "custom-c1"));
    assert!(!home_feed::row_on("custom-c1"));
    assert!(home_feed::row_on("similar"));
    assert!(prefs::home_rows().contains(&"-custom-c1".to_string()));
    // Hàng hỏng (không id) bị bỏ
    prefs::set_home_custom_raw(serde_json::json!([{"topic": "cn"}, {"id": "x", "type": "series"}, 5]));
    assert_eq!(home_feed::home_custom(), vec![CustomRow::filter("x", "series", "", "", "")]);
}

fn sample_card(url: &str, title: &str) -> Card {
    let names: HashSet<String> = [kk_phim::norm(title)].into_iter().collect();
    let link = FeedItem::new(url.into(), title.into(), Some("https://img/p.jpg".into()), Some("Tập 3".into()), names, Some(2025), NOW, Some(3.0), None, None);
    Card { film: Film::new(url, title, Some("https://img/p.jpg".into()), Some("Tập 3".into())), chip: Some("2 nguồn".into()), links: vec![link] }
}

#[test]
fn last_home_page_round_trip() {
    let _f = fresh();
    assert!(home_feed::last("p1").is_none());
    let snap = Snapshot {
        rows: vec![
            FeedRow::new("newest", "Mới cập nhật · mọi nguồn", vec![sample_card("https://yanhh3d.ee/a", "Phim A")], None, "newest"),
            FeedRow::new("custom-c1", "Hoạt hình Trung Quốc", vec![sample_card("https://yanhh3d.ee/b", "Phim B")], Some(format!("{KK}/api/x")), "custom-c1"),
        ],
        fresh: vec![FreshCard { card: sample_card("https://yanhh3d.ee/c", "Phim C"), key: "phim c".into(), ep: 10.5 }],
        categories: Some(vec![Category::new("Hoạt hình", "https://yanhh3d.ee/hoat-hinh")]),
        ..Default::default()
    };
    home_feed::save_last("p1", &snap);
    let back = home_feed::last("p1").expect("có trang chủ lần trước");
    assert!(back.from_cache);
    assert!(back.note.starts_with("Bản lưu lúc ") && back.note.ends_with(" · đang làm mới…"), "{}", back.note);
    assert_eq!(back.rows.len(), 2);
    assert_eq!(back.rows[1].more.as_deref(), Some(&*format!("{KK}/api/x")));
    assert_eq!(back.rows[1].slot, "custom-c1");
    let c = &back.rows[0].cards[0];
    assert_eq!((c.film.url.as_str(), c.film.title.as_str(), c.chip.as_deref()), ("https://yanhh3d.ee/a", "Phim A", Some("2 nguồn")));
    assert_eq!(c.film.badge.as_deref(), Some("Tập 3"));
    assert_eq!(c.links.len(), 1);
    assert!(c.links[0].names.is_empty()); // bản lưu bỏ tên chuẩn hoá cho gọn
    assert_eq!(back.fresh[0].ep, 10.5);
    assert_eq!(back.fresh[0].key, "phim c");
    assert_eq!(back.categories.as_ref().map(|c| c.len()), Some(1));
    // Bản lưu / trống thì không ghi
    home_feed::save_last("p2", &Snapshot { from_cache: true, ..snap.clone() });
    home_feed::save_last("p3", &Snapshot { empty: true, ..snap });
    assert!(home_feed::last("p2").is_none());
    assert!(home_feed::last("p3").is_none());
}

/// Bản lưu của từng nguồn → trang chủ hiện ngay (cached); nguồn chậm về muộn (bản lưu mới hơn lúc tải) → late dựng lại.
#[test]
fn cached_and_late_sources() {
    let _f = fresh();
    let save = |key: &str, items: &[FeedItem]| {
        let o = serde_json::json!({ "at": cbtv::util::now_ms(), "items": items.iter().map(|i| i.json()).collect::<Vec<_>>() });
        feed::write_text(key, &o.to_string());
    };
    let p_kk = part(Role::New, "new|kk", "kkphim.com", Vec::new(), None, "", State::NoData);
    let p_nc = part(Role::New, "new|nc", "phim.nguonc.com", Vec::new(), None, "", State::NoData);
    let pl = plan(&[p_kk.clone(), p_nc.clone()], false);
    assert!(home_feed::cached(&pl).is_none());
    save("new|kk", &kk_items());
    let c = home_feed::cached(&pl).expect("có bản lưu");
    assert!(c.from_cache);
    assert!(c.note.starts_with("Bản lưu lúc "));
    assert!(c.rows.iter().any(|r| r.id == "newest"));

    // Lần tải: KKPhim xong, NguonC quá hạn (chậm) → chờ; NguonC về muộn → late
    let started = cbtv::util::now_ms() - 1000;
    let mut slow = p_nc.1.clone();
    slow.error = Some("quá 6 giây".into());
    let snap = Snapshot {
        results: vec![(p_kk.0.clone(), LoadResult { state: State::Ok, items: kk_items(), ..p_kk.1.clone() }), (p_nc.0.clone(), slow)],
        plan: Some(pl.clone()),
        started_at: started,
        pending: true,
        ..Default::default()
    };
    let rebuilt = home_feed::build(pl.clone(), snap.results.clone(), false, started);
    assert!(rebuilt.pending);
    assert!(rebuilt.note.contains("đang chờ nguonc…"), "{}", rebuilt.note);
    assert!(rebuilt.note_gave_up.contains("chậm, dùng bản lưu: nguonc"), "{}", rebuilt.note_gave_up);
    assert!(futures::executor::block_on(home_feed::late(&snap)).is_none());
    save("new|nc", &nc_items());
    let late = futures::executor::block_on(home_feed::late(&snap)).expect("nguồn chậm đã về");
    assert!(!late.pending);
    assert!(late.results.iter().all(|x| x.1.state == State::Ok));
    assert!(home_feed::last(&pl.pid).is_some());
}

/// Cả đường tải với nguồn giả (không mạng): tải song song, gộp, dựng hàng, lưu bản từng nguồn + trang chủ lần trước.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn load_with_fake_sources() {
    let _f = fresh();
    let mut fail = ok(Role::New, "new|hong", "hong.test", Vec::new());
    fail.0.req.load = Arc::new(|| -> feed::LoadFut { Box::pin(async { Err(cbtv::Error::Http(502)) }) });
    let parts = vec![ok(Role::Web, "web|y", "yanhh3d.ee", web(&[("Tiên Nghịch", Some("Tập 168"))])), ok(Role::New, "new|kk", "kkphim.com", kk_items()), ok(Role::New, "new|nc", "phim.nguonc.com", nc_items()), fail];
    let mut pl = plan(&parts, false);
    pl.pid = "thu".into();
    let snap = home_feed::load(pl).await;
    assert!(!snap.empty);
    assert!(!snap.pending);
    assert!(snap.note.contains(" · lỗi: hong"), "{}", snap.note);
    let newest = snap.rows.iter().find(|r| r.id == "newest").expect("hàng mới cập nhật");
    assert!(newest.cards.iter().any(|c| c.film.title == "Tiên Nghịch"));
    assert!(feed::read_cache("new|kk").is_some());
    assert!(feed::read_cache("new|hong").is_none());
    let last = home_feed::last("thu").expect("đã lưu trang chủ");
    assert_eq!(last.rows.len(), snap.rows.len());
}

fn assert_send<T: Send>(_: &T) {}

/// Việc nền chạy trên tokio nhiều luồng: các hàm async phải trả Future gửi được sang luồng khác.
#[test]
fn futures_are_send() {
    let f = home_feed::load(Plan::default());
    assert_send(&f);
    drop(f);
    let snap = Snapshot::default();
    let f = home_feed::late(&snap);
    assert_send(&f);
    drop(f);
    let f = film_tags::lookup("Tiên Nghịch");
    assert_send(&f);
    drop(f);
    let f = cbtv::related::load("Tiên Nghịch", "https://yanhh3d.ee/tien-nghich");
    assert_send(&f);
    drop(f);
    let f = cbtv::imdb_top::page(false);
    assert_send(&f);
    drop(f);
    let f = cbtv::imdb_top::for_list("https://www.kkphim.com/api/movies/category/phim-le?page=1");
    assert_send(&f);
    drop(f);
}
