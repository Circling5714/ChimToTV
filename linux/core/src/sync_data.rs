//! Đồng bộ trong nhà (TV ↔ iPad / iPhone ↔ Windows ↔ Linux): dữ liệu trao đổi và cách gộp — giống hệt bản Android (Sync.kt),
//! iPad (SyncData.swift) và Windows (SyncData.cs), cùng định dạng JSON. Mỗi máy gửi toàn bộ dữ liệu của mình, máy kia gộp rồi
//! trả lại bản đã gộp; gộp xong hai máy như nhau. Xoá cũng được đồng bộ nhờ "bia mộ" (thời điểm xoá).
//!
//! Đọc / ghi kho cài đặt (C# `Prefs.SyncProfileData` / `ApplySync` / `SyncSitesData` / `ApplySites`) cũng nằm ở đây, trên các
//! hàm thô của prefs (`raw_profile_data`, `write_profile_data`, `write_sites_data`, `stamps`).

use crate::models::{Follow, HistoryItem, Position, Profile};
use crate::prefs;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Mốc thời gian theo khoá ({"khoá": ms}) — "bia mộ" (lúc xoá) hoặc lúc thêm.
pub type Stamps = BTreeMap<String, i64>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncHistory {
    #[serde(deserialize_with = "de::string")]
    pub film: String,
    #[serde(deserialize_with = "de::string")]
    pub title: String,
    #[serde(deserialize_with = "de::string")]
    pub poster: String,
    #[serde(deserialize_with = "de::string")]
    pub ep: String,
    #[serde(deserialize_with = "de::string")]
    pub ep_url: String,
    #[serde(deserialize_with = "de::int")]
    pub time: i64,
}

impl SyncHistory {
    pub fn new(film: &str, title: &str, poster: &str, ep: &str, ep_url: &str, time: i64) -> SyncHistory {
        SyncHistory {
            film: film.into(),
            title: title.into(),
            poster: poster.into(),
            ep: ep.into(),
            ep_url: ep_url.into(),
            time,
        }
    }
}

/// Chỗ đang xem một tập (giây; 0 = đã xem hết), t = lúc lưu (0 = không rõ).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncPos {
    #[serde(deserialize_with = "de::string")]
    pub ep: String,
    #[serde(deserialize_with = "de::int")]
    pub sec: i64,
    #[serde(deserialize_with = "de::string")]
    pub film: String,
    #[serde(deserialize_with = "de::int")]
    pub t: i64,
}

impl SyncPos {
    pub fn new(ep: &str, sec: i64, film: &str, t: i64) -> SyncPos {
        SyncPos { ep: ep.into(), sec, film: film.into(), t }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncFollow {
    #[serde(deserialize_with = "de::string")]
    pub film: String,
    #[serde(deserialize_with = "de::string")]
    pub title: String,
    #[serde(deserialize_with = "de::string")]
    pub poster: String,
    #[serde(deserialize_with = "de::int")]
    pub count: i64,
    #[serde(deserialize_with = "de::string")]
    pub latest: String,
    #[serde(deserialize_with = "de::int")]
    pub seen: i64,
    #[serde(deserialize_with = "de::int")]
    pub t: i64,
}

impl SyncFollow {
    pub fn new(film: &str, title: &str, poster: &str, count: i64, latest: &str, seen: i64, t: i64) -> SyncFollow {
        SyncFollow {
            film: film.into(),
            title: title.into(),
            poster: poster.into(),
            count,
            latest: latest.into(),
            seen,
            t,
        }
    }
}

/// Dữ liệu riêng một người xem (id "" = người xem đầu tiên).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncProfile {
    #[serde(deserialize_with = "de::string")]
    pub id: String,
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::list")]
    pub history: Vec<SyncHistory>,
    #[serde(deserialize_with = "de::stamps")]
    pub hist_del: Stamps,
    #[serde(deserialize_with = "de::list")]
    pub pos: Vec<SyncPos>,
    #[serde(deserialize_with = "de::list")]
    pub follows: Vec<SyncFollow>,
    #[serde(deserialize_with = "de::stamps")]
    pub follow_del: Stamps,
}

impl SyncProfile {
    /// Cùng dữ liệu (không so tên người xem).
    pub fn same_as(&self, o: &SyncProfile) -> bool {
        self.id == o.id
            && self.history == o.history
            && self.pos == o.pos
            && self.follows == o.follows
            && self.hist_del == o.hist_del
            && self.follow_del == o.follow_del
    }
}

/// Danh sách trang phim (dùng chung cả máy): thứ tự, lúc thêm, lúc bỏ.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncSites {
    #[serde(deserialize_with = "de::list")]
    pub list: Vec<String>,
    #[serde(deserialize_with = "de::stamps")]
    pub add: Stamps,
    #[serde(deserialize_with = "de::stamps")]
    pub del: Stamps,
}

