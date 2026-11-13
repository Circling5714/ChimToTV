//! «Vượt chặn nhà mạng» (bản TV: writeHello trong Socks5.kt + TcpTtl.kt; Windows: Unblock.cs; iPad: Socks5.swift). Nhà mạng
//! chặn trang phim theo tên trang trong gói bắt tay TLS (SNI); gửi riêng byte đầu tiên của gói bắt tay, rồi tên trang thành
//! từng mẩu 2 byte là qua được (mức 1). Mức 2 gửi byte đầu với TTL 1 (gói đó mất ở chặng đầu, máy gửi lại sau → máy chặn
//! thấy gói đảo thứ tự). Mọi kết nối của app (reqwest lẫn trình phát WebKitGTK) đi qua cổng SOCKS5 nội bộ (net.rs): cổng
//! gọi `bridge_first` để tách gói đầu, kết nối mở ở `net::connect`.
//!
//! Mức 2 trên Linux theo cách bản TV (Android cũng là nhân Linux), không theo cách riêng của Windows 1.13:
//! - Windows gửi lại MỌI byte chưa được xác nhận trong một gói, nên bản Windows phải gửi byte 0 (TTL 1), byte 1, 2, 3 từng gói
//!   rồi CHỜ máy chủ xác nhận (SIO_TCP_INFO) mới gửi tên trang. Nhân Linux chỉ gửi lại đúng đoạn bị mất: máy chủ nhận các
//!   đoạn sau (thiếu byte 0) thì báo SACK cho chúng, đoạn đã SACK không bao giờ bị gộp vào gói gửi lại (tcp_retrans_try_collapse
//!   bỏ qua đoạn đã SACK) → gói gửi lại chỉ có 1 byte. Đo trên TV box (tcpdump, README «Cách phát video»): byte đầu ttl 1 →
//!   phần sau ttl 64 → máy kia báo thiếu → byte đầu gửi lại ttl 64 sau ~11 ms. Nên ở đây: byte 0 với TTL 1, trả TTL về mặc
//!   định (-1), rồi gửi tiếp các mẩu như mức 1 ngay — không chờ xác nhận (chờ chỉ thêm 1–2 lượt đi về mỗi kết nối mà không lợi
//!   gì trên Linux).
//! - Riêng Linux phải lo «tự gộp gói» (tcp_autocorking, bật sẵn): đang có gói của kết nối nằm chờ ở card mạng thì lần ghi nhỏ
//!   sau được giữ lại và GỘP với lần ghi kế tiếp thành một gói — các mẩu của tên trang có thể dính lại. TCP_NODELAY không tắt
//!   được việc này. Nên sau mỗi mẩu: chờ tới khi mẩu đã rời hàng gửi của TCP (TCP_INFO `tcpi_notsent_bytes` = 0, nhân ≥ 4.6),
//!   mẩu kế mới thành gói riêng. Thường xong ngay (vài µs); khi cửa sổ gửi đầy (tên trang rất dài: hơn 10 gói đầu) thì chờ
//!   một lượt đi về. Tổng chờ tối đa SENT_WAIT_MS cho cả gói bắt tay; không hỏi được TCP_INFO thì gửi như bản TV.
//! - Byte TTL 1 cũng chờ rời hàng gửi rồi mới trả TTL (TTL đọc lúc gói thật sự được gửi đi).

use crate::net::{Conn, Duplex};
use std::collections::HashSet;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Mức đang bật (0 = tắt, 1 = tách gói bắt tay, 2 = tách + gửi đảo thứ tự).
pub fn level() -> i64 {
    crate::prefs::unblock_level()
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Gói đầu của bên gọi là gói bắt tay TLS (ClientHello).
pub fn is_tls_hello(b: &[u8]) -> bool {
    b.len() > 5 && b[0] == 0x16 && b[1] == 0x03
}

const HTTP_METHODS: [&[u8]; 7] = [b"GET ", b"POST ", b"HEAD ", b"PUT ", b"DELETE ", b"OPTIONS ", b"PATCH "];

/// Gói đầu là một yêu cầu HTTP thường (không mã hoá).
pub fn is_http_request(b: &[u8]) -> bool {
    HTTP_METHODS.iter().any(|m| b.len() > m.len() && b.starts_with(m))
}

/// Một mẩu của gói đầu: [start, start + len), gửi thành một gói TCP riêng. `ttl1` = gửi với TTL 1 (mức 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    pub start: usize,
    pub len: usize,
    pub ttl1: bool,
}

