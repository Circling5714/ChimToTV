//! Shadowsocks (SS 2022 + kiểu cũ AEAD) — bản Windows: Shadowsocks.cs. TẠM: chưa chuyển.

use crate::net::Duplex;
use crate::util::{Error, Result};
use std::net::IpAddr;
use std::pin::Pin;

#[derive(Debug, Clone, PartialEq)]
pub struct SsConfig {
    pub host: String,
    pub port: u16,
    pub method: String,
    pub password: String,
    pub remote_dns: bool,
    pub sites_only: bool,
}

/// Máy chủ Shadowsocks đang dùng; None = proxy tắt / đang chọn SOCKS5 / chưa nhập đủ.
pub fn config() -> Option<SsConfig> {
    None
}

pub async fn connect(_c: &SsConfig, _host: &str, _ip: Option<IpAddr>, _port: u16) -> Result<Pin<Box<dyn Duplex>>> {
    Err(Error::app("Shadowsocks chưa có trong bản này"))
}