impl SyncSites {
    pub fn same_as(&self, o: &SyncSites) -> bool {
        self.list == o.list && self.add == o.add && self.del == o.del
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncPayload {
    #[serde(deserialize_with = "de::int")]
    pub v: i64,
    #[serde(deserialize_with = "de::string")]
    pub device: String,
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::string")]
    pub platform: String,
    #[serde(deserialize_with = "de::list")]
    pub profiles: Vec<SyncProfile>,
    #[serde(deserialize_with = "de::nullable")]
    pub sites: SyncSites,
}

impl Default for SyncPayload {
    fn default() -> Self {
        SyncPayload {
            v: 1,
            device: String::new(),
            name: String::new(),
            platform: String::new(),
            profiles: Vec::new(),
            sites: SyncSites::default(),
        }
    }
}

impl SyncPayload {
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Đọc dữ liệu máy kia (trường thiếu / null → mặc định; số viết dạng chữ cũng nhận). Hỏng → None.
    pub fn parse(data: &[u8]) -> Option<SyncPayload> {
        let v: serde_json::Value = serde_json::from_slice(data).ok()?;
        if !v.is_object() {
            return None;
        }
        let mut p: SyncPayload = serde_json::from_value(v).ok()?;
        for pr in p.profiles.iter_mut() {
            pr.history.retain(|h| !h.film.is_empty());
            pr.pos.retain(|x| !x.ep.is_empty());
            pr.follows.retain(|f| !f.film.is_empty());
        }
        Some(p)
    }
}

/// Cách gộp dữ liệu hai máy (C# `SyncMerge`).
pub struct SyncMerge;

impl SyncMerge {
    pub const MAX_HISTORY: usize = 60;
    pub const MAX_POSITIONS: usize = 400;
    /// Giữ tối đa bấy nhiêu "bia mộ" mỗi loại (mới nhất).
    pub const MAX_TOMBSTONES: usize = prefs::MAX_TOMBSTONES;

    /// Ghép người xem hai máy: người đầu tiên (id "") với nhau, còn lại theo tên (không phân biệt hoa / thường, khoảng
    /// trắng). Trả về các cặp (chỉ số bên này, chỉ số bên kia). C# `Match`.
    pub fn match_profiles(local: &[SyncProfile], remote: &[SyncProfile]) -> Vec<(usize, usize)> {
        fn key(s: &str) -> String {
            s.trim().to_lowercase()
        }
        let mut out = Vec::new();
        let mut used = HashSet::new();
        let l0 = local.iter().position(|p| p.id.is_empty());
        let r0 = remote.iter().position(|p| p.id.is_empty());
        if let (Some(l0), Some(r0)) = (l0, r0) {
            out.push((l0, r0));
            used.insert(r0);
        }
        for (i, p) in local.iter().enumerate() {
            if p.id.is_empty() {
                continue;
            }
            let j = (0..remote.len())
                .find(|&k| !used.contains(&k) && !remote[k].id.is_empty() && key(&remote[k].name) == key(&p.name));
            let Some(j) = j else { continue };
            out.push((i, j));
            used.insert(j);
        }
        out
    }

    /// Gộp hai bảng mốc (lấy mốc mới hơn), giữ MAX_TOMBSTONES mục mới nhất.
    pub fn tombstones(a: &Stamps, b: &Stamps) -> Stamps {
        prefs::tombstones(a, b)
    }

    /// Gộp dữ liệu một người xem (local: máy này, remote: máy kia).
    pub fn profile(local: &SyncProfile, remote: &SyncProfile) -> SyncProfile {
        let hist_del = Self::tombstones(&local.hist_del, &remote.hist_del);

        // Lịch sử: mỗi phim một mục, lấy lần xem mới hơn; bỏ phim đã xoá sau lần xem đó
        let mut hist: HashMap<&str, &SyncHistory> = HashMap::new();
        for h in local.history.iter().chain(remote.history.iter()) {
            if let Some(old) = hist.get(h.film.as_str()) {
                if old.time >= h.time {
                    continue;
                }
            }
            hist.insert(h.film.as_str(), h);
        }
        let mut history: Vec<SyncHistory> = hist
            .into_values()
            .filter(|h| hist_del.get(&h.film).map(|d| *d < h.time).unwrap_or(true))
            .cloned()
            .collect();
        history.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.film.cmp(&b.film)));
        history.truncate(Self::MAX_HISTORY);

        // Chỗ đang xem: mỗi tập lấy lần lưu mới hơn; phim đã xoá khỏi lịch sử sau lần lưu đó thì bỏ
        let mut pos: HashMap<&str, &SyncPos> = HashMap::new();
        let mut order: Vec<&str> = Vec::new();
        for p in local.pos.iter().chain(remote.pos.iter()) {
            match pos.get(p.ep.as_str()) {
                Some(old) => {
                    if p.t > old.t {
                        pos.insert(p.ep.as_str(), p);
                    }
                }
                None => {
                    pos.insert(p.ep.as_str(), p);
                    order.push(p.ep.as_str());
                }
            }
        }
        let mut kept: Vec<SyncPos> = order
            .iter()
            .filter_map(|e| pos.get(e).copied())
            .filter(|p| hist_del.get(&p.film).map(|d| *d < p.t).unwrap_or(true))
            .cloned()
            .collect();
        // Thứ tự lưu: cũ nhất trước (không rõ lúc lưu — t = 0 — đứng đầu, giữ thứ tự cũ)
        kept.sort_by_key(|p| p.t);
        let skip = kept.len().saturating_sub(Self::MAX_POSITIONS);
        let positions: Vec<SyncPos> = kept.into_iter().skip(skip).collect();

        // Phim theo dõi: hợp hai bên; số tập / số tập đã thấy lấy số lớn hơn; bỏ phim đã bỏ theo dõi sau lần theo dõi đó
        let follow_del = Self::tombstones(&local.follow_del, &remote.follow_del);
        let mut fol: HashMap<String, SyncFollow> = HashMap::new();
        let mut forder: Vec<String> = Vec::new();
        for f in local.follows.iter().chain(remote.follows.iter()) {
            let Some(old) = fol.get(&f.film) else {
                fol.insert(f.film.clone(), f.clone());
                forder.push(f.film.clone());
                continue;
            };
            let latest = if f.count > old.count { f.latest.clone() } else { old.latest.clone() };
            let mut m = SyncFollow { latest, count: old.count.max(f.count), seen: old.seen.max(f.seen), ..old.clone() };
            if f.t > old.t {
                m.t = f.t;
                m.title = f.title.clone();
                if !f.poster.is_empty() {
                    m.poster = f.poster.clone();
                }
            }
            fol.insert(f.film.clone(), m);
        }
        let mut follows: Vec<SyncFollow> = forder
            .iter()
            .filter_map(|k| fol.remove(k))
            .filter(|f| follow_del.get(&f.film).map(|d| *d < f.t).unwrap_or(true))
            .collect();
        // sắp xếp ổn định: cùng lúc thì giữ thứ tự cũ
        follows.sort_by(|a, b| b.t.cmp(&a.t));

        SyncProfile {
            id: local.id.clone(),
            name: local.name.clone(),
            history,
            hist_del,
            pos: positions,
            follows,
            follow_del,
        }
    }