struct Plan {
    chunks: Vec<Chunk>,
    i: usize,
}

impl Plan {
    fn add(&mut self, start: usize, end: usize) {
        let s = start.max(self.i);
        if end > self.i && end > start {
            self.chunks.push(Chunk { start: s, len: end - s, ttl1: false });
        }
        self.i = self.i.max(end);
    }

    fn pieces(&mut self, start: usize, end: usize) {
        let mut k = start;
        while k < end {
            self.add(k, (k + 2).min(end));
            k += 2;
        }
    }
}

/// Cách tách gói đầu. Gói bắt tay TLS — byte đầu riêng, phần trước tên trang, tên trang từng mẩu 2 byte (tìm không phân biệt
/// hoa / thường), phần còn lại. Yêu cầu HTTP thường (từ 11/2026 — trước chỉ tách TLS): byte đầu riêng, tới giữa chữ «Host»,
/// phần còn lại của «Host: », giá trị Host từng mẩu 2 byte, phần còn lại. Mức 2: như mức 1, byte đầu gửi với TTL 1 (xem đầu
/// tệp: Linux không cần điểm chờ như Windows). Gói khác / tắt → cả gói một lần.
pub fn hello_chunks(buf: &[u8], host: &str, level: i64) -> Vec<Chunk> {
    let tls = is_tls_hello(buf);
    if (!tls && !is_http_request(buf)) || level <= 0 {
        return vec![Chunk { start: 0, len: buf.len(), ttl1: false }];
    }
    let mut p = Plan { chunks: vec![Chunk { start: 0, len: 1, ttl1: level >= 2 }], i: 1 };
    if tls {
        let name = host.to_lowercase();
        let name = name.as_bytes();
        let at = if name.len() >= 2 { index_of_ignore_case(&buf[p.i..], name) } else { None };
        if let Some(at) = at {
            let start = at + p.i;
            p.add(p.i, start);
            p.pieces(start, start + name.len());
        }
    } else if let Some(at) = index_of_ignore_case(buf, b"\r\nhost:") {
        let k = at + 2; // chữ «Host»
        let mut v = k + 5;
        while v < buf.len() && (buf[v] == b' ' || buf[v] == b'\t') {
            v += 1;
        }
        let mut e = v;
        while e < buf.len() && buf[e] != b'\r' && buf[e] != b'\n' {
            e += 1;
        }
        p.add(p.i, k + 2);
        p.add(k + 2, v);
        p.pieces(v, e);
    }
    p.add(p.i, buf.len());
    p.chunks
}

/// Vị trí của name (chữ thường) trong b, không phân biệt hoa / thường; None = không có.
fn index_of_ignore_case(b: &[u8], name: &[u8]) -> Option<usize> {
    if name.len() > b.len() {
        return None;
    }
    (0..=b.len() - name.len()).find(|&s| b[s..s + name.len()].iter().zip(name).all(|(&x, &y)| x.to_ascii_lowercase() == y))
}

// ---- Ổ cắm: TTL, trạng thái TCP ----

/// Đặt TTL (IPv4) / hop limit (IPv6) của ổ cắm (-1 = trả về mặc định của hệ thống). Đặt cả hai tầng (ổ IPv6 nối tới địa chỉ
/// IPv4 dùng IP_TTL), như TcpTtl.kt; true nếu ít nhất một tầng đặt được. Không cần quyền root.
pub fn set_ttl(fd: i32, ttl: i32) -> bool {
    let v: libc::c_int = ttl;
    let p = &v as *const libc::c_int as *const libc::c_void;
    let l = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: setsockopt chỉ đọc l byte từ p (một c_int còn sống); fd sai thì hàm trả lỗi, không hỏng bộ nhớ.
    let a = unsafe { libc::setsockopt(fd, libc::IPPROTO_IP, libc::IP_TTL, p, l) } == 0;
    // SAFETY: như trên.
    let b = unsafe { libc::setsockopt(fd, libc::IPPROTO_IPV6, libc::IPV6_UNICAST_HOPS, p, l) } == 0;
    a || b
}

/// TTL đang đặt (IPv4) — gỡ lỗi / test.
pub fn ttl_of(fd: i32) -> Option<i32> {
    let mut v: libc::c_int = 0;
    let mut l = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: getsockopt ghi tối đa l byte vào v (một c_int), cập nhật l.
    let r = unsafe { libc::getsockopt(fd, libc::IPPROTO_IP, libc::IP_TTL, &mut v as *mut libc::c_int as *mut libc::c_void, &mut l) };
    (r == 0).then_some(v)
}

