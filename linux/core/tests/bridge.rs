//! Cổng SOCKS5 nội bộ đầu-cuối (net::start_bridge): máy đích giả trên 127.0.0.1, khách SOCKS5 tối giản. Vượt chặn mức 0 / 1
//! / 2 đếm số gói thật sự tới máy đích (TCP_INFO tcpi_data_segs_in — trên lo không gộp gói), tự lên Mức 2 khi trang im, giao
//! thức máy chủ nói trước, Shadowsocks qua cổng (máy chủ SS 2022 giả viết theo đặc tả). Không ra mạng ngoài.

use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::Aes128Gcm;
use base64::Engine;
use cbtv::{net, prefs, unblock};
#[cfg(not(windows))]
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Once};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Cài đặt riêng (thư mục tạm), không ra mạng; các test chạy lần lượt (cùng đổi mức vượt chặn / proxy).
async fn setup() -> tokio::sync::MutexGuard<'static, ()> {
    static INIT: Once = Once::new();
    static LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));
    INIT.call_once(|| {
        let d = std::env::temp_dir().join(format!("cbtv-bridge-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        prefs::load(Some(d));
        prefs::set_secure_dns(false);
        prefs::set_proxy_on(false);
    });
    LOCK.lock().await
}

/// Trả cài đặt về như cũ khi test xong (kể cả khi hỏng giữa chừng) — test sau khỏi bị ảnh hưởng.
struct Restore(fn());

impl Drop for Restore {
    fn drop(&mut self) {
        (self.0)()
    }
}

/// Khách SOCKS5 tối giản: CONNECT tên miền:cổng qua cổng nội bộ.
async fn socks(host: &str, port: u16) -> TcpStream {
    let bp = net::start_bridge();
    let mut s = TcpStream::connect(("127.0.0.1", bp)).await.unwrap();
    s.set_nodelay(true).unwrap();
    s.write_all(&[5, 1, 0]).await.unwrap();
    let mut g = [0u8; 2];
    s.read_exact(&mut g).await.unwrap();
    assert_eq!(g, [5, 0]);
    let mut req = vec![5, 1, 0, 3, host.len() as u8];
    req.extend_from_slice(host.as_bytes());
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req).await.unwrap();
    let mut r = [0u8; 10];
    s.read_exact(&mut r).await.unwrap();
    assert_eq!(r[1], 0, "cổng nội bộ báo lỗi {}", r[1]);
    s
}

/// Gói bắt tay TLS giả (độ dài bản ghi đúng) có tên trang ở giữa.
fn tls_hello(name: &str) -> Vec<u8> {
    let mut body = vec![1u8, 2, 3];
    body.extend_from_slice(name.as_bytes());
    body.extend_from_slice(&[9, 9]);
    let mut h = vec![0x16, 0x03, 0x01, (body.len() >> 8) as u8, body.len() as u8];
    h.extend(body);
    h
}

/// Đọc đủ want byte; trả (byte, số gói có dữ liệu đã nhận).
async fn read_counted(s: &mut TcpStream, want: usize) -> (Vec<u8>, u32) {
    let mut data = Vec::new();
    let mut b = [0u8; 4096];
    while data.len() < want {
        let n = s.read(&mut b).await.unwrap();
        if n == 0 {
            break;
        }
        data.extend_from_slice(&b[..n]);
    }
    #[cfg(not(windows))]
    let segs = unblock::tcp_data_segs_in(s.as_raw_fd()).unwrap_or(0);
    #[cfg(windows)]
    let segs = 0; // Windows không đếm được số gói đã nhận
    (data, segs)
}

async fn listener() -> (TcpListener, u16) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    (l, port)
}

