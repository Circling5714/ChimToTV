//! Mọi kết nối ra mạng của app (trừ đồng bộ trong nhà) đi qua đây (bản Windows: Net.cs). App và trình phát (WebKitGTK) đều
//! dùng cổng SOCKS5 nội bộ 127.0.0.1 (`start_bridge`): cổng này đi thẳng / qua proxy SOCKS5 / Shadowsocks, hỏi DNS mã hoá,
//! tách gói bắt tay (vượt chặn nhà mạng). Đổi cài đặt là có hiệu lực ngay: `settings_changed` đóng các kết nối đang mở để
//! bên gọi mở lại theo cài đặt mới.

use crate::util::{now_ms, Error, Result};
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Proxy SOCKS5 người xem nhập trong Cài đặt. `remote_dns`: proxy phân giải tên miền (gửi tên miền); false: máy tự phân
/// giải rồi gửi IP. `sites_only`: chỉ trang phim (trong danh sách) đi qua proxy; video / ảnh CDN, GitHub… đi thẳng.
#[derive(Debug, Clone, PartialEq)]
pub struct SocksConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub remote_dns: bool,
    pub sites_only: bool,
}

/// Luồng hai chiều bất kỳ (ổ cắm TCP, luồng Shadowsocks đã mã hoá…).
pub trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}

/// Kết nối tới đích: `stream` để đọc / ghi; `tcp` = ổ cắm thật khi đi thẳng / qua SOCKS5 (để vượt chặn chia gói, đặt TTL);
/// None khi qua Shadowsocks (dữ liệu đã mã hoá, khỏi chia).
pub struct Conn {
    pub stream: Pin<Box<dyn Duplex>>,
    pub fd: Option<crate::unblock::Sock>,
    pub via_proxy: bool,
    pub ss: bool,
}

// ---- Proxy đang bật ----

/// Proxy SOCKS5 đang bật và đủ thông tin (None = đi thẳng / đang chọn Shadowsocks).
pub fn proxy() -> Option<SocksConfig> {
    use crate::prefs as p;
    if !p::proxy_on() || p::proxy_kind() != "socks5" || p::proxy_host().is_empty() {
        return None;
    }
    let port = p::proxy_port();
    if !(1..=65535).contains(&port) {
        return None;
    }
    let user = p::proxy_user();
    let pass = if user.is_empty() { String::new() } else { p::proxy_pass() };
    Some(SocksConfig {
        host: p::proxy_host(),
        port: port as u16,
        user,
        pass,
        remote_dns: p::proxy_remote_dns(),
        sites_only: p::proxy_sites_only(),
    })
}

/// Máy chủ Shadowsocks đang bật (None = đi thẳng / đang dùng SOCKS5).
pub fn ss() -> Option<crate::shadowsocks::SsConfig> {
    crate::shadowsocks::config()
}

/// Đích để «Kiểm tra proxy» SOCKS5 mở thử kết nối (cổng 443): trang phim đang dùng, không có thì github.com.
pub fn proxy_test_target() -> String {
    crate::site::host_of(&crate::prefs::base_url()).unwrap_or_else(|| "github.com".into())
}

/// Có đi qua proxy tới host này không (chế độ "chỉ trang phim": chỉ tên miền trong danh sách trang).
pub fn uses_proxy(sites_only: bool, host: &str) -> bool {
    if !sites_only {
        return true;
    }
    let h = host.to_lowercase();
    let mut list = crate::prefs::sites();
    list.push(crate::prefs::base_url());
    list.iter().filter_map(|s| crate::site::host_of(s)).any(|s| h == s || h.ends_with(&format!(".{s}")))
}

struct ProxyErr {
    msg: Option<String>,
    at: i64,
}
static LAST_PROXY_ERROR: Mutex<ProxyErr> = Mutex::new(ProxyErr { msg: None, at: 0 });

/// Ghi lỗi phía proxy (ghép vào câu báo lỗi cho người xem).
pub fn note_proxy_error(message: &str) {
    let mut e = LAST_PROXY_ERROR.lock().unwrap();
    e.msg = Some(message.to_string());
    e.at = now_ms();
}

/// Lời nhắc thêm vào câu báo lỗi khi đang bật proxy (kèm lỗi phía proxy trong 2 phút gần đây).
pub fn note() -> String {
    let ss = ss().is_some();
    if proxy().is_none() && !ss {
        return String::new();
    }
    let e = LAST_PROXY_ERROR.lock().unwrap();
    let recent = match &e.msg {
        Some(m) if now_ms() - e.at < 120_000 => format!(" Lỗi proxy: {m}."),
        _ => String::new(),
    };
    format!("\nĐang bật proxy {}.{recent}", if ss { "Shadowsocks" } else { "SOCKS5" })
}