    /// Gộp danh sách trang: trang có mặt nếu lần thêm mới hơn lần bỏ (trang có sẵn: lúc thêm = 0). Thứ tự: trang của máy này
    /// trước, trang mới của máy kia sau.
    pub fn sites(local: &SyncSites, remote: &SyncSites) -> SyncSites {
        let add = Self::tombstones(&local.add, &remote.add);
        let del = Self::tombstones(&local.del, &remote.del);
        let mut list: Vec<String> = Vec::new();
        for s in local.list.iter().chain(remote.list.iter()) {
            if list.contains(s) {
                continue;
            }
            if let Some(d) = del.get(s) {
                if *d >= add.get(s).copied().unwrap_or(0) {
                    continue;
                }
            }
            list.push(s.clone());
        }
        if list.is_empty() {
            list = local.list.clone(); // không để trống danh sách
        }
        SyncSites { list, add, del }
    }
}

// ---- Đọc / ghi kho cài đặt (C# Prefs.SyncProfileData / ApplySync / SyncSitesData / ApplySites) ----

/// Dữ liệu đồng bộ của người xem p.
pub fn sync_profile_data(p: &Profile) -> SyncProfile {
    let id = p.id.as_str();
    let (hist, pos, fol) = prefs::raw_profile_data(id);
    SyncProfile {
        id: id.to_string(),
        name: p.name.clone(),
        history: hist
            .into_iter()
            .map(|h| SyncHistory {
                film: h.film_url,
                title: h.title,
                poster: h.poster.unwrap_or_default(),
                ep: h.ep_title,
                ep_url: h.ep_url,
                time: h.time,
            })
            .collect(),
        hist_del: prefs::stamps(&prefs::pk(id, "hist_del")),
        pos: pos.into_iter().map(|x| SyncPos { ep: x.ep, sec: x.sec, film: x.film, t: x.t.unwrap_or(0) }).collect(),
        follows: fol
            .into_iter()
            .map(|f| SyncFollow {
                film: f.film_url,
                title: f.title,
                poster: f.poster.unwrap_or_default(),
                count: f.count,
                latest: f.latest,
                seen: f.seen,
                t: f.t.unwrap_or(0),
            })
            .collect(),
        follow_del: prefs::stamps(&prefs::pk(id, "follow_del")),
    }
}