/// struct tcp_info của nhân Linux (đọc thô): (byte, số byte nhân trả). Nhân cũ trả ít trường hơn.
fn tcp_info(fd: i32) -> Option<([u8; 232], usize)> {
    let mut buf = [0u8; 232];
    let mut len = buf.len() as libc::socklen_t;
    // SAFETY: getsockopt ghi tối đa len byte vào buf và cập nhật len.
    let r = unsafe { libc::getsockopt(fd, libc::IPPROTO_TCP, libc::TCP_INFO, buf.as_mut_ptr() as *mut libc::c_void, &mut len) };
    (r == 0).then_some((buf, len as usize))
}

fn tcp_info_u32(fd: i32, offset: usize) -> Option<u32> {
    let (b, n) = tcp_info(fd)?;
    if n < offset + 4 {
        return None;
    }
    Some(u32::from_ne_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]))
}

/// Số gói đã gửi mà máy kia chưa xác nhận (tcpi_unacked); None = không hỏi được.
pub fn tcp_unacked(fd: i32) -> Option<u32> {
    tcp_info_u32(fd, 24)
}

/// Số byte đã ghi mà TCP chưa gửi đi (tcpi_notsent_bytes, nhân ≥ 4.6); None = không hỏi được.
pub fn tcp_notsent(fd: i32) -> Option<u32> {
    tcp_info_u32(fd, 144)
}

/// Số gói có dữ liệu đã nhận (tcpi_data_segs_in, nhân ≥ 4.6) — test đếm số mẩu.
pub fn tcp_data_segs_in(fd: i32) -> Option<u32> {
    tcp_info_u32(fd, 152)
}

/// Số gói có dữ liệu đã gửi, kể cả gửi lại (tcpi_data_segs_out, nhân ≥ 4.6) — gỡ lỗi / test.
pub fn tcp_data_segs_out(fd: i32) -> Option<u32> {
    tcp_info_u32(fd, 156)
}

/// Tổng số gói đã gửi lại (tcpi_total_retrans) — gỡ lỗi / test (mức 2: byte đầu TTL 1 mất → gửi lại).
pub fn tcp_total_retrans(fd: i32) -> Option<u32> {
    tcp_info_u32(fd, 100)
}

/// Thời gian chờ tối đa (cả gói bắt tay) để từng mẩu rời hàng gửi của TCP thành gói riêng (xem đầu tệp).
const SENT_WAIT_MS: u64 = 1_000;

