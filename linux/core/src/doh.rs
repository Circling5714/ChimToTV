//! DNS mã hoá (từ 1.8 bật sẵn; bản TV: Doh.kt, iPad: Doh.swift, Windows: Doh.cs): hỏi địa chỉ tên miền qua DoH (RFC 8484)
//! rồi DoT (RFC 7858), mỗi giao thức lần lượt NextDNS (hồ sơ riêng của app — mã trong keys/nextdns.txt, đóng vào tệp chạy lúc
//! dựng bằng build.rs, không có trên git) → Cloudflare → Google; lỗi hết thì bên gọi dùng DNS của máy — app không bao giờ hỏng
//! vì DNS mã hoá bị chặn. Máy chủ gọi bằng IP, kiểm chứng chỉ theo tên máy chủ (chứng chỉ gốc của máy), đi thẳng (không qua
//! proxy / cổng nội bộ). Từ 1.8.1:
//! - NextDNS dùng máy chủ gần nhất: IP anycast từ mạng Viettel bị đưa sang Hồng Kông → lần đầu hỏi chính NextDNS tên
//!   «dns.nextdns.io» (NextDNS trả máy chủ gần IP của mạng này nhất), không được thì DNS của máy; nhớ 6 giờ; anycast đứng sau.
//! - Giữ kết nối TLS tới máy chủ DNS để dùng lại (DoT; DoH bằng HTTP/1.1 keep-alive): mở kết nối mới ~140–220 ms, dùng lại
//!   ~20–40 ms. Kết nối nghỉ quá 90 giây thì bỏ; kết nối cũ bị máy chủ đóng thì mở kết nối mới ngay.
//!
//! Máy chủ lỗi thì nghỉ 2 phút. Trả lời «không có tên miền» (NXDOMAIN — vd hồ sơ NextDNS chặn) là trả lời thật: không hỏi máy
//! chủ sau.
//!
//! Tắt DNS mã hoá mà đang Vượt chặn: DNS của máy không trả lời / trả địa chỉ chặn (0.0.0.0, ::…) thì vẫn hỏi lại qua chuỗi
//! này (`unblocked`) — vd ảnh bìa CLB Phim Xưa (clbphimxua.com) bị bộ lọc DNS trong nhà chặn.
//!
//! Không ghi mã NextDNS vào nhật ký (`scrub`).

use crate::util::{now_ms, Error, Result, Rx};
use futures::future::{BoxFuture, FutureExt, Shared};
use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, RwLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    DoH,
    DoT,
}

/// Một máy chủ DNS mã hoá. `steer` = NextDNS: IP lấy theo máy chủ gần nhất (`ips_for`) rồi mới tới `ips` cố định (anycast).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub label: String,
    pub kind: Kind,
    pub host: String,
    pub path: String,
    pub ips: Vec<String>,
    pub steer: bool,
}

impl Server {
    pub fn new(label: &str, kind: Kind, host: &str, path: &str, ips: &[&str], steer: bool) -> Server {
        Server {
            label: label.into(),
            kind,
            host: host.into(),
            path: path.into(),
            ips: ips.iter().map(|s| s.to_string()).collect(),
            steer,
        }
    }

    pub fn port(&self) -> u16 {
        if self.kind == Kind::DoT {
            853
        } else {
            443
        }
    }
}

const NEXT_IPS: [&str; 2] = ["45.90.28.0", "45.90.30.0"];
const GOOGLE_IPS: [&str; 2] = ["8.8.8.8", "8.8.4.4"];

/// Thứ tự hỏi: DoH NextDNS → Cloudflare → Google, rồi DoT cũng vậy (không có mã NextDNS thì bỏ NextDNS).
pub fn chain(next_id: &str) -> Vec<Server> {
    let id = next_id.trim();
    let mut l = Vec::new();
    if !id.is_empty() {
        l.push(Server::new("DoH NextDNS", Kind::DoH, "dns.nextdns.io", &format!("/{id}"), &NEXT_IPS, true));
    }
    l.push(Server::new("DoH Cloudflare", Kind::DoH, "dns.cloudflare.com", "/dns-query", &["104.16.248.249", "104.16.249.249"], false));
    l.push(Server::new("DoH Google", Kind::DoH, "dns.google", "/dns-query", &GOOGLE_IPS, false));
    if !id.is_empty() {
        l.push(Server::new("DoT NextDNS", Kind::DoT, &format!("{id}.dns.nextdns.io"), "", &NEXT_IPS, true));
    }
    l.push(Server::new("DoT Cloudflare", Kind::DoT, "1dot1dot1dot1.cloudflare-dns.com", "", &["1.1.1.1", "1.0.0.1"], false));
    l.push(Server::new("DoT Google", Kind::DoT, "dns.google", "", &GOOGLE_IPS, false));
    l
}

/// Chuỗi máy chủ của app (theo mã NextDNS đóng lúc dựng).
pub fn servers() -> Arc<Vec<Server>> {
    static S: OnceLock<Arc<Vec<Server>>> = OnceLock::new();
    S.get_or_init(|| Arc::new(chain(&next_dns_id()))).clone()
}

/// Tên các nhà cung cấp theo thứ tự, để hiện trong Cài đặt: "NextDNS → Cloudflare → Google".
pub fn providers() -> String {
    let list = servers();
    let mut names: Vec<&str> = Vec::new();
    for s in list.iter() {
        let n = s.label.split_once(' ').map(|(_, b)| b).unwrap_or(&s.label);
        if !names.contains(&n) {
            names.push(n);
        }
    }
    names.join(" → ")
}

/// Mã NextDNS đóng vào tệp chạy lúc dựng (build.rs đọc keys/nextdns.txt); không có thì "".
fn next_dns_id() -> String {
    let id = option_env!("CBTV_NEXTDNS_ID").unwrap_or("").trim();
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
        id.to_string()
    } else {
        String::new()
    }
}

