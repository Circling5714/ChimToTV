//! Đồng bộ trong nhà (TV / iPad / Windows / Mac / Linux) — bảng hàm cho SyncService.cs của app WPF. Sự kiện của lõi (dữ liệu
//! vừa gộp, trạng thái, lời báo) chuyển sang app qua callback(0, {"event": "sync.…"}).

use crate::{event, Res};
use cbtv::sync_service::{self, SyncEvent};
use serde_json::{json, Value};

fn s(a: &Value, k: &str) -> String {
    a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

/// Nối sự kiện của lõi sang app (một lần lúc mở lõi).
pub fn init() {
    sync_service::add_listener(|e| match e {
        SyncEvent::DataChanged => event("sync.dataChanged", Value::Null),
        SyncEvent::StateChanged => event("sync.stateChanged", Value::Null),
        SyncEvent::Message(m) => event("sync.message", json!(m)),
    });
}

pub fn sync(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        "sync.port" => Ok(json!(sync_service::port())),
        "sync.pairCode" => Ok(json!(sync_service::pair_code())),
        "sync.found" => Ok(json!(sync_service::found()
            .into_iter()
            .map(|f| json!({ "id": f.id, "name": f.name, "platform": f.platform, "host": f.host, "port": f.port }))
            .collect::<Vec<_>>())),
        "sync.busy" => Ok(json!(sync_service::busy())),
        "sync.listening" => Ok(json!(sync_service::listening())),
        "sync.peers" => serde_json::to_value(sync_service::peers()).map_err(|e| e.to_string()),
        "sync.unpair" => {
            sync_service::unpair(&s(a, "id"));
            Ok(Value::Null)
        }
        "sync.appStarted" => {
            sync_service::app_started();
            Ok(Value::Null)
        }
        "sync.start" => {
            sync_service::start();
            Ok(Value::Null)
        }
        "sync.stop" => {
            sync_service::stop();
            Ok(Value::Null)
        }
        "sync.openPairing" => {
            sync_service::open_pairing();
            Ok(Value::Null)
        }
        "sync.closePairing" => {
            sync_service::close_pairing();
            Ok(Value::Null)
        }
        "sync.parseAddress" => Ok(sync_service::parse_address(&s(a, "input")).map(|(h, p)| json!({ "host": h, "port": p })).unwrap_or(Value::Null)),
        "sync.soon" => {
            sync_service::sync_soon(a.get("seconds").and_then(|v| v.as_f64()).unwrap_or(3.0));
            Ok(Value::Null)
        }
        "sync.localIps" => Ok(json!(sync_service::local_ips())),
        "sync.addressText" => Ok(json!(sync_service::address_text())),
        _ => return None,
    })
}

pub async fn call(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        "sync.browse" => Ok(json!(sync_service::browse()
            .await
            .into_iter()
            .map(|f| json!({ "id": f.id, "name": f.name, "platform": f.platform, "host": f.host, "port": f.port }))
            .collect::<Vec<_>>())),
        // Ghép với máy kia bằng mã 4 số: lỗi → câu báo, được → null
        "sync.pair" => {
            let port = a.get("port").and_then(|v| v.as_u64()).unwrap_or(0).min(65535) as u16;
            Ok(json!(sync_service::pair(&s(a, "host"), port, &s(a, "code")).await))
        }
        "sync.all" => {
            sync_service::sync_all(a.get("manual").and_then(|v| v.as_bool()).unwrap_or(false)).await;
            Ok(Value::Null)
        }
        _ => return None,
    })
}
