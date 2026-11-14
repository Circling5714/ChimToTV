//! Cổng phát nội bộ cho trình phát native (libmpv) của app WPF — như bản Linux (app/src/player.rs): mpv chỉ tải từ
//! 127.0.0.1, lõi tải hộ qua mạng của app, bỏ quảng cáo, cắt phần ngụy trang PNG, nối khúc MP4; luồng có lớp bảo vệ riêng của
//! trang (streamfree, NguonC) thì không phát native.

use crate::Res;
use cbtv::media_proxy;
use serde_json::{json, Value};

fn s(a: &Value, k: &str) -> String {
    a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn opt_s(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string())
}

fn hls(a: &Value) -> bool {
    a.get("hls").and_then(|v| v.as_bool()).unwrap_or(false)
}

pub fn sync(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        // Mở phiên phát: {url, subtitle, token} cho mpv; null = địa chỉ không dùng được
        "media.open" => Ok(media_proxy::open(&s(a, "src"), opt_s(a, "sub").as_deref(), opt_s(a, "page").as_deref(), hls(a))
            .map(|o| json!({ "url": o.url, "subtitle": o.subtitle, "token": o.token }))
            .unwrap_or(Value::Null)),
        "media.isPlaylist" => Ok(json!(media_proxy::is_playlist(&s(a, "src")))),
        "media.closeAll" => {
            media_proxy::close_all();
            Ok(Value::Null)
        }
        _ => return None,
    })
}

pub async fn call(m: &str, a: &Value) -> Option<Res> {
    Some(match m {
        // Kiểm trước link luồng của trình phát nhúng (trang vẫn mở): HLS thường / tệp video → true
        "media.probe" => Ok(json!(media_proxy::probe(&s(a, "src"), opt_s(a, "page").as_deref(), hls(a)).await)),
        _ => return None,
    })
}