/// Bỏ mã NextDNS khỏi chữ sắp ghi nhật ký (vd lỗi chứng chỉ có tên «<mã>.dns.nextdns.io»).
fn scrub(text: &str) -> String {
    let id = next_dns_id();
    if id.is_empty() {
        text.to_string()
    } else {
        text.replace(&id, "<mã>")
    }
}

const CONNECT_MS: u64 = 2_500;
const READ_MS: u64 = 3_000;
const DOWN_MS: i64 = 120_000;
const IDLE_MS: i64 = 90_000;
const MAX_IDLE: usize = 4;
const POPS_MS: i64 = 6 * 3_600_000;

/// Khoá không «nhiễm độc» làm sập app: luồng nào lỡ panic lúc giữ khoá thì vẫn dùng tiếp dữ liệu.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Tên miền → (địa chỉ; None = tên miền không có (NXDOMAIN), hạn nhớ (ms)).
type AddrCache = HashMap<String, (Option<Ipv4Addr>, i64)>;

/// Địa chỉ đã hỏi.
static CACHE: LazyLock<Mutex<AddrCache>> = LazyLock::new(Default::default);
static DOWN_UNTIL: LazyLock<Mutex<HashMap<String, i64>>> = LazyLock::new(Default::default);
static NEXT_IP: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(Default::default);

/// Chỉ cổng gỡ lỗi (/dohfail): coi các máy chủ này (theo nhãn) như bị chặn, để thử chuỗi dự phòng.
static DEBUG_FAIL: LazyLock<RwLock<HashSet<String>>> = LazyLock::new(Default::default);

pub fn set_debug_fail(labels: HashSet<String>) {
    *DEBUG_FAIL.write().unwrap_or_else(|e| e.into_inner()) = labels;
}

pub fn debug_fail() -> HashSet<String> {
    DEBUG_FAIL.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Kết quả hỏi một máy chủ (máy chủ lỗi thì `ask` trả Err).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// `all` = mọi bản ghi A (theo thứ tự máy chủ trả); `addr` = cái đầu.
    Found { addr: Ipv4Addr, ttl: u32, all: Vec<Ipv4Addr> },
    /// NXDOMAIN: tên miền không có (hoặc hồ sơ DNS chặn) — không hỏi máy chủ khác.
    NoDomain,
    /// Có tên miền nhưng không có bản ghi A (vd chỉ có IPv6) — để DNS của máy thử.
    NoIpv4,
    /// SERVFAIL / REFUSED / gói hỏng — hỏi máy chủ sau.
    Failed,
}

fn no_domain(name: &str) -> Error {
    Error::Socks(4, format!("không tìm thấy tên miền {name}"))
}

/// Tên miền chuẩn để hỏi / nhớ: bỏ [ ] của IPv6, dấu chấm cuối, chữ thường.
fn clean(host: &str) -> String {
    host.trim_matches(|c| c == '[' || c == ']').trim_end_matches('.').to_lowercase()
}

/// Địa chỉ IPv4 qua chuỗi DNS mã hoá (nhớ theo TTL, 30 giây – 10 phút). Ok(None) = IP / localhost, hoặc mọi máy chủ lỗi /
/// không có bản ghi A → bên gọi dùng DNS của máy. Tên miền không có → Err(Socks(4, …)).
pub async fn resolve(host: &str) -> Result<Option<IpAddr>> {
    resolve_with(host, servers()).await
}

type Job = Shared<BoxFuture<'static, Result<Option<IpAddr>>>>;

/// Đang hỏi: tên miền → (số việc, việc). Nhiều kết nối cùng lúc tới một tên miền (ảnh bìa, API…): chỉ hỏi một lần, các kết
/// nối kia chờ kết quả đó.
static INFLIGHT: LazyLock<Mutex<HashMap<String, (u64, Job)>>> = LazyLock::new(Default::default);
static JOB_ID: AtomicU64 = AtomicU64::new(1);

/// Như `resolve`, với chuỗi máy chủ riêng (test).
pub async fn resolve_with(host: &str, servers: Arc<Vec<Server>>) -> Result<Option<IpAddr>> {
    let name = clean(host);
    if name.parse::<IpAddr>().is_ok() || name == "localhost" {
        return Ok(None);
    }
    if let Some(hit) = cached(&name) {
        return hit;
    }
    let job = {
        let mut map = lock(&INFLIGHT);
        match map.get(&name) {
            Some((_, j)) => j.clone(),
            None => {
                let id = JOB_ID.fetch_add(1, Ordering::Relaxed);
                let n = name.clone();
                // Chạy riêng (không theo bên gọi): bên gọi bỏ chờ giữa chừng thì việc hỏi vẫn xong cho các bên khác — như
                // bản Windows (dùng chung cho mọi kết nối đang chờ, chỉ dừng theo hạn giờ của từng máy chủ)
                let handle = crate::rt().spawn(async move {
                    let r = lookup(&n, &servers).await;
                    let mut map = lock(&INFLIGHT);
                    if map.get(&n).map(|(i, _)| *i) == Some(id) {
                        map.remove(&n);
                    }
                    r
                });
                let j: Job = async move { handle.await.unwrap_or(Ok(None)) }.boxed().shared();
                map.insert(name.clone(), (id, j.clone()));
                j
            }
        }
    };
    job.await
}

fn cached(name: &str) -> Option<Result<Option<IpAddr>>> {
    let c = lock(&CACHE);
    let (addr, until) = c.get(name)?;
    if *until <= now_ms() {
        return None;
    }
    Some(match addr {
        Some(a) => Ok(Some(IpAddr::V4(*a))),
        None => Err(no_domain(name)),
    })
}

