//! Đọc trang phim, trang chủ tổng hợp, phim liên quan, IMDb, chặn quảng cáo — bảng hàm cho Site.cs / HomeFeed.cs… của app WPF.
//! Kế hoạch tải (Plan) và kết quả (Snapshot) của trang chủ ở lại trong lõi, app giữ mã số (handle) + phần hiện lên màn hình.

use crate::Res;
use cbtv::home_feed::{self, Plan, Snapshot};
use cbtv::{ad_block, api_site, feed, imdb_top, kk_phim, next_site, related, site, EpisodeGroup, EpisodeSource, Episode, FilmDetail, Server};
use serde_json::{json, Value};
use std::sync::Mutex;

fn s(a: &Value, k: &str) -> String {
    a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn opt_s(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string())
}

fn ok<T: serde::Serialize>(v: T) -> Res {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn de<T: serde::de::DeserializeOwned>(a: &Value, k: &str) -> Result<T, String> {
    serde_json::from_value(a.get(k).cloned().unwrap_or(Value::Null)).map_err(|e| format!("tham số {k}: {e}"))
}

fn err(e: cbtv::util::Error) -> String {
    cbtv::util::friendly(&e)
}

fn cats(v: &[(&str, &str)]) -> Value {
    Value::Array(v.iter().map(|(name, slug)| json!({ "name": name, "slug": slug })).collect())
}

/// Hàm nhanh.
pub fn sync(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        "site.cachedDetail" => ok(site::cached_detail(&s(a, "url"))),
        "site.displayName" => ok(site::display_name(&s(a, "host"))),
        "site.tag" => ok(site::tag(&s(a, "host"), &de::<Vec<String>>(a, "sites").unwrap_or_default())),
        "site.menu" => ok(site::menu(&s(a, "baseUrl"))),
        "api.isKnown" => ok(api_site::is_known(&s(a, "url"))),
        "api.filters" => {
            let (g, c) = api_site::filters(&s(a, "baseUrl"));
            ok(json!({ "genres": g, "countries": c }))
        }
        "api.countryName" => ok(api_site::country_name(&s(a, "slug"))),
        "api.genreName" => ok(api_site::genre_name(&s(a, "slug"))),
        "api.genreList" => Ok(cats(&api_site::genre_list())),
        "api.countryList" => Ok(cats(&api_site::country_list())),
        "next.isKnown" => ok(next_site::is_known(&s(a, "url"))),
        "kk.lang" => ok(kk_phim::lang(&s(a, "label"))),
        "kk.norm" => ok(kk_phim::norm(&s(a, "s"))),
        "imdb.canRank" => ok(imdb_top::can_rank(&s(a, "url"))),
        "imdb.setCacheFile" => {
            imdb_top::set_cache_file(opt_s(a, "path").map(std::path::PathBuf::from));
            Ok(Value::Null)
        }
        "feed.restingHosts" => ok(feed::resting_hosts()),
        "feed.clearRest" => {
            feed::clear_rest();
            Ok(Value::Null)
        }
        "ad.filters" => ok(ad_block::filters()),
        "ad.isAd" => ok(ad_block::is_ad(opt_s(a, "host").as_deref())),
        "home.allSources" => ok(home_feed::all_sources()),
        "home.markSeen" => {
            let d: FilmDetail = match de(a, "detail") {
                Ok(d) => d,
                Err(e) => return Some(Err(e)),
            };
            home_feed::mark_seen(&d);
            Ok(Value::Null)
        }
        "home.makePlan" => Ok(keep_plan(home_feed::make_plan(a.get("tagsKnownOnly").and_then(|v| v.as_bool()).unwrap_or(false)))),
        "home.cached" => Ok(plan_of(a).and_then(|p| home_feed::cached(&p)).map(view).unwrap_or(Value::Null)),
        "home.last" => Ok(home_feed::last(&s(a, "pid")).map(view).unwrap_or(Value::Null)),
        _ => return None,
    })
}

