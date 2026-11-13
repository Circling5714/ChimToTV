//! Máy chủ Shadowsocks (Cài đặt → Proxy → Kiểu: Shadowsocks), vd lối vào Shadowsocks của 3X-UI / Xray (bản TV:
//! Shadowsocks.kt, Windows: Shadowsocks.cs, iPad: Shadowsocks.swift). Mật khẩu: SS 2022 = các khoá base64 cách nhau bằng ":"
//! (3X-UI luôn nhiều người dùng: "khoá-máy-chủ:khoá-người-dùng"); kiểu cũ (aes-256-gcm, chacha20-ietf-poly1305…) = mật khẩu.
//!
//! Phía khách Shadowsocks (TCP): SS 2022 (SIP022, có phần nhận dạng người dùng EIH cho máy chủ nhiều người dùng) + AEAD cũ.
//! SS 2022: khoá phiên = BLAKE3 derive_key("shadowsocks 2022 session subkey", PSK + salt); phần nhận dạng = AES-ECB(derive_key(
//! "shadowsocks 2022 identity subkey", khoá-máy-chủ + salt), BLAKE3(khoá-người-dùng)[0..16]). Kiểu cũ: khoá = EVP_BytesToKey
//! (MD5), khoá phiên = HKDF-SHA1(khoá, salt, "ss-subkey"). Bản Windows tự viết BLAKE3; ở đây dùng thư viện blake3 (kiểm bằng
//! cùng bộ số chuẩn trong test).
//!
//! Lỗi: `Error::Socks(1, câu báo)` (SsError / SocksError của bản Windows) — giao diện hiện nguyên câu báo. Lỗi giữa chừng
//! trên luồng (đọc / ghi) là io::Error bọc `SsError` (`ss_error_message` lấy lại câu báo).

use crate::net::Duplex;
use crate::util::{now_ms, Error, Result, Rx};
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes128Gcm, Aes256Gcm};
use chacha20poly1305::ChaCha20Poly1305;
use rand::{Rng, RngCore};
use std::fmt;
use std::io;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{ready, Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;

/// Máy chủ Shadowsocks người xem nhập (hoặc dán link ss://). `method` = `SsMethod::id`.
#[derive(Debug, Clone, PartialEq)]
pub struct SsConfig {
    pub host: String,
    pub port: u16,
    pub method: String,
    pub password: String,
    pub remote_dns: bool,
    pub sites_only: bool,
}

impl SsConfig {
    pub fn new(host: &str, port: u16, method: &str, password: &str) -> SsConfig {
        SsConfig {
            host: host.into(),
            port,
            method: method.into(),
            password: password.into(),
            remote_dns: true,
            sites_only: false,
        }
    }
}

/// Kiểu mã hoá hỗ trợ (TCP). `key_size` = số byte khoá = số byte salt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SsMethod {
    pub id: &'static str,
    pub key_size: usize,
    pub chacha: bool,
    pub is_2022: bool,
}

impl SsMethod {
    pub const B3_AES256: SsMethod = SsMethod { id: "2022-blake3-aes-256-gcm", key_size: 32, chacha: false, is_2022: true };
    pub const B3_AES128: SsMethod = SsMethod { id: "2022-blake3-aes-128-gcm", key_size: 16, chacha: false, is_2022: true };
    pub const B3_CHACHA: SsMethod = SsMethod { id: "2022-blake3-chacha20-poly1305", key_size: 32, chacha: true, is_2022: true };
    pub const AES256: SsMethod = SsMethod { id: "aes-256-gcm", key_size: 32, chacha: false, is_2022: false };
    pub const AES128: SsMethod = SsMethod { id: "aes-128-gcm", key_size: 16, chacha: false, is_2022: false };
    pub const CHACHA_IETF: SsMethod = SsMethod { id: "chacha20-ietf-poly1305", key_size: 32, chacha: true, is_2022: false };

    pub const ALL: [SsMethod; 6] =
        [Self::B3_AES256, Self::B3_AES128, Self::B3_CHACHA, Self::AES256, Self::AES128, Self::CHACHA_IETF];
    pub const DEFAULT: SsMethod = Self::B3_AES256;

    /// Theo tên (không phân biệt hoa / thường; trống = mặc định); "chacha20-poly1305" của Xray / 3X-UI = chacha20-ietf-poly1305.
    pub fn of(name: &str) -> Option<SsMethod> {
        let n = name.trim().to_lowercase();
        if n.is_empty() {
            return Some(Self::DEFAULT);
        }
        if n == "chacha20-poly1305" {
            return Some(Self::CHACHA_IETF);
        }
        Self::ALL.iter().copied().find(|m| m.id == n)
    }

    pub fn names() -> String {
        Self::ALL.iter().map(|m| m.id).collect::<Vec<_>>().join(", ")
    }

    /// Máy có hỗ trợ không — trên Linux luôn có (mã hoá viết bằng Rust; bản Windows cần ChaCha20 của Windows).
    pub fn supported(&self) -> bool {
        true
    }
}

/// Câu báo lỗi Shadowsocks cho người xem (sai khoá, máy chủ không trả lời…) — bọc trong io::Error của luồng.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsError(pub String);

impl fmt::Display for SsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SsError {}

fn ss_io(msg: impl Into<String>) -> io::Error {
    io::Error::other(SsError(msg.into()))
}

/// Câu báo Shadowsocks trong một lỗi đọc / ghi của luồng (None = lỗi mạng thường).
pub fn ss_error_message(e: &io::Error) -> Option<String> {
    e.get_ref().and_then(|x| x.downcast_ref::<SsError>()).map(|x| x.0.clone())
}

fn ss_err(msg: impl Into<String>) -> Error {
    Error::Socks(1, msg.into())
}

// ---- Link ss:// (SIP002, cả kiểu 3X-UI cho SS 2022: "ss://method:khoá-máy-chủ:khoá-người-dùng@host:cổng?type=tcp#tên") ----

/// Đọc link ss://; sai thì Err(Socks(1, câu báo cho người xem)).
pub fn parse_link(link: &str, remote_dns: bool, sites_only: bool) -> Result<SsConfig> {
    let s = link.trim();
    if !s.get(..5).map(|p| p.eq_ignore_ascii_case("ss://")).unwrap_or(false) {
        return Err(ss_err("Link Shadowsocks phải bắt đầu bằng ss://"));
    }
    let mut s = &s[5..];
    if let Some(h) = s.find('#') {
        s = &s[..h];
    }
    let mut query = "";
    if let Some(q) = s.find('?') {
        query = &s[q + 1..];
        s = &s[..q];
    }
    let s = s.trim_end_matches('/');
    for p in query.split('&').filter(|x| !x.is_empty()) {
        let (k, v) = match p.find('=') {
            Some(eq) => (p[..eq].to_lowercase(), decode(&p[eq + 1..]).to_lowercase()),
            None => (p.to_lowercase(), String::new()),
        };
        if k == "type" && !v.is_empty() && v != "tcp" {
            return Err(ss_err(format!(
                "Link dùng truyền tải «{v}» — app chỉ hỗ trợ Shadowsocks qua TCP thường (3X-UI: Transmission = TCP, không TLS)."
            )));
        }
        if k == "security" && !v.is_empty() && v != "none" {
            return Err(ss_err(format!(
                "Link dùng «security={v}» — app chỉ hỗ trợ Shadowsocks qua TCP thường (3X-UI: Security = None)."
            )));
        }
        if k == "plugin" && !v.is_empty() {
            return Err(ss_err(format!("Link dùng plugin ({}) — app chưa hỗ trợ plugin.", v.split(';').next().unwrap_or(""))));
        }
        if k == "headertype" && !v.is_empty() && v != "none" {
            return Err(ss_err("Link dùng ngụy trang HTTP — app chưa hỗ trợ (3X-UI: TCP header = None)."));
        }
    }
    // Kiểu cũ: ss://base64(method:pass@host:port)
    let s: String = if s.contains('@') {
        s.to_string()
    } else {
        match base64_decode(s) {
            Some(d) => String::from_utf8_lossy(&d).into_owned(),
            None => return Err(ss_err("Link ss:// không đọc được.")),
        }
    };
    let at = s.rfind('@').ok_or_else(|| ss_err("Link ss:// thiếu máy chủ / cổng."))?;
    let user = &s[..at];
    let host_port = &s[at + 1..];
    let info = if user.contains(':') || user.contains('%') {
        decode(user)
    } else {
        match base64_decode(user).map(|b| String::from_utf8_lossy(&b).into_owned()) {
            Some(t) if t.contains(':') => t,
            _ => decode(user),
        }
    };
    let colon = info.find(':').ok_or_else(|| ss_err("Link ss:// thiếu kiểu mã hoá / khoá."))?;
    let method = &info[..colon];
    let password = &info[colon + 1..];
    let (host, port) = split_host_port(host_port).ok_or_else(|| ss_err("Link ss:// thiếu máy chủ / cổng."))?;
    let m = SsMethod::of(method)
        .ok_or_else(|| ss_err(format!("Kiểu mã hoá «{method}» chưa hỗ trợ. App hỗ trợ: {}.", SsMethod::names())))?;
    keys(&m, password)?; // kiểm tra khoá ngay lúc dán
    Ok(SsConfig { host, port, method: m.id.into(), password: password.into(), remote_dns, sites_only })
}