/// Mức 0 / 1 / 2 với gói bắt tay TLS: máy đích nhận đủ, đúng thứ tự, đúng số mẩu (byte đầu riêng, tên trang từng 2 byte);
/// trả lời đầu và dữ liệu sau chuyển tiếp như thường.
#[cfg(not(windows))] // đếm gói (TCP_INFO của Linux)
#[tokio::test]
async fn tls_hello_is_split_per_level() {
    let _g = setup().await;
    let _r = Restore(|| prefs::set_unblock_level(1));
    unblock::level2_memo::clear();
    for level in [0, 1, 2] {
        prefs::set_unblock_level(level);
        let (l, port) = listener().await;
        let hello = tls_hello("localhost");
        let len = hello.len();
        let srv = tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let seen = read_counted(&mut s, len).await;
            s.write_all(b"server-hello").await.unwrap();
            let mut b = [0u8; 4];
            s.read_exact(&mut b).await.unwrap();
            assert_eq!(&b, b"ping");
            s.write_all(b"pong").await.unwrap();
            seen
        });
        let before = unblock::disorder_count();
        let mut c = socks("localhost", port).await;
        c.write_all(&hello).await.unwrap();
        let mut r = [0u8; 12];
        c.read_exact(&mut r).await.unwrap();
        assert_eq!(&r, b"server-hello");
        c.write_all(b"ping").await.unwrap();
        let mut r = [0u8; 4];
        c.read_exact(&mut r).await.unwrap();
        assert_eq!(&r, b"pong");
        let (data, segs) = srv.await.unwrap();
        assert_eq!(data, hello);
        let want = unblock::hello_chunks(&hello, "localhost", level).len() as u32;
        assert_eq!(segs, want, "mức {level}");
        if level >= 1 {
            assert!(want >= 7, "tên trang phải được tách: {want}");
        }
        assert_eq!(unblock::disorder_count() > before, level == 2, "mức {level}");
    }
}

/// Yêu cầu HTTP thường: tách quanh dòng Host (mức 1), trả lời về đủ.
#[cfg(not(windows))] // đếm gói (TCP_INFO của Linux)
#[tokio::test]
async fn plain_http_is_split_around_host() {
    let _g = setup().await;
    prefs::set_unblock_level(1);
    let (l, port) = listener().await;
    let req = b"GET /phim HTTP/1.1\r\nHost: localhost\r\nAccept: */*\r\n\r\n".to_vec();
    let len = req.len();
    let srv = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let seen = read_counted(&mut s, len).await;
        s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await.unwrap();
        seen
    });
    let mut c = socks("localhost", port).await;
    c.write_all(&req).await.unwrap();
    let mut r = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), c.read_to_end(&mut r)).await.unwrap().unwrap();
    assert!(r.ends_with(b"\r\n\r\nok"));
    let (data, segs) = srv.await.unwrap();
    assert_eq!(data, req);
    assert_eq!(segs, unblock::hello_chunks(&req, "localhost", 1).len() as u32);
    assert_eq!(segs, 9); // G|ET /phim HTTP/1.1\r\nHo|st: |lo|ca|lh|os|t|\r\n…
}

/// Đi thẳng, Mức 1: kết nối đầu trang im (giả nhà mạng ghép được các mẩu) → cổng tự mở kết nối mới, gửi lại theo Mức 2;
/// bên gọi nhận trả lời như thường; trang được nhớ — lần sau đi Mức 2 ngay.
#[tokio::test]
async fn silent_site_at_level1_retries_at_level2() {
    let _g = setup().await;
    let _r = Restore(|| {
        unblock::set_first_reply_ms(4000);
        unblock::level2_memo::clear();
    });
    prefs::set_unblock_level(1);
    unblock::level2_memo::clear();
    unblock::set_first_reply_ms(500);
    let (l, port) = listener().await;
    let hello = tls_hello("localhost");
    let len = hello.len();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let accepts = Arc::new(AtomicUsize::new(0));
    {
        let (seen, accepts) = (seen.clone(), accepts.clone());
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let first = accepts.fetch_add(1, Ordering::SeqCst) == 0;
                let seen = seen.clone();
                tokio::spawn(async move {
                    let (data, _) = read_counted(&mut s, len).await;
                    seen.lock().unwrap().push(data.len());
                    if first {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    } else {
                        let _ = s.write_all(b"hello-back").await;
                        tokio::time::sleep(Duration::from_millis(300)).await;
                    }
                });
            }
        });
    }
    let auto = unblock::level2_memo::auto_count();
    let t0 = Instant::now();
    let mut c = socks("localhost", port).await;
    c.write_all(&hello).await.unwrap();
    let mut r = [0u8; 10];
    tokio::time::timeout(Duration::from_secs(5), c.read_exact(&mut r)).await.unwrap().unwrap();
    assert_eq!(&r, b"hello-back");
    assert!(t0.elapsed() >= Duration::from_millis(450));
    assert_eq!(*seen.lock().unwrap(), vec![len, len]); // gửi lại đủ gói bắt tay ở kết nối thứ hai
    assert!(unblock::level2_memo::needs("localhost")); // đặt được TTL → Mức 2 thật → nhớ trang
    assert_eq!(unblock::level2_memo::auto_count(), auto + 1);
    // Lần sau: Mức 2 ngay, không chờ
    let before = unblock::disorder_count();
    let t1 = Instant::now();
    let mut c2 = socks("localhost", port).await;
    c2.write_all(&hello).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), c2.read_exact(&mut r)).await.unwrap().unwrap();
    assert_eq!(&r, b"hello-back");
    assert!(t1.elapsed() < Duration::from_millis(450), "{:?}", t1.elapsed());
    assert!(unblock::disorder_count() > before);
    assert_eq!(accepts.load(Ordering::SeqCst), 3);
}