async fn lookup(name: &str, servers: &[Server]) -> Result<Option<IpAddr>> {
    // Tên miền sai (nhãn rỗng / dài quá 63 byte…): khỏi hỏi, để DNS của máy báo lỗi — không coi máy chủ nào là lỗi
    if dns_wire::query(name, 0).is_err() {
        return Ok(None);
    }
    for s in servers {
        let down = lock(&DOWN_UNTIL).get(&s.label).copied().unwrap_or(0);
        if down > now_ms() {
            continue;
        }
        let ips = ips_for(s).await;
        if ips.is_empty() {
            continue;
        }
        let k = lock(&NEXT_IP).get(&s.label).copied().unwrap_or(0);
        let ip = ips[k % ips.len()].clone();
        let t0 = now_ms();
        let answer = match ask(s, &ip, name).await {
            Ok(a) => a,
            Err(e) => {
                // Máy chủ bị chặn / không trả lời: nghỉ 2 phút, lần sau thử IP kế của nó
                lock(&DOWN_UNTIL).insert(s.label.clone(), now_ms() + DOWN_MS);
                *lock(&NEXT_IP).entry(s.label.clone()).or_insert(0) += 1;
                crate::log::d("dns", &scrub(&format!("{} · {ip} lỗi: {:?} {e}", s.label, e.kind())));
                continue;
            }
        };
        match answer {
            Answer::Found { addr, ttl, .. } => {
                let keep = (ttl as i64).clamp(30, 600) * 1000;
                lock(&CACHE).insert(name.to_string(), (Some(addr), now_ms() + keep));
                if cfg!(debug_assertions) {
                    crate::log::d("dns", &format!("{name} → {addr} ({} · {ip}, {} ms)", s.label, now_ms() - t0));
                }
                return Ok(Some(IpAddr::V4(addr)));
            }
            Answer::NoDomain => {
                // nhớ 30 giây: kết nối khác khỏi hỏi lại
                lock(&CACHE).insert(name.to_string(), (None, now_ms() + 30_000));
                return Err(no_domain(name));
            }
            Answer::NoIpv4 => return Ok(None),
            Answer::Failed => {}
        }
    }
    Ok(None)
}

// ---- NextDNS: máy chủ gần nhất ----

static POPS: Mutex<(Vec<String>, i64)> = Mutex::new((Vec::new(), 0));
static POPS_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

/// IP để gọi máy chủ s: NextDNS = máy chủ gần nhất rồi anycast; còn lại = IP cố định.
pub async fn ips_for(s: &Server) -> Vec<String> {
    if !s.steer {
        return s.ips.clone();
    }
    let mut l = next_pops().await;
    for ip in &s.ips {
        if !l.contains(ip) {
            l.push(ip.clone());
        }
    }
    l
}

fn fresh_pops() -> Option<Vec<String>> {
    let p = lock(&POPS);
    if now_ms() < p.1 {
        Some(p.0.clone())
    } else {
        None
    }
}

async fn next_pops() -> Vec<String> {
    if let Some(p) = fresh_pops() {
        return p;
    }
    let _g = POPS_LOCK.lock().await;
    if let Some(p) = fresh_pops() {
        return p;
    }
    let found = discover_pops().await;
    *lock(&POPS) = (found.clone(), now_ms() + if found.is_empty() { 600_000 } else { POPS_MS });
    let list = if found.is_empty() { "(không tìm được — dùng anycast)".to_string() } else { found.join(", ") };
    crate::log::d("dns", &format!("NextDNS gần nhất: {list}"));
    found
}

/// Hỏi chính NextDNS (anycast, đã mã hoá) tên «dns.nextdns.io» — NextDNS trả máy chủ gần IP của mạng này; không được thì
/// DNS máy.
async fn discover_pops() -> Vec<String> {
    let all = servers();
    let any = all.iter().find(|x| x.steer && x.kind == Kind::DoH).or_else(|| all.iter().find(|x| x.steer));
    let Some(any) = any else {
        return Vec::new();
    };
    for ip in &any.ips {
        // lỗi thì thử IP anycast kế
        if let Ok(Answer::Found { all, .. }) = ask(any, ip, "dns.nextdns.io").await {
            return distinct(all.into_iter().map(IpAddr::V4).filter(|a| !blocked(a)).map(|a| a.to_string()));
        }
    }
    match tokio::net::lookup_host(("dns.nextdns.io", 0)).await {
        Ok(it) => distinct(it.map(|a| a.ip()).filter(|a| a.is_ipv4() && !blocked(a)).map(|a| a.to_string())),
        Err(_) => Vec::new(),
    }
}

fn distinct(it: impl Iterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for x in it {
        if !v.contains(&x) {
            v.push(x);
        }
    }
    v
}

/// Hỏi máy chủ s ở địa chỉ ip.
pub async fn ask(s: &Server, ip: &str, name: &str) -> io::Result<Answer> {
    if DEBUG_FAIL.read().unwrap_or_else(|e| e.into_inner()).contains(&s.label) {
        return Err(io::Error::other("giả bị chặn"));
    }
    let id: u16 = if s.kind == Kind::DoT { rand::random() } else { 0 }; // DoH khuyên id 0 (dễ lưu đệm)
    let query = dns_wire::query(name, id)?;
    Ok(match exchange(s, ip, &query).await? {
        None => Answer::Failed,
        Some(reply) => dns_wire::answer(&reply, id),
    })
}

// ---- Kết nối dùng lại ----

struct DnsConn {
    key: String,
    io: BufReader<tokio_rustls::client::TlsStream<TcpStream>>,
    last_used: i64,
}