/// "host:cổng" hoặc "[ipv6]:cổng" → (host, cổng).
pub fn split_host_port(s: &str) -> Option<(String, u16)> {
    let t = s.trim();
    let (host, port) = if let Some(rest) = t.strip_prefix('[') {
        let close = rest.find(']')?;
        let after = &rest[close + 1..];
        (&rest[..close], after.strip_prefix(':')?)
    } else {
        let c = t.rfind(':')?;
        (&t[..c], &t[c + 1..])
    };
    let p: i64 = port.trim().parse().ok()?;
    if host.is_empty() || !(1..=65535).contains(&p) {
        return None;
    }
    Some((host.to_string(), p as u16))
}

fn decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8().map(|c| c.into_owned()).unwrap_or_else(|_| s.to_string())
}

/// Base64 thường lẫn base64url, có hoặc không dấu "=" (khoá 3X-UI, link SIP002). None = không phải base64.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let t = s.trim().trim_end_matches('=');
    if t.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(t.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for ch in t.chars() {
        let v = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => return None,
        };
        acc = ((acc << 6) | v) & 0xFF_FFFF;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    if bits >= 6 {
        return None;
    }
    Some(out)
}

// ---- Khoá ----

/// Khoá từ mật khẩu. SS 2022: mỗi phần (cách nhau bằng ":") là khoá base64 đúng cỡ — phần cuối là khoá người dùng, các phần
/// trước là khoá nhận dạng (khoá máy chủ của 3X-UI). Kiểu cũ: EVP_BytesToKey(MD5). Sai → Err(Socks(1, câu báo)).
pub fn keys(m: &SsMethod, password: &str) -> Result<Vec<Vec<u8>>> {
    if password.is_empty() {
        return Err(ss_err("Chưa nhập khoá / mật khẩu Shadowsocks."));
    }
    if !m.is_2022 {
        return Ok(vec![evp_bytes_to_key(password.as_bytes(), m.key_size)]);
    }
    let parts: Vec<&str> = password.split(':').map(|p| p.trim()).collect();
    if parts.len() > 1 && m.chacha {
        return Err(ss_err(format!(
            "{} không dùng được với máy chủ nhiều người dùng (khoá dạng «khoá-máy-chủ:khoá-người-dùng») — chọn 2022-blake3-aes-256-gcm.",
            m.id
        )));
    }
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let which = if parts.len() == 1 {
            "Khoá"
        } else if i == parts.len() - 1 {
            "Khoá người dùng"
        } else {
            "Khoá máy chủ"
        };
        match base64_decode(p) {
            Some(k) if k.len() == m.key_size => out.push(k),
            _ => {
                return Err(ss_err(format!(
                    "{which} không đúng: {} cần khoá base64 dài {} byte ({} ký tự).",
                    m.id,
                    m.key_size,
                    m.key_size.div_ceil(3) * 4
                )))
            }
        }
    }
    Ok(out)
}

fn blake3_derive(context: &str, material: &[&[u8]], len: usize) -> Vec<u8> {
    let mut h = blake3::Hasher::new_derive_key(context);
    for m in material {
        h.update(m);
    }
    let mut out = vec![0u8; len];
    h.finalize_xof().fill(&mut out);
    out
}

/// Khoá phiên của một chiều (theo salt của chiều đó).
pub fn subkey(m: &SsMethod, key: &[u8], salt: &[u8]) -> Vec<u8> {
    if m.is_2022 {
        return blake3_derive("shadowsocks 2022 session subkey", &[key, salt], m.key_size);
    }
    let hk = hkdf::Hkdf::<sha1::Sha1>::new(Some(salt), key);
    let mut okm = vec![0u8; m.key_size];
    let _ = hk.expand(b"ss-subkey", &mut okm); // chỉ lỗi khi xin quá 255 × 20 byte
    okm
}

pub fn evp_bytes_to_key(pass: &[u8], size: usize) -> Vec<u8> {
    use md5::{Digest, Md5};
    let mut o = Vec::new();
    let mut prev: Vec<u8> = Vec::new();
    while o.len() < size {
        let mut h = Md5::new();
        h.update(&prev);
        h.update(pass);
        prev = h.finalize().to_vec();
        o.extend_from_slice(&prev);
    }
    o.truncate(size);
    o
}

/// AES-ECB một khối (phần nhận dạng người dùng của SS 2022). Khoá 16 / 32 byte.
fn aes_ecb(key: &[u8], block: &[u8]) -> Option<[u8; 16]> {
    use aes::cipher::BlockEncrypt;
    let mut b = aes::Block::clone_from_slice(block.get(..16)?);
    match key.len() {
        16 => aes::Aes128::new_from_slice(key).ok()?.encrypt_block(&mut b),
        32 => aes::Aes256::new_from_slice(key).ok()?.encrypt_block(&mut b),
        _ => return None,
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&b);
    Some(out)
}

pub fn no_reply(m: &SsMethod) -> String {
    format!(
        "máy chủ Shadowsocks không trả lời — thường do sai khoá / sai kiểu mã hoá{} (kiểm tra lại link ss:// trong 3X-UI)",
        if m.is_2022 { ", hoặc giờ trên máy lệch quá 30 giây" } else { "" }
    )
}

/// Địa chỉ đích dạng SOCKS5 (ATYP + địa chỉ + cổng).
pub fn target(host: &str, ip: Option<IpAddr>, port: u16) -> Result<Vec<u8>> {
    let mut a = Vec::new();
    match ip.or_else(|| host.trim_matches(|c| c == '[' || c == ']').parse().ok()) {
        Some(IpAddr::V4(v)) => {
            a.push(1);
            a.extend_from_slice(&v.octets());
        }
        Some(IpAddr::V6(v)) => {
            a.push(4);
            a.extend_from_slice(&v.octets());
        }
        None => {
            let h = host.as_bytes();
            if h.len() > 255 {
                return Err(Error::Socks(1, "tên miền quá dài".into()));
            }
            a.push(3);
            a.push(h.len() as u8);
            a.extend_from_slice(h);
        }
    }
    a.push((port >> 8) as u8);
    a.push(port as u8);
    Ok(a)
}

// ---- AEAD một chiều ----

enum Cipher {
    A128(Box<Aes128Gcm>),
    A256(Box<Aes256Gcm>),
    Chacha(Box<ChaCha20Poly1305>),
}

