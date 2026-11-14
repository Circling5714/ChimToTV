//! Đồng bộ trong nhà (bản Android: SyncNet.kt, iPad: SyncService.swift, Windows: SyncService.cs — cùng giao thức). Mỗi máy mở
//! cổng 8791 trong mạng nhà và quảng bá `_conbangtv._tcp` (mDNS, TXT `id`, `p`):
//!   GET  /hello  — mã máy, tên, có đang cho ghép không
//!   POST /pair   — {device, name, platform, code}: đúng mã ghép đang hiện trên máy này → trả khoá chung (token)
//!   POST /sync   — thân = SyncPayload; ký X-Device / X-Time / X-Sig = HMAC-SHA256(token, "time\nthân"). Máy nhận gộp
//!                  rồi trả lại SyncPayload đã gộp (cũng ký) — máy gửi gộp nốt. Mọi yêu cầu kèm X-Port (cổng của máy gửi)
//!                  để máy kia nhớ địa chỉ gọi lại (khác lớp mạng thì mDNS không thấy).
//! Cổng chỉ mở khi đã ghép ít nhất một máy hoặc đang mở trang Đồng bộ. Đồng bộ đi thẳng mạng nhà (TcpStream thường, không qua
//! proxy / cổng SOCKS nội bộ của app).
//!
//! Giao diện: `add_listener` nhận các sự kiện (`SyncEvent`, gọi từ luồng nền — chuyển về luồng GTK trước khi vẽ), `status()`
//! cho trang Đồng bộ; `open_pairing` / `close_pairing` khi mở / rời trang; `sync_soon` sau khi thoát phim, theo dõi…

