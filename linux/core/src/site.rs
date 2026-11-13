//! Đọc trang phim (yanhh3d / HaLim / Next.js / API) — bản Windows: Site.cs. TẠM: mới có host_of.

/// Tên miền (chữ thường, bỏ "www.") của một địa chỉ; sai thì None.
pub fn host_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let h = u.host_str()?.to_lowercase();
    Some(h.strip_prefix("www.").map(|s| s.to_string()).unwrap_or(h))
}