/// AEAD một chiều của phiên: khoá phiên + nonce 12 byte little-endian, tăng sau mỗi lần mã hoá / giải mã.
pub struct SsAead {
    c: Cipher,
    nonce: [u8; 12],
}

impl SsAead {
    /// None = khoá sai cỡ.
    pub fn new(m: &SsMethod, key: &[u8]) -> Option<SsAead> {
        let c = if m.chacha {
            Cipher::Chacha(Box::new(ChaCha20Poly1305::new_from_slice(key).ok()?))
        } else if key.len() == 16 {
            Cipher::A128(Box::new(Aes128Gcm::new_from_slice(key).ok()?))
        } else {
            Cipher::A256(Box::new(Aes256Gcm::new_from_slice(key).ok()?))
        };
        Some(SsAead { c, nonce: [0; 12] })
    }

    /// Bản mã + thẻ 16 byte, nối vào out.
    pub fn seal_into(&mut self, p: &[u8], out: &mut Vec<u8>) {
        let at = out.len();
        out.extend_from_slice(p);
        let n = GenericArray::from_slice(&self.nonce);
        let buf = &mut out[at..];
        // chỉ lỗi khi một lần mã hoá quá 64 GB
        let tag = match &self.c {
            Cipher::A128(c) => c.encrypt_in_place_detached(n, &[], buf).map(|t| t.to_vec()),
            Cipher::A256(c) => c.encrypt_in_place_detached(n, &[], buf).map(|t| t.to_vec()),
            Cipher::Chacha(c) => c.encrypt_in_place_detached(n, &[], buf).map(|t| t.to_vec()),
        };
        out.extend_from_slice(&tag.unwrap_or_else(|_| vec![0; 16]));
        self.inc();
    }

    pub fn seal(&mut self, p: &[u8]) -> Vec<u8> {
        let mut o = Vec::with_capacity(p.len() + 16);
        self.seal_into(p, &mut o);
        o
    }

    /// Sai thẻ xác thực (sai khoá / dữ liệu hỏng) → None.
    pub fn open(&mut self, c: &[u8]) -> Option<Vec<u8>> {
        if c.len() < 16 {
            return None;
        }
        let (ct, tag) = c.split_at(c.len() - 16);
        let mut p = ct.to_vec();
        let n = GenericArray::from_slice(&self.nonce);
        let tag = GenericArray::from_slice(tag);
        let ok = match &self.c {
            Cipher::A128(x) => x.decrypt_in_place_detached(n, &[], &mut p, tag).is_ok(),
            Cipher::A256(x) => x.decrypt_in_place_detached(n, &[], &mut p, tag).is_ok(),
            Cipher::Chacha(x) => x.decrypt_in_place_detached(n, &[], &mut p, tag).is_ok(),
        };
        if !ok {
            return None;
        }
        self.inc();
        Some(p)
    }

    fn inc(&mut self) {
        for b in self.nonce.iter_mut() {
            *b = b.wrapping_add(1);
            if *b != 0 {
                break;
            }
        }
    }
}

// ---- Luồng đã mã hoá ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadState {
    /// chờ salt của máy chủ
    Salt,
    /// SS 2022: chờ phần đầu trả lời (loại, giờ, salt của mình, độ dài)
    Header,
    /// chờ chunk độ dài
    Len,
    /// chờ chunk dữ liệu dài n
    Payload(usize),
    /// máy chủ đã đóng
    Eof,
}

/// Một lần ghi nhận tối đa chừng này byte của bên gọi (giới hạn bộ nhớ đệm bản mã).
const MAX_WRITE: usize = 256 * 1024;

/// Kết nối qua máy chủ Shadowsocks (luồng đã mã hoá chồng trên ổ cắm). Phần đầu (salt, phần nhận dạng người dùng, địa chỉ
/// đích) gửi chung một lần ghi với dữ liệu đầu tiên của bên gọi; ghi 0 byte khi chưa gửi gì = gửi phần đầu không kèm dữ liệu
/// (thêm đệm ngẫu nhiên) — StartAsync của bản Windows, dùng khi bên gọi chưa nói gì (giao thức máy chủ nói trước).
struct SsStream {
    raw: TcpStream,
    m: SsMethod,
    keys: Arc<Vec<Vec<u8>>>,
    target: Vec<u8>,
    salt: Vec<u8>,
    enc: SsAead,
    started: bool,
    /// bản mã chờ gửi + đã gửi tới đâu
    out: Vec<u8>,
    out_pos: usize,
    /// số byte của bên gọi đã mã hoá vào `out` — trả cho bên gọi khi `out` gửi xong
    consumed: Option<usize>,
    dec: Option<SsAead>,
    /// byte thô đã nhận chưa giải mã
    rbuf: Vec<u8>,
    rpos: usize,
    rx: ReadState,
    plain: Vec<u8>,
    plain_pos: usize,
    failed: Option<String>,
}

impl SsStream {
    fn new(raw: TcpStream, m: SsMethod, keys: Arc<Vec<Vec<u8>>>, target: Vec<u8>) -> Option<SsStream> {
        let mut salt = vec![0u8; m.key_size];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        let enc = SsAead::new(&m, &subkey(&m, keys.last()?, &salt))?;
        Some(SsStream {
            raw,
            m,
            keys,
            target,
            salt,
            enc,
            started: false,
            out: Vec::new(),
            out_pos: 0,
            consumed: None,
            dec: None,
            rbuf: Vec::new(),
            rpos: 0,
            rx: ReadState::Salt,
            plain: Vec::new(),
            plain_pos: 0,
            failed: None,
        })
    }

    /// Ghi phần đầu kèm tối đa một phần dữ liệu vào `out`; trả số byte dữ liệu đã gửi kèm. Cả phần đầu đi trong một lần ghi
    /// (chuẩn SS 2022: không để lộ cỡ từng phần).
    fn header(&mut self, b: &[u8]) -> usize {
        self.started = true;
        let mut rng = rand::thread_rng();
        self.out.extend_from_slice(&self.salt);
        let ks = self.m.key_size;
        if self.m.is_2022 {
            // Phần nhận dạng (EIH): mỗi khoá trước khoá người dùng mã hoá 16 byte đầu BLAKE3(khoá kế tiếp) bằng AES-ECB
            for i in 0..self.keys.len().saturating_sub(1) {
                let sub = blake3_derive("shadowsocks 2022 identity subkey", &[&self.keys[i], &self.salt], ks);
                let next = blake3::hash(&self.keys[i + 1]);
                if let Some(eih) = aes_ecb(&sub, &next.as_bytes()[..16]) {
                    self.out.extend_from_slice(&eih);
                }
            }
            let padding = if b.len() < 900 { rng.gen_range(1..=900usize) } else { 0 };
            let sent = b.len().min(0xFFFF - self.target.len() - 2 - padding);
            let var_len = self.target.len() + 2 + padding + sent;
            let mut fixed = [0u8; 11];
            fixed[1..9].copy_from_slice(&unix_secs().to_be_bytes());
            fixed[9] = (var_len >> 8) as u8;
            fixed[10] = var_len as u8;
            let mut vh = Vec::with_capacity(var_len);
            vh.extend_from_slice(&self.target);
            vh.push((padding >> 8) as u8);
            vh.push(padding as u8);
            let at = vh.len();
            vh.resize(at + padding, 0);
            rng.fill_bytes(&mut vh[at..]);
            vh.extend_from_slice(&b[..sent]);
            self.enc.seal_into(&fixed, &mut self.out);
            self.enc.seal_into(&vh, &mut self.out);
            sent
        } else {
            // Kiểu cũ: chunk đầu = địa chỉ đích + dữ liệu đầu
            let sent = b.len().min(0x3FFF - self.target.len());
            let mut first = self.target.clone();
            first.extend_from_slice(&b[..sent]);
            self.enc.seal_into(&[(first.len() >> 8) as u8, first.len() as u8], &mut self.out);
            self.enc.seal_into(&first, &mut self.out);
            sent
        }
    }