static IDLE: LazyLock<Mutex<HashMap<String, Vec<DnsConn>>>> = LazyLock::new(Default::default);

fn take(key: &str) -> Option<DnsConn> {
    let mut idle = lock(&IDLE);
    let q = idle.get_mut(key)?;
    // nghỉ quá lâu thì bỏ (đóng khi rơi khỏi phạm vi)
    while let Some(c) = q.pop() {
        if now_ms() - c.last_used < IDLE_MS {
            return Some(c);
        }
    }
    None
}

fn give(mut c: DnsConn) {
    c.last_used = now_ms();
    let mut idle = lock(&IDLE);
    let q = idle.entry(c.key.clone()).or_default();
    if q.len() < MAX_IDLE {
        q.push(c);
    }
}

/// Một lần hỏi: dùng lại kết nối đang nghỉ (hỏng thì mở mới ngay), không có thì mở kết nối mới.
async fn exchange(s: &Server, ip: &str, query: &[u8]) -> io::Result<Option<Vec<u8>>> {
    let key = format!("{}|{ip}", s.label);
    if let Some(old) = take(&key) {
        // lỗi: máy chủ đã đóng kết nối cũ — không phải máy chủ lỗi, mở kết nối mới bên dưới
        if let Ok(r) = round_trip(s, old, query).await {
            return Ok(r);
        }
    }
    let c = open(s, ip, key).await?;
    round_trip(s, c, query).await
}

/// Cấu hình TLS chung: chứng chỉ gốc của máy (như reqwest), bộ mã hoá ring.
fn tls_config() -> Option<Arc<rustls::ClientConfig>> {
    static C: OnceLock<Option<Arc<rustls::ClientConfig>>> = OnceLock::new();
    C.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        for c in rustls_native_certs::load_native_certs().certs {
            let _ = roots.add(c);
        }
        let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .ok()?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Some(Arc::new(cfg))
    })
    .clone()
}

fn timed_out(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, format!("{what} quá thời gian"))
}

async fn open(s: &Server, ip: &str, key: String) -> io::Result<DnsConn> {
    let addr: IpAddr = ip.parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "IP máy chủ DNS sai"))?;
    let tcp = tokio::time::timeout(Duration::from_millis(CONNECT_MS), TcpStream::connect(SocketAddr::new(addr, s.port())))
        .await
        .map_err(|_| timed_out("kết nối"))??;
    tcp.set_nodelay(true)?;
    let cfg = tls_config().ok_or_else(|| io::Error::other("không tạo được TLS"))?;
    // Kiểm chứng chỉ theo tên máy chủ
    let name = rustls::pki_types::ServerName::try_from(s.host.clone())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "tên máy chủ DNS sai"))?;
    let tls = tokio::time::timeout(Duration::from_millis(READ_MS), tokio_rustls::TlsConnector::from(cfg).connect(name, tcp))
        .await
        .map_err(|_| timed_out("bắt tay TLS"))??;
    Ok(DnsConn { key, io: BufReader::with_capacity(4096, tls), last_used: now_ms() })
}

/// Gửi một câu hỏi trên kết nối c, đọc đúng một trả lời rồi trả kết nối về chỗ nghỉ (lỗi / máy chủ báo đóng thì đóng).
async fn round_trip(s: &Server, mut c: DnsConn, query: &[u8]) -> io::Result<Option<Vec<u8>>> {
    match tokio::time::timeout(Duration::from_millis(READ_MS), talk(s, &mut c, query)).await {
        Ok(Ok((reply, keep))) => {
            if keep {
                give(c);
            }
            Ok(reply)
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(timed_out("máy chủ DNS trả lời")),
    }
}

async fn talk(s: &Server, c: &mut DnsConn, query: &[u8]) -> io::Result<(Option<Vec<u8>>, bool)> {
    if s.kind == Kind::DoT {
        let mut msg = vec![(query.len() >> 8) as u8, query.len() as u8];
        msg.extend_from_slice(query);
        c.io.get_mut().write_all(&msg).await?;
        c.io.get_mut().flush().await?;
        let mut len = [0u8; 2];
        c.io.read_exact(&mut len).await?;
        let mut reply = vec![0u8; ((len[0] as usize) << 8) | len[1] as usize];
        c.io.read_exact(&mut reply).await?;
        return Ok((Some(reply), true));
    }
    let head = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/dns-message\r\nAccept: application/dns-message\r\nContent-Length: {}\r\n\r\n",
        s.path,
        s.host,
        query.len()
    );
    let mut msg = head.into_bytes();
    msg.extend_from_slice(query);
    c.io.get_mut().write_all(&msg).await?;
    c.io.get_mut().flush().await?;
    let r = doh_http::read(&mut c.io).await?;
    Ok((if r.status == 200 { Some(r.body) } else { None }, r.keep_alive))
}

/// Đổi cài đặt mạng: quên địa chỉ đã hỏi và máy chủ đang nghỉ.
pub fn clear_cache() {
    lock(&CACHE).clear();
    lock(&DOWN_UNTIL).clear();
    // Đóng cả kết nối đang nghỉ: bật proxy (DNS hỏi qua proxy) / tắt DNS mã hoá thì không còn lần hỏi nào lấy chúng ra, máy
    // chủ đóng phía bên kia → socket nằm CloseWait tới lúc tắt app
    lock(&IDLE).clear();
}

// ---- Vượt chặn khi tắt DNS mã hoá ----

/// Địa chỉ DNS chặn trả: trỏ về chính máy chứ không phải trang thật (0.0.0.0, ::, 127.x, ::1).
pub fn blocked(a: &IpAddr) -> bool {
    match a {
        IpAddr::V4(v) => v.is_unspecified() || v.is_loopback(),
        IpAddr::V6(v) => {
            v.is_unspecified()
                || v.is_loopback()
                || v.to_ipv4_mapped().map(|m| m.is_unspecified() || m.is_loopback()).unwrap_or(false)
        }
    }
}

