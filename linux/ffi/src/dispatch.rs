//! Bảng hàm của lõi mà app WPF gọi được: «nhóm.hàm» → hàm của crate cbtv. Tham số / kết quả là JSON (tên trường camelCase
//! như thuộc tính C#). Thêm hàm: thêm một dòng ở bảng nhanh (sync) hoặc bảng chạy nền (call) và hàm tương ứng trong
//! windows/ConBangTV/Core.

use crate::{emit, Res};
use cbtv::{http, log, net, prefs};
use serde_json::{json, Value};

/// Việc làm một lần lúc mở lõi.
pub fn init() {}

// ---- Đọc tham số ----

fn s(a: &Value, k: &str) -> String {
    a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn opt_s(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn b(a: &Value, k: &str) -> bool {
    a.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn i(a: &Value, k: &str) -> i64 {
    a.get(k).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))).unwrap_or(0)
}

fn ok<T: serde::Serialize>(v: T) -> Res {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn err(e: cbtv::util::Error) -> String {
    cbtv::util::friendly(&e)
}

// ---- Bảng hàm nhanh ----

/// Hàm nhanh (chạy ngay trong luồng gọi). None = không có trong bảng này.
pub fn sync(m: &str, a: &Value) -> Option<Res> {
    if let Some(r) = crate::sites::sync(m, a) {
        return Some(r);
    }
    let (group, name) = m.split_once('.')?;
    match group {
        "prefs" => prefs_fn(name, a),
        "log" => log_fn(name, a),
        "net" => net_fn(name, a),
        "app" => app_fn(name, a),
        _ => None,
    }
}

/// Kho cài đặt (prefs.json) — Prefs.cs đọc / ghi từng khoá, các hàm nhỏ vẫn ở C#.
fn prefs_fn(name: &str, a: &Value) -> Option<Res> {
    Some(match name {
        "get" => Ok(prefs::get_raw(&s(a, "key")).unwrap_or(Value::Null)),
        "set" => {
            prefs::set_raw(&s(a, "key"), a.get("value").cloned());
            Ok(Value::Null)
        }
        "has" => ok(prefs::has(&s(a, "key"))),
        "keys" => ok(prefs::keys()),
        // Mật khẩu proxy / khoá Shadowsocks (Windows: DPAPI, khoá <tên>_dpapi)
        "secret" => ok(cbtv::secret::get(&s(a, "name"))),
        "setSecret" => {
            cbtv::secret::set(&s(a, "name"), &s(a, "value"));
            Ok(Value::Null)
        }
        "flush" => {
            prefs::flush();
            Ok(Value::Null)
        }
        "dataDir" => ok(prefs::data_dir().to_string_lossy().to_string()),
        "dump" => Ok(serde_json::from_str(&prefs::debug_dump()).unwrap_or(Value::Null)),
        _ => return None,
    })
}

fn log_fn(name: &str, a: &Value) -> Option<Res> {
    Some(match name {
        "d" => {
            log::d(&s(a, "tag"), &s(a, "message"));
            Ok(Value::Null)
        }
        "since" => {
            let (seq, lines) = log::since(i(a, "since").max(0) as u64);
            ok(json!({ "seq": seq, "lines": lines }))
        }
        _ => return None,
    })
}

fn net_fn(name: &str, _a: &Value) -> Option<Res> {
    Some(match name {
        // Cổng SOCKS5 nội bộ 127.0.0.1 — WebView2 (--proxy-server) và HttpClient của app (ảnh bìa) đi qua đây
        "startBridge" => ok(net::start_bridge()),
        "bridgePort" => ok(net::bridge_port()),
        "settingsChanged" => {
            net::settings_changed();
            Ok(Value::Null)
        }
        _ => return None,
    })
}

fn app_fn(name: &str, a: &Value) -> Option<Res> {
    Some(match name {
        "version" => ok(cbtv::version()),
        "userAgent" => ok(http::user_agent()),
        "setUserAgent" => {
            http::set_user_agent(&s(a, "ua"));
            Ok(Value::Null)
        }
        _ => return None,
    })
}

// ---- Bảng hàm chạy nền ----

/// Hàm chạy nền (mạng…). Hàm nhanh cũng gọi được theo cách này.
pub async fn call(id: u64, m: &str, a: Value) -> Res {
    if let Some(r) = sync(m, &a) {
        return r;
    }
    if let Some(r) = crate::sites::call(m, &a).await {
        return r;
    }
    match m {
        // Tải một trang / tệp nhỏ qua mạng của app (DNS mã hoá, vượt chặn, proxy) → {url, code, contentType, text}
        "http.get" => {
            let headers: Vec<(String, String)> = a
                .get("headers")
                .and_then(|h| h.as_object())
                .map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
                .unwrap_or_default();
            let hs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let r = http::get(&s(&a, "url"), &hs).await.map_err(err)?;
            let text = r.body();
            ok(json!({ "url": r.url, "code": r.code, "contentType": r.content_type, "text": text }))
        }
        // Tải tệp lớn vào đường dẫn path, báo tiến độ ({"part": 0..1}) — bản cập nhật
        "http.download" => {
            let path = std::path::PathBuf::from(s(&a, "path"));
            let last = std::sync::Mutex::new(-1i32);
            http::download(&s(&a, "url"), &path, |p| {
                let pct = (p * 100.0) as i32;
                let mut l = last.lock().unwrap();
                if pct != *l {
                    *l = pct;
                    emit(id, &json!({ "part": p }));
                }
            })
            .await
            .map_err(err)?;
            Ok(Value::Null)
        }
        _ => {
            let _ = (opt_s(&a, ""), b(&a, ""));
            Err(format!("Lõi không có hàm «{m}»"))
        }
    }
}
