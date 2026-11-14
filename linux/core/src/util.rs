//! Tiện ích nhỏ dùng chung: lỗi có câu báo tiếng Việt, regex, chuỗi, thời gian, chạy có giới hạn thời gian
//! (bản Windows: Util.cs, iPad: Util.swift).

use regex::Regex;
use std::fmt;
use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

/// Lỗi của app — mỗi loại có câu báo dễ hiểu cho người xem (`friendly`).
#[derive(Debug, Clone)]
pub enum Error {
    /// Lỗi có sẵn câu báo tiếng Việt (AppError)
    App(String),
    /// Trang trả lời bằng mã lỗi 4xx / 5xx (thử lại cũng vậy)
    Http(u16),
    /// Lỗi phía proxy (mã SOCKS5 + câu báo)
    Socks(u8, String),
    /// Hết thời gian chờ
    Timeout,
    /// Lỗi mạng khác (câu báo đã dịch)
    Net(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&friendly(self))
    }
}

impl std::error::Error for Error {}

impl Error {
    pub fn app(msg: impl Into<String>) -> Error {
        Error::App(msg.into())
    }

    pub fn http_code(&self) -> Option<u16> {
        match self {
            Error::Http(c) => Some(*c),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            return Error::Timeout;
        }
        if let Some(s) = e.status() {
            return Error::Http(s.as_u16());
        }
        // Tìm lỗi gốc (ổ cắm / TLS / proxy nội bộ) để báo đúng
        let mut text = e.to_string();
        let mut src: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(&e);
        while let Some(s) = src {
            text.push_str(" | ");
            text.push_str(&s.to_string());
            src = s.source();
        }
        Error::Net(net_message(&text))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::TimedOut {
            return Error::Timeout;
        }
        Error::Net(net_message(&e.to_string()))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::App(format!("Dữ liệu trang không đúng định dạng ({e})"))
    }
}

/// Dịch lỗi mạng (chuỗi lỗi của reqwest / hyper / rustls / ổ cắm) ra câu dễ hiểu.
fn net_message(text: &str) -> String {
    let t = text.to_lowercase();
    // Cổng SOCKS nội bộ trả mã lỗi (bên trong là lỗi của trang / proxy thật)
    if t.contains("host unreachable") || t.contains("failed to lookup") || t.contains("dns error") || t.contains("name or service not known") {
        return "Không tìm thấy tên miền (trang đổi tên miền hoặc bị chặn?).".into();
    }
    if t.contains("connection refused") {
        return "Không kết nối được tới trang.".into();
    }
    if t.contains("connection reset") || t.contains("broken pipe") || t.contains("unexpected eof") || t.contains("connection closed") {
        return format!("Lỗi kết nối bảo mật / mất kết nối.{}", unblock_hint());
    }
    if t.contains("timed out") || t.contains("deadline") {
        return format!("Trang không trả lời (quá thời gian chờ).{}", unblock_hint());
    }
    if t.contains("network is unreachable") || t.contains("network unreachable") {
        return "Không có mạng.".into();
    }
    if t.contains("certificate") || t.contains("tls") || t.contains("handshake") {
        return format!("Lỗi kết nối bảo mật / mất kết nối.{}", unblock_hint());
    }
    if t.contains("socks") || t.contains("proxy") {
        return format!("Lỗi proxy: {}.", crate::net::proxy_note_short());
    }
    format!("Lỗi mạng: {}{}", text.split(" | ").next().unwrap_or(text), crate::net::note())
}

/// Bắt tay HTTPS hỏng / treo: hay là nhà mạng chặn theo tên trang → gợi ý bật Vượt chặn nhà mạng.
fn unblock_hint() -> &'static str {
    if crate::prefs::unblock_level() == 0 {
        "\nNếu nhà mạng chặn trang này: bật Cài đặt → Vượt chặn nhà mạng."
    } else {
        ""
    }
}

/// Câu báo lỗi dễ hiểu cho người xem.
pub fn friendly(e: &Error) -> String {
    match e {
        Error::App(m) => m.clone(),
        Error::Http(c) => format!("Trang trả về lỗi HTTP {c}"),
        Error::Socks(_, m) => format!("Lỗi proxy: {m}."),
        Error::Timeout => format!("Trang không trả lời (quá thời gian chờ).{}", unblock_hint()),
        Error::Net(m) => m.clone(),
    }
}

// ---- Regex viết sẵn trong code ----

/// Regex dịch một lần, dùng lại (như `Rx` của bản Windows). Khai báo: `static RE: Rx = Rx::new(r"...");`
pub struct Rx {
    pattern: &'static str,
    re: OnceLock<Regex>,
}

impl Rx {
    pub const fn new(pattern: &'static str) -> Rx {
        Rx { pattern, re: OnceLock::new() }
    }

    pub fn re(&self) -> &Regex {
        self.re.get_or_init(|| Regex::new(self.pattern).unwrap_or_else(|e| panic!("regex sai {}: {e}", self.pattern)))
    }

    /// Nhóm i của lần khớp đầu (0 = cả chuỗi khớp); không khớp → None; nhóm không khớp → "".
    pub fn group(&self, i: usize, s: &str) -> Option<String> {
        self.re().captures(s).map(|c| c.get(i).map(|m| m.as_str().to_string()).unwrap_or_default())
    }

    /// Mọi nhóm của lần khớp đầu.
    pub fn groups(&self, s: &str) -> Option<Vec<String>> {
        self.re()
            .captures(s)
            .map(|c| (0..c.len()).map(|i| c.get(i).map(|m| m.as_str().to_string()).unwrap_or_default()).collect())
    }