/// Lỗi proxy gần nhất (ngắn) — cho câu báo lỗi.
pub fn proxy_note_short() -> String {
    LAST_PROXY_ERROR.lock().unwrap().msg.clone().unwrap_or_else(|| "không kết nối được".into())
}

// ---- Phía khách SOCKS5 (RFC 1928, đăng nhập RFC 1929): đi qua proxy thật tới một đích ----

fn socks_reply_text(rep: u8) -> String {
    match rep {
        1 => "lỗi chung".into(),
        2 => "proxy không cho phép".into(),
        3 => "mạng không tới được".into(),
        4 => "máy đích không tới được".into(),
        5 => "máy đích từ chối".into(),
        6 => "hết thời gian".into(),
        7 => "lệnh không hỗ trợ".into(),
        8 => "kiểu địa chỉ không hỗ trợ".into(),
        n => format!("mã {n}"),
    }
}

pub async fn socks5_connect(c: &SocksConfig, host: &str, port: u16) -> Result<TcpStream> {
    let s = match tokio::time::timeout(Duration::from_secs(10), TcpStream::connect((c.host.as_str(), c.port))).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            return Err(Error::Socks(
                1,
                if e.to_string().to_lowercase().contains("lookup") || e.kind() == std::io::ErrorKind::NotFound {
                    format!("không tìm thấy proxy {}", c.host)
                } else {
                    format!("không kết nối được proxy {}:{}", c.host, c.port)
                },
            ))
        }
        Err(_) => return Err(Error::Socks(1, format!("không kết nối được proxy {}:{}", c.host, c.port))),
    };
    let _ = s.set_nodelay(true);
    let mut s = s;
    match tokio::time::timeout(Duration::from_secs(15), handshake(&mut s, c, host, port)).await {
        Ok(Ok(())) => Ok(s),
        Ok(Err(e @ Error::Socks(..))) => Err(e),
        Ok(Err(e)) => Err(Error::Socks(1, format!("proxy {}:{} ngắt kết nối ({e})", c.host, c.port))),
        Err(_) => Err(Error::Socks(1, format!("proxy {}:{} ngắt kết nối (hết thời gian)", c.host, c.port))),
    }
}

async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(st: &mut S, c: &SocksConfig, host: &str, port: u16) -> Result<()> {
    let login = !c.user.is_empty();
    st.write_all(if login { &[5, 2, 0, 2] } else { &[5, 1, 0] }).await?;
    let mut g = [0u8; 2];
    st.read_exact(&mut g).await?;
    if g[0] != 5 {
        return Err(Error::Socks(1, format!("{}:{} không phải proxy SOCKS5", c.host, c.port)));
    }
    match g[1] {
        0 => {}
        2 => {
            if !login {
                return Err(Error::Socks(1, "proxy cần tên đăng nhập / mật khẩu".into()));
            }
            let (u, p) = (c.user.as_bytes(), c.pass.as_bytes());
            if u.len() > 255 || p.len() > 255 {
                return Err(Error::Socks(1, "tên đăng nhập / mật khẩu quá dài (tối đa 255 byte)".into()));
            }
            let mut m = vec![1, u.len() as u8];
            m.extend_from_slice(u);
            m.push(p.len() as u8);
            m.extend_from_slice(p);
            st.write_all(&m).await?;
            let mut a = [0u8; 2];
            st.read_exact(&mut a).await?;
            if a[1] != 0 {
                return Err(Error::Socks(1, "proxy báo sai tên đăng nhập / mật khẩu".into()));
            }
        }
        _ => {
            return Err(Error::Socks(
                1,
                if login { "proxy không nhận cách đăng nhập này" } else { "proxy cần tên đăng nhập / mật khẩu" }.into(),
            ))
        }
    }
    let mut req = vec![5, 1, 0];
    req.extend(socks_address(c, host).await?);
    req.push((port >> 8) as u8);
    req.push(port as u8);
    st.write_all(&req).await?;
    let mut r = [0u8; 4];
    st.read_exact(&mut r).await?;
    if r[0] != 5 {
        return Err(Error::Socks(1, "proxy trả lời sai giao thức".into()));
    }
    let rep = r[1];
    let skip = match r[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut l = [0u8; 1];
            st.read_exact(&mut l).await?;
            l[0] as usize
        }
        _ => 0,
    };
    let mut rest = vec![0u8; skip + 2];
    st.read_exact(&mut rest).await?;
    // Mã 2 khi có đăng nhập: nhiều proxy nhận mọi mật khẩu ở bước đăng nhập rồi mới từ chối ở đây
    if rep == 2 && login {
        return Err(Error::Socks(2, "proxy từ chối (thường do sai tên đăng nhập / mật khẩu)".into()));
    }
    if rep != 0 {
        return Err(Error::Socks(rep, format!("proxy báo lỗi: {} ({host}:{port})", socks_reply_text(rep))));
    }
    Ok(())
}