/// Giao thức máy chủ nói trước (bên gọi im): cổng chờ 0,3 s rồi chuyển tiếp như thường.
#[tokio::test]
async fn server_speaks_first() {
    let _g = setup().await;
    prefs::set_unblock_level(1);
    let (l, port) = listener().await;
    let srv = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        s.write_all(b"220 xin chao\r\n").await.unwrap();
        let mut b = [0u8; 6];
        s.read_exact(&mut b).await.unwrap();
        b
    });
    let mut c = socks("localhost", port).await;
    let mut r = [0u8; 14];
    tokio::time::timeout(Duration::from_secs(3), c.read_exact(&mut r)).await.unwrap().unwrap();
    assert_eq!(&r, b"220 xin chao\r\n");
    c.write_all(b"QUIT\r\n").await.unwrap();
    assert_eq!(&srv.await.unwrap(), b"QUIT\r\n");
}

/// Đích không mở cổng: cổng nội bộ trả mã lỗi SOCKS (không treo).
#[tokio::test]
async fn closed_target_reports_socks_error() {
    let _g = setup().await;
    let (l, port) = listener().await;
    drop(l);
    let bp = net::start_bridge();
    let mut s = TcpStream::connect(("127.0.0.1", bp)).await.unwrap();
    s.write_all(&[5, 1, 0]).await.unwrap();
    let mut g = [0u8; 2];
    s.read_exact(&mut g).await.unwrap();
    let mut req = vec![5, 1, 0, 1, 127, 0, 0, 1];
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req).await.unwrap();
    let mut r = [0u8; 10];
    tokio::time::timeout(Duration::from_secs(5), s.read_exact(&mut r)).await.unwrap().unwrap();
    assert_ne!(r[1], 0);
}

// ---- Shadowsocks qua cổng nội bộ (máy chủ SS 2022 giả: 2022-blake3-aes-128-gcm, một người dùng) ----

fn derive(psk: &[u8], salt: &[u8]) -> Vec<u8> {
    let mut h = blake3::Hasher::new_derive_key("shadowsocks 2022 session subkey");
    h.update(psk);
    h.update(salt);
    h.finalize().as_bytes()[..16].to_vec()
}

fn nonce(n: u64) -> [u8; 12] {
    let mut b = [0u8; 12];
    b[..8].copy_from_slice(&n.to_le_bytes());
    b
}

fn seal(key: &[u8], n: u64, p: &[u8]) -> Vec<u8> {
    Aes128Gcm::new_from_slice(key).unwrap().encrypt(GenericArray::from_slice(&nonce(n)), p).unwrap()
}

fn open(key: &[u8], n: u64, c: &[u8]) -> Vec<u8> {
    Aes128Gcm::new_from_slice(key).unwrap().decrypt(GenericArray::from_slice(&nonce(n)), c).unwrap()
}

async fn exact(s: &mut TcpStream, n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    s.read_exact(&mut b).await.unwrap();
    b
}

/// Một kết nối SS: đọc yêu cầu, báo (đích, dữ liệu đầu), trả "echo:" + dữ liệu đầu, giữ kết nối tới khi khách đóng.
/// Các yêu cầu máy chủ SS giả đã thấy: (đích, dữ liệu đầu).
type Seen = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