    pub fn matches(&self, s: &str) -> bool {
        self.re().is_match(s)
    }

    /// Mọi lần khớp (mỗi lần: danh sách nhóm).
    pub fn all(&self, s: &str) -> Vec<Vec<String>> {
        self.re()
            .captures_iter(s)
            .map(|c| (0..c.len()).map(|i| c.get(i).map(|m| m.as_str().to_string()).unwrap_or_default()).collect())
            .collect()
    }

    pub fn replace(&self, s: &str, rep: &str) -> String {
        self.re().replace_all(s, rep).into_owned()
    }
}

// ---- Chuỗi ----

/// None khi rỗng (sau khi bỏ khoảng trắng).
pub fn non_empty(s: Option<&str>) -> Option<String> {
    let t = s?.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Phần sau lần xuất hiện cuối của ký tự (không có thì cả chuỗi).
pub fn after_last(s: &str, ch: char) -> &str {
    match s.rfind(ch) {
        Some(i) => &s[i + ch.len_utf8()..],
        None => s,
    }
}

/// Phần trước lần xuất hiện đầu của chuỗi tách (không có thì cả chuỗi).
pub fn before<'a>(s: &'a str, sep: &str) -> &'a str {
    match s.find(sep) {
        Some(i) => &s[..i],
        None => s,
    }
}

/// Phần sau lần xuất hiện đầu (không có thì "").
pub fn after<'a>(s: &'a str, sep: &str) -> &'a str {
    match s.find(sep) {
        Some(i) => &s[i + sep.len()..],
        None => "",
    }
}

/// Mã hoá cho tham số trong địa chỉ (như URLEncoder của Java: dấu cách → +).
pub fn form_encoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        let c = b as char;
        if b < 128 && (c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '*')) {
            out.push(c);
        } else if c == ' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Gộp khoảng trắng liên tiếp thành một dấu cách (như text() của jsoup).
pub fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cắt chuỗi tối đa n ký tự (không cắt giữa ký tự UTF-8).
pub fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ---- Thời gian ----

/// "1:05", "1:02:03"
pub fn format_time(seconds: f64) -> String {
    let s = if seconds.is_finite() { seconds.max(0.0) as i64 } else { 0 };
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// Thời gian hiện tại (ms kể từ 1970).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Chạy việc với giới hạn thời gian; quá hạn thì báo lỗi.
pub async fn with_timeout<T>(seconds: f64, work: impl Future<Output = Result<T>>) -> Result<T> {
    match tokio::time::timeout(Duration::from_secs_f64(seconds), work).await {
        Ok(r) => r,
        Err(_) => Err(Error::App(format!("Quá {} giây không trả lời", seconds as i64))),
    }
}

/// Giờ địa phương của mốc giây (Linux: localtime_r, Windows: _localtime64_s).
pub fn local_tm_of(secs: i64) -> libc::tm {
    // SAFETY: struct tm toàn số nguyên — khởi tạo 0 hợp lệ; hàm C chỉ ghi vào tm
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let tt = secs as libc::time_t;
    #[cfg(unix)]
    unsafe {
        libc::localtime_r(&tt, &mut tm);
    }
    #[cfg(windows)]
    unsafe {
        libc::localtime_s(&mut tm, &tt);
    }
    tm
}

/// Mốc giây của một giờ địa phương (ngày tràn tháng tự chuyển — mktime / _mktime64).
pub fn mktime_local(tm: &mut libc::tm) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: mktime đọc / chuẩn hoá tm hợp lệ
        unsafe { libc::mktime(tm) as i64 }
    }
    #[cfg(windows)]
    {
        extern "C" {
            fn _mktime64(tm: *mut libc::tm) -> i64;
        }
        // SAFETY: như trên (CRT của Windows)
        unsafe { _mktime64(tm) }
    }
}

/// Giờ địa phương (phút trong ngày, ngày "yyyy-MM-dd") — dùng cho giới hạn giờ xem.
pub fn local_now() -> (String, i32) {
    let t = now_ms() / 1000;
    let tm = crate::util::local_tm_of(t as i64);
    (format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday), tm.tm_hour * 60 + tm.tm_min)
}

/// "HH:mm" giờ địa phương của mốc ms.
pub fn local_hm(ms: i64) -> String {
    let tm = crate::util::local_tm_of((ms / 1000) as i64);
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

/// "dd/MM HH:mm" giờ địa phương của mốc ms.
pub fn local_dm_hm(ms: i64) -> String {
    let tm = crate::util::local_tm_of((ms / 1000) as i64);
    format!("{:02}/{:02} {:02}:{:02}", tm.tm_mday, tm.tm_mon + 1, tm.tm_hour, tm.tm_min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(format_time(65.0), "1:05");
        assert_eq!(format_time(3723.0), "1:02:03");
        assert_eq!(form_encoded("đấu phá a-b"), "%C4%91%E1%BA%A5u+ph%C3%A1+a-b");
        assert_eq!(collapse_spaces("  a \n\t b  "), "a b");
        assert_eq!(after_last("a/b/c", '/'), "c");
        assert_eq!(before("a?b", "?"), "a");
        static RE: Rx = Rx::new(r"(\d+)-(\d+)?");
        assert_eq!(RE.group(1, "x 12- y"), Some("12".into()));
        assert_eq!(RE.group(2, "x 12- y"), Some("".into()));
        assert_eq!(RE.group(1, "none"), None);
    }
}