fn ip_bytes(ip: IpAddr) -> Vec<u8> {
    match ip {
        IpAddr::V4(v) => {
            let mut b = vec![1];
            b.extend_from_slice(&v.octets());
            b
        }
        IpAddr::V6(v) => {
            let mut b = vec![4];
            b.extend_from_slice(&v.octets());
            b
        }
    }
}

/// Địa chỉ đích: tên miền (DNS của proxy) hoặc IP (máy tự phân giải).
async fn socks_address(c: &SocksConfig, host: &str) -> Result<Vec<u8>> {
    if let Ok(ip) = host.trim_matches(|ch| ch == '[' || ch == ']').parse::<IpAddr>() {
        return Ok(ip_bytes(ip));
    }
    if c.remote_dns {
        let b = host.as_bytes();
        if b.len() > 255 {
            return Err(Error::Socks(1, "tên miền quá dài".into()));
        }
        let mut v = vec![3, b.len() as u8];
        v.extend_from_slice(b);
        return Ok(v);
    }
    // DNS mã hoá (mặc định), hoặc Vượt chặn: DNS của máy chặn tên miền (trả 0.0.0.0…) → địa chỉ qua DNS mã hoá
    if let Some(real) = crate::doh::for_connect(host).await? {
        return Ok(ip_bytes(real));
    }
    let ip = system_resolve(host, 443).await.map_err(|_| Error::Socks(4, format!("không tìm thấy tên miền {host} (DNS của máy)")))?;
    Ok(ip_bytes(ip.ip()))
}

/// Hỏi DNS của máy (ưu tiên IPv4).
pub async fn system_resolve(host: &str, port: u16) -> std::io::Result<SocketAddr> {
    let list: Vec<SocketAddr> = tokio::net::lookup_host((host, port)).await?.collect();
    list.iter()
        .find(|a| a.is_ipv4())
        .or(list.first())
        .copied()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "không có địa chỉ"))
}

// ---- Kết nối tới đích theo cài đặt ----

/// Kết nối tới host:port theo cài đặt (thẳng / SOCKS5 / Shadowsocks, DNS mã hoá).
pub async fn connect(host: &str, port: u16) -> Result<Conn> {
    if let Some(c) = proxy() {
        if uses_proxy(c.sites_only, host) {
            return match socks5_connect(&c, host, port).await {
                Ok(s) => Ok(tcp_conn(s, true)),
                Err(e) => {
                    if let Error::Socks(_, m) = &e {
                        note_proxy_error(m);
                    }
                    Err(e)
                }
            };
        }
    }
    if let Some(ss) = ss() {
        if uses_proxy(ss.sites_only, host) {
            let r = async {
                // DNS của máy (không phải của máy chủ SS): DNS mã hoá, lỗi thì DNS của máy
                let mut ip = host.trim_matches(|ch| ch == '[' || ch == ']').parse::<IpAddr>().ok();
                if !ss.remote_dns && ip.is_none() {
                    ip = Some(match crate::doh::for_connect(host).await? {
                        Some(x) => x,
                        None => system_resolve(host, port)
                            .await
                            .map_err(|_| Error::Net("Không tìm thấy tên miền (trang đổi tên miền hoặc bị chặn?).".into()))?
                            .ip(),
                    });
                }
                crate::shadowsocks::connect(&ss, host, ip, port).await
            }
            .await;
            return match r {
                Ok(stream) => Ok(Conn { stream, fd: None, via_proxy: true, ss: true }),
                Err(e) => {
                    if let Error::Socks(_, m) = &e {
                        note_proxy_error(m);
                    }
                    Err(e)
                }
            };
        }
    }
    // DNS mã hoá (mặc định), hoặc Vượt chặn: DNS của máy chặn tên miền (trả 0.0.0.0…) → địa chỉ hỏi qua DNS mã hoá;
    // None = DNS của máy
    let target = match crate::doh::for_connect(host).await? {
        Some(ip) => SocketAddr::new(ip, port),
        None => match host.trim_matches(|ch| ch == '[' || ch == ']').parse::<IpAddr>() {
            Ok(ip) => SocketAddr::new(ip, port),
            Err(_) => system_resolve(host, port).await.map_err(|_| Error::Net("Không tìm thấy tên miền (trang đổi tên miền hoặc bị chặn?).".into()))?,
        },
    };
    let s = match tokio::time::timeout(Duration::from_secs(15), TcpStream::connect(target)).await {
        Ok(r) => r?,
        Err(_) => return Err(Error::Timeout),
    };
    Ok(tcp_conn(s, false))
}