/// Chờ mẩu vừa ghi rời hàng gửi của TCP (đã thành một gói riêng). Không hỏi được / quá hạn thì thôi.
async fn wait_sent(fd: i32, deadline: Instant) {
    let mut spins = 0;
    while matches!(tcp_notsent(fd), Some(n) if n > 0) && Instant::now() < deadline {
        if spins < 50 {
            spins += 1;
            tokio::task::yield_now().await;
        } else {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}

async fn write_piece(s: &mut Pin<Box<dyn Duplex>>, b: &[u8]) -> io::Result<()> {
    s.write_all(b).await?;
    s.flush().await
}

static DISORDER_COUNT: AtomicU64 = AtomicU64::new(0);

/// Số gói bắt tay đã gửi đảo thứ tự (mức 2) — gỡ lỗi / test.
pub fn disorder_count() -> u64 {
    DISORDER_COUNT.load(Ordering::Relaxed)
}

/// Gửi gói đầu đã tách: mỗi mẩu một lần ghi (TCP_NODELAY) và chờ mẩu rời hàng gửi (chống tự gộp gói của Linux). Qua proxy:
/// không đảo thứ tự (proxy gửi lại theo thứ tự), chờ 40 ms sau byte đầu để proxy kịp gửi riêng (như bản TV). Mức 2 cần ổ
/// cắm thật (đi thẳng) và đặt được TTL — không được thì gửi như mức 1. Trả về mức đã gửi thật.
pub async fn write_hello(conn: &mut Conn, buf: &[u8], host: &str, level: i64, via_proxy: bool) -> io::Result<i64> {
    let fd = conn.fd;
    let mut level = level;
    if level >= 2 && (via_proxy || fd.is_none()) {
        level = 1;
    }
    let chunks = hello_chunks(buf, host, level);
    let deadline = Instant::now() + Duration::from_millis(SENT_WAIT_MS);
    let last = chunks.len().saturating_sub(1);
    for (idx, c) in chunks.iter().enumerate() {
        let piece = &buf[c.start..c.start + c.len];
        match fd {
            Some(f) if c.ttl1 && set_ttl(f, 1) => {
                // Byte đầu với TTL 1: chết ở router đầu tiên; hệ điều hành gửi lại sau với TTL mặc định
                let r = write_piece(&mut conn.stream, piece).await;
                wait_sent(f, deadline).await;
                set_ttl(f, -1); // gói gửi lại (và mọi gói sau) đi với TTL mặc định
                r?;
                DISORDER_COUNT.fetch_add(1, Ordering::Relaxed);
            }
            _ => {
                if c.ttl1 {
                    level = 1; // không đặt được TTL: gửi như mức 1
                }
                write_piece(&mut conn.stream, piece).await?;
            }
        }
        if idx < last {
            if let Some(f) = fd {
                wait_sent(f, deadline).await;
            }
            if idx == 0 && via_proxy {
                tokio::time::sleep(Duration::from_millis(40)).await;
            }
        }
    }
    Ok(level)
}

// ---- Tự lên Mức 2 ----

static FIRST_REPLY_MS: AtomicU64 = AtomicU64::new(4_000);

/// Ở Mức 1, chờ trang trả lời gói bắt tay bao lâu thì coi là bị chặn và thử Mức 2.
pub fn first_reply_ms() -> u64 {
    FIRST_REPLY_MS.load(Ordering::Relaxed)
}

/// Đặt thời gian chờ ở Mức 1 (test đặt ngắn).
pub fn set_first_reply_ms(ms: u64) {
    FIRST_REPLY_MS.store(ms, Ordering::Relaxed);
}

/// Ở Mức 2 (cách cuối cùng): chờ lâu hơn lúc nhà mạng giữ kết nối treo (đo: 15 giây).
pub const LEVEL2_REPLY_MS: u64 = 20_000;

/// Đọc gói đầu của bên gọi; gói bắt tay TLS thì đọc trọn bản ghi đầu (có thể dài hơn một lần đọc, chờ thêm tối đa 2 giây).
/// 0 = bên gọi đã đóng.
pub async fn read_first<R: AsyncRead + Unpin + ?Sized>(s: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let n = s.read(buf).await?;
    finish_record(s, buf, n).await
}

/// Đã đọc n byte đầu: gói bắt tay TLS thì đọc nốt bản ghi đầu (chờ thêm tối đa 2 giây).
async fn finish_record<R: AsyncRead + Unpin + ?Sized>(s: &mut R, buf: &mut [u8], n: usize) -> io::Result<usize> {
    let mut n = n;
    if n >= 5 && buf[0] == 0x16 && buf[1] == 0x03 {
        let need = buf.len().min(5 + (((buf[3] as usize) << 8) | buf[4] as usize));
        let until = tokio::time::Instant::now() + Duration::from_millis(2000);
        while n < need {
            match tokio::time::timeout_at(until, s.read(&mut buf[n..need])).await {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(m)) => n += m,
                Ok(Err(e)) => return Err(e),
            }
        }
    }
    Ok(n)
}

/// Chờ trang trả lời (tối đa ms). 0 = trang im / cắt kết nối / lỗi — kiểu nhà mạng chặn.
pub async fn first_reply<R: AsyncRead + Unpin + ?Sized>(s: &mut R, buf: &mut [u8], ms: u64) -> usize {
    match tokio::time::timeout(Duration::from_millis(ms), s.read(buf)).await {
        Ok(Ok(n)) => n,
        _ => 0,
    }
}

/// Tên miền gốc, để nhớ theo trang chứ không theo từng tên con: img.phimmoi.net → phimmoi.net, www.abc.com.vn → abc.com.vn.
pub fn base_domain(host: &str) -> String {
    let h = host.trim_matches(|c| c == '[' || c == ']').trim_end_matches('.').to_lowercase();
    if h.parse::<std::net::IpAddr>().is_ok() {
        return h;
    }
    let labels: Vec<&str> = h.split('.').collect();
    if labels.len() <= 2 {
        return h;
    }
    let n = labels.len();
    let three = labels[n - 1].len() == 2 && SECOND_LEVEL.contains(&labels[n - 2]);
    labels[n - if three { 3 } else { 2 }..].join(".")
}

const SECOND_LEVEL: [&str; 13] = ["com", "net", "org", "edu", "gov", "co", "ac", "info", "biz", "name", "pro", "int", "health"];

/// Vượt chặn tự lên Mức 2 (11/2026, như bản TV): đang Mức 1 mà trang không trả lời gói bắt tay (nhà mạng đã ghép được các
/// mẩu và chặn) thì mở kết nối mới, gửi lại theo Mức 2; lọt thì nhớ trang (tên miền gốc) dùng Mức 2 trong 7 ngày — các kết
/// nối sau đi Mức 2 ngay, khỏi chờ. Nhà mạng thôi chặn thì sau 7 ngày tự về Mức 1. Lưu ở khoá `unblock_l2` (dùng chung cả nhà).
pub mod level2_memo {
    use super::{base_domain, lock};
    use crate::util::now_ms;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;

    pub const KEEP_MS: i64 = 7 * 24 * 3600 * 1000;

    static HOSTS: Mutex<Option<HashMap<String, i64>>> = Mutex::new(None);
    static PERSIST: AtomicBool = AtomicBool::new(true);
    static AUTO_COUNT: AtomicU64 = AtomicU64::new(0);

    /// Lưu xuống máy (mặc định có; test tắt để chỉ giữ trong bộ nhớ).
    pub fn set_persist(on: bool) {
        PERSIST.store(on, Ordering::Relaxed);
    }

    fn persist() -> bool {
        PERSIST.load(Ordering::Relaxed)
    }

    fn with<T>(f: impl FnOnce(&mut HashMap<String, i64>) -> T) -> T {
        let mut g = lock(&HOSTS);
        let hosts = g.get_or_insert_with(|| if persist() { crate::prefs::level2_hosts() } else { HashMap::new() });
        f(hosts)
    }

    /// Nạp danh sách đã lưu, bỏ mục quá 7 ngày.
    pub fn load(saved: HashMap<String, i64>, now: i64) {
        *lock(&HOSTS) = Some(saved.into_iter().filter(|(_, t)| now - t >= 0 && now - t < KEEP_MS).collect());
    }

    pub fn needs(host: &str) -> bool {
        needs_at(host, now_ms())
    }

    pub fn needs_at(host: &str, now: i64) -> bool {
        let key = base_domain(host);
        with(|h| h.get(&key).map(|t| now - t >= 0 && now - t < KEEP_MS).unwrap_or(false))
    }

    pub fn remember(host: &str) {
        remember_at(host, now_ms());
    }

    pub fn remember_at(host: &str, now: i64) {
        let key = base_domain(host);
        let copy = with(|h| {
            h.insert(key, now);
            h.clone()
        });
        if persist() {
            crate::prefs::set_level2_hosts(&copy);
        }
    }

    pub fn all() -> HashMap<String, i64> {
        with(|h| h.clone())
    }

    pub fn clear() {
        *lock(&HOSTS) = Some(HashMap::new());
        if persist() {
            crate::prefs::set_level2_hosts(&HashMap::new());
        }
    }

    /// Số lần tự lên Mức 2 từ lúc mở app — gỡ lỗi.
    pub fn auto_count() -> u64 {
        AUTO_COUNT.load(Ordering::Relaxed)
    }

    pub(super) fn count_auto() {
        AUTO_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

/// Chỉ cổng gỡ lỗi (/unblockhang): ở Mức 1, gửi nguyên gói bắt tay tới các trang này — như nhà mạng ghép được các mẩu — để
/// thử tự lên Mức 2 với chính máy soi của nhà mạng.
static DEBUG_WHOLE_HELLO: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

pub fn set_debug_whole_hello(hosts: HashSet<String>) {
    *lock(&DEBUG_WHOLE_HELLO) = hosts;
}

pub fn debug_whole_hello() -> HashSet<String> {
    lock(&DEBUG_WHOLE_HELLO).clone()
}

/// Gửi gói bắt tay theo mức (Mức 1 + trang đã nhớ → Mức 2). Trả về mức đã gửi thật.
pub async fn send_hello(conn: &mut Conn, hello: &[u8], host: &str, level: i64) -> io::Result<i64> {
    let mut level = level;
    if level == 1 && level2_memo::needs(host) {
        level = 2;
    }
    if level == 1 && lock(&DEBUG_WHOLE_HELLO).contains(&host.trim_end_matches('.').to_lowercase()) {
        write_piece(&mut conn.stream, hello).await?;
        return Ok(1);
    }
    let via = conn.via_proxy;
    write_hello(conn, hello, host, level, via).await
}

/// Cổng nội bộ (net::serve), sau khi đã báo bên gọi «kết nối được»: vượt chặn (qua Shadowsocks thì khỏi: nhà mạng chỉ thấy
/// dữ liệu đã mã hoá, không đọc được tên trang) — gói đầu của bên gọi (gói bắt tay TLS / yêu cầu HTTP thường) được tách. Bên
/// gọi im quá 0,3 s (giao thức máy chủ nói trước) thì thôi. Đi thẳng + gói bắt tay TLS ở Mức 1: trang im / cắt kết nối →
/// kết nối mới, gửi lại theo Mức 2 (như bản TV); lọt thì nhớ trang. Bên gọi chưa nhận byte nào của trang nên gửi lại không
/// sai. Trả kết nối để chuyển tiếp tiếp (có thể là kết nối mới); None = bỏ kết nối.
pub async fn bridge_first(client: &mut TcpStream, conn: Conn, host: &str, port: u16) -> Option<Conn> {
    let level = level();
    let mut conn = conn;
    if conn.ss || level <= 0 {
        return Some(conn);
    }
    // Chờ gói đầu bằng chính lần đọc (đọc của TcpStream huỷ giữa chừng không mất byte nào). Không dùng readable(): sau bắt
    // tay SOCKS, tokio còn nhớ trạng thái «đọc được» cũ (lần đọc cuối chưa gặp WouldBlock) nên readable() trả ngay dù bên gọi
    // chưa gửi gì
    let mut first = vec![0u8; 17 * 1024];
    let n = match tokio::time::timeout(Duration::from_millis(300), client.read(&mut first)).await {
        Err(_) => return Some(conn),
        Ok(Ok(n)) if n > 0 => n,
        Ok(_) => return None,
    };
    let n0 = match finish_record(client, &mut first, n).await {
        Ok(n) if n > 0 => n,
        _ => return None,
    };
    let first = &first[..n0];
    if conn.via_proxy || !is_tls_hello(first) {
        let via = conn.via_proxy;
        write_hello(&mut conn, first, host, level, via).await.ok()?;
        return Some(conn);
    }
    let sent = send_hello(&mut conn, first, host, level).await.ok()?;
    let mut reply = vec![0u8; 16 * 1024];
    let wait = if sent >= 2 { LEVEL2_REPLY_MS } else { first_reply_ms() };
    let mut m = first_reply(&mut conn.stream, &mut reply, wait).await;
    if m == 0 && sent < 2 {
        // Mức 1 không lọt: kết nối mới, gửi lại theo Mức 2
        let c2 = match tokio::time::timeout(Duration::from_secs(15), crate::net::connect(host, port)).await {
            Ok(Ok(c)) => c,
            _ => return None,
        };
        drop(conn);
        conn = c2;
        let via = conn.via_proxy;
        let applied = write_hello(&mut conn, first, host, 2, via).await.ok()?;
        m = first_reply(&mut conn.stream, &mut reply, LEVEL2_REPLY_MS).await;
        crate::log::d(
            "net",
            &format!(
                "vượt chặn: {host} không trả lời ở mức 1 → mức 2: {}{}",
                if m > 0 { "lọt" } else { "vẫn không được" },
                if applied >= 2 { "" } else { " (không đặt được TTL)" }
            ),
        );
        if m > 0 && applied >= 2 {
            level2_memo::remember(host);
            level2_memo::count_auto();
        }
    }
    if m == 0 {
        return None;
    }
    client.write_all(&reply[..m]).await.ok()?;
    Some(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn hello(name: &str) -> Vec<u8> {
        let mut h = vec![0x16, 0x03, 0x01, 0x00, 0x20, 1, 2, 3];
        h.extend_from_slice(name.as_bytes());
        h.extend_from_slice(&[9, 9]);
        h
    }

    fn c(start: usize, len: usize, ttl1: bool) -> Chunk {
        Chunk { start, len, ttl1 }
    }

    #[test]
    fn splits_client_hello() {
        // Gói bắt tay giả: 16 03 01 … rồi tên trang ở giữa
        let h = hello("phimmois.fun");
        let ch = hello_chunks(&h, "PhimMois.fun", 1);
        assert_eq!(ch[0], c(0, 1, false));
        assert_eq!(ch[1], c(1, 7, false));
        assert_eq!(ch[2..8].iter().map(|x| x.len).collect::<Vec<_>>(), vec![2; 6]);
        assert_eq!(*ch.last().unwrap(), c(20, 2, false));
        assert_eq!(ch.iter().map(|x| x.len).sum::<usize>(), h.len());
        assert!(!ch.iter().any(|x| x.ttl1));
        assert_eq!(hello_chunks(&h, "phimmois.fun", 0).len(), 1);
        assert_eq!(hello_chunks(&[0x17, 3, 3, 0, 1, 5, 6], "x.y", 1).len(), 1);
        // Tên trang viết hoa trong gói vẫn tìm được
        let upper = hello("PHIMMOIS.FUN");
        assert_eq!(hello_chunks(&upper, "phimmois.fun", 1).len(), 9);
    }

    /// Mức 2 trên Linux: như mức 1, chỉ byte đầu gửi với TTL 1 (không điểm chờ như Windows — xem đầu tệp).
    #[test]
    fn level2_is_level1_with_ttl1_first_byte() {
        let h = hello("phimmois.fun");
        let l1 = hello_chunks(&h, "phimmois.fun", 1);
        let l2 = hello_chunks(&h, "phimmois.fun", 2);
        assert_eq!(l2[0], c(0, 1, true));
        assert_eq!(l1.len(), l2.len());
        assert!(l1.iter().zip(&l2).skip(1).all(|(a, b)| a == b));
        // Các mẩu nối tiếp nhau và phủ hết gói; chỉ một mẩu TTL 1, không chứa tên trang
        let mut at = 0;
        for x in &l2 {
            assert_eq!(at, x.start);
            at += x.len;
        }
        assert_eq!(at, h.len());
        assert_eq!(l2.iter().filter(|x| x.ttl1).count(), 1);
        // Gói bắt tay không có tên trang (vd bên gọi gửi IP): byte đầu rồi phần còn lại
        let other = hello_chunks(&h, "khac.vn", 2);
        assert_eq!(other, vec![c(0, 1, true), c(1, h.len() - 1, false)]);
    }

    /// Yêu cầu HTTP thường: tách quanh tên trang trong dòng Host (như bản TV); mức 2 như mức 1 + TTL 1 ở byte đầu.
    #[test]
    fn splits_http_request_around_host() {
        let req = b"GET /phim HTTP/1.1\r\nHost: phim.test\r\nAccept: */*\r\n\r\n";
        let parts = |level| {
            hello_chunks(req, "phim.test", level)
                .iter()
                .map(|x| String::from_utf8_lossy(&req[x.start..x.start + x.len]).into_owned())
                .collect::<Vec<_>>()
                .join("|")
        };
        assert_eq!(parts(1), "G|ET /phim HTTP/1.1\r\nHo|st: |ph|im|.t|es|t|\r\nAccept: */*\r\n\r\n");
        assert_eq!(parts(2), parts(1));
        assert!(hello_chunks(req, "phim.test", 2)[0].ttl1);
        assert_eq!(hello_chunks(b"SSH-2.0-x\r\n", "phim.test", 1).len(), 1);
        // «host:» viết hoa / thường, không có dấu cách
        let req2 = b"POST / HTTP/1.1\r\nHOST:a.b\r\n\r\n";
        let p2: Vec<String> = hello_chunks(req2, "a.b", 1)
            .iter()
            .map(|x| String::from_utf8_lossy(&req2[x.start..x.start + x.len]).into_owned())
            .collect();
        assert_eq!(p2.join("|"), "P|OST / HTTP/1.1\r\nHO|ST:|a.|b|\r\n\r\n");
    }

    #[test]
    fn short_or_odd_packets_do_not_panic() {
        for b in [&b""[..], b"G", b"GET ", b"\x16\x03", b"\x16\x03\x01\x00\x00\x00", b"GET /\r\nhost:"] {
            let ch = hello_chunks(b, "a.b", 2);
            assert_eq!(ch.iter().map(|x| x.len).sum::<usize>(), b.len());
        }
    }

    #[test]
    fn base_domains_and_level2_memo() {
        assert_eq!(base_domain("img.phimmoi.net"), "phimmoi.net");
        assert_eq!(base_domain("www.abc.com.vn"), "abc.com.vn");
        assert_eq!(base_domain("1.2.3.4"), "1.2.3.4");
        assert_eq!(base_domain("[::1]"), "::1");
        assert_eq!(base_domain("Phim.TEST."), "phim.test");
        let _g = crate::net::test_lock();
        level2_memo::set_persist(false);
        const NOW: i64 = 1_000_000_000_000;
        let mut saved = HashMap::new();
        saved.insert("phimmoi.test".to_string(), NOW - level2_memo::KEEP_MS - 1);
        saved.insert("kkphim.test".to_string(), NOW - 1000);
        level2_memo::load(saved, NOW);
        assert!(!level2_memo::needs_at("phimmoi.test", NOW)); // quá 7 ngày: về Mức 1
        assert!(level2_memo::needs_at("www.kkphim.test", NOW)); // theo tên miền gốc
        level2_memo::remember_at("a.b.phim.test", NOW);
        assert!(level2_memo::needs_at("phim.test", NOW + 1));
        level2_memo::clear();
        assert!(level2_memo::all().is_empty());
    }

    async fn pair() -> (Conn, TcpStream) {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(l.local_addr().unwrap()).await.unwrap();
        client.set_nodelay(true).unwrap();
        let (server, _) = l.accept().await.unwrap();
        use std::os::fd::AsRawFd;
        let fd = client.as_raw_fd();
        (Conn { stream: Box::pin(client), fd: Some(fd), via_proxy: false, ss: false }, server)
    }

    /// Gửi thật qua một kết nối trong máy: mọi mức đều phải tới đủ, đúng thứ tự, mỗi mẩu một gói, không treo.
    #[tokio::test]
    async fn write_hello_delivers_every_byte_in_order() {
        use std::os::fd::AsRawFd;
        let h = hello("phimmois.fun");
        for (level, via) in [(1, false), (2, false), (2, true), (0, false)] {
            let (mut conn, mut server) = pair().await;
            assert_eq!(tcp_notsent(conn.fd.unwrap()), Some(0)); // hỏi được trạng thái TCP (nhân ≥ 4.6)
            let before = disorder_count();
            let t0 = Instant::now();
            let applied = write_hello(&mut conn, &h, "phimmois.fun", level, via).await.unwrap();
            assert!(t0.elapsed() < Duration::from_millis(1000), "mức {level} mất {:?}", t0.elapsed());
            assert_eq!(applied, if via { 1 } else { level });
            if level == 2 && !via {
                assert!(disorder_count() > before);
                // TTL đã trả về mặc định
                assert_ne!(ttl_of(conn.fd.unwrap()), Some(1));
            }
            conn.stream.shutdown().await.unwrap();
            let mut got = Vec::new();
            server.read_to_end(&mut got).await.unwrap();
            assert_eq!(got, h);
            // Trên máy (lo) không gộp gói: số gói có dữ liệu = số mẩu
            let want = hello_chunks(&h, "phimmois.fun", if via { 1 } else { level }).len() as u32;
            assert_eq!(tcp_data_segs_in(server.as_raw_fd()), Some(want), "mức {level}, qua proxy {via}");
        }
    }

    #[tokio::test]
    async fn ttl_can_be_set_and_restored() {
        let (conn, _server) = pair().await;
        let fd = conn.fd.unwrap();
        let def = ttl_of(fd).unwrap();
        assert!(set_ttl(fd, 1));
        assert_eq!(ttl_of(fd), Some(1));
        assert!(set_ttl(fd, -1));
        assert_eq!(ttl_of(fd), Some(def));
        assert_eq!(tcp_unacked(fd), Some(0));
    }

    #[tokio::test]
    async fn read_first_reads_whole_tls_record() {
        let (mut conn, mut server) = pair().await;
        let mut rec = vec![0x16, 0x03, 0x01, 0x01, 0x00];
        rec.extend((0..256).map(|i| i as u8));
        let rec2 = rec.clone();
        let w = tokio::spawn(async move {
            conn.stream.write_all(&rec2[..100]).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            conn.stream.write_all(&rec2[100..]).await.unwrap();
            conn
        });
        let mut buf = vec![0u8; 17 * 1024];
        let n = read_first(&mut server, &mut buf).await.unwrap();
        assert_eq!(&buf[..n], &rec[..]);
        drop(w.await.unwrap());
        // trang im → 0 sau đúng thời gian chờ
        let (_c, mut s2) = pair().await;
        let t0 = Instant::now();
        assert_eq!(first_reply(&mut s2, &mut buf, 150).await, 0);
        assert!(t0.elapsed() >= Duration::from_millis(140));
    }
}