    /// Mã hoá dữ liệu của bên gọi vào `out` (kèm phần đầu nếu chưa gửi); trả số byte đã nhận.
    fn encrypt(&mut self, buf: &[u8]) -> usize {
        let mut d = &buf[..buf.len().min(MAX_WRITE)];
        let total = d.len();
        if !self.started {
            let k = self.header(d);
            d = &d[k..];
        }
        let max = if self.m.is_2022 { 0xFFFF } else { 0x3FFF };
        while !d.is_empty() {
            let k = d.len().min(max);
            self.enc.seal_into(&[(k >> 8) as u8, k as u8], &mut self.out);
            self.enc.seal_into(&d[..k], &mut self.out);
            d = &d[k..];
        }
        total
    }

    /// Gửi nốt bản mã đang chờ.
    fn poll_out(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.out_pos < self.out.len() {
            let n = ready!(Pin::new(&mut self.raw).poll_write(cx, &self.out[self.out_pos..]))?;
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
            self.out_pos += n;
        }
        self.out.clear();
        self.out_pos = 0;
        Poll::Ready(Ok(()))
    }

    fn take(&mut self, n: usize) -> Option<std::ops::Range<usize>> {
        if self.rbuf.len() - self.rpos < n {
            return None;
        }
        let r = self.rpos..self.rpos + n;
        self.rpos += n;
        Some(r)
    }

    /// Giải mã một bước từ byte đã nhận. Ok(true) = tiến được; Ok(false) = cần thêm byte; Err = câu báo lỗi.
    fn step(&mut self) -> std::result::Result<bool, String> {
        let ks = self.m.key_size;
        let broken = "dữ liệu từ máy chủ Shadowsocks bị hỏng";
        match self.rx {
            ReadState::Salt => {
                let Some(r) = self.take(ks) else { return Ok(false) };
                let user = self.keys.last().ok_or_else(|| no_reply(&self.m))?;
                let key = subkey(&self.m, user, &self.rbuf[r]);
                self.dec = Some(SsAead::new(&self.m, &key).ok_or_else(|| no_reply(&self.m))?);
                self.rx = if self.m.is_2022 { ReadState::Header } else { ReadState::Len };
            }
            ReadState::Header => {
                let hl = 1 + 8 + ks + 2;
                let Some(r) = self.take(hl + 16) else { return Ok(false) };
                let h = self
                    .dec
                    .as_mut()
                    .and_then(|d| d.open(&self.rbuf[r]))
                    .ok_or("không giải mã được trả lời của máy chủ Shadowsocks — sai khoá hoặc sai kiểu mã hoá")?;
                if h[0] != 1 || h[9..9 + ks] != self.salt[..] {
                    return Err("máy chủ Shadowsocks trả lời sai giao thức".into());
                }
                let mut ts = [0u8; 8];
                ts.copy_from_slice(&h[1..9]);
                let skew = (unix_secs() as i128 - u64::from_be_bytes(ts) as i128).abs();
                if skew > 30 {
                    return Err(format!("giờ trên máy lệch {skew} giây so với máy chủ (Shadowsocks 2022 cho lệch tối đa 30 giây)"));
                }
                self.rx = ReadState::Payload(((h[hl - 2] as usize) << 8) | h[hl - 1] as usize);
            }
            ReadState::Len => {
                let Some(r) = self.take(18) else { return Ok(false) };
                let l = self.dec.as_mut().and_then(|d| d.open(&self.rbuf[r])).ok_or(broken)?;
                self.rx = ReadState::Payload(((l[0] as usize) << 8) | l[1] as usize);
            }
            ReadState::Payload(n) => {
                let Some(r) = self.take(n + 16) else { return Ok(false) };
                self.plain = self.dec.as_mut().and_then(|d| d.open(&self.rbuf[r])).ok_or(broken)?;
                self.plain_pos = 0;
                self.rx = ReadState::Len;
            }
            ReadState::Eof => return Ok(false),
        }
        // dọn phần đã dùng
        if self.rpos == self.rbuf.len() {
            self.rbuf.clear();
            self.rpos = 0;
        } else if self.rpos > 64 * 1024 {
            self.rbuf.drain(..self.rpos);
            self.rpos = 0;
        }
        Ok(true)
    }

    fn fail(&mut self, msg: String) -> Poll<io::Result<()>> {
        self.failed = Some(msg.clone());
        Poll::Ready(Err(ss_io(msg)))
    }
}

fn unix_secs() -> u64 {
    (now_ms() / 1000).max(0) as u64
}

impl AsyncRead for SsStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let me = self.get_mut();
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            if me.plain_pos < me.plain.len() {
                let n = buf.remaining().min(me.plain.len() - me.plain_pos);
                buf.put_slice(&me.plain[me.plain_pos..me.plain_pos + n]);
                me.plain_pos += n;
                return Poll::Ready(Ok(()));
            }
            if let Some(m) = me.failed.clone() {
                return Poll::Ready(Err(ss_io(m)));
            }
            if me.rx == ReadState::Eof {
                return Poll::Ready(Ok(()));
            }
            match me.step() {
                Ok(true) => continue,
                Ok(false) => {}
                Err(m) => return me.fail(m),
            }
            // Đọc thêm từ máy chủ
            let at = me.rbuf.len();
            me.rbuf.resize(at + 16 * 1024, 0);
            let mut rb = ReadBuf::new(&mut me.rbuf[at..]);
            let r = Pin::new(&mut me.raw).poll_read(cx, &mut rb);
            let got = rb.filled().len();
            me.rbuf.truncate(at + got);
            match r {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => {
                    // Máy chủ không nhận ra khoá thì đóng / ngắt / im lặng — chưa có trả lời nào là coi như bị từ chối
                    if me.rx == ReadState::Salt {
                        let m = no_reply(&me.m);
                        return me.fail(m);
                    }
                    return Poll::Ready(Err(e));
                }
                Poll::Ready(Ok(())) if got == 0 => {
                    let pending = me.rbuf.len() - me.rpos;
                    match me.rx {
                        ReadState::Salt => {
                            let m = no_reply(&me.m);
                            return me.fail(m);
                        }
                        ReadState::Len if pending == 0 => {
                            me.rx = ReadState::Eof;
                            return Poll::Ready(Ok(()));
                        }
                        _ => {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "máy chủ Shadowsocks ngắt kết nối giữa chừng",
                            )))
                        }
                    }
                }
                Poll::Ready(Ok(())) => {}
            }
        }
    }
}

impl AsyncWrite for SsStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let me = self.get_mut();
        ready!(me.poll_out(cx))?;
        if let Some(n) = me.consumed.take() {
            return Poll::Ready(Ok(n));
        }
        if buf.is_empty() && me.started {
            return Poll::Ready(Ok(0));
        }
        // buf rỗng mà chưa gửi gì: gửi phần đầu ngay (kèm đệm)
        let n = me.encrypt(buf);
        me.consumed = Some(n);
        ready!(me.poll_out(cx))?;
        me.consumed = None;
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = self.get_mut();
        ready!(me.poll_out(cx))?;
        Pin::new(&mut me.raw).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = self.get_mut();
        ready!(me.poll_out(cx))?;
        me.consumed = None;
        Pin::new(&mut me.raw).poll_shutdown(cx)
    }
}

// ---- Mở kết nối ----

