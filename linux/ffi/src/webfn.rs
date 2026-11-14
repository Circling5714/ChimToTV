//! Đọc hộ trang bật chống bot của Cloudflare bằng WebView của app (cbtv::web_fetch). Lõi cần đọc → sự kiện
//! `web.fetch` {id, url, method, body, headers}; app (Core/WebFetch.cs) đọc bằng WebView2 ẩn rồi trả `web.reply`
//! {id, code, url, type, b64} hoặc {id, err}. App gọi `web.enable` khi WebView sẵn sàng nhận việc.

use crate::{event, Res};
use cbtv::web_fetch::{self, WebReply, WebRequest};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

type Pending = HashMap<u64, oneshot::Sender<Result<WebReply, String>>>;

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);

fn pending<T>(f: impl FnOnce(&mut Pending) -> T) -> T {
    let mut g = PENDING.lock().unwrap();
    f(g.get_or_insert_with(HashMap::new))
}

/// Bỏ việc khỏi bảng chờ khi bên gọi thôi chờ (hết giờ / huỷ) — app trả muộn thì bỏ qua.
struct Forget(u64);

impl Drop for Forget {
    fn drop(&mut self) {
        pending(|p| p.remove(&self.0));
    }
}

fn provider() -> web_fetch::Provider {
    Arc::new(|req: WebRequest| {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        pending(|p| p.insert(id, tx));
        let headers: serde_json::Map<String, Value> = req.headers.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        event("web.fetch", json!({ "id": id, "url": req.url, "method": req.method, "body": req.body, "headers": headers }));
        Box::pin(async move {
            let _forget = Forget(id);
            rx.await.unwrap_or_else(|_| Err("app thôi đọc trang".into()))
        })
    })
}

fn s(a: &Value, k: &str) -> String {
    a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

pub fn sync(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        // App sẵn sàng (true) / thôi (false) đọc hộ
        "web.enable" => {
            let on = a.get("on").and_then(|v| v.as_bool()).unwrap_or(true);
            web_fetch::set_provider(if on { Some(provider()) } else { None });
            Ok(Value::Null)
        }
        "web.reply" => {
            let id = a.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let reply = if let Some(e) = a.get("err").and_then(|v| v.as_str()) {
                Err(e.to_string())
            } else {
                web_fetch::decode_b64(&s(a, "b64")).map(|data| WebReply {
                    code: a.get("code").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
                    url: s(a, "url"),
                    data,
                    content_type: a.get("type").and_then(|v| v.as_str()).map(|t| t.to_string()),
                })
            };
            if let Some(tx) = pending(|p| p.remove(&id)) {
                let _ = tx.send(reply);
            }
            Ok(Value::Null)
        }
        // Trang của địa chỉ này đang chặn (đã nhớ) — ảnh bìa của trang đó đi qua WebView
        "web.needs" => Ok(json!(web_fetch::needs(&s(a, "url")))),
        // Ghi nhớ trang chặn (app gặp chặn khi tải ảnh)
        "web.remember" => {
            web_fetch::remember(&web_fetch::host_of(&s(a, "url")));
            Ok(Value::Null)
        }
        "web.isChallenge" => Ok(json!(web_fetch::is_challenge(
            a.get("code").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
            a.get("cf").and_then(|v| v.as_str()),
            s(a, "body").as_bytes(),
        ))),
        "web.hosts" => Ok(json!(web_fetch::hosts())),
        _ => return None,
    })
}