/// Ghi dữ liệu đã gộp cho người xem s.id. true = có thay đổi.
pub fn apply_sync(s: &SyncProfile) -> bool {
    let Some(p) = prefs::profiles().into_iter().find(|x| x.id == s.id) else { return false };
    if sync_profile_data(&p).same_as(s) {
        return false;
    }
    let history = s
        .history
        .iter()
        .map(|h| HistoryItem {
            film_url: h.film.clone(),
            title: h.title.clone(),
            poster: crate::util::non_empty(Some(h.poster.as_str())),
            ep_title: h.ep.clone(),
            ep_url: h.ep_url.clone(),
            time: h.time,
        })
        .collect();
    let pos = s
        .pos
        .iter()
        .map(|x| Position { ep: x.ep.clone(), sec: x.sec, film: x.film.clone(), t: if x.t == 0 { None } else { Some(x.t) } })
        .collect();
    let follows = s
        .follows
        .iter()
        .map(|f| Follow {
            film_url: f.film.clone(),
            title: f.title.clone(),
            poster: crate::util::non_empty(Some(f.poster.as_str())),
            count: f.count,
            latest: f.latest.clone(),
            seen: f.seen,
            t: if f.t == 0 { None } else { Some(f.t) },
        })
        .collect();
    prefs::write_profile_data(&s.id, history, pos, follows, s.hist_del.clone(), s.follow_del.clone());
    true
}

/// Danh sách trang phim (dùng chung cả máy) kèm mốc thêm / bỏ.
pub fn sync_sites_data() -> SyncSites {
    SyncSites { list: prefs::sites(), add: prefs::stamps("sites_add"), del: prefs::stamps("sites_del") }
}

/// Ghi danh sách trang đã gộp (không ghi lại mốc thêm / bỏ như khi người xem tự sửa). true = có thay đổi.
pub fn apply_sites(s: &SyncSites) -> bool {
    if s.list.is_empty() || s.same_as(&sync_sites_data()) {
        return false;
    }
    prefs::write_sites_data(s.list.clone(), s.add.clone(), s.del.clone());
    true
}

/// Đọc JSON dễ dãi như bản Windows (null → mặc định, số viết dạng chữ cũng nhận).
mod de {
    use super::Stamps;
    use serde::de::{DeserializeOwned, Error};
    use serde::{Deserialize, Deserializer};
    use serde_json::Value;