/// Chỉ cổng gỡ lỗi (/dnsblock): coi DNS của máy trả địa chỉ này cho các tên miền này, để thử Vượt chặn.
static DEBUG_DNS_BLOCK: LazyLock<RwLock<(HashSet<String>, IpAddr)>> =
    LazyLock::new(|| RwLock::new((HashSet::new(), IpAddr::V4(Ipv4Addr::UNSPECIFIED))));

pub fn set_debug_dns_block(hosts: HashSet<String>, ip: IpAddr) {
    *DEBUG_DNS_BLOCK.write().unwrap_or_else(|e| e.into_inner()) = (hosts, ip);
}

pub fn debug_dns_block() -> (HashSet<String>, IpAddr) {
    DEBUG_DNS_BLOCK.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// DNS của máy không trả lời / trả địa chỉ chặn / trả địa chỉ đáng ngờ → địa chỉ qua DNS mã hoá; Ok(None) = cứ dùng DNS của
/// máy. Xét mọi địa chỉ trong câu trả lời (có một địa chỉ chặn là cả câu trả lời không dùng được — đo DNS Viettel 13/11/2026:
/// tên miền bị chặn trả 127.0.0.1).
pub async fn unblocked(host: &str) -> Result<Option<IpAddr>> {
    let name = clean(host);
    if name.parse::<IpAddr>().is_ok() || name == "localhost" {
        return Ok(None);
    }
    // lỗi: DNS của máy không trả lời → hỏi DNS mã hoá
    if let Ok(it) = tokio::net::lookup_host((name.as_str(), 0)).await {
        let mut list: Vec<IpAddr> = Vec::new();
        for a in it {
            if !list.contains(&a.ip()) {
                list.push(a.ip());
            }
        }
        {
            let d = DEBUG_DNS_BLOCK.read().unwrap_or_else(|e| e.into_inner());
            if d.0.contains(&name) {
                list = vec![d.1];
            }
        }
        if !list.is_empty() && !list.iter().any(blocked) && !shared_answer(&name, &list) {
            return Ok(None);
        }
    }
    resolve(&name).await
}

struct Answered {
    /// địa chỉ → tên miền gốc đầu tiên nhận địa chỉ đó
    first: HashMap<IpAddr, String>,
    /// địa chỉ đã thấy trả cho hai tên miền gốc khác nhau
    suspects: HashSet<IpAddr>,
}

static ANSWERED: LazyLock<Mutex<Answered>> =
    LazyLock::new(|| Mutex::new(Answered { first: HashMap::new(), suspects: HashSet::new() }));

/// Trang cảnh báo của nhà mạng: DNS chặn trả cùng một địa chỉ (máy chủ trang cảnh báo) cho mọi trang bị chặn. Câu trả lời có
/// địa chỉ đã trả cho một tên miền gốc khác thì đáng ngờ → Vượt chặn hỏi lại DNS mã hoá cho chắc; từ đó mọi trang nhận địa
/// chỉ ấy (kể cả trang đầu tiên) cũng vậy. Hai trang dùng chung CDN cũng trùng — DNS mã hoá trả địa chỉ đúng, không hại gì.
pub fn shared_answer(host: &str, all: &[IpAddr]) -> bool {
    let bas = crate::unblock::base_domain(host);
    let mut shared = false;
    let mut a = lock(&ANSWERED);
    if a.first.len() > 512 {
        a.first.clear();
    }
    for ip in all {
        if a.suspects.contains(ip) {
            shared = true;
            continue;
        }
        match a.first.get(ip) {
            None => {
                a.first.insert(*ip, bas.clone());
            }
            Some(prev) if *prev != bas => {
                a.suspects.insert(*ip);
                shared = true;
            }
            Some(_) => {}
        }
    }
    shared
}

pub fn forget_answers() {
    let mut a = lock(&ANSWERED);
    a.first.clear();
    a.suspects.clear();
}

/// Địa chỉ cho một kết nối mà máy tự phân giải (đi thẳng, hoặc proxy dùng «DNS của máy»): DNS mã hoá nếu bật, không thì
/// Vượt chặn DNS; Ok(None) = dùng DNS của máy như thường. Tên miền không có (DNS mã hoá trả NXDOMAIN) → Err(Socks(4, …)).
pub async fn for_connect(host: &str) -> Result<Option<IpAddr>> {
    if crate::prefs::secure_dns() {
        resolve(host).await
    } else if crate::unblock::level() > 0 {
        unblocked(host).await
    } else {
        Ok(None)
    }
}

/// Gói DNS (RFC 1035): hỏi bản ghi A, đọc trả lời.
pub mod dns_wire {
    use super::Answer;
    use std::io;
    use std::net::Ipv4Addr;

    pub fn query(name: &str, id: u16) -> io::Result<Vec<u8>> {
        let mut q = vec![(id >> 8) as u8, id as u8, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]; // RD, 1 câu hỏi
        for label in name.trim_end_matches('.').split('.') {
            let b = label.as_bytes();
            if b.is_empty() || b.len() > 63 || !label.is_ascii() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("tên miền sai: {name}")));
            }
            q.push(b.len() as u8);
            q.extend_from_slice(b);
        }
        q.extend_from_slice(&[0, 0, 1, 0, 1]); // hết tên, kiểu A, lớp IN
        Ok(q)
    }

    /// Trả lời → Answer; id khác (DoT) hay gói hỏng → Failed.
    pub fn answer(b: &[u8], id: u16) -> Answer {
        if b.len() < 12 {
            return Answer::Failed;
        }
        // chỉ gọi khi i + 1 < b.len()
        let u16_at = |i: usize| ((b[i] as usize) << 8) | b[i + 1] as usize;
        if u16_at(0) != id as usize {
            return Answer::Failed;
        }
        let rcode = b[3] & 0x0F;
        if rcode == 3 {
            return Answer::NoDomain;
        }
        if rcode != 0 {
            return Answer::Failed;
        }
        let mut i = 12;
        for _ in 0..u16_at(4) {
            i = skip_name(b, i) + 4;
        }
        let mut all = Vec::new();
        const NONE: i64 = i32::MAX as i64;
        let mut ttl = NONE;
        for _ in 0..u16_at(6) {
            i = skip_name(b, i);
            if i + 10 > b.len() {
                return Answer::Failed;
            }
            let kind = u16_at(i);
            let t = u32::from_be_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as i64;
            let len = u16_at(i + 8);
            i += 10;
            if i + len > b.len() {
                return Answer::Failed;
            }
            if kind == 1 && len == 4 {
                all.push(Ipv4Addr::new(b[i], b[i + 1], b[i + 2], b[i + 3]));
                ttl = ttl.min(t.min(NONE));
            }
            i += len;
        }
        if all.is_empty() {
            return Answer::NoIpv4;
        }
        Answer::Found { addr: all[0], ttl: if ttl == NONE { 60 } else { ttl as u32 }, all }
    }

    /// Bỏ qua một tên (có thể nén bằng con trỏ 0xC0).
    fn skip_name(b: &[u8], mut i: usize) -> usize {
        while i < b.len() {
            let l = b[i] as usize;
            if l == 0 {
                return i + 1;
            }
            if l & 0xC0 == 0xC0 {
                return i + 2;
            }
            i += 1 + l;
        }
        b.len()
    }
}

