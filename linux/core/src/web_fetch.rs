//! Trang bật lớp chống bot của Cloudflare («Just a moment…», 403 + `cf-mitigated: challenge`): mọi trình tải không phải trình
//! duyệt bị chặn — hoathinh3d.pw từ 14/11/2026, cả trang, API, admin-ajax lẫn ảnh. Trình duyệt thật (WebView) tự qua lớp chặn,
//! nên app đọc hộ bằng một WebView ẩn: mở trang chủ một lần, rồi mỗi yêu cầu là `fetch()` ngay trong trang đó (cùng gốc, cookie
//! của trình duyệt). Không giả / không giải hộ lớp chặn: trang đòi bấm xác nhận thì báo lỗi.
//!
//! Lõi hỏi app qua `Provider` — bản Windows: sự kiện `web.fetch` của conbang_core.dll, app trả bằng `web.reply` (Core/
//! WebFetch.cs); chưa có app đăng ký (bản Linux, test) thì báo lỗi «đang bật chống bot». Trang đã bị chặn được nhớ 12 giờ: các
//! yêu cầu sau đi thẳng qua WebView, khỏi tốn một lần bị 403.

use crate::http::Response;
use crate::util::{Error, Result};
use base64::Engine;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

/// Một yêu cầu nhờ app đọc hộ.
#[derive(Debug, Clone, PartialEq)]
pub struct WebRequest {
    pub url: String,
    /// "GET" / "POST"
    pub method: String,
    /// Thân POST (đã mã hoá form)
    pub body: Option<String>,
    /// Đầu thư cần gửi (Accept, Referer, X-Requested-With, Content-Type…) — WebView tự đặt User-Agent / cookie
    pub headers: Vec<(String, String)>,
}

/// Trả lời của app.
#[derive(Debug, Clone, Default)]
pub struct WebReply {
    pub code: u16,
    /// Địa chỉ cuối (sau chuyển hướng)
    pub url: String,
    pub data: Vec<u8>,
    pub content_type: Option<String>,
}

/// Hàm đọc hộ của app: Err = câu báo lỗi (tiếng Việt).
pub type Provider = Arc<dyn Fn(WebRequest) -> BoxFuture<'static, std::result::Result<WebReply, String>> + Send + Sync>;

static PROVIDER: RwLock<Option<Provider>> = RwLock::new(None);
/// Trang (tên máy) bị chặn → lúc phát hiện (ms)
static HOSTS: Mutex<Option<HashMap<String, i64>>> = Mutex::new(None);

/// Nhớ trang bị chặn bao lâu.
pub const KEEP_MS: i64 = 12 * 3600 * 1000;
/// Chờ app tối đa (lần đầu có cả lúc trình duyệt qua lớp chặn, ~5–15 giây).
pub const TIMEOUT: Duration = Duration::from_secs(60);

/// App đăng ký (None = bỏ).
pub fn set_provider(p: Option<Provider>) {
    *PROVIDER.write().unwrap() = p;
}

pub fn available() -> bool {
    PROVIDER.read().unwrap().is_some()
}

/// Tên máy (chữ thường) của địa chỉ.
pub fn host_of(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_lowercase())).unwrap_or_default()
}

fn with<T>(f: impl FnOnce(&mut HashMap<String, i64>) -> T) -> T {
    let mut g = HOSTS.lock().unwrap();
    f(g.get_or_insert_with(HashMap::new))
}

/// Ghi nhớ: trang này đang chặn.
pub fn remember(host: &str) {
    remember_at(host, crate::util::now_ms());
}

pub fn remember_at(host: &str, now: i64) {
    if !host.is_empty() {
        with(|h| h.insert(host.to_lowercase(), now));
    }
}

/// Trang của địa chỉ này đang bị nhớ là chặn (trong 12 giờ).
pub fn needs(url: &str) -> bool {
    needs_at(url, crate::util::now_ms())
}

pub fn needs_at(url: &str, now: i64) -> bool {
    let host = host_of(url);
    with(|h| h.get(&host).map(|t| now - t >= 0 && now - t < KEEP_MS).unwrap_or(false))
}

pub fn forget_all() {
    with(|h| h.clear());
}

/// Các trang đang bị nhớ là chặn — gỡ lỗi.
pub fn hosts() -> Vec<String> {
    let mut v: Vec<String> = with(|h| h.keys().cloned().collect());
    v.sort();
    v
}