/// Hàm chạy nền (mạng).
pub async fn call(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        "site.detail" => site::detail(&s(a, "url")).await.map_err(err).and_then(ok),
        "site.homeOf" => site::home_of(opt_s(a, "baseUrl").as_deref()).await.map_err(err).and_then(ok),
        "site.check" => site::check(&s(a, "baseUrl")).await.map_err(err).and_then(ok),
        "site.list" => site::list(&s(a, "url")).await.map_err(err).and_then(ok),
        "site.search" => site::search(&s(a, "query"), opt_s(a, "baseUrl").as_deref()).await.map_err(err).and_then(ok),
        "site.searchAll" => site::search_all(&s(a, "query")).await.map_err(err).and_then(ok),
        "site.videoOf" => {
            let src: EpisodeSource = match de(a, "src") {
                Ok(x) => x,
                Err(e) => return Some(Err(e)),
            };
            let server: Option<Server> = de(a, "server").unwrap_or(None);
            site::video_of(&src, server.as_ref()).await.map_err(err).and_then(ok)
        }
        "next.filters" => next_site::filters(&s(a, "baseUrl")).await.map_err(err).map(|(g, c)| json!({ "genres": g, "countries": c })),
        "kk.source" => {
            let d: FilmDetail = match de(a, "detail") {
                Ok(x) => x,
                Err(e) => return Some(Err(e)),
            };
            let g: Option<EpisodeGroup> = de(a, "group").unwrap_or(None);
            let ep: Episode = match de(a, "ep") {
                Ok(x) => x,
                Err(e) => return Some(Err(e)),
            };
            kk_phim::source(&d, g.as_ref(), &ep).await.map_err(err).and_then(ok)
        }
        "related.load" => ok(related::load(&s(a, "title"), &s(a, "url")).await),
        "imdb.forList" => imdb_top::for_list(&s(a, "url")).await.map_err(err).and_then(ok),
        "imdb.page" => imdb_top::page(a.get("kids").and_then(|v| v.as_bool()).unwrap_or(false)).await.map_err(err).and_then(ok),
        "home.load" => match plan_of(a) {
            Some(p) => Ok(view(home_feed::load(p).await)),
            None => Err("Kế hoạch trang chủ đã hết hạn".into()),
        },
        "home.late" => {
            let snap = snap_of(a);
            match snap {
                Some(s) => Ok(home_feed::late(&s).await.map(view).unwrap_or(Value::Null)),
                None => Ok(Value::Null),
            }
        }
        _ => return None,
    })
}

// ---- Kế hoạch / kết quả trang chủ giữ trong lõi ----

const KEEP: usize = 8;

struct Store {
    next: u64,
    plans: Vec<(u64, Plan)>,
    snaps: Vec<(u64, Snapshot)>,
}

static STORE: Mutex<Store> = Mutex::new(Store { next: 1, plans: Vec::new(), snaps: Vec::new() });

fn keep_plan(p: Plan) -> Value {
    let count = p.src.count();
    let mut st = STORE.lock().unwrap();
    let h = st.next;
    st.next += 1;
    st.plans.push((h, p));
    let n = st.plans.len();
    if n > KEEP {
        st.plans.drain(..n - KEEP);
    }
    json!({ "handle": h, "srcCount": count })
}

fn plan_of(a: &Value) -> Option<Plan> {
    let h = a.get("plan").and_then(|v| v.as_u64())?;
    STORE.lock().unwrap().plans.iter().find(|(k, _)| *k == h).map(|(_, p)| p.clone())
}

fn snap_of(a: &Value) -> Option<Snapshot> {
    let h = a.get("snap").and_then(|v| v.as_u64())?;
    STORE.lock().unwrap().snaps.iter().find(|(k, _)| *k == h).map(|(_, s)| s.clone())
}

/// Phần hiện lên màn hình của một kết quả + mã số để hỏi «nguồn chậm đã về chưa» (home.late).
fn view(snap: Snapshot) -> Value {
    let v = json!({
        "rows": snap.rows,
        "fresh": snap.fresh,
        "categories": snap.categories,
        "movedTo": snap.moved_to.as_ref().map(|(o, n)| json!({ "old": o, "new": n })),
        "note": snap.note,
        "noteGaveUp": snap.note_gave_up,
        "empty": snap.empty,
        "fromCache": snap.from_cache,
        "pending": snap.pending,
        "srcCount": snap.plan.as_ref().map(|p| p.src.count()).unwrap_or(0),
    });
    let mut st = STORE.lock().unwrap();
    let h = st.next;
    st.next += 1;
    st.snaps.push((h, snap));
    let n = st.snaps.len();
    if n > KEEP {
        st.snaps.drain(..n - KEEP);
    }
    let mut v = v;
    v["handle"] = json!(h);
    v
}