    pub fn string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        match Value::deserialize(d)? {
            Value::Null => Ok(String::new()),
            Value::String(s) => Ok(s),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            _ => Err(D::Error::custom("cần chuỗi")),
        }
    }

    fn int_of<E: Error>(v: &Value) -> Result<i64, E> {
        match v {
            Value::Null => Ok(0),
            Value::Number(n) => n
                .as_i64()
                .or_else(|| n.as_u64().map(|u| u.min(i64::MAX as u64) as i64))
                .or_else(|| n.as_f64().map(|f| f as i64))
                .ok_or_else(|| E::custom("cần số")),
            Value::String(s) => s.trim().parse::<i64>().map_err(|_| E::custom("cần số")),
            _ => Err(E::custom("cần số")),
        }
    }

    pub fn int<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        int_of(&Value::deserialize(d)?)
    }

    pub fn stamps<'de, D: Deserializer<'de>>(d: D) -> Result<Stamps, D::Error> {
        match Value::deserialize(d)? {
            Value::Null => Ok(Stamps::new()),
            Value::Object(o) => {
                let mut m = Stamps::new();
                for (k, v) in o {
                    m.insert(k, int_of::<D::Error>(&v)?);
                }
                Ok(m)
            }
            _ => Err(D::Error::custom("cần bảng mốc")),
        }
    }

    pub fn list<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Vec<T>, D::Error> {
        match Value::deserialize(d)? {
            Value::Null => Ok(Vec::new()),
            Value::Array(a) => a
                .into_iter()
                .filter(|v| !v.is_null())
                .map(|v| T::deserialize(v).map_err(D::Error::custom))
                .collect(),
            _ => Err(D::Error::custom("cần mảng")),
        }
    }

    pub fn nullable<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(d: D) -> Result<T, D::Error> {
        match Value::deserialize(d)? {
            Value::Null => Ok(T::default()),
            v => T::deserialize(v).map_err(D::Error::custom),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Cùng ca với SyncTests.cs (Windows) / SyncMergeTests.swift / SyncTest.kt.

    fn h(film: &str, time: i64, ep: &str) -> SyncHistory {
        SyncHistory::new(film, &film.to_uppercase(), "", ep, &format!("{film}/{ep}"), time)
    }

    fn f(film: &str, count: i64, seen: i64, t: i64) -> SyncFollow {
        SyncFollow::new(film, film, "", count, &format!("Tập {count}"), seen, t)
    }

    fn prof(name: &str) -> SyncProfile {
        SyncProfile { name: name.into(), ..Default::default() }
    }

    fn stamps(items: &[(&str, i64)]) -> Stamps {
        items.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn films(l: &[SyncHistory]) -> Vec<&str> {
        l.iter().map(|x| x.film.as_str()).collect()
    }

    #[test]
    fn history_newest_wins_and_union() {
        let a = SyncProfile { history: vec![h("x", 100, "e1"), h("y", 50, "e1")], ..prof("Cả nhà") };
        let b = SyncProfile { history: vec![h("x", 200, "e2"), h("z", 150, "e1")], ..prof("Cả nhà") };
        let m = SyncMerge::profile(&a, &b);
        assert_eq!(films(&m.history), ["x", "z", "y"]);
        assert_eq!(m.history[0].ep, "e2");
        assert_eq!(m.history, SyncMerge::profile(&b, &a).history);
    }

    #[test]
    fn deleted_history_stays_deleted() {
        let a = SyncProfile { hist_del: stamps(&[("x", 300)]), ..prof("n") };
        let b = SyncProfile {
            history: vec![h("x", 200, "e1")],
            pos: vec![SyncPos::new("x/e1", 30, "x", 200), SyncPos::new("y/e1", 5, "y", 10)],
            ..prof("n")
        };
        let m = SyncMerge::profile(&a, &b);
        assert!(m.history.is_empty());
        assert_eq!(m.pos.iter().map(|p| p.ep.as_str()).collect::<Vec<_>>(), ["y/e1"]);
        assert_eq!(m.hist_del["x"], 300);
        let c = SyncProfile { history: vec![h("x", 400, "e1")], ..prof("n") };
        assert_eq!(films(&SyncMerge::profile(&m, &c).history), ["x"]);
    }

    #[test]
    fn positions_newest_wins_oldest_first() {
        let a = SyncProfile { pos: vec![SyncPos::new("old", 1, "f", 0), SyncPos::new("e", 100, "f", 500)], ..prof("n") };
        let b = SyncProfile { pos: vec![SyncPos::new("e", 250, "f", 600), SyncPos::new("n", 7, "f", 50)], ..prof("n") };
        let m = SyncMerge::profile(&a, &b);
        assert_eq!(m.pos.iter().map(|p| p.ep.as_str()).collect::<Vec<_>>(), ["old", "n", "e"]);
        assert_eq!(m.pos.last().unwrap().sec, 250);
    }

    #[test]
    fn follows_merge_and_unfollow() {
        let a = SyncProfile { follows: vec![f("x", 10, 10, 100), f("y", 3, 1, 50)], ..prof("n") };
        let b = SyncProfile { follows: vec![f("x", 12, 9, 100)], follow_del: stamps(&[("y", 80)]), ..prof("n") };
        let m = SyncMerge::profile(&a, &b);
        assert_eq!(m.follows.iter().map(|x| x.film.as_str()).collect::<Vec<_>>(), ["x"]);
        assert_eq!(m.follows[0].count, 12);
        assert_eq!(m.follows[0].seen, 10);
        assert_eq!(m.follows[0].latest, "Tập 12");
        let c = SyncProfile { follows: vec![f("y", 3, 3, 90)], ..prof("n") };
        let mut got: Vec<String> = SyncMerge::profile(&m, &c).follows.into_iter().map(|x| x.film).collect();
        got.sort();
        assert_eq!(got, ["x", "y"]);
    }

    #[test]
    fn follows_newer_title_and_poster() {
        let a = SyncProfile { follows: vec![SyncFollow::new("x", "Cũ", "p1", 5, "Tập 5", 5, 100)], ..prof("n") };
        let b = SyncProfile { follows: vec![SyncFollow::new("x", "Mới", "", 4, "Tập 4", 4, 200)], ..prof("n") };
        let m = SyncMerge::profile(&a, &b);
        let x = &m.follows[0];
        assert_eq!((x.title.as_str(), x.poster.as_str(), x.count, x.latest.as_str(), x.t), ("Mới", "p1", 5, "Tập 5", 200));
    }

    #[test]
    fn match_profiles() {
        let p = |id: &str, name: &str| SyncProfile { id: id.into(), name: name.into(), ..Default::default() };
        let local = vec![p("", "Cả nhà"), p("p1", "Bố"), p("p2", "Bé Na")];
        let remote = vec![p("p3", " bé na "), p("", "Nhà mình"), p("p1", "Mẹ")];
        let got: Vec<String> =
            SyncMerge::match_profiles(&local, &remote).iter().map(|(l, r)| format!("{l}-{r}")).collect();
        assert_eq!(got, ["0-1", "2-0"]);
    }

    #[test]
    fn sites() {
        let a = SyncSites {
            list: vec!["a".into(), "b".into(), "c".into()],
            add: stamps(&[("c", 100)]),
            del: stamps(&[("d", 100)]),
        };
        let b = SyncSites {
            list: vec!["a".into(), "d".into(), "e".into()],
            add: stamps(&[("d", 50), ("e", 200)]),
            del: stamps(&[("b", 300)]),
        };
        let m = SyncMerge::sites(&a, &b);
        assert_eq!(m.list, ["a", "c", "e"]);
        let mut x = m.list.clone();
        let mut y = SyncMerge::sites(&b, &a).list;
        x.sort();
        y.sort();
        assert_eq!(x, y);
        // không để trống danh sách
        let gone = SyncSites { list: vec![], add: Stamps::new(), del: stamps(&[("a", 999)]) };
        let only = SyncSites { list: vec!["a".into()], ..Default::default() };
        assert_eq!(SyncMerge::sites(&only, &gone).list, ["a"]);
    }

    #[test]
    fn tombstone_cap() {
        let big: Stamps = (0..SyncMerge::MAX_TOMBSTONES + 20).map(|i| (format!("k{i}"), i as i64)).collect();
        let m = SyncMerge::tombstones(&big, &Stamps::new());
        assert_eq!(m.len(), SyncMerge::MAX_TOMBSTONES);
        assert!(!m.contains_key("k0"));
        assert!(m.contains_key(&format!("k{}", SyncMerge::MAX_TOMBSTONES + 19)));
    }

    fn keys(v: &serde_json::Value) -> Vec<String> {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    }

    fn sorted(a: &[&str]) -> Vec<String> {
        let mut k: Vec<String> = a.iter().map(|s| s.to_string()).collect();
        k.sort();
        k
    }

    #[test]
    fn json_shape_matches_other_apps() {
        let p = SyncPayload {
            device: "d".into(),
            name: "PC".into(),
            platform: "linux".into(),
            profiles: vec![SyncProfile { history: vec![h("x", 1, "e1")], ..prof("Cả nhà") }],
            sites: SyncSites { list: vec!["a".into()], ..Default::default() },
            ..Default::default()
        };
        let root: serde_json::Value = serde_json::from_slice(&p.to_json()).unwrap();
        assert_eq!(root["v"], 1);
        assert_eq!(keys(&root), sorted(&["v", "device", "name", "platform", "profiles", "sites"]));
        let p0 = &root["profiles"][0];
        assert_eq!(keys(p0), sorted(&["id", "name", "history", "histDel", "pos", "follows", "followDel"]));
        assert_eq!(keys(&p0["history"][0]), sorted(&["film", "title", "poster", "ep", "epUrl", "time"]));
        assert_eq!(keys(&root["sites"]), sorted(&["list", "add", "del"]));
        let full = SyncProfile {
            pos: vec![SyncPos::new("e", 1, "f", 2)],
            follows: vec![f("x", 1, 1, 1)],
            ..prof("n")
        };
        let v = serde_json::to_value(&full).unwrap();
        assert_eq!(keys(&v["pos"][0]), sorted(&["ep", "sec", "film", "t"]));
        assert_eq!(keys(&v["follows"][0]), sorted(&["film", "title", "poster", "count", "latest", "seen", "t"]));
    }

    #[test]
    fn parses_android_payload_with_missing_fields() {
        let json = r#"
        {"v":1,"device":"tv1","name":"TX3","platform":"android","profiles":[{"id":"","name":"Cả nhà",
         "history":[{"film":"f","title":"T","poster":"","ep":"Tập 1","epUrl":"f/1","time":5}],
         "histDel":{},"pos":[{"ep":"f/1","sec":30,"film":"f","t":5}],"follows":[],"followDel":{"g":7}}],
         "sites":{"list":["https://a"],"add":{},"del":{}}}
        "#;
        let p = SyncPayload::parse(json.as_bytes()).unwrap();
        assert_eq!(p.platform, "android");
        assert_eq!(p.profiles[0].pos[0].sec, 30);
        assert_eq!(p.profiles[0].follow_del["g"], 7);
        let bare =
            SyncPayload::parse(br#"{"device":"x","profiles":[{"id":"","history":[{"film":"f","time":1}]}]}"#).unwrap();
        assert_eq!(bare.v, 1);
        assert_eq!(bare.profiles[0].history[0].title, "");
        assert!(bare.sites.list.is_empty());
    }

    #[test]
    fn parse_is_lenient_but_rejects_garbage() {
        // null → mặc định, số dạng chữ (AllowReadingFromString), mục thiếu phim / tập bị bỏ
        let json = br#"{"device":null,"profiles":[{"id":null,"name":"A","history":[{"film":"","time":1},null,{"film":"g","time":"9"}],
            "pos":[{"ep":null,"sec":1},{"ep":"e","sec":"12","t":3.0}],"follows":null,"histDel":{"x":"5"},"followDel":null}],
            "sites":null}"#;
        let p = SyncPayload::parse(json).unwrap();
        assert_eq!(p.device, "");
        let pr = &p.profiles[0];
        assert_eq!(pr.id, "");
        assert_eq!(films(&pr.history), ["g"]);
        assert_eq!(pr.history[0].time, 9);
        assert_eq!(pr.pos, [SyncPos::new("e", 12, "", 3)]);
        assert!(pr.follows.is_empty());
        assert_eq!(pr.hist_del["x"], 5);
        assert!(SyncPayload::parse(b"not json").is_none());
        assert!(SyncPayload::parse(b"[1,2]").is_none());
        assert!(SyncPayload::parse(br#"{"profiles":[{"history":[{"film":{"a":1}}]}]}"#).is_none());
    }

    #[test]
    fn same_as_ignores_name() {
        let a = SyncProfile { history: vec![h("x", 1, "e1")], ..prof("A") };
        let mut b = a.clone();
        b.name = "B".into();
        assert!(a.same_as(&b));
        b.hist_del.insert("y".into(), 1);
        assert!(!a.same_as(&b));
    }
}