async fn ss_conn(mut s: TcpStream, psk: Vec<u8>, seen: Seen) {
    let salt = exact(&mut s, 16).await;
    let key = derive(&psk, &salt);
    let f = open(&key, 0, &exact(&mut s, 11 + 16).await);
    assert_eq!(f[0], 0);
    let vl = ((f[9] as usize) << 8) | f[10] as usize;
    let v = open(&key, 1, &exact(&mut s, vl + 16).await);
    assert_eq!(v[0], 3, "gửi tên miền (DNS của proxy)");
    let hl = v[1] as usize;
    let host = String::from_utf8_lossy(&v[2..2 + hl]).into_owned();
    let port = ((v[2 + hl] as u16) << 8) | v[3 + hl] as u16;
    let at = 4 + hl;
    let pad = ((v[at] as usize) << 8) | v[at + 1] as usize;
    let first = v[at + 2 + pad..].to_vec();
    seen.lock().unwrap().push((format!("{host}:{port}"), first.clone()));
    let rsalt = [7u8; 16];
    let wkey = derive(&psk, &rsalt);
    let reply = [b"echo:".to_vec(), first].concat();
    let mut h = vec![1u8];
    h.extend_from_slice(&(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()).to_be_bytes());
    h.extend_from_slice(&salt);
    h.extend_from_slice(&(reply.len() as u16).to_be_bytes());
    let mut o = rsalt.to_vec();
    o.extend(seal(&wkey, 0, &h));
    o.extend(seal(&wkey, 1, &reply));
    s.write_all(&o).await.unwrap();
    let mut b = [0u8; 1024];
    while let Ok(n) = s.read(&mut b).await {
        if n == 0 {
            break;
        }
    }
}

#[tokio::test]
async fn shadowsocks_through_bridge() {
    let _g = setup().await;
    let _r = Restore(|| {
        prefs::set_proxy_on(false);
        net::settings_changed();
    });
    prefs::set_unblock_level(1);
    let psk: Vec<u8> = (0..16).map(|i| i * 7 + 1).collect();
    let (l, port) = listener().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    {
        let (psk, seen) = (psk.clone(), seen.clone());
        tokio::spawn(async move {
            while let Ok((s, _)) = l.accept().await {
                tokio::spawn(ss_conn(s, psk.clone(), seen.clone()));
            }
        });
    }
    prefs::set_proxy_kind("ss");
    prefs::set_ss_host("127.0.0.1");
    prefs::set_ss_port(port as i64);
    prefs::set_ss_method_id("2022-blake3-aes-128-gcm");
    prefs::set_ss_key(&base64::engine::general_purpose::STANDARD.encode(&psk));
    prefs::set_proxy_remote_dns(true);
    prefs::set_proxy_sites_only(false);
    prefs::set_proxy_on(true);
    net::settings_changed();
    assert!(net::ss().is_some());
    assert!(net::note().contains("Shadowsocks"));

    // Bên gọi nói trước: phần đầu đi cùng dữ liệu đầu
    let mut c = socks("phim.example", 443).await;
    c.write_all(b"ping").await.unwrap();
    let mut r = [0u8; 9];
    tokio::time::timeout(Duration::from_secs(5), c.read_exact(&mut r)).await.unwrap().unwrap();
    assert_eq!(&r, b"echo:ping");
    // Bên gọi im (máy chủ nói trước): sau 0,3 s cổng gửi phần đầu kèm đệm, máy chủ trả lời được
    let t0 = Instant::now();
    let mut c2 = socks("phim.example", 25).await;
    let mut r2 = [0u8; 5];
    tokio::time::timeout(Duration::from_secs(5), c2.read_exact(&mut r2)).await.unwrap().unwrap();
    assert_eq!(&r2, b"echo:");
    assert!(t0.elapsed() >= Duration::from_millis(250));
    {
        let s = seen.lock().unwrap();
        assert_eq!(s[0], ("phim.example:443".to_string(), b"ping".to_vec()));
        assert_eq!(s[1], ("phim.example:25".to_string(), Vec::new()));
    }
    drop((c, c2));
    prefs::set_proxy_on(false);
    net::settings_changed();
    assert!(net::ss().is_none());
}