use crate::json;
use crate::prefs;
use crate::sync_data::{self, SyncMerge, SyncPayload};
use crate::util::{now_ms, Error, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Máy đã ghép để đồng bộ trong nhà (lưu ở khoá `sync_peers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncPeer {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub token: String,
    /// Địa chỉ lần cuối kết nối được (khi không tìm thấy máy trong mạng).
    pub host: Option<String>,
    pub port: Option<u16>,
    /// Lần đồng bộ được gần nhất (ms) và lời báo lần thử gần nhất.
    pub last: Option<i64>,
    pub status: Option<String>,
}

impl SyncPeer {
    /// Lần đồng bộ được gần nhất: "14:05 13/11" hoặc "chưa".
    pub fn last_label(&self) -> String {
        match self.last {
            Some(ms) => {
                let dm_hm = crate::util::local_dm_hm(ms); // "dd/MM HH:mm"
                let (dm, hm) = dm_hm.split_once(' ').unwrap_or((&dm_hm, ""));
                format!("{hm} {dm}")
            }
            None => "chưa".into(),
        }
    }

    /// Dòng mô tả trên trang Đồng bộ: "📺  TX3  ·  Đã khớp  ·  lần cuối: 14:05 13/11".
    pub fn line(&self) -> String {
        format!(
            "{}  {}  ·  {}  ·  lần cuối: {}",
            platform_icon(&self.platform),
            self.name,
            self.status.as_deref().unwrap_or(""),
            self.last_label()
        )
    }
}

/// Máy tìm thấy trong mạng (mDNS), chưa chắc đã ghép.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncFound {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub host: String,
    pub port: u16,
}

/// Biểu tượng theo nền tảng của máy kia.
pub fn platform_icon(platform: &str) -> &'static str {
    match platform {
        "android" => "📺",
        "ios" => "📱",
        "windows" => "💻",
        _ => "🖥",
    }
}

/// Sự kiện cho giao diện (gọi từ luồng nền).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    /// Dữ liệu máy này vừa đổi do đồng bộ (giao diện vẽ lại lịch sử, Xem tiếp, theo dõi, danh sách trang).
    DataChanged,
    /// Danh sách máy / mã ghép / trạng thái đổi (trang Đồng bộ vẽ lại).
    StateChanged,
    /// Lời báo ngắn cho người xem (toast).
    Message(String),
}

pub const SERVICE_TYPE: &str = "_conbangtv._tcp";
#[cfg(not(windows))]
pub const PLATFORM: &str = "linux";
#[cfg(windows)]
pub const PLATFORM: &str = "windows";
/// Cổng mặc định khi người dùng gõ địa chỉ không kèm cổng.
pub const DEFAULT_PORT: u16 = 8791;

/// Cổng đồng bộ: 8791; bản thử (dựng debug hoặc có biến CONBANG_DEBUG) 8792 — chạy song song bản chính trên cùng máy.
pub fn port() -> u16 {
    if cfg!(debug_assertions) || std::env::var_os("CONBANG_DEBUG").is_some() {
        8792
    } else {
        8791
    }
}

/// Tên máy (hostname, bỏ phần tên miền).
pub fn device_name() -> String {
    #[cfg(windows)]
    {
        // Như Environment.MachineName của bản C# (tên NetBIOS)
        std::env::var("COMPUTERNAME").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "Windows".into())
    }
    #[cfg(not(windows))]
    {
    let mut buf = [0u8; 256];
    let r = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if r == 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let s = String::from_utf8_lossy(&buf[..end]);
        let s = s.split('.').next().unwrap_or("").trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    "Linux".into()
    }
}

// ---- Sự kiện ----

type Listener = Arc<dyn Fn(&SyncEvent) + Send + Sync>;

static LISTENERS: Mutex<Vec<(u64, Listener)>> = Mutex::new(Vec::new());
static NEXT_LISTENER: AtomicU64 = AtomicU64::new(1);

/// Nhận sự kiện đồng bộ (gọi từ luồng nền). Trả về mã để `remove_listener`.
pub fn add_listener(f: impl Fn(&SyncEvent) + Send + Sync + 'static) -> u64 {
    let id = NEXT_LISTENER.fetch_add(1, Ordering::Relaxed);
    lock(&LISTENERS).push((id, Arc::new(f)));
    id
}

pub fn remove_listener(id: u64) {
    lock(&LISTENERS).retain(|(i, _)| *i != id);
}

fn emit(e: SyncEvent) {
    let list: Vec<Listener> = lock(&LISTENERS).iter().map(|(_, f)| f.clone()).collect();
    for f in list {
        f(&e);
    }
}

fn message(text: impl Into<String>) {
    emit(SyncEvent::Message(text.into()));
}

// ---- Trạng thái ----

struct State {
    listener: Option<tokio::task::JoinHandle<()>>,
    pair_code: Option<String>,
    pair_tries: u32,
    found: Vec<SyncFound>,
    soon: Option<tokio::task::JoinHandle<()>>,
}

static STATE: Mutex<State> = Mutex::new(State { listener: None, pair_code: None, pair_tries: 0, found: Vec::new(), soon: None });
static BUSY: AtomicBool = AtomicBool::new(false);
static BROWSING: AtomicBool = AtomicBool::new(false);
/// Gộp lần lượt (máy kia gọi tới đúng lúc máy này đang gọi đi).
static ABSORB: Mutex<()> = Mutex::new(());

fn lock<T>(m: &'static Mutex<T>) -> MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn state() -> MutexGuard<'static, State> {
    lock(&STATE)
}

/// Mã ghép 4 số đang hiện (None = trang Đồng bộ đang đóng, không cho ghép).
pub fn pair_code() -> Option<String> {
    state().pair_code.clone()
}

/// Máy tìm thấy trong mạng lần tìm gần nhất (kể cả máy đã ghép).
pub fn found() -> Vec<SyncFound> {
    state().found.clone()
}

/// Đang đồng bộ.
pub fn busy() -> bool {
    BUSY.load(Ordering::SeqCst)
}

/// Đang tìm máy trong mạng ("Đang tìm…").
pub fn browsing() -> bool {
    BROWSING.load(Ordering::SeqCst)
}

/// Cổng đang mở cho máy khác gọi tới.
pub fn listening() -> bool {
    state().listener.is_some()
}

/// Máy đã ghép.
pub fn peers() -> Vec<SyncPeer> {
    match prefs::sync_peers_raw() {
        serde_json::Value::Array(a) => a
            .into_iter()
            .filter_map(|v| serde_json::from_value::<SyncPeer>(v).ok())
            .filter(|p| !p.id.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn save_peers(list: &[SyncPeer]) {
    prefs::set_sync_peers_raw(serde_json::to_value(list).unwrap_or(serde_json::Value::Array(Vec::new())));
}

fn update_peer(id: &str, change: impl FnOnce(&mut SyncPeer)) {
    {
        let _s = state();
        let mut list = peers();
        let Some(p) = list.iter_mut().find(|x| x.id == id) else { return };
        change(p);
        save_peers(&list);
    }
    emit(SyncEvent::StateChanged);
}

fn add_peer(p: SyncPeer) {
    {
        let _s = state();
        let mut list: Vec<SyncPeer> = peers().into_iter().filter(|x| x.id != p.id).collect();
        list.push(p);
        save_peers(&list);
    }
    emit(SyncEvent::StateChanged);
}

/// Bỏ ghép (dữ liệu đã có vẫn giữ).
pub fn unpair(id: &str) {
    {
        let _s = state();
        let list: Vec<SyncPeer> = peers().into_iter().filter(|x| x.id != id).collect();
        save_peers(&list);
    }
    if peers().is_empty() && pair_code().is_none() {
        stop();
    }
    emit(SyncEvent::StateChanged);
}

/// Ảnh chụp trạng thái cho trang Đồng bộ.
#[derive(Debug, Clone)]
pub struct SyncStatus {
    /// Dòng «Máy này»: "<device_name> · địa chỉ: <address>".
    pub device_name: String,
    /// `address_text()`: địa chỉ IPv4 trong mạng (kèm cổng nếu không phải 8791) hoặc "chưa có mạng".
    pub address: String,
    pub pair_code: Option<String>,
    pub peers: Vec<SyncPeer>,
    /// Máy trong mạng chưa ghép.
    pub found: Vec<SyncFound>,
    /// Đang đồng bộ (nút «Đang đồng bộ…» thay «⇄  Đồng bộ ngay»).
    pub busy: bool,
    /// Đang tìm máy trong mạng («Đang tìm…»).
    pub browsing: bool,
    pub listening: bool,
    /// Tự đồng bộ (prefs::sync_auto).
    pub auto: bool,
}

pub fn status() -> SyncStatus {
    let peers = peers();
    let found = found().into_iter().filter(|f| peers.iter().all(|p| p.id != f.id)).collect();
    SyncStatus {
        device_name: device_name(),
        address: address_text(),
        pair_code: pair_code(),
        peers,
        found,
        busy: busy(),
        browsing: browsing(),
        listening: listening(),
        auto: prefs::sync_auto(),
    }
}

/// Địa chỉ IPv4 trong mạng của máy này (bỏ loopback, 169.254.x.x, card mạng chưa cắm).
#[cfg(windows)]
pub fn local_ips() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in if_addrs::get_if_addrs().unwrap_or_default() {
        if let IpAddr::V4(ip) = i.ip() {
            let s = ip.to_string();
            if !i.is_loopback() && !ip.is_loopback() && !ip.is_link_local() && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

#[cfg(not(windows))]
pub fn local_ips() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return out;
        }
        let mut p = ifap;
        while !p.is_null() {
            let ifa = &*p;
            let up = libc::IFF_UP as libc::c_uint | libc::IFF_RUNNING as libc::c_uint;
            if !ifa.ifa_addr.is_null()
                && (*ifa.ifa_addr).sa_family as libc::c_int == libc::AF_INET
                && ifa.ifa_flags & up == up
                && ifa.ifa_flags & libc::IFF_LOOPBACK as libc::c_uint == 0
            {
                let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                let s = ip.to_string();
                if !ip.is_loopback() && !ip.is_link_local() && !out.contains(&s) {
                    out.push(s);
                }
            }
            p = ifa.ifa_next;
        }
        libc::freeifaddrs(ifap);
    }
    out
}

/// "192.0.2.5, 198.51.100.7" (kèm ":8792" nếu không phải cổng mặc định) hoặc "chưa có mạng".
pub fn address_text() -> String {
    let ips: Vec<String> =
        local_ips().into_iter().map(|ip| if port() == DEFAULT_PORT { ip } else { format!("{ip}:{}", port()) }).collect();
    if ips.is_empty() {
        "chưa có mạng".into()
    } else {
        ips.join(", ")
    }
}

// ---- Mở cổng / quảng bá ----

/// Mở app: đã ghép thì mở cổng, đồng bộ.
pub fn app_started() {
    if peers().is_empty() {
        return;
    }
    start();
    sync_soon(2.0);
}

pub fn start() {
    start_listener();
    mdns::advertise(&device_name(), port(), &prefs::device_id(), PLATFORM);
}

/// Đóng cổng, thôi quảng bá (thoát app, bỏ ghép máy cuối cùng).
pub fn stop() {
    if let Some(h) = state().listener.take() {
        h.abort();
    }
    mdns::stop_advertise();
    emit(SyncEvent::StateChanged);
}

/// Mở cổng nghe cả IPv6 lẫn IPv4 (không có IPv6 thì chỉ IPv4).
fn bind(port: u16) -> std::io::Result<std::net::TcpListener> {
    use socket2::{Domain, Protocol, Socket, Type};
    let make = |v6: bool| -> std::io::Result<Socket> {
        let s = Socket::new(if v6 { Domain::IPV6 } else { Domain::IPV4 }, Type::STREAM, Some(Protocol::TCP))?;
        if v6 {
            s.set_only_v6(false)?;
        }
        s.set_reuse_address(true)?;
        let addr: SocketAddr =
            if v6 { (Ipv6Addr::UNSPECIFIED, port).into() } else { (Ipv4Addr::UNSPECIFIED, port).into() };
        s.bind(&addr.into())?;
        s.listen(16)?;
        s.set_nonblocking(true)?;
        Ok(s)
    };
    let s = match make(true) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => return Err(e),
        Err(_) => make(false)?,
    };
    Ok(s.into())
}

fn start_listener() {
    let p = port();
    let failed = {
        let mut s = state();
        if s.listener.is_some() {
            return;
        }
        let rt = crate::rt();
        let res = bind(p).and_then(|l| {
            let _g = rt.enter();
            tokio::net::TcpListener::from_std(l)
        });
        match res {
            Ok(l) => {
                crate::log::d("sync", &format!("mở cổng {p}"));
                s.listener = Some(rt.spawn(async move {
                    loop {
                        match l.accept().await {
                            Ok((c, _)) => {
                                tokio::spawn(serve(c));
                            }
                            Err(e) => {
                                // hết chỗ mở tệp…: nghỉ chút rồi nghe tiếp
                                crate::log::d("sync", &format!("nhận kết nối lỗi: {e}"));
                                tokio::time::sleep(Duration::from_millis(200)).await;
                            }
                        }
                    }
                }));
                None
            }
            Err(e) => Some(e),
        }
    };
    if let Some(e) = failed {
        crate::log::d("sync", &format!("không mở được cổng {p}: {e}"));
        message(format!("Không mở được cổng {p} để máy khác gọi tới ({e})."));
    }
    emit(SyncEvent::StateChanged);
}

fn new_code() -> String {
    use rand::Rng;
    format!("{:04}", rand::thread_rng().gen_range(0..10000))
}

/// Trang Đồng bộ mở: hiện mã ghép 4 số (đổi mỗi lần mở), mở cổng.
pub fn open_pairing() {
    {
        let mut s = state();
        s.pair_code = Some(new_code());
        s.pair_tries = 0;
    }
    start();
    emit(SyncEvent::StateChanged);
}

/// Rời trang Đồng bộ: thôi cho ghép; chưa ghép máy nào thì đóng cổng.
pub fn close_pairing() {
    state().pair_code = None;
    if peers().is_empty() {
        stop();
    }
    emit(SyncEvent::StateChanged);
}

/// Đặt thẳng mã ghép (test: không mở cổng / quảng bá).
#[doc(hidden)]
pub fn set_pair_code(code: Option<&str>) {
    let mut s = state();
    s.pair_code = code.map(|c| c.to_string());
    s.pair_tries = 0;
}

// ---- Máy chủ (máy kia gọi tới) ----

/// Phục vụ một kết nối tới cổng đồng bộ (đọc một yêu cầu, trả lời, đóng). Tối đa 15 giây.
pub async fn serve(mut stream: TcpStream) {
    let remote = stream.peer_addr().ok().map(|a| host_string(&a));
    let work = async {
        let Some(mut req) = http1::read(&mut stream, true).await? else { return Ok(()) };
        req.remote_host = remote;
        let resp = handle(&req);
        stream.write_all(&resp).await?;
        let _ = stream.shutdown().await;
        Ok::<(), std::io::Error>(())
    };
    match tokio::time::timeout(Duration::from_secs(15), work).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => crate::log::d("sync", &format!("phục vụ lỗi: {e}")),
        Err(_) => crate::log::d("sync", "phục vụ lỗi: quá giờ"),
    }
}

/// Địa chỉ máy gọi tới dạng chữ (IPv4 trong IPv6 → IPv4; IPv6 cục bộ giữ "%chỉ số card mạng").
fn host_string(a: &SocketAddr) -> String {
    match a {
        SocketAddr::V4(v4) => v4.ip().to_string(),
        SocketAddr::V6(v6) => {
            if let Some(v4) = v6.ip().to_ipv4_mapped() {
                v4.to_string()
            } else if v6.scope_id() != 0 {
                format!("{}%{}", v6.ip(), v6.scope_id())
            } else {
                v6.ip().to_string()
            }
        }
    }
}

fn json_body(v: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&v).unwrap_or_default()
}

fn error_response(status: u16, error: &str) -> Vec<u8> {
    http1::response(status, &json_body(serde_json::json!({ "error": error })), None)
}

/// Trả lời một yêu cầu (máy chủ đồng bộ).
pub fn handle(r: &http1::Message) -> Vec<u8> {
    match (r.method.as_str(), r.path.as_str()) {
        ("GET", "/hello") => http1::response(
            200,
            &json_body(serde_json::json!({
                "app": "conbangtv", "device": prefs::device_id(), "name": device_name(), "platform": PLATFORM,
                "version": crate::update_check::current_version(), "pairing": pair_code().is_some(),
            })),
            None,
        ),
        ("POST", "/pair") => pair_request(r),
        ("POST", "/sync") => {
            let id = r.headers.get("x-device").cloned().unwrap_or_default();
            let Some(peer) = peers().into_iter().find(|p| !id.is_empty() && p.id == id) else {
                return error_response(403, "unpaired");
            };
            if !http1::verify(r, &peer.token) {
                return error_response(401, "sig");
            }
            let Some(remote) = SyncPayload::parse(&r.body) else { return error_response(400, "json") };
            let changed = absorb(&remote);
            let back = callback(r);
            update_peer(&id, |p| {
                p.name = remote.name.clone();
                p.last = Some(now_ms());
                p.status = Some(if changed { "Đã nhận dữ liệu mới" } else { "Đã khớp" }.into());
                if let Some((h, port)) = back {
                    p.host = Some(h);
                    p.port = Some(port);
                }
            });
            if changed {
                emit(SyncEvent::DataChanged);
            }
            crate::log::d("sync", &format!("{} gọi đồng bộ: {}", remote.name, if changed { "có thay đổi" } else { "đã khớp" }));
            http1::response(200, &payload().to_json(), Some(&peer.token))
        }
        _ => error_response(404, "not found"),
    }
}

fn pair_request(r: &http1::Message) -> Vec<u8> {
    let Some(code) = pair_code() else { return error_response(403, "closed") };
    let o = json::parse_object(&r.body);
    let device = json::str(o.as_ref(), "device");
    if device.is_empty() || !o.as_ref().map(|o| o.contains_key("code")).unwrap_or(false) {
        return error_response(400, "json");
    }
    if json::str(o.as_ref(), "code") != code {
        {
            let mut s = state();
            // Đoán sai nhiều lần: đổi mã
            s.pair_tries += 1;
            if s.pair_tries >= 5 {
                s.pair_code = Some(new_code());
                s.pair_tries = 0;
            }
        }
        emit(SyncEvent::StateChanged);
        return error_response(403, "code");
    }
    let token: String = rand::random::<[u8; 32]>().iter().map(|b| format!("{b:02x}")).collect();
    let back = callback(r);
    let peer = SyncPeer {
        id: device,
        name: crate::util::non_empty(Some(json::str(o.as_ref(), "name").as_str())).unwrap_or_else(|| "Máy khác".into()),
        platform: json::str(o.as_ref(), "platform"),
        token: token.clone(),
        host: back.as_ref().map(|b| b.0.clone()).or_else(|| r.remote_host.clone()),
        port: back.as_ref().map(|b| b.1),
        last: None,
        status: Some("Vừa ghép".into()),
    };
    let name = peer.name.clone();
    add_peer(peer);
    {
        let mut s = state();
        s.pair_code = Some(new_code()); // mã dùng một lần
        s.pair_tries = 0;
    }
    emit(SyncEvent::StateChanged);
    message(format!("Đã ghép với {name}."));
    http1::response(
        200,
        &json_body(serde_json::json!({
            "device": prefs::device_id(), "name": device_name(), "platform": PLATFORM, "token": token,
        })),
        None,
    )
}

/// Máy gọi tới báo cổng của nó (X-Port) → địa chỉ để gọi lại.
fn callback(m: &http1::Message) -> Option<(String, u16)> {
    let host = m.remote_host.clone()?;
    let p: u16 = m.headers.get("x-port")?.trim().parse().ok()?;
    if p == 0 {
        return None;
    }
    Some((host, p))
}

// ---- Dữ liệu ----

/// Toàn bộ dữ liệu đồng bộ của máy này.
pub fn payload() -> SyncPayload {
    SyncPayload {
        v: 1,
        device: prefs::device_id(),
        name: device_name(),
        platform: PLATFORM.into(),
        profiles: prefs::profiles().iter().map(sync_data::sync_profile_data).collect(),
        sites: sync_data::sync_sites_data(),
    }
}

/// Gộp dữ liệu máy kia vào máy này. true = máy này có thay đổi.
pub fn absorb(remote: &SyncPayload) -> bool {
    let _g = lock(&ABSORB);
    let local: Vec<_> = prefs::profiles().iter().map(sync_data::sync_profile_data).collect();
    let mut changed = false;
    for (l, r) in SyncMerge::match_profiles(&local, &remote.profiles) {
        if sync_data::apply_sync(&SyncMerge::profile(&local[l], &remote.profiles[r])) {
            changed = true;
        }
    }
    if sync_data::apply_sites(&SyncMerge::sites(&sync_data::sync_sites_data(), &remote.sites)) {
        changed = true;
    }
    changed
}

// ---- Máy khách (máy này gọi tới máy kia) ----

/// Bỏ cờ «đang tìm» khi xong (kể cả khi việc bị huỷ giữa chừng).
struct BrowseGuard;

impl Drop for BrowseGuard {
    fn drop(&mut self) {
        BROWSING.store(false, Ordering::SeqCst);
    }
}

/// Tìm máy trong mạng (mDNS, ~2 giây).
pub async fn browse() -> Vec<SyncFound> {
    BROWSING.store(true, Ordering::SeqCst);
    let guard = BrowseGuard;
    emit(SyncEvent::StateChanged);
    let me = prefs::device_id();
    let list: Vec<SyncFound> =
        mdns::browse(SERVICE_TYPE, Duration::from_secs(2)).await.into_iter().filter(|f| f.id != me).collect();
    state().found = list.clone();
    drop(guard);
    emit(SyncEvent::StateChanged);
    list
}

/// Bấm «↻  Tìm lại» / mở trang Đồng bộ (chạy nền; xong thì báo StateChanged).
pub fn browse_now() {
    crate::rt().spawn(browse());
}

/// Mã ghép gõ vào sai dạng → lời báo; None = được.
pub fn code_error(code: &str) -> Option<&'static str> {
    let c = code.trim();
    if c.chars().count() != 4 || !c.chars().all(|ch| ch.is_ascii_digit()) {
        Some("Mã ghép gồm 4 chữ số.")
    } else {
        None
    }
}

/// Ghép bằng ô địa chỉ + ô mã (C# SyncPage.PairManual / DoPair) → lời báo cho người xem.
pub async fn pair_from_input(address: &str, code: &str) -> String {
    let Some((host, port)) = parse_address(address) else { return "Nhập địa chỉ IP của máy kia.".into() };
    pair_checked(&host, port, code).await
}

/// Ghép với máy tìm thấy trong mạng / địa chỉ đã tách (kiểm mã trước) → lời báo cho người xem.
pub async fn pair_checked(host: &str, port: u16, code: &str) -> String {
    if let Some(e) = code_error(code) {
        return e.into();
    }
    pair(host, port, code.trim()).await.unwrap_or_else(|| "Đã ghép và đồng bộ xong.".into())
}

/// "Đang ghép với 192.0.2.5…" (báo trước khi ghép).
pub fn pairing_text(host: &str) -> String {
    format!("Đang ghép với {host}…")
}

/// "10.0.0.5", "10.0.0.5:8791", "[fe80::1%5]:8791" → (máy, cổng).
pub fn parse_address(input: &str) -> Option<(String, u16)> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix('[') {
        let end = rest.find(']')?;
        let h = &rest[..end];
        let after = &rest[end + 1..];
        let p = after.strip_prefix(':').filter(|x| !x.is_empty()).and_then(|x| x.parse().ok()).unwrap_or(DEFAULT_PORT);
        return Some((h.to_string(), p));
    }
    if let Some(colon) = s.rfind(':') {
        if colon > 0 && s.find(':') == Some(colon) {
            if let Ok(port) = s[colon + 1..].parse::<u16>() {
                return Some((s[..colon].to_string(), port));
            }
        }
    }
    Some((s.to_string(), DEFAULT_PORT))
}

/// Ghép với máy kia bằng mã 4 số đang hiện trên máy đó. Trả về lời báo lỗi (None = được).
pub async fn pair(host: &str, port: u16, code: &str) -> Option<String> {
    let body = json_body(serde_json::json!({
        "device": prefs::device_id(), "name": device_name(), "platform": PLATFORM, "code": code,
    }));
    start(); // mở cổng trước để máy kia gọi lại được
    let r = match http1::send(host, port, "POST", "/pair", &body, None).await {
        Ok(r) => r,
        Err(e) => return Some(format!("Không kết nối được: {e}")),
    };
    match r.status {
        200 => {}
        403 => {
            let err = json::str(json::parse_object(&r.body).as_ref(), "error");
            return Some(
                if err == "closed" { "Máy kia chưa mở màn Đồng bộ (mở để hiện mã ghép)." } else { "Sai mã ghép." }.into(),
            );
        }
        s => return Some(format!("Máy kia trả lỗi {s}.")),
    }
    let o = json::parse_object(&r.body);
    let id = json::str(o.as_ref(), "device");
    let token = json::str(o.as_ref(), "token");
    if id.is_empty() || token.is_empty() {
        return Some("Máy kia trả dữ liệu lạ.".into());
    }
    add_peer(SyncPeer {
        id,
        name: crate::util::non_empty(Some(json::str(o.as_ref(), "name").as_str())).unwrap_or_else(|| "Máy khác".into()),
        platform: json::str(o.as_ref(), "platform"),
        token,
        host: Some(host.to_string()),
        port: Some(port),
        last: None,
        status: Some("Vừa ghép".into()),
    });
    sync_all(true).await;
    None
}

/// Chờ mặc định của `sync_soon` (C# `SyncSoon(seconds = 3)`).
pub const SYNC_SOON_SECONDS: f64 = 3.0;

/// Đồng bộ sau một lúc (gộp các lần gọi sát nhau: thoát phim, theo dõi phim…). Tắt «Tự đồng bộ» hoặc chưa ghép thì thôi.
/// Bản Windows gọi: mở app (2 s, trong `app_started`), thoát phim / quay lại cửa sổ sau hơn 1 phút (1 s), theo dõi / bỏ
/// theo dõi, xoá phim / xoá cả lịch sử, thêm người xem (mặc định 3 s).
pub fn sync_soon(seconds: f64) {
    if !prefs::sync_auto() || peers().is_empty() {
        return;
    }
    let wait = if seconds.is_finite() { seconds.clamp(0.0, 3600.0) } else { SYNC_SOON_SECONDS };
    let mut s = state();
    if let Some(old) = s.soon.take() {
        old.abort();
    }
    s.soon = Some(crate::rt().spawn(async move {
        tokio::time::sleep(Duration::from_secs_f64(wait)).await;
        // chạy riêng: lần gọi sau chỉ huỷ được lúc còn đang chờ
        tokio::spawn(sync_all(false));
    }));
}

/// Bấm «Đồng bộ ngay» (chạy nền; kết quả báo qua sự kiện).
pub fn sync_now() {
    crate::rt().spawn(sync_all(true));
}

/// Bỏ cờ «đang đồng bộ» khi xong (kể cả khi việc bị huỷ giữa chừng).
struct BusyGuard;

impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
        emit(SyncEvent::StateChanged);
    }
}

/// Đồng bộ với mọi máy đã ghép (manual: bấm tay — tìm lại máy trong mạng và báo kết quả).
pub async fn sync_all(manual: bool) {
    if peers().is_empty() || BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let _busy = BusyGuard;
    emit(SyncEvent::StateChanged);
    start();
    // Tìm lại địa chỉ trong mạng (máy kia đổi IP)
    if found().is_empty() || manual {
        browse().await;
    }
    let mut changed = false;
    for p in peers() {
        if sync_peer(&p).await {
            changed = true;
        }
    }
    if changed {
        emit(SyncEvent::DataChanged);
    }
    if manual {
        message(if changed { "Đã đồng bộ — có dữ liệu mới." } else { "Đã đồng bộ." });
    }
}

/// Đồng bộ với một máy (thử địa chỉ tìm thấy trong mạng rồi địa chỉ lần trước). true = máy này có thay đổi.
pub async fn sync_peer(p: &SyncPeer) -> bool {
    let mut targets: Vec<(String, u16)> = Vec::new();
    if let Some(f) = found().into_iter().find(|x| x.id == p.id) {
        targets.push((f.host, f.port));
    }
    if let Some(h) = &p.host {
        let t = (h.clone(), p.port.unwrap_or(DEFAULT_PORT));
        if !targets.contains(&t) {
            targets.push(t);
        }
    }
    if targets.is_empty() {
        update_peer(&p.id, |x| x.status = Some("Không thấy máy trong mạng".into()));
        return false;
    }
    let body = payload().to_json();
    let mut last: Option<Error> = None;
    for (host, port) in targets {
        match http1::send(&host, port, "POST", "/sync", &body, Some(&p.token)).await {
            Ok(r) => {
                if r.status == 403 {
                    update_peer(&p.id, |x| x.status = Some("Máy kia đã bỏ ghép — ghép lại".into()));
                    return false;
                }
                let remote =
                    if r.status == 200 && http1::verify(&r, &p.token) { SyncPayload::parse(&r.body) } else { None };
                let Some(remote) = remote else {
                    update_peer(&p.id, |x| x.status = Some(format!("Lỗi {}", r.status)));
                    return false;
                };
                let changed = absorb(&remote);
                update_peer(&p.id, |x| {
                    x.name = remote.name.clone();
                    x.last = Some(now_ms());
                    x.status = Some(if changed { "Đã nhận dữ liệu mới" } else { "Đã khớp" }.into());
                    x.host = Some(host.clone());
                    x.port = Some(port);
                });
                crate::log::d(
                    "sync",
                    &format!("đồng bộ với {} ({host}:{port}): {}", remote.name, if changed { "có thay đổi" } else { "đã khớp" }),
                );
                return changed;
            }
            Err(e) => last = Some(e),
        }
    }
    update_peer(&p.id, |x| x.status = Some("Không kết nối được".into()));
    crate::log::d("sync", &format!("không đồng bộ được với {}: {}", p.name, last.map(|e| e.to_string()).unwrap_or_default()));
    false
}

/// HTTP/1.1 tối giản trên TCP cho đồng bộ (giống SyncHTTP của bản iPad, Http1 của bản Windows).
pub mod http1 {
    use super::*;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::collections::HashMap;
    use tokio::io::{AsyncRead, AsyncReadExt};

    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct Message {
        pub method: String,
        pub path: String,
        pub status: u16,
        /// Tên đầu mục viết thường.
        pub headers: HashMap<String, String>,
        pub body: Vec<u8>,
        pub remote_host: Option<String>,
    }

    /// Đọc tối đa bấy nhiêu byte một yêu cầu / trả lời.
    pub const MAX_SIZE: usize = 8 << 20;

    /// Đọc đủ một yêu cầu / trả lời (đầu + thân theo Content-Length). Đóng kết nối trước khi đủ → None.
    pub async fn read<S: AsyncRead + Unpin + Send>(s: &mut S, request: bool) -> std::io::Result<Option<Message>> {
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 16 * 1024];
        while buf.len() < MAX_SIZE {
            let n = s.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(m) = parse(&buf, request) {
                return Ok(Some(m));
            }
        }
        Ok(None)
    }

    fn find(data: &[u8], what: &[u8]) -> Option<usize> {
        data.windows(what.len()).position(|w| w == what)
    }

    /// Đọc một yêu cầu / trả lời đã nhận đủ; chưa đủ (thiếu đầu hoặc thân) → None.
    pub fn parse(data: &[u8], request: bool) -> Option<Message> {
        let end = find(data, b"\r\n\r\n")?;
        let head = String::from_utf8_lossy(&data[..end]);
        let mut lines = head.split("\r\n");
        let first: Vec<&str> = lines.next()?.split(' ').collect();
        if first.len() < 2 {
            return None;
        }
        let mut m = Message::default();
        for l in lines {
            if let Some(i) = l.find(':') {
                if i > 0 {
                    m.headers.insert(l[..i].trim().to_lowercase(), l[i + 1..].trim().to_string());
                }
            }
        }
        let length = m.headers.get("content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        let start = end + 4;
        if data.len() - start < length {
            return None;
        }
        m.body = data[start..start + length].to_vec();
        if request {
            m.method = first[0].to_string();
            m.path = first[1].split('?').next().unwrap_or("").to_string();
        } else {
            m.status = first[1].parse().unwrap_or(0);
        }
        Some(m)
    }

    /// HMAC-SHA256(token, "time\nthân") dạng hex chữ thường — cùng cách ký bản TV / iPad / Windows.
    pub fn sign(token: &str, time: &str, body: &[u8]) -> String {
        let Ok(mut h) = <Hmac<Sha256> as Mac>::new_from_slice(token.as_bytes()) else { return String::new() };
        h.update(time.as_bytes());
        h.update(b"\n");
        h.update(body);
        h.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Chữ ký đúng và thời điểm lệch không quá 10 phút.
    pub fn verify(m: &Message, token: &str) -> bool {
        let (Some(t), Some(sig)) = (m.headers.get("x-time"), m.headers.get("x-sig")) else { return false };
        let Ok(ms) = t.parse::<i64>() else { return false };
        if now_ms().abs_diff(ms) >= 10 * 60_000 {
            return false;
        }
        let want = sign(token, t, &m.body);
        let got = sig.to_lowercase();
        // so không lộ thời gian (như FixedTimeEquals)
        want.len() == got.len() && want.bytes().zip(got.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }

    pub fn response(status: u16, body: &[u8], token: Option<&str>) -> Vec<u8> {
        let mut head = format!(
            "HTTP/1.1 {status} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n",
            if status == 200 { "OK" } else { "Error" },
            body.len()
        );
        if let Some(token) = token {
            let time = now_ms().to_string();
            head.push_str(&format!("X-Time: {time}\r\nX-Sig: {}\r\n", sign(token, &time, body)));
        }
        head.push_str("\r\n");
        let mut out = head.into_bytes();
        out.extend_from_slice(body);
        out
    }

    /// "fe80::1%5" / "fe80::1%eth0" / "192.0.2.5" → địa chỉ; tên máy → None (hỏi DNS).
    fn socket_addr(host: &str, port: u16) -> Option<SocketAddr> {
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Some(SocketAddr::new(ip, port));
        }
        let (ip, scope) = host.split_once('%')?;
        let ip: Ipv6Addr = ip.parse().ok()?;
        let scope_id = match scope.parse::<u32>() {
            Ok(n) => n,
            Err(_) => {
                let c = std::ffi::CString::new(scope).ok()?;
                #[cfg(not(windows))]
                let i = unsafe { libc::if_nametoindex(c.as_ptr()) };
                // SAFETY: chuỗi C hợp lệ trong lúc gọi
                #[cfg(windows)]
                let i = unsafe { windows_sys::Win32::NetworkManagement::IpHelper::if_nametoindex(c.as_ptr() as *const u8) };
                if i == 0 {
                    return None;
                }
                i
            }
        };
        Some(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, scope_id)))
    }

    /// Lỗi ổ cắm → câu ngắn.
    fn io_error(e: std::io::Error) -> Error {
        use std::io::ErrorKind;
        let unreachable = matches!(e.raw_os_error(), Some(libc::EHOSTUNREACH) | Some(libc::ENETUNREACH));
        Error::App(match e.kind() {
            ErrorKind::ConnectionRefused => "máy kia không nhận kết nối (app chưa mở?)".into(),
            ErrorKind::TimedOut => "quá giờ".into(),
            _ if unreachable => "không tới được máy kia".into(),
            _ => e.to_string(),
        })
    }

    /// Gửi một yêu cầu tới máy kia (thẳng trong mạng nhà, không qua proxy), chờ tối đa 8 giây.
    pub async fn send(host: &str, port: u16, method: &str, path: &str, body: &[u8], token: Option<&str>) -> Result<Message> {
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: conbangtv\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nX-Port: {}\r\n",
            body.len(),
            super::port()
        );
        if let Some(token) = token {
            let time = now_ms().to_string();
            head.push_str(&format!(
                "X-Device: {}\r\nX-Time: {time}\r\nX-Sig: {}\r\n",
                prefs::device_id(),
                sign(token, &time, body)
            ));
        }
        head.push_str("\r\n");
        let mut data = head.into_bytes();
        data.extend_from_slice(body);
        let work = async {
            let mut s = match socket_addr(host, port) {
                Some(a) => TcpStream::connect(a).await?,
                None => TcpStream::connect((host, port)).await?,
            };
            s.set_nodelay(true)?;
            s.write_all(&data).await?;
            read(&mut s, false).await
        };
        match tokio::time::timeout(Duration::from_secs(8), work).await {
            Ok(Ok(Some(mut m))) => {
                m.remote_host = Some(host.to_string());
                Ok(m)
            }
            Ok(Ok(None)) => Err(Error::app("máy kia đóng kết nối")),
            Ok(Err(e)) => Err(io_error(e)),
            Err(_) => Err(Error::app("quá giờ")),
        }
    }
}

/// mDNS / DNS-SD bằng thư viện mdns-sd: quảng bá máy này (`_conbangtv._tcp`, TXT id / p) và tìm máy khác. Tên máy trong bản
/// ghi SRV là "conbangtv-<hostname>.local" để khỏi đụng tên máy mà avahi đang quảng bá.
pub mod mdns {
    use super::{lock, SyncFound, DEFAULT_PORT};
    use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
    use std::net::IpAddr;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    static DAEMON: Mutex<Option<ServiceDaemon>> = Mutex::new(None);
    /// Tên đầy đủ của dịch vụ đang quảng bá.
    static ADVERTISED: Mutex<Option<String>> = Mutex::new(None);

    fn daemon() -> Option<ServiceDaemon> {
        let mut d = lock(&DAEMON);
        if d.is_none() {
            match ServiceDaemon::new() {
                Ok(x) => *d = Some(x),
                Err(e) => crate::log::d("sync", &format!("không mở được mDNS: {e}")),
            }
        }
        d.clone()
    }

    fn full_type(service_type: &str) -> String {
        format!("{service_type}.local.")
    }

    /// Nhãn DNS từ tên máy (chữ thường, số, "-").
    fn host_label(device_name: &str) -> String {
        let s: String = device_name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let s = s.trim_matches('-').to_string();
        format!("conbangtv-{}", if s.is_empty() { crate::sync_service::PLATFORM.to_string() } else { s })
    }

    pub fn advertise(device_name: &str, port: u16, id: &str, platform: &str) {
        let mut adv = lock(&ADVERTISED);
        if adv.is_some() {
            return;
        }
        let Some(d) = daemon() else { return };
        let instance = device_name.replace('.', " ");
        let host = format!("{}.local.", host_label(device_name));
        let props = [("id", id), ("p", platform)];
        match ServiceInfo::new(&full_type(super::SERVICE_TYPE), &instance, &host, (), port, &props[..]) {
            Ok(info) => {
                let info = info.enable_addr_auto();
                let full = info.get_fullname().to_string();
                match d.register(info) {
                    Ok(()) => {
                        crate::log::d("sync", &format!("quảng bá mDNS: {full}"));
                        *adv = Some(full);
                    }
                    Err(e) => crate::log::d("sync", &format!("không quảng bá được mDNS: {e}")),
                }
            }
            // Không quảng bá được: máy kia nhập IP của máy này để ghép
            Err(e) => crate::log::d("sync", &format!("không quảng bá được mDNS: {e}")),
        }
    }

    pub fn stop_advertise() {
        let Some(full) = lock(&ADVERTISED).take() else { return };
        if let Some(d) = lock(&DAEMON).clone() {
            let _ = d.unregister(&full);
        }
    }

    /// Tìm máy trong mạng trong khoảng `wait`.
    pub async fn browse(service_type: &str, wait: Duration) -> Vec<SyncFound> {
        let Some(d) = daemon() else { return Vec::new() };
        let ty = full_type(service_type);
        let rx = match d.browse(&ty) {
            Ok(r) => r,
            Err(e) => {
                crate::log::d("sync", &format!("tìm máy (mDNS) lỗi: {e}"));
                return Vec::new();
            }
        };
        let ty2 = ty.clone();
        let found = tokio::task::spawn_blocking(move || {
            let deadline = Instant::now() + wait;
            let mut out: Vec<SyncFound> = Vec::new();
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                match rx.recv_timeout(left) {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        let addrs: Vec<IpAddr> = info.get_addresses().iter().map(|a| a.to_ip_addr()).collect();
                        if let Some(f) = found_from(
                            info.get_fullname(),
                            &ty2,
                            info.get_property_val_str("id"),
                            info.get_property_val_str("p"),
                            &addrs,
                            info.get_port(),
                        ) {
                            out.push(f);
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            out
        })
        .await
        .unwrap_or_default();
        let _ = d.stop_browse(&ty);
        let mut list: Vec<SyncFound> = Vec::new();
        for f in found {
            // mỗi máy một mục (lần sau ghi đè lần trước: địa chỉ mới hơn)
            list.retain(|x| x.id != f.id);
            list.push(f);
        }
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    /// Bản ghi đã phân giải → máy tìm thấy (không có TXT id thì bỏ). Ưu tiên địa chỉ IPv4.
    pub fn found_from(
        fullname: &str,
        full_type: &str,
        id: Option<&str>,
        platform: Option<&str>,
        addrs: &[IpAddr],
        port: u16,
    ) -> Option<SyncFound> {
        let id = id.map(str::trim).filter(|s| !s.is_empty())?;
        let mut sorted: Vec<IpAddr> = addrs.to_vec();
        sorted.sort_by_key(|a| match a {
            IpAddr::V4(v4) if !v4.is_link_local() => (0, *a),
            IpAddr::V4(_) => (1, *a),
            IpAddr::V6(v6) if (v6.segments()[0] & 0xffc0) != 0xfe80 => (2, *a),
            IpAddr::V6(_) => (3, *a),
        });
        let host = sorted.first()?.to_string();
        let suffix = format!(".{}", full_type.trim_end_matches('.'));
        let name = fullname.trim_end_matches('.');
        let cut = name.len().saturating_sub(suffix.len());
        let label = if cut > 0 && name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(&suffix) {
            &name[..cut]
        } else {
            name
        };
        Some(SyncFound {
            id: id.to_string(),
            name: label.replace("\\032", " ").replace('\\', ""),
            platform: platform.unwrap_or("").to_string(),
            host,
            port: if port == 0 { DEFAULT_PORT } else { port },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_and_http_parsing() {
        // HMAC-SHA256("k", "123\n{}") — cùng cách ký bản TV / iPad / Windows
        assert_eq!(http1::sign("k", "123", b"{}"), "3317e7dfdb031bbbe0279631aadd48916bbed405865b3a698b8c6884f0f90685");
        let body = br#"{"a":1}"#;
        let resp = http1::response(200, body, Some("tok"));
        let m = http1::parse(&resp, false).unwrap();
        assert_eq!(m.status, 200);
        assert_eq!(m.body, body);
        assert!(http1::verify(&m, "tok"));
        assert!(!http1::verify(&m, "khac"));
        assert!(http1::parse(&resp[..resp.len() - 1], false).is_none()); // chưa đủ thân
        assert_eq!(parse_address("192.0.2.5"), Some(("192.0.2.5".into(), 8791)));
        assert_eq!(parse_address(" 192.0.2.5:8792 "), Some(("192.0.2.5".into(), 8792)));
        assert_eq!(parse_address("[fe80::1%5]:8791"), Some(("fe80::1%5".into(), 8791)));
        assert_eq!(parse_address("[fe80::1%5]"), Some(("fe80::1%5".into(), 8791)));
        assert_eq!(parse_address("tivi.local"), Some(("tivi.local".into(), 8791)));
        assert_eq!(parse_address("fe80::1"), Some(("fe80::1".into(), 8791)));
        assert_eq!(parse_address("  "), None);
        assert_eq!(parse_address("[fe80::1"), None);
    }

    #[tokio::test]
    async fn pair_input_checks() {
        assert_eq!(code_error("1234"), None);
        assert_eq!(code_error(" 0042 "), None);
        assert_eq!(code_error("12a4"), Some("Mã ghép gồm 4 chữ số."));
        assert_eq!(code_error("12345"), Some("Mã ghép gồm 4 chữ số."));
        // kiểm xong mới gọi mạng: không mở cổng, không gửi gì
        assert_eq!(pair_from_input("   ", "1234").await, "Nhập địa chỉ IP của máy kia.");
        assert_eq!(pair_from_input("192.0.2.5", "12").await, "Mã ghép gồm 4 chữ số.");
        assert_eq!(pairing_text("192.0.2.5"), "Đang ghép với 192.0.2.5…");
    }

    #[test]
    fn request_parsing_and_bad_input() {
        let req = b"POST /sync?x=1 HTTP/1.1\r\nHost: conbangtv\r\nX-Port: 8791\r\ncontent-length: 2\r\nX-Sig: ABC\r\n\r\n{}";
        let m = http1::parse(req, true).unwrap();
        assert_eq!((m.method.as_str(), m.path.as_str()), ("POST", "/sync"));
        assert_eq!(m.headers["x-port"], "8791");
        assert_eq!(m.headers["x-sig"], "ABC");
        assert_eq!(m.body, b"{}");
        // dữ liệu hỏng: không hoảng
        assert!(http1::parse(b"\r\n\r\n", true).is_none());
        assert!(http1::parse(b"GET\r\n\r\n", true).is_none());
        let huge = http1::parse(b"GET / HTTP/1.1\r\nContent-Length: 99999999999999999999\r\n\r\n", true).unwrap();
        assert!(huge.body.is_empty());
        assert!(http1::parse(b"GET / HTTP/1.1\r\nContent-Length: 50\r\n\r\nabc", true).is_none());
        let bad = http1::parse(b"HTTP/1.1 abc\r\n\r\n", false).unwrap();
        assert_eq!(bad.status, 0);
        // chữ ký: thiếu / sai giờ / lệch quá 10 phút
        let mut m = http1::parse(&http1::response(200, b"x", Some("t")), false).unwrap();
        assert!(http1::verify(&m, "t"));
        m.headers.insert("x-sig".into(), m.headers["x-sig"].to_uppercase());
        assert!(http1::verify(&m, "t"));
        let old = (now_ms() - 11 * 60_000).to_string();
        m.headers.insert("x-sig".into(), http1::sign("t", &old, b"x"));
        m.headers.insert("x-time".into(), old);
        assert!(!http1::verify(&m, "t"));
        m.headers.insert("x-time".into(), i64::MIN.to_string());
        assert!(!http1::verify(&m, "t"));
        m.headers.remove("x-time");
        assert!(!http1::verify(&m, "t"));
    }

    #[tokio::test]
    async fn read_waits_for_whole_body() {
        let (mut a, mut b) = tokio::io::duplex(64);
        let w = tokio::spawn(async move {
            let resp = http1::response(200, &[b'x'; 1000], Some("k"));
            for part in resp.chunks(7) {
                a.write_all(part).await.unwrap();
            }
        });
        let m = http1::read(&mut b, false).await.unwrap().unwrap();
        assert_eq!(m.body.len(), 1000);
        assert!(http1::verify(&m, "k"));
        w.await.unwrap();
        // đóng giữa chừng → None
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc").await.unwrap();
        drop(a);
        assert!(http1::read(&mut b, false).await.unwrap().is_none());
    }

    #[test]
    fn mdns_record_to_found() {
        let ty = "_conbangtv._tcp.local.";
        let v4: IpAddr = "192.0.2.3".parse().unwrap();
        let ll: IpAddr = "fe80::1".parse().unwrap();
        let f = mdns::found_from("TX3._conbangtv._tcp.local.", ty, Some("abc"), Some("android"), &[ll, v4], 8791).unwrap();
        assert_eq!(f, SyncFound { id: "abc".into(), name: "TX3".into(), platform: "android".into(), host: "192.0.2.3".into(), port: 8791 });
        let f = mdns::found_from("Phòng\\032khách._conbangtv._tcp.local.", ty, Some("x"), None, &[ll], 0).unwrap();
        assert_eq!((f.name.as_str(), f.platform.as_str(), f.host.as_str(), f.port), ("Phòng khách", "", "fe80::1", 8791));
        assert!(mdns::found_from("A._conbangtv._tcp.local.", ty, None, Some("ios"), &[v4], 8791).is_none());
        assert!(mdns::found_from("A._conbangtv._tcp.local.", ty, Some("x"), None, &[], 8791).is_none());
    }

    #[test]
    fn peer_json_and_labels() {
        let p = SyncPeer { id: "d".into(), name: "TX3".into(), platform: "android".into(), token: "t".into(), ..Default::default() };
        let v = serde_json::to_value(&p).unwrap();
        let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["host", "id", "last", "name", "platform", "port", "status", "token"]);
        assert_eq!(p.last_label(), "chưa");
        assert!(p.line().starts_with("📺  TX3  ·  "));
        let mut q = p.clone();
        q.last = Some(now_ms());
        assert_eq!(q.last_label().len(), "14:05 13/11".len());
        assert_eq!(platform_icon("linux"), "🖥");
        assert!(port() == 8791 || port() == 8792);
        assert!(!device_name().is_empty());
    }

    #[test]
    fn public_futures_are_send() {
        fn send<T: Send>(_: T) {}
        send(sync_all(true));
        send(pair("192.0.2.1", 8791, "1234"));
        send(pair_from_input("192.0.2.1", "1234"));
        send(pair_checked("192.0.2.1", 8791, "1234"));
        send(browse());
        let p = SyncPeer::default();
        send(sync_peer(&p));
        send(http1::send("192.0.2.1", 8791, "GET", "/hello", b"", None));
        send(mdns::browse(SERVICE_TYPE, Duration::from_millis(1)));
        let r = crate::update_check::Release {
            version: String::new(),
            deb_url: String::new(),
            notes: String::new(),
            tag: String::new(),
            sha256: None,
            size: 0,
            api_url: None,
        };
        send(crate::update_check::latest(None));
        send(crate::update_check::check(false));
        send(crate::update_check::download(&r, |_| {}));
        send(crate::update_check::install(std::path::Path::new("/x.deb")));
    }
}