/// Khoá đã tính gần nhất: (kiểu mã hoá, mật khẩu, khoá) — khỏi tính lại mỗi kết nối.
type KeyCache = Option<(&'static str, String, Arc<Vec<Vec<u8>>>)>;

static KEY_CACHE: Mutex<KeyCache> = Mutex::new(None);

/// Mở kết nối tới host:port qua máy chủ c (ip: đích đã phân giải; None = gửi tên miền). Chưa gửi gì: phần đầu đi cùng dữ
/// liệu đầu tiên; ghi 0 byte (`stream.write(&[])`) khi chưa gửi gì = gửi phần đầu ngay (bên gọi im — máy chủ nói trước).
/// Máy chủ không xác nhận kết nối — sai khoá chỉ lộ ra khi đọc trả lời (lỗi đọc bọc SsError).
pub async fn connect(c: &SsConfig, host: &str, ip: Option<IpAddr>, port: u16) -> Result<Pin<Box<dyn Duplex>>> {
    let m = SsMethod::of(&c.method).ok_or_else(|| ss_err(format!("kiểu mã hoá Shadowsocks «{}» chưa hỗ trợ", c.method)))?;
    let cached = {
        let k = KEY_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        match &*k {
            Some((id, p, keys)) if *id == m.id && *p == c.password => Some(keys.clone()),
            _ => None,
        }
    };
    let keys = match cached {
        Some(k) => k,
        None => {
            let k = Arc::new(keys(&m, &c.password)?);
            *KEY_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((m.id, c.password.clone(), k.clone()));
            k
        }
    };
    let t = target(host, ip, port)?;
    let fail = || ss_err(format!("không kết nối được máy chủ Shadowsocks {}:{}", c.host, c.port));
    let raw = match tokio::time::timeout(Duration::from_secs(10), TcpStream::connect((c.host.as_str(), c.port))).await {
        Ok(Ok(s)) => s,
        _ => return Err(fail()),
    };
    let _ = raw.set_nodelay(true);
    let s = SsStream::new(raw, m, keys, t).ok_or_else(fail)?;
    Ok(Box::pin(s))
}

/// Máy chủ Shadowsocks đang dùng; None = proxy tắt / đang chọn SOCKS5 / chưa nhập đủ.
pub fn config() -> Option<SsConfig> {
    use crate::prefs as p;
    if !p::proxy_on() || p::proxy_kind() != "ss" || p::ss_host().is_empty() {
        return None;
    }
    let port = p::ss_port();
    if !(1..=65535).contains(&port) {
        return None;
    }
    let key = p::ss_key();
    if key.is_empty() {
        return None;
    }
    Some(SsConfig {
        host: p::ss_host(),
        port: port as u16,
        method: p::ss_method_id(),
        password: key,
        remote_dns: p::proxy_remote_dns(),
        sites_only: p::proxy_sites_only(),
    })
}

static IP_LINE: Rx = Rx::new(r"(?m)^ip=(\S+)");
static LOC_LINE: Rx = Rx::new(r"(?m)^loc=(\S+)");
static TS_LINE: Rx = Rx::new(r"(?m)^ts=(\d+)");
static CONTENT_LENGTH: Rx = Rx::new(r"(?im)^content-length:\s*(\d+)");

/// Nút «Thử kết nối»: hỏi Cloudflare (cdn-cgi/trace, HTTP — chặng mạng nhà → máy chủ đã mã hoá) qua máy chủ c, trả IP ra
/// ngoài kèm nơi đặt máy, vd "103.1.2.3 (SG)". SS 2022 không trả lời thì xem giờ trên máy có lệch không. Lỗi → Socks(1, câu
/// báo) — giao diện hiện «Lỗi Shadowsocks: <câu báo>».
pub async fn check(c: &SsConfig) -> Result<String> {
    const HOST: &str = "www.cloudflare.com";
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut s = match tokio::time::timeout_at(deadline, connect(c, HOST, None, 80)).await {
        Ok(r) => r?,
        Err(_) => return Err(ss_err(format!("không kết nối được máy chủ Shadowsocks {}:{}", c.host, c.port))),
    };
    let req = format!("GET /cdn-cgi/trace HTTP/1.1\r\nHost: {HOST}\r\nUser-Agent: Mozilla/5.0\r\nConnection: close\r\n\r\n");
    let r = tokio::time::timeout_at(deadline, async {
        s.write_all(req.as_bytes()).await?;
        read_http(&mut s).await
    })
    .await;
    let mut message = None;
    match r {
        Ok(Ok(text)) => {
            if let Some(ip) = IP_LINE.group(1, &text) {
                return Ok(match LOC_LINE.group(1, &text) {
                    Some(loc) => format!("{ip} ({loc})"),
                    None => ip,
                });
            }
            // không đọc được kết quả từ Cloudflare — coi như máy chủ không trả lời (như bản Windows)
        }
        Ok(Err(e)) => message = ss_error_message(&e),
        Err(_) => {}
    }
    let m = SsMethod::of(&c.method).unwrap_or(SsMethod::DEFAULT);
    if m.is_2022 {
        if let Some(skew) = clock_skew().await {
            return Err(ss_err(format!(
                "giờ trên máy lệch {skew} giây — Shadowsocks 2022 cần giờ đúng (lệch tối đa 30 giây): chỉnh ngày giờ của máy"
            )));
        }
    }
    Err(ss_err(message.unwrap_or_else(|| no_reply(&m))))
}

/// Đọc một trả lời HTTP: phần đầu + đủ Content-Length thì thôi, không chờ máy chủ đóng kết nối (Xray giữ thêm 1 giây sau khi
/// trang đã đóng — thời gian thử báo ra sẽ sai).
pub async fn read_http<R: AsyncRead + Unpin + ?Sized>(s: &mut R) -> io::Result<String> {
    let mut ms: Vec<u8> = Vec::new();
    let mut b = [0u8; 4096];
    loop {
        let n = s.read(&mut b).await?;
        if n == 0 {
            break;
        }
        ms.extend_from_slice(&b[..n]);
        let Some(h) = ms.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
        let head: String = ms[..h].iter().map(|&x| x as char).collect(); // Latin-1
        if let Some(len) = CONTENT_LENGTH.group(1, &head).and_then(|l| l.parse::<usize>().ok()) {
            if ms.len() >= h + 4 + len {
                break;
            }
        }
    }
    Ok(String::from_utf8_lossy(&ms).into_owned())
}

/// Giờ trên máy lệch bao nhiêu giây so với Cloudflare (đi thẳng); None = không lệch quá 30 giây / không hỏi được.
async fn clock_skew() -> Option<i64> {
    let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(8)).build().ok()?;
    let body = client.get("https://www.cloudflare.com/cdn-cgi/trace").send().await.ok()?.bytes().await.ok()?;
    let ts: i64 = TS_LINE.group(1, &String::from_utf8_lossy(&body))?.parse().ok()?;
    let skew = (now_ms() / 1000 - ts).abs();
    (skew > 30).then_some(skew)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockDecrypt;
    use base64::Engine;
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn input(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    fn b64(b: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(b)
    }

    fn msg(e: Error) -> String {
        match e {
            Error::Socks(_, m) => m,
            e => panic!("không phải lỗi Shadowsocks: {e:?}"),
        }
    }

    /// Bộ số chuẩn của BLAKE3 (BLAKE3-team/BLAKE3 test_vectors.json — 32 byte đầu của hash / derive_key): kiểm cách dùng
    /// thư viện blake3 (cắt ngắn đầu ra = lấy phần đầu) cho đúng như BLAKE3 tự viết của bản Windows.
    #[test]
    fn blake3_official_vectors() {
        const CTX: &str = "BLAKE3 2019-12-27 16:29:52 test vectors context";
        let cases: [(usize, &str, &str); 7] = [
            (0, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", "2cc39783c223154fea8dfb7c1b1660f2ac2dcbd1c1de8277b0b0dd39b7e50d7d"),
            (1, "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213", "b3e2e340a117a499c6cf2398a19ee0d29cca2bb7404c73063382693bf66cb06c"),
            (63, "e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b", "b6451e30b953c206e34644c6803724e9d2725e0893039cfc49584f991f451af3"),
            (64, "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98", "a5c4a7053fa86b64746d4bb688d06ad1f02a18fce9afd3e818fefaa7126bf73e"),
            (65, "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee", "51fd05c3c1cfbc8ed67d139ad76f5cf8236cd2acd26627a30c104dfd9d3ff8a8"),
            (1023, "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11", "74a16c1c3d44368a86e1ca6df64be6a2f64cce8f09220787450722d85725dea5"),
            (1024, "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7", "7356cd7720d5b66b6d0697eb3177d9f8d73a4a5c5e968896eb6a689684302706"),
        ];
        for (n, h, d) in cases {
            assert_eq!(hex(blake3::hash(&input(n)).as_bytes()), h);
            assert_eq!(hex(&blake3_derive(CTX, &[&input(n)], 32)), d);
        }
        assert_eq!(hex(&blake3::hash(&input(63)).as_bytes()[..16]), cases[2].1[..32]);
        assert_eq!(hex(&blake3_derive(CTX, &[&input(64)], 16)), cases[3].2[..32]);
        // ghép nhiều mẩu = một mẩu liền
        assert_eq!(blake3_derive(CTX, &[&input(10), &input(20)], 32), blake3_derive(CTX, &[&[input(10), input(20)].concat()], 32));
    }

    /// Số kiểm bằng Python (hashlib MD5 + cryptography HKDF-SHA1).
    #[test]
    fn legacy_key_derivation() {
        let k = evp_bytes_to_key("test-password".as_bytes(), 32);
        assert_eq!(hex(&k), "dfb450efddbb5387197c84460623675b69f2cebcd8ef520a3dfddef7c3d540b2");
        assert_eq!(hex(&evp_bytes_to_key("test-password".as_bytes(), 16)), "dfb450efddbb5387197c84460623675b");
        let salt: Vec<u8> = (0..32).collect();
        assert_eq!(hex(&subkey(&SsMethod::AES256, &k, &salt)), "7b6ca12d45ef79467432717aaa6a6ae6e1dd046a505fb2ebaaac602210919595");
    }

    #[test]
    fn base64_both_alphabets() {
        let raw: Vec<u8> = (0..32u32).map(|i| (i * 37 + 251) as u8).collect();
        assert_eq!(base64_decode(&b64(&raw)), Some(raw.clone()));
        let url = b64(&raw).replace('+', "-").replace('/', "_");
        assert_eq!(base64_decode(url.trim_end_matches('=')), Some(raw));
        assert_eq!(base64_decode("abc!"), None);
        assert_eq!(base64_decode(""), None);
        assert_eq!(base64_decode("A"), None);
    }

    fn server_psk() -> Vec<u8> {
        (0..32).map(|i| i + 1).collect()
    }

    fn user_psk() -> Vec<u8> {
        (0..32).map(|i| 100 + i).collect()
    }

    fn esc(s: &str) -> String {
        percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
    }

    #[test]
    fn parse_3xui_2022_link() {
        let link = format!(
            "ss://2022-blake3-aes-256-gcm:{}:{}@vpn.example.com:24443?type=tcp#Nh%C3%A0",
            esc(&b64(&server_psk())),
            esc(&b64(&user_psk()))
        );
        let c = parse_link(&link, true, false).unwrap();
        let want = SsConfig::new("vpn.example.com", 24443, "2022-blake3-aes-256-gcm", &format!("{}:{}", b64(&server_psk()), b64(&user_psk())));
        assert_eq!(c, want);
        let k = keys(&SsMethod::B3_AES256, &c.password).unwrap();
        assert_eq!(k[0], server_psk());
        assert_eq!(k[1], user_psk());
    }

    #[test]
    fn parse_sip002_and_old_links() {
        let url = |s: &str| b64(s.as_bytes()).replace('+', "-").replace('/', "_").trim_end_matches('=').to_string();
        assert_eq!(
            parse_link(&format!("ss://{}@1.2.3.4:8388/?type=tcp#x", url("aes-256-gcm:mật khẩu")), true, false).unwrap(),
            SsConfig::new("1.2.3.4", 8388, "aes-256-gcm", "mật khẩu")
        );
        assert_eq!(
            parse_link(&format!("ss://{}#tên", b64("chacha20-poly1305:pa:ss@host.vn:443".as_bytes())), true, false).unwrap(),
            SsConfig::new("host.vn", 443, "chacha20-ietf-poly1305", "pa:ss")
        );
        let c = parse_link(&format!("ss://{}@[2001:db8::1]:8443", url(&format!("2022-blake3-aes-128-gcm:{}", b64(&[0u8; 16])))), true, false)
            .unwrap();
        assert_eq!((c.host.as_str(), c.port), ("2001:db8::1", 8443));
        // DNS / dùng cho: theo tham số
        let c = parse_link(&format!("SS://{}@h.vn:1", url("aes-128-gcm:x")), false, true).unwrap();
        assert!(!c.remote_dns && c.sites_only);
    }

    #[test]
    fn parse_rejects_transports() {
        for (query, word) in [("?type=ws&path=%2F", "ws"), ("?type=tcp&security=tls", "tls"), ("?plugin=obfs-local%3Bobfs%3Dhttp", "plugin")] {
            let e = parse_link(&format!("ss://2022-blake3-aes-256-gcm:{}@h:1{query}", b64(&server_psk())), true, false).unwrap_err();
            assert!(msg(e).contains(word), "{query}");
        }
        assert!(msg(parse_link("http://x", true, false).unwrap_err()).contains("ss://"));
        assert!(msg(parse_link("ss://", true, false).unwrap_err()).contains("không đọc được"));
        assert!(msg(parse_link("ss://a:b@nohost", true, false).unwrap_err()).contains("máy chủ / cổng"));
        assert!(parse_link("ssé://x", true, false).is_err()); // không cắt giữa ký tự UTF-8
    }

    #[test]
    fn parse_rejects_bad_keys() {
        let e = parse_link(&format!("ss://2022-blake3-aes-256-gcm:{}@h:1", b64(&[0u8; 16])), true, false).unwrap_err();
        assert!(msg(e).contains("32 byte"));
        assert!(msg(parse_link("ss://rc4-md5:abc@h:1", true, false).unwrap_err()).contains("chưa hỗ trợ"));
        let e = parse_link(&format!("ss://2022-blake3-chacha20-poly1305:{}:{}@h:1", b64(&server_psk()), b64(&user_psk())), true, false)
            .unwrap_err();
        assert!(msg(e).contains("nhiều người dùng"));
    }

    #[test]
    fn host_port_and_target() {
        assert_eq!(split_host_port("h:80"), Some(("h".into(), 80)));
        assert_eq!(split_host_port("[::1]:80"), Some(("::1".into(), 80)));
        assert_eq!(split_host_port("h:0"), None);
        assert_eq!(split_host_port("h:65536"), None);
        assert_eq!(split_host_port(":80"), None);
        assert_eq!(split_host_port("[::1]80"), None);
        assert_eq!(target("phim.vn", None, 443).unwrap(), [&[3u8, 7][..], b"phim.vn", &[1, 187]].concat());
        assert_eq!(target("x", Some("1.2.3.4".parse().unwrap()), 80).unwrap(), vec![1, 1, 2, 3, 4, 0, 80]);
        assert_eq!(target("[::1]", None, 1).unwrap()[0], 4);
        assert!(target(&"a".repeat(256), None, 1).is_err());
        assert_eq!(SsMethod::of(""), Some(SsMethod::DEFAULT));
        assert_eq!(SsMethod::of(" AES-128-GCM "), Some(SsMethod::AES128));
        assert_eq!(SsMethod::of("rc4-md5"), None);
    }

    // ---- Máy chủ thử (viết riêng theo đặc tả, không dùng SsStream / SsAead) ----

    fn nonce(n: u64) -> [u8; 12] {
        let mut b = [0u8; 12];
        b[..8].copy_from_slice(&n.to_le_bytes());
        b
    }

    fn t_open(m: &SsMethod, key: &[u8], n: u64, c: &[u8]) -> Vec<u8> {
        use aes_gcm::aead::Aead;
        let nn = nonce(n);
        let nn = GenericArray::from_slice(&nn);
        let r = if m.chacha {
            ChaCha20Poly1305::new_from_slice(key).unwrap().decrypt(nn, c)
        } else if key.len() == 16 {
            Aes128Gcm::new_from_slice(key).unwrap().decrypt(nn, c)
        } else {
            Aes256Gcm::new_from_slice(key).unwrap().decrypt(nn, c)
        };
        r.expect("máy chủ thử: giải mã hỏng")
    }

    fn t_seal(m: &SsMethod, key: &[u8], n: u64, p: &[u8]) -> Vec<u8> {
        use aes_gcm::aead::Aead;
        let nn = nonce(n);
        let nn = GenericArray::from_slice(&nn);
        let r = if m.chacha {
            ChaCha20Poly1305::new_from_slice(key).unwrap().encrypt(nn, p)
        } else if key.len() == 16 {
            Aes128Gcm::new_from_slice(key).unwrap().encrypt(nn, p)
        } else {
            Aes256Gcm::new_from_slice(key).unwrap().encrypt(nn, p)
        };
        r.unwrap()
    }

    fn t_sub(m: &SsMethod, psk: &[u8], salt: &[u8]) -> Vec<u8> {
        if m.is_2022 {
            let mut h = blake3::Hasher::new_derive_key("shadowsocks 2022 session subkey");
            h.update(psk);
            h.update(salt);
            h.finalize().as_bytes()[..m.key_size].to_vec()
        } else {
            let mut okm = vec![0u8; m.key_size];
            hkdf::Hkdf::<sha1::Sha1>::new(Some(salt), psk).expand(b"ss-subkey", &mut okm).unwrap();
            okm
        }
    }

    async fn t_read(s: &mut TcpStream, n: usize) -> Option<Vec<u8>> {
        let mut b = vec![0u8; n];
        let mut got = 0;
        while got < n {
            let r = s.read(&mut b[got..]).await.ok()?;
            if r == 0 {
                assert_eq!(got, 0, "máy chủ thử: khách ngắt giữa chừng");
                return None;
            }
            got += r;
        }
        Some(b)
    }

    fn addr_len(v: &[u8]) -> usize {
        match v[0] {
            1 => 7,
            4 => 19,
            _ => 4 + v[1] as usize,
        }
    }

    fn describe(a: &[u8]) -> String {
        let port = ((a[a.len() - 2] as u16) << 8) | a[a.len() - 1] as u16;
        let host = if a[0] == 3 {
            String::from_utf8_lossy(&a[2..2 + a[1] as usize]).into_owned()
        } else if a[0] == 1 {
            std::net::Ipv4Addr::new(a[1], a[2], a[3], a[4]).to_string()
        } else {
            let mut b = [0u8; 16];
            b.copy_from_slice(&a[1..17]);
            std::net::Ipv6Addr::from(b).to_string()
        };
        format!("{host}:{port}")
    }

    /// Máy chủ thử nhận một kết nối: giải mã yêu cầu, báo (đích, dữ liệu đầu) qua `seen`, trả "echo:" + dữ liệu đầu, rồi
    /// trả lại nguyên mọi chunk sau. Người dùng lạ (EIH sai) → đóng.
    struct TestServer {
        port: u16,
        seen: oneshot::Receiver<(String, Vec<u8>)>,
    }

    fn test_server(m: SsMethod, identity: Option<Vec<u8>>, users: Vec<Vec<u8>>) -> TestServer {
        let std_l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std_l.set_nonblocking(true).unwrap();
        let port = std_l.local_addr().unwrap().port();
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let l = TcpListener::from_std(std_l).unwrap();
            let (mut s, _) = l.accept().await.unwrap();
            drop(l);
            serve(&mut s, m, identity, users, tx).await;
        });
        TestServer { port, seen: rx }
    }

    async fn serve(s: &mut TcpStream, m: SsMethod, identity: Option<Vec<u8>>, users: Vec<Vec<u8>>, seen: oneshot::Sender<(String, Vec<u8>)>) {
        let ks = m.key_size;
        let salt = t_read(s, ks).await.unwrap();
        let mut psk = users[0].clone();
        if let Some(id) = identity {
            let mut h = blake3::Hasher::new_derive_key("shadowsocks 2022 identity subkey");
            h.update(&id);
            h.update(&salt);
            let sub = &h.finalize().as_bytes()[..ks].to_vec();
            let mut blk = aes::Block::clone_from_slice(&t_read(s, 16).await.unwrap());
            if ks == 16 {
                aes::Aes128::new_from_slice(sub).unwrap().decrypt_block(&mut blk);
            } else {
                aes::Aes256::new_from_slice(sub).unwrap().decrypt_block(&mut blk);
            }
            match users.iter().find(|u| blake3::hash(u).as_bytes()[..16] == blk[..]) {
                Some(u) => psk = u.clone(),
                None => return, // người dùng lạ: đóng
            }
        }
        let key = t_sub(&m, &psk, &salt);
        let mut rn = 0u64;
        let (addr, first);
        if m.is_2022 {
            let f = t_open(&m, &key, rn, &t_read(s, 11 + 16).await.unwrap());
            rn += 1;
            assert_eq!(f[0], 0);
            let ts = u64::from_be_bytes(f[1..9].try_into().unwrap()) as i64;
            assert!((now_ms() / 1000 - ts).abs() <= 30);
            let vl = ((f[9] as usize) << 8) | f[10] as usize;
            let v = t_open(&m, &key, rn, &t_read(s, vl + 16).await.unwrap());
            rn += 1;
            let al = addr_len(&v);
            addr = v[..al].to_vec();
            let pad = ((v[al] as usize) << 8) | v[al + 1] as usize;
            assert!(pad > 0 || v.len() > al + 2);
            first = v[al + 2 + pad..].to_vec();
        } else {
            let l = t_open(&m, &key, rn, &t_read(s, 18).await.unwrap());
            rn += 1;
            let n = ((l[0] as usize) << 8) | l[1] as usize;
            let v = t_open(&m, &key, rn, &t_read(s, n + 16).await.unwrap());
            rn += 1;
            let al = addr_len(&v);
            addr = v[..al].to_vec();
            first = v[al..].to_vec();
        }
        let _ = seen.send((describe(&addr), first.clone()));
        let mut rsalt = vec![0u8; ks];
        rand::rngs::OsRng.fill_bytes(&mut rsalt);
        let wkey = t_sub(&m, &psk, &rsalt);
        let mut wn = 0u64;
        let reply = [b"echo:".to_vec(), first].concat();
        let mut o = rsalt.clone();
        if m.is_2022 {
            let mut h = vec![0u8; 1 + 8 + ks + 2];
            h[0] = 1;
            h[1..9].copy_from_slice(&((now_ms() / 1000) as u64).to_be_bytes());
            h[9..9 + ks].copy_from_slice(&salt);
            let hl = h.len();
            h[hl - 2] = (reply.len() >> 8) as u8;
            h[hl - 1] = reply.len() as u8;
            o.extend(t_seal(&m, &wkey, wn, &h));
        } else {
            o.extend(t_seal(&m, &wkey, wn, &[(reply.len() >> 8) as u8, reply.len() as u8]));
        }
        wn += 1;
        o.extend(t_seal(&m, &wkey, wn, &reply));
        wn += 1;
        s.write_all(&o).await.unwrap();
        while let Some(lc) = t_read(s, 18).await {
            let l = t_open(&m, &key, rn, &lc);
            rn += 1;
            let n = ((l[0] as usize) << 8) | l[1] as usize;
            let data = t_open(&m, &key, rn, &t_read(s, n + 16).await.unwrap());
            rn += 1;
            let mut out = t_seal(&m, &wkey, wn, &[(data.len() >> 8) as u8, data.len() as u8]);
            wn += 1;
            out.extend(t_seal(&m, &wkey, wn, &data));
            wn += 1;
            s.write_all(&out).await.unwrap();
        }
    }

    async fn round_trip(m: SsMethod, password: &str, server: TestServer, initial: &str) {
        let c = SsConfig::new("127.0.0.1", server.port, m.id, password);
        tokio::time::timeout(Duration::from_secs(10), async move {
            let mut s = connect(&c, "phim.example", None, 443).await.unwrap();
            let first = initial.as_bytes().to_vec();
            s.write_all(&first).await.unwrap();
            let (target, got) = server.seen.await.unwrap();
            assert_eq!(target, "phim.example:443");
            assert_eq!(got, first);
            let mut back = vec![0u8; 5 + first.len()];
            s.read_exact(&mut back).await.unwrap();
            assert_eq!(back, [b"echo:".to_vec(), first].concat());
            // Chunk lớn (quá 0x3FFF với kiểu cũ → chia nhiều chunk)
            let big: Vec<u8> = (0..40_000).map(|i| (i % 97) as u8).collect();
            s.write_all(&big).await.unwrap();
            let mut got = vec![0u8; big.len()];
            s.read_exact(&mut got).await.unwrap();
            assert_eq!(got, big);
            s.shutdown().await.unwrap();
            assert_eq!(s.read(&mut [0u8; 1]).await.unwrap(), 0);
        })
        .await
        .expect("quá 10 giây");
    }

    #[tokio::test]
    async fn ss2022_multi_user_round_trip() {
        let s = test_server(SsMethod::B3_AES256, Some(server_psk()), vec![vec![7u8; 32], user_psk()]);
        round_trip(SsMethod::B3_AES256, &format!("{}:{}", b64(&server_psk()), b64(&user_psk())), s, "GET / HTTP/1.1\r\n\r\n").await;
    }

    #[tokio::test]
    async fn ss2022_aes128_single_user() {
        let k: Vec<u8> = (0..16).map(|i| i * 3).collect();
        let s = test_server(SsMethod::B3_AES128, None, vec![k.clone()]);
        round_trip(SsMethod::B3_AES128, &b64(&k), s, "xin chào").await;
    }

    #[tokio::test]
    async fn ss2022_chacha_single_user() {
        let k: Vec<u8> = (0..32).map(|i| i * 5).collect();
        let s = test_server(SsMethod::B3_CHACHA, None, vec![k.clone()]);
        round_trip(SsMethod::B3_CHACHA, &b64(&k), s, "chacha").await;
    }

    #[tokio::test]
    async fn legacy_aead_round_trip() {
        for m in [SsMethod::AES256, SsMethod::AES128, SsMethod::CHACHA_IETF] {
            let key = evp_bytes_to_key("mật khẩu".as_bytes(), m.key_size);
            let s = test_server(m, None, vec![key]);
            round_trip(m, "mật khẩu", s, &format!("legacy {}", m.id)).await;
        }
    }

    /// Bên gọi chưa nói gì: ghi 0 byte → gửi phần đầu ngay (kèm đệm), máy chủ trả lời được.
    #[tokio::test]
    async fn start_without_data() {
        for m in [SsMethod::B3_AES256, SsMethod::AES128] {
            let k = if m.is_2022 { b64(&user_psk()) } else { "mk".to_string() };
            let key = if m.is_2022 { user_psk() } else { evp_bytes_to_key(b"mk", m.key_size) };
            let server = test_server(m, None, vec![key]);
            let c = SsConfig::new("127.0.0.1", server.port, m.id, &k);
            let mut s = connect(&c, "1.2.3.4", None, 25).await.unwrap();
            assert_eq!(s.write(&[]).await.unwrap(), 0);
            let (target, first) = tokio::time::timeout(Duration::from_secs(5), server.seen).await.unwrap().unwrap();
            assert_eq!(target, "1.2.3.4:25");
            assert!(first.is_empty());
            let mut back = [0u8; 5];
            s.read_exact(&mut back).await.unwrap();
            assert_eq!(&back, b"echo:");
            assert_eq!(s.write(&[]).await.unwrap(), 0); // đã gửi phần đầu: ghi 0 byte không làm gì
        }
    }

    #[tokio::test]
    async fn wrong_user_key_reports_clear_error() {
        let server = test_server(SsMethod::B3_AES256, Some(server_psk()), vec![user_psk()]);
        let c = SsConfig::new("127.0.0.1", server.port, SsMethod::B3_AES256.id, &format!("{}:{}", b64(&server_psk()), b64(&[9u8; 32])));
        let mut s = connect(&c, "phim.example", None, 443).await.unwrap();
        s.write_all(b"hello").await.unwrap();
        let e = tokio::time::timeout(Duration::from_secs(10), s.read(&mut [0u8; 1])).await.unwrap().unwrap_err();
        assert!(ss_error_message(&e).unwrap().contains("sai khoá"));
        // đọc lại vẫn báo đúng lỗi
        let e2 = s.read(&mut [0u8; 1]).await.unwrap_err();
        assert!(ss_error_message(&e2).unwrap().contains("sai khoá"));
    }

    /// Máy chủ trả rác (không phải Shadowsocks): báo lỗi rõ, không panic.
    #[tokio::test]
    async fn garbage_server_does_not_panic() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut b = [0u8; 4096];
            let _ = s.read(&mut b).await;
            let _ = s.write_all(&[0x55u8; 200]).await;
        });
        let c = SsConfig::new("127.0.0.1", port, SsMethod::B3_AES128.id, &b64(&[1u8; 16]));
        let mut s = connect(&c, "phim.example", None, 443).await.unwrap();
        s.write_all(b"x").await.unwrap();
        let e = s.read(&mut [0u8; 8]).await.unwrap_err();
        assert!(ss_error_message(&e).unwrap().contains("sai khoá"));
    }

    #[tokio::test]
    async fn connect_errors_are_socks_errors() {
        let c = SsConfig::new("127.0.0.1", 1, "rc4-md5", "x");
        assert!(msg(connect(&c, "a.b", None, 1).await.err().unwrap()).contains("chưa hỗ trợ"));
        let c = SsConfig::new("127.0.0.1", 1, SsMethod::AES128.id, "");
        assert!(msg(connect(&c, "a.b", None, 1).await.err().unwrap()).contains("Chưa nhập"));
        // cổng đóng
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let c = SsConfig::new("127.0.0.1", port, SsMethod::AES128.id, "x");
        assert!(msg(connect(&c, "a.b", None, 1).await.err().unwrap()).contains("không kết nối được máy chủ Shadowsocks"));
    }

    #[tokio::test]
    async fn read_http_stops_at_content_length() {
        let text = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nip=1\n";
        let (mut a, mut b) = tokio::io::duplex(1024);
        a.write_all(text).await.unwrap();
        // không đóng a: phải dừng nhờ Content-Length
        let got = tokio::time::timeout(Duration::from_secs(2), read_http(&mut b)).await.unwrap().unwrap();
        assert!(got.ends_with("ip=1\n"));
        assert_eq!(IP_LINE.group(1, &got).as_deref(), Some("1"));
    }

    /// Thử với máy chủ thật (3X-UI / Xray…): chỉ chạy khi có biến môi trường SS_TEST_LINK (một hay nhiều link ss://, cách
    /// nhau bằng dấu cách) và gọi `cargo test -- --ignored`. Không đưa link thật vào code.
    #[tokio::test]
    #[ignore]
    async fn real_server_when_given() {
        let links = std::env::var("SS_TEST_LINK").unwrap_or_default();
        for l in links.split(' ').filter(|x| !x.is_empty()) {
            let ip = check(&parse_link(l, true, false).unwrap()).await.unwrap();
            assert!(!ip.is_empty());
        }
    }
}