fn tcp_conn(s: TcpStream, via: bool) -> Conn {
    let _ = s.set_nodelay(true);
    let fd = crate::unblock::sock_of(&s);
    Conn { stream: Box::pin(s), fd: Some(fd), via_proxy: via, ss: false }
}

// ---- Cổng SOCKS5 nội bộ (127.0.0.1, không mật khẩu) — app (reqwest) và trình phát (WebKitGTK) luôn đi qua đây ----

static BRIDGE_PORT: OnceLock<u16> = OnceLock::new();
static GENERATION: OnceLock<tokio::sync::watch::Sender<u64>> = OnceLock::new();

fn generation() -> &'static tokio::sync::watch::Sender<u64> {
    GENERATION.get_or_init(|| tokio::sync::watch::channel(0).0)
}

pub fn bridge_port() -> u16 {
    *BRIDGE_PORT.get().unwrap_or(&0)
}

/// Mở cổng nội bộ (gọi nhiều lần cũng chỉ mở một lần). Trả số cổng.
pub fn start_bridge() -> u16 {
    *BRIDGE_PORT.get_or_init(|| {
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("không mở được cổng nội bộ");
        std_listener.set_nonblocking(true).ok();
        let port = std_listener.local_addr().map(|a| a.port()).unwrap_or(0);
        crate::rt().spawn(async move {
            let listener = match TcpListener::from_std(std_listener) {
                Ok(l) => l,
                Err(e) => {
                    crate::log::d("net", &format!("cổng nội bộ lỗi: {e}"));
                    return;
                }
            };
            loop {
                match listener.accept().await {
                    Ok((c, _)) => {
                        tokio::spawn(serve(c));
                    }
                    Err(e) => {
                        crate::log::d("net", &format!("accept lỗi: {e}"));
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        });
        crate::log::d("net", &format!("Cổng nội bộ 127.0.0.1:{port}"));
        port
    })
}

/// Đổi cài đặt mạng (proxy / vượt chặn / DNS mã hoá): đóng các kết nối đang mở để bên gọi mở lại theo cài đặt mới.
pub fn settings_changed() {
    crate::doh::clear_cache();
    generation().send_modify(|g| *g += 1);
    LAST_PROXY_ERROR.lock().unwrap().msg = None;
    crate::http::reset_client();
}

async fn serve(mut client: TcpStream) {
    let _ = client.set_nodelay(true);
    let mut gen_rx = generation().subscribe();
    let r = tokio::time::timeout(Duration::from_secs(30), accept_socks(&mut client)).await;
    let (host, port) = match r {
        Ok(Ok(Some(x))) => x,
        _ => return,
    };
    // Tên miền quảng cáo / theo dõi (ad_block — như bản Windows trả 403 cho các yêu cầu này): từ chối ngay ở cổng nội bộ —
    // cổng này là đường ra mạng duy nhất của trình phát WebKitGTK (crate webkit2gtk chưa có bộ lọc nội dung)
    if crate::ad_block::is_ad(Some(&host)) {
        let _ = client.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).await;
        return;
    }
    let conn = match connect(&host, port).await {
        Ok(c) => c,
        Err(e) => {
            let code = match &e {
                Error::Socks(c, _) => (*c).clamp(1, 8),
                _ => 4,
            };
            let _ = client.write_all(&[5, code, 0, 1, 0, 0, 0, 0, 0, 0]).await;
            return;
        }
    };
    if client.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await.is_err() {
        return;
    }
    // Vượt chặn: tách gói đầu của bên gọi, tự lên Mức 2 (unblock.rs). Đổi cài đặt giữa chừng thì bỏ kết nối
    let up = tokio::select! {
        r = crate::unblock::bridge_first(&mut client, conn, &host, port) => match r {
            Some(c) => c,
            None => return,
        },
        _ = gen_rx.changed() => return,
    };
    relay(client, up, &mut gen_rx).await;
}

/// Đọc yêu cầu SOCKS5 CONNECT của bên gọi; trả (host, port). Lệnh khác → trả lời "không hỗ trợ".
async fn accept_socks(cs: &mut TcpStream) -> std::io::Result<Option<(String, u16)>> {
    let mut g = [0u8; 2];
    cs.read_exact(&mut g).await?;
    if g[0] != 5 {
        return Ok(None);
    }
    let mut methods = vec![0u8; g[1] as usize];
    cs.read_exact(&mut methods).await?;
    cs.write_all(&[5, 0]).await?;
    let mut r = [0u8; 4];
    cs.read_exact(&mut r).await?;
    if r[1] != 1 {
        cs.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        return Ok(None);
    }
    let host = match r[3] {
        1 => {
            let mut v = [0u8; 4];
            cs.read_exact(&mut v).await?;
            std::net::Ipv4Addr::from(v).to_string()
        }
        4 => {
            let mut v = [0u8; 16];
            cs.read_exact(&mut v).await?;
            std::net::Ipv6Addr::from(v).to_string()
        }
        3 => {
            let mut l = [0u8; 1];
            cs.read_exact(&mut l).await?;
            let mut name = vec![0u8; l[0] as usize];
            cs.read_exact(&mut name).await?;
            String::from_utf8_lossy(&name).to_string()
        }
        _ => return Ok(None),
    };
    let mut pb = [0u8; 2];
    cs.read_exact(&mut pb).await?;
    Ok(Some((host, u16::from_be_bytes(pb))))
}

/// Chuyển dữ liệu hai chiều tới khi một bên đóng (chiều kia chờ thêm tối đa 2 giây), hoặc cài đặt mạng đổi.
async fn relay(client: TcpStream, up: Conn, gen_rx: &mut tokio::sync::watch::Receiver<u64>) {
    use futures::future::Either;
    let ss = up.ss;
    let (mut cr, mut cw) = client.into_split();
    let (mut ur, mut uw) = tokio::io::split(up.stream);
    let to_up = async {
        let mut buf = vec![0u8; 32 * 1024];
        // Shadowsocks: phần đầu gửi cùng dữ liệu đầu tiên của bên gọi (HTTPS: gói bắt tay). Bên gọi im quá 0,3 s (giao thức
        // máy chủ nói trước) thì gửi phần đầu trước (ghi 0 byte — xem shadowsocks::connect) để máy chủ mở kết nối tới đích.
        // Chờ bằng chính lần đọc (huỷ giữa chừng không mất byte), không bằng readable(): sau bắt tay SOCKS tokio còn nhớ
        // trạng thái «đọc được» cũ nên readable() trả ngay
        if ss {
            match tokio::time::timeout(Duration::from_millis(300), cr.read(&mut buf)).await {
                Err(_) => {
                    let _started: usize = uw.write(&[]).await?;
                }
                Ok(r) => {
                    let n = r?;
                    if n == 0 {
                        return uw.shutdown().await;
                    }
                    uw.write_all(&buf[..n]).await?;
                }
            }
        }
        loop {
            let n = cr.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            uw.write_all(&buf[..n]).await?;
        }
        uw.shutdown().await
    };
    let to_client = async {
        let r = tokio::io::copy(&mut ur, &mut cw).await;
        if let Err(e) = &r {
            if let Some(m) = crate::shadowsocks::ss_error_message(e) {
                note_proxy_error(&m);
            }
        }
        let _ = cw.shutdown().await;
        r.map(|_| ())
    };
    let both = async {
        match futures::future::select(Box::pin(to_up), Box::pin(to_client)).await {
            Either::Left((_, rest)) => {
                let _ = tokio::time::timeout(Duration::from_secs(2), rest).await;
            }
            Either::Right((_, rest)) => {
                let _ = tokio::time::timeout(Duration::from_secs(2), rest).await;
            }
        }
    };
    tokio::select! {
        _ = both => {}
        _ = gen_rx.changed() => {}
    }
}

/// Test của các phần mạng đổi cài đặt chung (mức vượt chặn, DNS mã hoá…) chạy lần lượt.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}