/// Trả lời này là lớp chặn của Cloudflare (không phải lỗi thật của trang): đầu thư `cf-mitigated: challenge`, hoặc 403 / 503
/// mà nội dung là trang «Just a moment…» / tải mã `/cdn-cgi/challenge-platform/` của trang chặn (`_cf_chl_opt`).
pub fn is_challenge(code: u16, cf_mitigated: Option<&str>, body: &[u8]) -> bool {
    if cf_mitigated.map(|v| v.trim().eq_ignore_ascii_case("challenge")).unwrap_or(false) {
        return true;
    }
    if code != 403 && code != 503 {
        return false;
    }
    let head = String::from_utf8_lossy(&body[..body.len().min(8192)]).to_lowercase();
    head.contains("<title>just a moment") || head.contains("_cf_chl_opt") || head.contains("/cdn-cgi/challenge-platform/h/")
}

/// Yêu cầu GET / POST theo kiểu của http.rs.
pub fn request(method: &str, url: &str, body: Option<String>, headers: &[(&str, &str)]) -> WebRequest {
    WebRequest {
        url: url.to_string(),
        method: method.to_string(),
        body,
        headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
    }
}

/// Nhờ app đọc hộ. Chưa có app đăng ký → lỗi «đang bật chống bot»; app báo lỗi → câu báo của app; mã ≥ 400 → lỗi HTTP như
/// http.rs.
pub async fn fetch(req: WebRequest) -> Result<Response> {
    let host = host_of(&req.url);
    let Some(p) = PROVIDER.read().unwrap().clone() else {
        return Err(Error::Challenge(host));
    };
    let t0 = crate::util::now_ms();
    let url = req.url.clone();
    let r = tokio::time::timeout(TIMEOUT, p(req)).await;
    let ms = crate::util::now_ms() - t0;
    match r {
        Err(_) => {
            crate::log::d("web", &format!("{url}: WebView không trả lời sau {} giây", TIMEOUT.as_secs()));
            Err(Error::Timeout)
        }
        Ok(Err(m)) => {
            crate::log::d("web", &format!("{url}: {m} ({ms} ms)"));
            Err(Error::App(m))
        }
        Ok(Ok(r)) => {
            crate::log::d("web", &format!("{url} → {} qua WebView, {} byte, {ms} ms", r.code, r.data.len()));
            if is_challenge(r.code, None, &r.data) {
                return Err(Error::Challenge(host));
            }
            if r.code >= 400 {
                return Err(Error::Http(r.code));
            }
            Ok(Response { url: if r.url.is_empty() { url } else { r.url }, code: r.code, data: r.data, content_type: r.content_type })
        }
    }
}

/// Giải nội dung base64 của app (WebView trả chuỗi base64 cho mọi kiểu nội dung, kể cả ảnh).
pub fn decode_b64(s: &str) -> std::result::Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD.decode(s.trim()).map_err(|e| format!("nội dung base64 hỏng: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cloudflare_challenge() {
        assert!(is_challenge(403, Some("challenge"), b""));
        assert!(is_challenge(200, Some(" Challenge "), b"")); // đầu thư là đủ
        let page = br#"<!DOCTYPE html><html lang="en-US"><head><title>Just a moment...</title>"#;
        assert!(is_challenge(403, None, page));
        assert!(is_challenge(503, None, b"<script>window._cf_chl_opt={cvId:'3'}</script>"));
        // Lỗi thật của trang / trang thường có mã JS của Cloudflare (jsd) thì không phải lớp chặn
        assert!(!is_challenge(403, None, b"<html><title>403 Forbidden</title></html>"));
        assert!(!is_challenge(404, None, page));
        assert!(!is_challenge(200, None, br#"<script src="/cdn-cgi/challenge-platform/scripts/jsd/main.js"></script>"#));
    }

    #[test]
    fn remembers_blocked_hosts_for_12_hours() {
        let now = 1_800_000_000_000;
        remember_at("HoatHinh3D.pw", now);
        assert!(needs_at("https://hoathinh3d.pw/phim/abc", now + 1000));
        assert!(needs_at("https://hoathinh3d.pw/", now + KEEP_MS - 1));
        assert!(!needs_at("https://hoathinh3d.pw/", now + KEEP_MS));
        assert!(!needs_at("https://hoathinh3d.ee/", now + 1000));
        assert!(!needs_at("không phải địa chỉ", now));
    }

    #[tokio::test]
    async fn without_app_reports_challenge() {
        set_provider(None);
        let e = fetch(request("GET", "https://chan.test/", None, &[])).await.unwrap_err();
        assert!(matches!(e, Error::Challenge(ref h) if h == "chan.test"));
        assert!(e.to_string().contains("chống bot"));
    }
}
