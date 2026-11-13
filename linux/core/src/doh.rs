//! DNS mã hoá (DoH / DoT) — bản Windows: Doh.cs. TẠM: chưa chuyển, luôn dùng DNS của máy.

use std::net::IpAddr;

/// Địa chỉ để kết nối tới host qua DNS mã hoá; None = dùng DNS của máy.
pub async fn for_connect(_host: &str) -> Option<IpAddr> {
    None
}

pub fn clear_cache() {}