/// Trả lời HTTP/1.1 của máy chủ DoH: đọc đúng một trả lời trên kết nối (để dùng lại kết nối).
pub mod doh_http {
    use super::Rx;
    use std::io;
    use tokio::io::{AsyncRead, AsyncReadExt};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Reply {
        pub status: u16,
        pub body: Vec<u8>,
        pub keep_alive: bool,
    }

    static CLOSE: Rx = Rx::new(r"(?im)^connection:\s*close");
    static CHUNKED: Rx = Rx::new(r"(?im)^transfer-encoding:\s*chunked");
    static LENGTH: Rx = Rx::new(r"(?im)^content-length:\s*(\d+)");

    const MAX_BODY: usize = 70_000;

    fn broken(msg: &str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
    }

    async fn byte<R: AsyncRead + Unpin + ?Sized>(s: &mut R) -> io::Result<u8> {
        let mut one = [0u8; 1];
        if s.read(&mut one).await? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "máy chủ ngắt giữa chừng"));
        }
        Ok(one[0])
    }

    /// Đọc đầu trả lời rồi thân theo Content-Length / chunked; không có độ dài thì đọc tới khi máy chủ đóng.
    pub async fn read<R: AsyncRead + Unpin + ?Sized>(s: &mut R) -> io::Result<Reply> {
        let mut head = Vec::new();
        let mut last: u32 = 0;
        while last != 0x0D0A_0D0A {
            let b = byte(s).await?;
            head.push(b);
            last = (last << 8) | b as u32;
            if head.len() > 16_384 {
                return Err(broken("đầu trả lời quá dài"));
            }
        }
        let h: String = head.iter().map(|&b| b as char).collect(); // Latin-1
        let status = h
            .split("\r\n")
            .next()
            .and_then(|l| l.split(' ').nth(1))
            .and_then(|x| x.parse::<u16>().ok())
            .ok_or_else(|| broken("trả lời HTTP sai"))?;
        let close = CLOSE.matches(&h);
        if CHUNKED.matches(&h) {
            return Ok(Reply { status, body: dechunk(s).await?, keep_alive: !close });
        }
        let Some(len) = LENGTH.group(1, &h) else {
            return Ok(Reply { status, body: read_to_end(s).await?, keep_alive: false });
        };
        let len: usize = len.parse().map_err(|_| broken("trả lời DNS quá dài"))?;
        if len > MAX_BODY {
            return Err(broken("trả lời DNS quá dài"));
        }
        let mut body = vec![0u8; len];
        s.read_exact(&mut body).await?;
        Ok(Reply { status, body, keep_alive: !close })
    }

    /// Trả lời đã nhận trọn: thân khi mã 200, None nếu mã khác / hỏng (để test).
    pub fn body(resp: &[u8]) -> Option<Vec<u8>> {
        let mut r = resp;
        match futures::executor::block_on(read(&mut r)) {
            Ok(x) if x.status == 200 => Some(x.body),
            _ => None,
        }
    }

    async fn line<R: AsyncRead + Unpin + ?Sized>(s: &mut R) -> io::Result<String> {
        let mut b = String::new();
        loop {
            let c = byte(s).await?;
            if c == b'\n' {
                return Ok(b.trim_end_matches('\r').to_string());
            }
            b.push(c as char);
            if b.len() > 1024 {
                return Err(broken("dòng quá dài"));
            }
        }
    }

    async fn dechunk<R: AsyncRead + Unpin + ?Sized>(s: &mut R) -> io::Result<Vec<u8>> {
        let mut o = Vec::new();
        loop {
            let l = line(s).await?;
            let size = l.split(';').next().unwrap_or("").trim();
            let n = usize::from_str_radix(size, 16).map_err(|_| broken("chunk sai"))?;
            if n == 0 {
                // bỏ phần đuôi (trailer)
                while !line(s).await?.is_empty() {}
                return Ok(o);
            }
            if o.len().saturating_add(n) > MAX_BODY {
                return Err(broken("trả lời DNS quá dài"));
            }
            let at = o.len();
            o.resize(at + n, 0);
            s.read_exact(&mut o[at..]).await?;
            line(s).await?; // CRLF sau mỗi khúc
        }
    }

    async fn read_to_end<R: AsyncRead + Unpin + ?Sized>(s: &mut R) -> io::Result<Vec<u8>> {
        let mut o = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = s.read(&mut buf).await?;
            if n == 0 {
                return Ok(o);
            }
            o.extend_from_slice(&buf[..n]);
            if o.len() > MAX_BODY {
                return Err(broken("trả lời DNS quá dài"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..t.len()).step_by(2).map(|i| u8::from_str_radix(&t[i..i + 2], 16).unwrap()).collect()
    }

    fn to_hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn blocked_answers() {
        for a in ["0.0.0.0", "::", "127.0.0.1", "::1"] {
            assert!(blocked(&ip(a)), "{a}");
        }
        for a in ["104.21.35.220", "2606:4700:3035::6815:23dc", "10.0.0.1"] {
            assert!(!blocked(&ip(a)), "{a}");
        }
    }

    /// Trang cảnh báo của nhà mạng: cùng một địa chỉ cho hai tên miền gốc khác nhau → đáng ngờ, kể cả với trang đầu tiên.
    #[test]
    fn same_answer_for_different_sites_is_suspicious() {
        forget_answers();
        let warn = [ip("203.0.113.9")];
        assert!(!shared_answer("phimmoi.test", &warn));
        assert!(!shared_answer("img.phimmoi.test", &warn)); // cùng tên miền gốc
        assert!(shared_answer("www.kkphim.test", &warn));
        assert!(shared_answer("phimmoi.test", &warn)); // từ giờ trang đầu tiên cũng hỏi lại DNS mã hoá
        assert!(!shared_answer("khac.test", &[ip("198.51.100.7")]));
        forget_answers();
    }

    #[test]
    fn query_for_a() {
        assert_eq!(to_hex(&dns_wire::query("a.bc", 0).unwrap()), "000001000001000000000000016102626300".to_string() + "00010001");
        assert!(dns_wire::query("a..b", 0).is_err());
        assert!(dns_wire::query(&"x".repeat(64), 0).is_err());
    }

    #[test]
    fn answer_with_cname_and_compression() {
        let reply = hex(
            "0000 8180 0001 0003 0000 0000\
             03 777777 01 78 02 766e 00 0001 0001\
             c00c 0005 0001 00000e10 0004 c010 c010\
             c010 0001 0001 0000012c 0004 68 15 23 dc\
             c010 0001 0001 0000003c 0004 ac 43 b4 3b",
        );
        match dns_wire::answer(&reply, 0) {
            Answer::Found { addr, ttl, all } => {
                assert_eq!(addr, "104.21.35.220".parse::<Ipv4Addr>().unwrap());
                assert_eq!(ttl, 60);
                // mọi bản ghi A
                assert_eq!(all.iter().map(|a| a.to_string()).collect::<Vec<_>>(), vec!["104.21.35.220", "172.67.180.59"]);
            }
            a => panic!("{a:?}"),
        }
    }

    #[test]
    fn no_domain_no_ipv4_and_wrong_id() {
        assert_eq!(dns_wire::answer(&hex("0000 8183 0001 0000 0000 0000 01 78 00 0001 0001"), 0), Answer::NoDomain);
        let empty = hex("0000 8180 0001 0000 0000 0000 01 78 00 0001 0001");
        assert_eq!(dns_wire::answer(&empty, 0), Answer::NoIpv4);
        assert_eq!(dns_wire::answer(&empty, 0x1234), Answer::Failed); // DoT: id khác → bỏ
        assert_eq!(dns_wire::answer(&hex("0000 8182 0001 0000 0000 0000 01 78 00 0001 0001"), 0), Answer::Failed);
        // gói cụt / rác: không panic
        assert_eq!(dns_wire::answer(&hex("0000 8180 0001 0001 0000 0000 01 78 00 0001 0001 c00c 0001"), 0), Answer::Failed);
        assert_eq!(dns_wire::answer(&[0u8; 5], 0), Answer::Failed);
        assert_eq!(dns_wire::answer(&hex("0000 8180 ffff ffff 0000 0000 3f"), 0), Answer::Failed);
    }

    #[test]
    fn http_body_plain_and_chunked() {
        assert_eq!(
            doh_http::body(b"HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: 3\r\n\r\nabc").as_deref(),
            Some(&b"abc"[..])
        );
        assert_eq!(
            doh_http::body(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n3;x=1\r\ncde\r\n0\r\n\r\n").as_deref(),
            Some(&b"abcde"[..])
        );
        assert_eq!(doh_http::body(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n"), None);
        assert_eq!(doh_http::body(b"HTTP/1.1 200 OK\r\nContent-Length: 99999999999999999999999\r\n\r\n"), None);
        assert_eq!(doh_http::body(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\n"), None);
    }

    #[test]
    fn chain_order() {
        let labels = |l: Vec<Server>| l.into_iter().map(|s| s.label).collect::<Vec<_>>();
        assert_eq!(
            labels(chain("abc123")),
            vec!["DoH NextDNS", "DoH Cloudflare", "DoH Google", "DoT NextDNS", "DoT Cloudflare", "DoT Google"]
        );
        assert_eq!(chain("abc123")[0].path, "/abc123");
        assert_eq!(chain("abc123")[3].host, "abc123.dns.nextdns.io");
        assert_eq!(labels(chain("")), vec!["DoH Cloudflare", "DoH Google", "DoT Cloudflare", "DoT Google"]);
        assert!(providers().ends_with("Cloudflare → Google"));
    }

    #[tokio::test]
    async fn dead_servers_fall_back_to_device() {
        // Mọi máy chủ không tới được (cổng đóng trên máy) → None: bên gọi dùng DNS của máy, không báo lỗi
        let dead = Arc::new(vec![Server::new("Chết", Kind::DoT, "dns.test", "", &["127.0.0.1"], false)]);
        assert_eq!(resolve_with("phim.test", dead.clone()).await.unwrap(), None);
        assert_eq!(resolve_with("localhost", dead.clone()).await.unwrap(), None); // IP / localhost: không hỏi
        assert_eq!(resolve_with("[::1]", dead).await.unwrap(), None);
    }

    /// Máy chủ lỗi → nghỉ 2 phút, lần sau thử IP kế của nó; đổi cài đặt (clear_cache) thì hỏi lại ngay.
    #[tokio::test]
    async fn failed_server_rests_then_clear_cache() {
        let s = Arc::new(vec![Server::new("Giả nghỉ", Kind::DoT, "dns.test", "", &["127.0.0.1", "127.0.0.2"], false)]);
        assert_eq!(resolve_with("nghi.test", s.clone()).await.unwrap(), None);
        assert!(lock(&DOWN_UNTIL).get("Giả nghỉ").copied().unwrap_or(0) > now_ms());
        assert_eq!(lock(&NEXT_IP).get("Giả nghỉ").copied(), Some(1));
        // đang nghỉ: không hỏi (IP kế vẫn là 1)
        assert_eq!(resolve_with("nghi2.test", s.clone()).await.unwrap(), None);
        assert_eq!(lock(&NEXT_IP).get("Giả nghỉ").copied(), Some(1));
        clear_cache();
        assert!(!lock(&DOWN_UNTIL).contains_key("Giả nghỉ"));
    }

    /// Hồ sơ chặn (NXDOMAIN) nhớ 30 giây: hỏi lại trả lỗi ngay, không hỏi máy chủ.
    #[tokio::test]
    async fn no_domain_is_remembered() {
        lock(&CACHE).insert("khong-co.test".into(), (None, now_ms() + 30_000));
        match resolve("khong-co.test").await {
            Err(Error::Socks(4, m)) => assert!(m.contains("khong-co.test")),
            r => panic!("{r:?}"),
        }
        lock(&CACHE).insert("co.test".into(), (Some(Ipv4Addr::new(203, 0, 113, 5)), now_ms() + 30_000));
        assert_eq!(resolve("CO.test.").await.unwrap(), Some(ip("203.0.113.5")));
    }

    const RN: &str = "\r\n";

    #[tokio::test]
    async fn keep_alive_reads_one_reply_at_a_time() {
        // Ba trả lời liền nhau trên một kết nối (keep-alive): đọc đúng từng cái; máy chủ báo đóng thì không dùng lại
        let all = [
            "HTTP/1.1 200 OK", RN, "Content-Length: 3", RN, RN, "abc", "HTTP/1.1 200 OK", RN, "Transfer-Encoding: chunked", RN, RN,
            "2", RN, "de", RN, "0", RN, RN, "HTTP/1.1 200 OK", RN, "Content-Length: 1", RN, "Connection: close", RN, RN, "f",
        ]
        .concat();
        let mut st = BufReader::new(all.as_bytes());
        let r1 = doh_http::read(&mut st).await.unwrap();
        let r2 = doh_http::read(&mut st).await.unwrap();
        let r3 = doh_http::read(&mut st).await.unwrap();
        assert_eq!((r1.body.as_slice(), r1.keep_alive), (&b"abc"[..], true));
        assert_eq!((r2.body.as_slice(), r2.keep_alive), (&b"de"[..], true));
        assert_eq!((r3.body.as_slice(), r3.keep_alive), (&b"f"[..], false));
    }

    #[tokio::test]
    async fn only_next_dns_is_steered() {
        assert_eq!(chain("abc123").iter().map(|s| s.steer).collect::<Vec<_>>(), vec![true, false, false, true, false, false]);
        assert_eq!(ips_for(&chain("abc123")[4]).await, vec!["1.1.1.1", "1.0.0.1"]);
    }

    #[tokio::test]
    async fn no_lookup_for_ip_or_localhost() {
        assert_eq!(unblocked("127.0.0.1").await.unwrap(), None);
        assert_eq!(unblocked("localhost").await.unwrap(), None);
        assert_eq!(unblocked("[::1]").await.unwrap(), None);
    }

    /// Mặc định mới về mạng (1.7.1): Vượt chặn mức 1 + DNS mã hoá, máy đang dùng cũng bật một lần.
    #[test]
    fn net_defaults_new_install_and_upgrade() {
        let _g = crate::net::test_lock();
        crate::prefs::test_dir();
        crate::prefs::remove("net_defaults_v1");
        crate::prefs::remove("unblock");
        crate::prefs::remove("unblock_level");
        crate::prefs::remove("doh");
        crate::prefs::apply_net_defaults();
        assert_eq!(crate::prefs::unblock_level(), 1);
        assert!(crate::prefs::secure_dns());
        // Sau đó người xem tắt thì giữ tắt
        crate::prefs::set_unblock_level(0);
        crate::prefs::set_secure_dns(false);
        crate::prefs::apply_net_defaults();
        assert_eq!(crate::prefs::unblock_level(), 0);
        assert!(!crate::prefs::secure_dns());
        // Đang mức 2 thì giữ mức 2
        crate::prefs::remove("net_defaults_v1");
        crate::prefs::set_unblock_level(2);
        crate::prefs::apply_net_defaults();
        assert_eq!(crate::prefs::unblock_level(), 2);
        assert!(crate::prefs::secure_dns());
        crate::prefs::set_unblock_level(1);
    }
}
