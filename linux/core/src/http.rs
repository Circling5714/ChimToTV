//! Tải trang bằng reqwest qua cổng SOCKS nội bộ (net.rs) — nên DNS mã hoá, vượt chặn, proxy áp cho cả app lẫn trình phát.
//! Tự đi theo chuyển hướng (kể cả đổi tên miền). Bản Windows: Http.cs.

use crate::util::{Error, Result};
use std::sync::{Mutex, RwLock};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Response {
    /// Địa chỉ cuối cùng (sau chuyển hướng)
    pub url: String,
    pub code: u16,
    pub data: Vec<u8>,
    pub content_type: Option<String>,
}

impl Response {
    pub fn body(&self) -> String {
        text(&self.data, self.content_type.as_deref())
    }
}

/// User-Agent chung cho app (trang thấy một trình duyệt bình thường).
static USER_AGENT: RwLock<String> = RwLock::new(String::new());

pub const DEFAULT_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";

pub fn user_agent() -> String {
    let u = USER_AGENT.read().unwrap();
    if u.is_empty() {
        DEFAULT_UA.to_string()
    } else {
        u.clone()
    }
}

pub fn set_user_agent(ua: &str) {
    *USER_AGENT.write().unwrap() = ua.to_string();
    reset_client();
}

static CLIENT: Mutex<Option<reqwest::Client>> = Mutex::new(None);

/// Máy khách dùng chung (tạo lại sau khi đổi cài đặt mạng — kết nối giữ lại không theo cài đặt mới).
pub fn client() -> reqwest::Client {
    let mut c = CLIENT.lock().unwrap();
    if let Some(x) = c.as_ref() {
        return x.clone();
    }
    let port = crate::net::start_bridge();
    let mut b = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .cookie_store(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(25))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(6)
        .http1_only()
        .user_agent(user_agent());
    if let Ok(p) = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}")) {
        b = b.proxy(p);
    }
    let x = b.build().expect("reqwest");
    *c = Some(x.clone());
    x
}

pub fn reset_client() {
    *CLIENT.lock().unwrap() = None;
}

fn check_url(url: &str) -> Result<()> {
    match url::Url::parse(url) {
        Ok(u) if u.scheme() == "http" || u.scheme() == "https" => Ok(()),
        _ => Err(Error::App(format!("Địa chỉ không hợp lệ: {url}"))),
    }
}

fn apply(mut r: reqwest::RequestBuilder, headers: &[(&str, &str)]) -> reqwest::RequestBuilder {
    r = r.header("Accept-Language", "vi-VN,vi;q=0.9,en;q=0.8");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("accept")) {
        r = r.header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8");
    }
    for (k, v) in headers {
        r = r.header(*k, *v);
    }
    r
}

async fn send(make: impl Fn() -> reqwest::RequestBuilder) -> Result<Response> {
    let resp = match make().send().await {
        Ok(r) => r,
        // Kết nối giữ lại từ lần trước bị ngắt ngầm → thử lại một lần
        Err(e) if e.is_connect() || e.is_request() => make().send().await?,
        Err(e) => return Err(e.into()),
    };
    let code = resp.status().as_u16();
    if code >= 400 {
        return Err(Error::Http(code));
    }
    let url = resp.url().to_string();
    let ct = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let data = resp.bytes().await?.to_vec();
    Ok(Response { url, code, data, content_type: ct })
}

pub async fn get(url: &str, headers: &[(&str, &str)]) -> Result<Response> {
    check_url(url)?;
    let c = client();
    send(|| apply(c.get(url), headers)).await
}

/// POST dạng form (admin-ajax.php của WordPress).
pub async fn post(url: &str, form: &[(&str, &str)], headers: &[(&str, &str)]) -> Result<Response> {
    check_url(url)?;
    let body = form
        .iter()
        .map(|(k, v)| format!("{}={}", crate::util::form_encoded(k), crate::util::form_encoded(v)))
        .collect::<Vec<_>>()
        .join("&");
    let c = client();
    send(|| {
        apply(c.post(url), headers)
            .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
            .body(body.clone())
    })
    .await
}

/// Tải tệp lớn (bản cập nhật) vào tệp, báo tiến độ (0..1).
pub async fn download(url: &str, to: &std::path::Path, progress: impl Fn(f64)) -> Result<()> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;
    check_url(url)?;
    let c = client();
    let resp = apply(c.get(url), &[("Accept", "application/octet-stream")]).timeout(Duration::from_secs(600)).send().await?;
    let code = resp.status().as_u16();
    if code >= 400 {
        return Err(Error::Http(code));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut f = tokio::fs::File::create(to).await?;
    let mut done = 0u64;
    let mut s = resp.bytes_stream();
    while let Some(chunk) = s.next().await {
        let chunk = chunk?;
        f.write_all(&chunk).await?;
        done += chunk.len() as u64;
        if total > 0 {
            progress(done as f64 / total as f64);
        }
    }
    f.flush().await?;
    Ok(())
}

/// Giải mã theo charset trong Content-Type (mặc định UTF-8).
pub fn text(data: &[u8], content_type: Option<&str>) -> String {
    if let Some(ct) = content_type.map(|c| c.to_lowercase()) {
        if let Some(i) = ct.find("charset=") {
            let name = crate::util::before(ct[i + 8..].trim_matches(|c| c == '"' || c == '\'' || c == ' '), ";").to_string();
            if let Some(enc) = encoding_rs::Encoding::for_label(name.trim().as_bytes()) {
                return enc.decode(data).0.into_owned();
            }
        }
    }
    String::from_utf8_lossy(data).into_owned()
}