/// Thử thật ra mạng (chạy tay: `cargo test -p conbang-core --test bridge -- --ignored --nocapture`): DNS mã hoá hỏi được
/// tên miền, tải HTTPS + HTTP qua cổng nội bộ ở mức vượt chặn 0 / 1 / 2.
#[tokio::test]
#[ignore]
async fn real_network_through_bridge() {
    let _g = setup().await;
    let _r = Restore(|| {
        prefs::set_secure_dns(false);
        prefs::set_unblock_level(1);
        net::settings_changed();
    });
    let ip = cbtv::doh::resolve("www.cloudflare.com").await.unwrap();
    println!("DNS mã hoá ({}): www.cloudflare.com → {ip:?}", cbtv::doh::providers());
    assert!(ip.is_some());
    prefs::set_secure_dns(true);
    for level in [0, 1, 2] {
        prefs::set_unblock_level(level);
        net::settings_changed();
        let before = unblock::disorder_count();
        for url in ["https://www.cloudflare.com/cdn-cgi/trace", "http://www.cloudflare.com/cdn-cgi/trace"] {
            let t0 = Instant::now();
            let r = cbtv::http::get(url, &[]).await.unwrap();
            let body = r.body();
            println!("mức {level} {url}: {} ms, {}", t0.elapsed().as_millis(), body.lines().find(|l| l.starts_with("ip=")).unwrap_or("?"));
            assert!(body.contains("ip="));
        }
        println!("mức {level}: số lần gửi đảo thứ tự +{}", unblock::disorder_count() - before);
    }
}

/// Thử thật trên card mạng (chạy tay, --ignored): gửi gói bắt tay TLS thật (rustls) thẳng tới www.cloudflare.com theo mức 1 /
/// 2, đọc TCP_INFO của chính ổ cắm (không cần quyền root): số gói có dữ liệu đã gửi (mỗi mẩu một gói — không bị Linux tự gộp)
/// và số lần gửi lại (mức 2: byte đầu TTL 1 mất ở chặng đầu → nhân gửi lại); trang vẫn trả ServerHello.
#[cfg(not(windows))] // đếm gói (TCP_INFO của Linux)
#[tokio::test]
#[ignore]
async fn real_nic_pieces_and_ttl1() {
    let _g = setup().await;
    let host = "www.cloudflare.com";
    let ip = cbtv::doh::resolve(host).await.unwrap().unwrap();
    for level in [1, 2] {
        let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        let mut cc = rustls::ClientConnection::new(Arc::new(cfg), host.try_into().unwrap()).unwrap();
        let mut hello = Vec::new();
        cc.write_tls(&mut hello).unwrap();
        let s = TcpStream::connect((ip, 443)).await.unwrap();
        s.set_nodelay(true).unwrap();
        let fd = s.as_raw_fd();
        let mut conn = net::Conn { stream: Box::pin(s), fd: Some(fd), via_proxy: false, ss: false };
        let segs0 = unblock::tcp_data_segs_out(fd).unwrap();
        let t0 = Instant::now();
        let applied = unblock::write_hello(&mut conn, &hello, host, level, false).await.unwrap();
        let mut b = [0u8; 5];
        tokio::time::timeout(Duration::from_secs(5), conn.stream.read_exact(&mut b)).await.unwrap().unwrap();
        assert_eq!(b[0], 0x16, "ServerHello");
        let sent = unblock::tcp_data_segs_out(fd).unwrap() - segs0;
        let retrans = unblock::tcp_total_retrans(fd).unwrap();
        let want = unblock::hello_chunks(&hello, host, level).len() as u32;
        println!(
            "mức {level} (đã gửi mức {applied}): gói bắt tay {} byte, {want} mẩu → {sent} gói có dữ liệu, gửi lại {retrans}, trả lời sau {} ms",
            hello.len(),
            t0.elapsed().as_millis()
        );
        assert_eq!(applied, level);
        assert_eq!(sent, want + retrans, "mỗi mẩu một gói");
        if level == 2 {
            assert!(retrans >= 1, "byte đầu TTL 1 phải được gửi lại");
        }
    }
}
