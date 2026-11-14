//! Cổng phát nội bộ cho trình phát native (libmpv): `http://127.0.0.1:<cổng>/<mã phiên>/<loại>/<địa chỉ gốc mã hoá>.<đuôi>`.
//! Trình phát chỉ nói chuyện với cổng này; cổng tải hộ qua máy khách của app (http::client → cổng SOCKS nội bộ → DNS mã hoá,
//! vượt chặn, proxy) và làm những việc hls.html làm trong WebView:
//! - `p/…m3u8` danh sách phát: bỏ khối quảng cáo chèn giữa phim (KKPhim), đổi mọi địa chỉ con thành địa chỉ của cổng này.
//! - `s/…ts`   đoạn video: chuyển tiếp từng phần; đoạn ngụy trang ảnh PNG (VSMOV) thì bỏ phần ảnh.
//! - `k/…key`  khoá AES-128 của luồng.
//! - `f/…mp4`  tệp video thường (MP4 của CLB Phim Xưa): chuyển tiếp cả Range (tua).
//! - `t/…vtt`  phụ đề rời ngụy trang ảnh PNG (server HD): bỏ phần trước «WEBVTT».
//! - `i/<n>.m3u8` danh sách phát có sẵn nội dung (trình phát nhúng tự giải mã ra `data:…;base64` — streamfree).
//!
//! Mã phiên ngẫu nhiên; chỉ vài phiên mới nhất dùng được — trang web khác trên máy không mượn được cổng này để tải hộ.

use base64::Engine;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

/// Số phiên giữ lại (phiên đang phát + vài phiên trước còn đang đóng dở)
const KEEP_SESSIONS: usize = 3;

struct Session {
    token: String,
    /// Referer gửi lên máy chủ video (dạng gốc trang — như trình duyệt gửi cho yêu cầu khác tên miền)
    referer: Option<String>,
    /// Danh sách phát có sẵn nội dung (i/<n>.m3u8)
    inline: Vec<String>,
    links: Links,
}

/// Bảng địa chỉ gốc của một phiên: đường dẫn trên cổng chỉ mang số thứ tự — link gốc có thể dài hàng nghìn ký tự (streamc của
/// NguonC: token lồng token), FFmpeg cắt địa chỉ quá 4096 ký tự.
#[derive(Default)]
pub struct Links {
    urls: Vec<String>,
    index: std::collections::HashMap<String, usize>,
}

impl Links {
    fn id(&mut self, abs: &str) -> usize {
        if let Some(&i) = self.index.get(abs) {
            return i;
        }
        self.urls.push(abs.to_string());
        self.index.insert(abs.to_string(), self.urls.len() - 1);
        self.urls.len() - 1
    }
    fn get(&self, id: usize) -> Option<&String> {
        self.urls.get(id)
    }
}

static PORT: OnceLock<u16> = OnceLock::new();
static SESSIONS: Mutex<Vec<Session>> = Mutex::new(Vec::new());

/// Địa chỉ đưa cho trình phát.
#[derive(Debug, Clone, PartialEq)]
pub struct Opened {
    pub url: String,
    pub subtitle: Option<String>,
    pub token: String,
}

/// Mở cổng (gọi nhiều lần cũng chỉ mở một lần). Trả số cổng.
pub fn start() -> u16 {
    *PORT.get_or_init(|| {
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("không mở được cổng phát nội bộ");
        std_listener.set_nonblocking(true).ok();
        let port = std_listener.local_addr().map(|a| a.port()).unwrap_or(0);
        crate::rt().spawn(async move {
            let listener = match TcpListener::from_std(std_listener) {
                Ok(l) => l,
                Err(e) => {
                    crate::log::d("media", &format!("cổng phát lỗi: {e}"));
                    return;
                }
            };
            loop {
                match listener.accept().await {
                    Ok((c, _)) => {
                        tokio::spawn(serve(c));
                    }
                    Err(e) => {
                        crate::log::d("media", &format!("accept lỗi: {e}"));
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        });
        crate::log::d("media", &format!("Cổng phát nội bộ 127.0.0.1:{port}"));
        port
    })
}

/// Referer kiểu trình duyệt cho yêu cầu khác tên miền: chỉ gốc trang («https://trang/»).
pub fn referer_of(page: &str) -> Option<String> {
    let u = url::Url::parse(page).ok()?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return None;
    }
    Some(format!("{}/", u.origin().ascii_serialization()))
}

/// Luồng HLS (.m3u8, hoặc data:…mpegurl) hay tệp video thường.
pub fn is_playlist(src: &str) -> bool {
    if src.starts_with("data:") {
        return src[..src.find(',').unwrap_or(src.len())].contains("mpegurl");
    }
    url::Url::parse(src).map(|u| u.path().to_ascii_lowercase().ends_with(".m3u8")).unwrap_or(false)
}

/// Luồng có khoá riêng của trang (DRM / hệ khoá tự chế: KEYFORMAT khác «identity», vd streamfree dùng SAMPLE-AES với khoá
/// gói kín chỉ trình phát của họ mở được). Không phát native — app không vượt lớp bảo vệ nội dung; giữ trình phát của trang.
pub fn protected(src: &str) -> bool {
    let text = if src.starts_with("data:") { data_text(src).unwrap_or_default() } else { src.to_string() };
    text.lines().filter(|l| l.starts_with("#EXT-X-KEY") || l.starts_with("#EXT-X-SESSION-KEY")).any(|l| {
        let fmt = l.find("KEYFORMAT=\"").map(|i| &l[i + 11..]).and_then(|s| s.split('"').next()).unwrap_or("identity");
        !l.contains("METHOD=NONE") && !fmt.eq_ignore_ascii_case("identity")
    })
}

/// Danh sách phát dùng được cho trình phát native: đúng dạng HLS (#EXTM3U + đoạn / danh sách con) và không có lớp bảo vệ riêng
/// của trang. NguonC (streamc) mã hoá cả danh sách bằng AES-GCM («#ENC-AESGCM;iv=…» + một khối dữ liệu) — không phải HLS thường.
pub fn playable(text: &str) -> bool {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    t.starts_with("#EXTM3U")
        && (t.contains("#EXTINF") || t.contains("#EXT-X-STREAM-INF"))
        && !t.lines().any(|l| l.starts_with("#ENC-"))
        && !protected(t)
}

/// Kiểm trước link luồng mà trình phát nhúng để lộ (trang vẫn mở): tải danh sách phát qua mạng của app; danh sách tổng thì kiểm
/// thêm danh sách con đầu tiên. Tệp video thường (MP4) luôn được.
pub async fn probe(src: &str, page: Option<&str>, hls: bool) -> bool {
    if src.starts_with("data:") {
        return data_text(src).map(|t| playable(&t)).unwrap_or(false);
    }
    if !(hls || is_playlist(src)) {
        return true;
    }
    let referer = page.and_then(referer_of);
    let mut url = src.to_string();
    for _ in 0..2 {
        let Ok(mut resp) = fetch(&url, referer.as_deref(), None, false).await else { return false };
        if !resp.status().is_success() {
            return false;
        }
        let base = resp.url().clone();
        let Ok(data) = read_all(&mut resp).await else { return false };
        let text = crate::http::text(&data, None);
        if !playable(&text) {
            return false;
        }
        if !text.contains("#EXT-X-STREAM-INF") {
            return true;
        }
        match text.lines().map(str::trim).find(|l| is_uri(l)) {
            Some(v) => url = resolve(Some(&base), v),
            None => return false,
        }
    }
    true
}

/// Mở phiên phát mới: `src` là http(s)://… (luồng .m3u8 / tệp video) hoặc `data:application/vnd.apple.mpegurl;base64,…`;
/// `page` = trang chứa trình phát (để gửi Referer); `hls` = biết chắc là luồng HLS dù link không có đuôi .m3u8 (trình phát
/// nhúng báo — NguonC / streamc dùng link token). None khi địa chỉ không dùng được.
pub fn open(src: &str, subtitle: Option<&str>, page: Option<&str>, hls: bool) -> Option<Opened> {
    let port = start();
    let token = format!("{:032x}", rand::random::<u128>());
    let prefix = format!("http://127.0.0.1:{port}/{token}");
    let mut inline = Vec::new();
    let mut links = Links::default();
    let url = if src.starts_with("data:") {
        inline.push(data_text(src)?);
        format!("{prefix}/i/0.m3u8")
    } else {
        let u = url::Url::parse(src).ok()?;
        if u.scheme() != "http" && u.scheme() != "https" {
            return None;
        }
        local(&prefix, if hls || is_playlist(src) { 'p' } else { 'f' }, src, &mut links)
    };
    let subtitle = subtitle.filter(|s| s.starts_with("http")).map(|s| local(&prefix, 't', s, &mut links));
    let mut ss = SESSIONS.lock().unwrap();
    ss.push(Session { token: token.clone(), referer: page.and_then(referer_of), inline, links });
    let n = ss.len();
    if n > KEEP_SESSIONS {
        ss.drain(..n - KEEP_SESSIONS);
    }
    Some(Opened { url, subtitle, token })
}

/// Đóng mọi phiên (thoát trình phát): địa chỉ cũ hết dùng được.
pub fn close_all() {
    SESSIONS.lock().unwrap().clear();
}

/// Nội dung của `data:…;base64,…` (hoặc `data:…,văn bản%xx`).
fn data_text(src: &str) -> Option<String> {
    let (head, body) = src.split_once(',')?;
    if head.ends_with(";base64") {
        let raw = base64::engine::general_purpose::STANDARD.decode(body.trim()).ok()?;
        String::from_utf8(raw).ok()
    } else {
        Some(percent_encoding::percent_decode_str(body).decode_utf8_lossy().into_owned())
    }
}

/// Địa chỉ trên cổng cho một địa chỉ gốc: <prefix>/<loại>/<số thứ tự trong bảng của phiên>.<đuôi>.
fn local(prefix: &str, kind: char, abs: &str, links: &mut Links) -> String {
    let ext = match kind {
        'p' | 'i' => "m3u8",
        'k' => "key",
        't' => "vtt",
        // Đuôi theo tệp gốc (FFmpeg mới chỉ nhận đoạn HLS có đuôi tệp video quen thuộc)
        _ => {
            let path = url::Url::parse(abs).map(|u| u.path().to_ascii_lowercase()).unwrap_or_default();
            const KNOWN: [&str; 10] = ["ts", "m4s", "mp4", "m4v", "m4a", "aac", "mp3", "vtt", "webm", "mkv"];
            let e = path.rsplit_once('.').map(|x| x.1).unwrap_or("");
            match KNOWN.iter().find(|k| **k == e) {
                Some(k) => k,
                None if kind == 'f' => "mp4",
                None => "ts",
            }
        }
    };
    format!("{prefix}/{kind}/{}.{ext}", links.id(abs))
}

/// Số thứ tự trong tên «12.ts».
fn id_of(name: &str) -> Option<usize> {
    name.split('.').next()?.parse().ok()
}

// ---- Danh sách phát ----

fn ad_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    // KKPhim chèn quảng cáo cờ bạc ~30 s giữa phim: một khối DISCONTINUITY mà mọi đoạn đều có dạng
    // /vN/<32 ký tự hex>/segment_NNNN.ts (đoạn phim tên ngẫu nhiên nằm cạnh index.m3u8). Như hls.html.
    RE.get_or_init(|| regex::Regex::new(r"/v\d+/[0-9a-f]{32}/segment_\d+\.ts").unwrap())
}

fn is_uri(l: &str) -> bool {
    !l.is_empty() && !l.starts_with('#')
}

/// Bỏ khối quảng cáo (giống hệt strip() của hls.html): khối DISCONTINUITY mà mọi đoạn đều là đoạn quảng cáo thì bỏ cả khối;
/// khối quảng cáo nằm cuối thì giữ dấu hết danh sách. Khối convertvN/… (phim có chữ quảng cáo đè lên) giữ lại.
pub fn strip_ads(text: &str) -> String {
    if !text.contains("#EXT-X-DISCONTINUITY") {
        return text.to_string();
    }
    let mut out: Vec<&str> = Vec::new();
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut in_block = false;
    for l in text.split('\n') {
        if l.starts_with("#EXT-X-DISCONTINUITY") && !l.starts_with("#EXT-X-DISCONTINUITY-SEQUENCE") {
            blocks.push(vec![l]);
            in_block = true;
        } else if in_block || l.starts_with("#EXTINF") || is_uri(l) {
            if !in_block {
                blocks.push(Vec::new());
                in_block = true;
            }
            blocks.last_mut().unwrap().push(l);
        } else {
            out.push(l); // phần đầu danh sách (#EXTM3U, #EXT-X-TARGETDURATION…)
        }
    }
    for b in blocks {
        let segs: Vec<&&str> = b.iter().filter(|l| is_uri(l)).collect();
        if !segs.is_empty() && segs.iter().all(|u| ad_re().is_match(u)) {
            out.extend(b.iter().filter(|l| l.starts_with("#EXT-X-ENDLIST")));
            continue;
        }
        out.extend(b);
    }
    out.join("\n")
}

fn resolve(base: Option<&url::Url>, s: &str) -> String {
    if s.starts_with("http://") || s.starts_with("https://") {
        return s.to_string();
    }
    match base.and_then(|b| b.join(s).ok()) {
        Some(u) => u.to_string(),
        None => s.to_string(),
    }
}

/// Đổi giá trị URI="…" trong một dòng thẻ.
fn rewrite_uri_attr(line: &str, base: Option<&url::Url>, prefix: &str, kind: char, links: &mut Links) -> String {
    let Some(i) = line.find("URI=\"") else { return line.to_string() };
    let start = i + 5;
    let Some(len) = line[start..].find('"') else { return line.to_string() };
    let abs = resolve(base, &line[start..start + len]);
    format!("{}{}{}", &line[..start], local(prefix, kind, &abs, links), &line[start + len..])
}

/// Viết lại danh sách phát: bỏ quảng cáo, mọi địa chỉ con (danh sách con, đoạn, khoá, đoạn khởi tạo) thành địa chỉ trên cổng.
pub fn rewrite_playlist(text: &str, base: Option<&url::Url>, prefix: &str, links: &mut Links) -> String {
    let text = strip_ads(&text.replace("\r\n", "\n"));
    let master = text.contains("#EXT-X-STREAM-INF");
    let mut out = String::with_capacity(text.len() * 2);
    for l in text.split('\n') {
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with('#') {
            if t.starts_with("#EXT-X-I-FRAME-STREAM-INF") {
                continue; // danh sách khung chính (tua nhanh có hình) — trình phát không cần
            }
            let line = if t.starts_with("#EXT-X-KEY") || t.starts_with("#EXT-X-SESSION-KEY") {
                rewrite_uri_attr(t, base, prefix, 'k', links)
            } else if t.starts_with("#EXT-X-MAP") {
                rewrite_uri_attr(t, base, prefix, 's', links)
            } else if t.starts_with("#EXT-X-MEDIA") {
                rewrite_uri_attr(t, base, prefix, 'p', links)
            } else {
                t.to_string()
            };
            out.push_str(&line);
        } else {
            out.push_str(&local(prefix, if master { 'p' } else { 's' }, &resolve(base, t), links));
        }
        out.push('\n');
    }
    out
}

/// Đoạn video ngụy trang ảnh PNG (VSMOV: ảnh nhỏ đứng trước dữ liệu MPEG-TS) → vị trí byte đồng bộ 0x47 đầu tiên mà 3 gói
/// 188 byte sau cũng bắt đầu bằng 0x47 (như unwrap() của hls.html). Đoạn bình thường → 0.
pub fn png_skip(b: &[u8]) -> usize {
    if b.len() < 8 || b[..4] != [0x89, 0x50, 0x4E, 0x47] {
        return 0;
    }
    let n = b.len().saturating_sub(188 * 3).min(65536);
    (8..n).find(|&i| b[i] == 0x47 && b[i + 188] == 0x47 && b[i + 376] == 0x47 && b[i + 564] == 0x47).unwrap_or(0)
}

// ---- Máy chủ HTTP nhỏ (một kết nối giữ lại cho nhiều yêu cầu, như FFmpeg dùng) ----

struct Request {
    method: String,
    path: String,
    range: Option<String>,
    close: bool,
}

async fn read_request<R: tokio::io::AsyncBufRead + Unpin>(r: &mut R) -> Option<Request> {
    let mut line = String::new();
    let n = tokio::time::timeout(Duration::from_secs(120), r.read_line(&mut line)).await.ok()?.ok()?;
    if n == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let http10 = parts.next().map(|v| v == "HTTP/1.0").unwrap_or(false);
    let (mut range, mut close) = (None, http10);
    let mut total = 0usize;
    loop {
        let mut h = String::new();
        let n = tokio::time::timeout(Duration::from_secs(30), r.read_line(&mut h)).await.ok()?.ok()?;
        total += n;
        if n == 0 || total > 32 * 1024 {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            if k == "range" {
                range = Some(v.to_string());
            } else if k == "connection" {
                close = v.eq_ignore_ascii_case("close");
            }
        }
    }
    Some(Request { method, path, range, close })
}

async fn serve(sock: TcpStream) {
    let _ = sock.set_nodelay(true);
    let (r, mut w) = sock.into_split();
    let mut r = BufReader::new(r);
    while let Some(req) = read_request(&mut r).await {
        let keep = handle(&req, &mut w).await;
        if !keep || req.close {
            break;
        }
    }
    let _ = w.shutdown().await;
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        206 => "Partial Content",
        400 => "Bad Request",
        404 => "Not Found",
        416 => "Range Not Satisfiable",
        _ => "Bad Gateway",
    }
}

async fn write_head<W: AsyncWrite + Unpin>(w: &mut W, code: u16, headers: &[(&str, String)], keep: bool) -> std::io::Result<()> {
    let mut s = format!("HTTP/1.1 {code} {}\r\n", reason(code));
    for (k, v) in headers {
        s.push_str(&format!("{k}: {v}\r\n"));
    }
    s.push_str(if keep { "Connection: keep-alive\r\n\r\n" } else { "Connection: close\r\n\r\n" });
    w.write_all(s.as_bytes()).await
}

/// Trả lời trọn gói (độ dài biết trước). true = giữ kết nối.
async fn reply<W: AsyncWrite + Unpin>(w: &mut W, code: u16, ct: &str, body: &[u8], head_only: bool) -> bool {
    let hs = [("Content-Type", ct.to_string()), ("Content-Length", body.len().to_string())];
    if write_head(w, code, &hs, true).await.is_err() {
        return false;
    }
    if !head_only && !body.is_empty() && w.write_all(body).await.is_err() {
        return false;
    }
    w.flush().await.is_ok()
}

fn session(token: &str) -> Option<(Option<String>, Vec<String>)> {
    let ss = SESSIONS.lock().unwrap();
    ss.iter().find(|s| s.token == token).map(|s| (s.referer.clone(), s.inline.clone()))
}

/// Dùng bảng địa chỉ của phiên (viết lại danh sách phát / tra số thứ tự).
fn with_links<R>(token: &str, f: impl FnOnce(&mut Links) -> R) -> Option<R> {
    let mut ss = SESSIONS.lock().unwrap();
    ss.iter_mut().find(|s| s.token == token).map(|s| f(&mut s.links))
}

/// Tải từ máy chủ gốc qua máy khách của app; lỗi kết nối / 5xx / 429 thì thử lại (1 → 2 → 4 s) như hls.html.
async fn fetch(url: &str, referer: Option<&str>, range: Option<&str>, long: bool) -> Result<reqwest::Response, String> {
    let mut last = String::new();
    for attempt in 0..4u32 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500 << attempt)).await;
        }
        let mut rb = crate::http::client()
            .get(url)
            .header("Accept", "*/*")
            .header("Accept-Language", "vi-VN,vi;q=0.9,en;q=0.8")
            .timeout(Duration::from_secs(if long { 6 * 3600 } else { 180 }));
        if let Some(r) = referer {
            rb = rb.header("Referer", r);
        }
        if let Some(r) = range {
            rb = rb.header("Range", r);
        }
        match rb.send().await {
            Ok(resp) if resp.status().is_server_error() || resp.status().as_u16() == 429 => last = format!("HTTP {}", resp.status().as_u16()),
            Ok(resp) => return Ok(resp),
            Err(e) => last = e.to_string(),
        }
    }
    Err(last)
}

/// Đọc tiếp một phần thân; im quá 30 s coi như đứt.
async fn chunk(resp: &mut reqwest::Response) -> Result<Option<bytes::Bytes>, String> {
    match tokio::time::timeout(Duration::from_secs(30), resp.chunk()).await {
        Ok(Ok(c)) => Ok(c),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("hết thời gian chờ dữ liệu".into()),
    }
}

async fn read_all(resp: &mut reqwest::Response) -> Result<Vec<u8>, String> {
    let mut v = Vec::with_capacity(resp.content_length().unwrap_or(0).min(64 << 20) as usize);
    while let Some(c) = chunk(resp).await? {
        v.extend_from_slice(&c);
    }
    Ok(v)
}

async fn handle<W: AsyncWrite + Unpin>(req: &Request, w: &mut W) -> bool {
    let head_only = req.method == "HEAD";
    if req.method != "GET" && !head_only {
        return reply(w, 400, "text/plain", b"", false).await;
    }
    let path = req.path.split('?').next().unwrap_or("");
    let mut it = path.trim_start_matches('/').splitn(3, '/');
    let (token, kind, name) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
    let Some((referer, inline)) = session(token) else {
        return reply(w, 404, "text/plain", b"", head_only).await;
    };
    let prefix = format!("http://127.0.0.1:{}/{token}", start());
    let referer = referer.as_deref();
    if kind == "i" {
        let idx: usize = name.split('.').next().and_then(|n| n.parse().ok()).unwrap_or(usize::MAX);
        return match inline.get(idx) {
            Some(text) if playable(text) => {
                let t = with_links(token, |l| rewrite_playlist(text, None, &prefix, l)).unwrap_or_default();
                reply(w, 200, "application/vnd.apple.mpegurl", t.as_bytes(), head_only).await
            }
            _ => reply(w, 404, "text/plain", b"", head_only).await,
        };
    }
    let Some(url) = id_of(name).and_then(|id| with_links(token, |l| l.get(id).cloned()).flatten()) else {
        return reply(w, 404, "text/plain", b"", head_only).await;
    };
    match kind {
        "p" => {
            let r = async {
                let mut resp = fetch(&url, referer, None, false).await?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status().as_u16()));
                }
                let base = resp.url().clone();
                let ct = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
                let data = read_all(&mut resp).await?;
                let text = crate::http::text(&data, ct.as_deref());
                if !playable(&text) {
                    // Trình phát báo lỗi → app phát lại bằng trình phát của trang
                    return Err("không phải HLS thường (mã hoá / khoá riêng của trang — không phát native)".into());
                }
                with_links(token, |l| rewrite_playlist(&text, Some(&base), &prefix, l)).ok_or_else(|| "phiên đã đóng".to_string())
            }
            .await;
            match r {
                Ok(t) => reply(w, 200, "application/vnd.apple.mpegurl", t.as_bytes(), head_only).await,
                Err(e) => {
                    crate::log::d("media", &format!("danh sách phát lỗi ({e}): {}", short(&url)));
                    reply(w, 502, "text/plain", b"", head_only).await
                }
            }
        }
        "k" | "t" => {
            let r = async {
                let mut resp = fetch(&url, referer, None, false).await?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status().as_u16()));
                }
                read_all(&mut resp).await
            }
            .await;
            match r {
                Ok(data) if kind == "t" => {
                    let t = String::from_utf8_lossy(&data).into_owned();
                    let t = t.find("WEBVTT").map(|i| t[i..].to_string()).unwrap_or(t);
                    reply(w, 200, "text/vtt; charset=utf-8", t.as_bytes(), head_only).await
                }
                Ok(data) => reply(w, 200, "application/octet-stream", &data, head_only).await,
                Err(e) => {
                    crate::log::d("media", &format!("tải {kind} lỗi ({e}): {}", short(&url)));
                    reply(w, 502, "text/plain", b"", head_only).await
                }
            }
        }
        "s" => segment(&url, referer, w, head_only).await,
        "f" => file(&url, referer, req.range.as_deref(), w, head_only).await,
        _ => reply(w, 404, "text/plain", b"", head_only).await,
    }
}

fn short(u: &str) -> String {
    u.chars().take(120).collect()
}

/// Đoạn video: biết trước độ dài và không phải PNG thì chuyển tiếp từng phần ngay (trình phát bắt đầu giải mã sớm);
/// đoạn ngụy trang PNG / không rõ độ dài thì tải trọn rồi trả.
async fn segment<W: AsyncWrite + Unpin>(url: &str, referer: Option<&str>, w: &mut W, head_only: bool) -> bool {
    let mut resp = match fetch(url, referer, None, false).await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            crate::log::d("media", &format!("đoạn video HTTP {}: {}", r.status().as_u16(), short(url)));
            return reply(w, 502, "text/plain", b"", head_only).await;
        }
        Err(e) => {
            crate::log::d("media", &format!("đoạn video lỗi ({e}): {}", short(url)));
            return reply(w, 502, "text/plain", b"", head_only).await;
        }
    };
    let ct = if url.contains(".m4s") || url.contains(".mp4") { "video/mp4" } else { "video/mp2t" };
    let total = resp.content_length();
    // Gom ít nhất 8 byte đầu để nhận ra PNG
    let mut first: Vec<u8> = Vec::new();
    let mut done = false;
    while first.len() < 8 {
        match chunk(&mut resp).await {
            Ok(Some(c)) => first.extend_from_slice(&c),
            Ok(None) => {
                done = true;
                break;
            }
            Err(e) => {
                crate::log::d("media", &format!("đoạn video đứt ({e}): {}", short(url)));
                return reply(w, 502, "text/plain", b"", head_only).await;
            }
        }
    }
    let png = first.len() >= 4 && first[..4] == [0x89, 0x50, 0x4E, 0x47];
    if png || total.is_none() || done {
        if !done {
            match read_all(&mut resp).await {
                Ok(rest) => first.extend_from_slice(&rest),
                Err(e) => {
                    crate::log::d("media", &format!("đoạn video đứt ({e}): {}", short(url)));
                    return reply(w, 502, "text/plain", b"", head_only).await;
                }
            }
        }
        let skip = png_skip(&first);
        return reply(w, 200, ct, &first[skip..], head_only).await;
    }
    let total = total.unwrap_or(0);
    let hs = [("Content-Type", ct.to_string()), ("Content-Length", total.to_string())];
    if write_head(w, 200, &hs, true).await.is_err() {
        return false;
    }
    if head_only {
        return w.flush().await.is_ok();
    }
    if w.write_all(&first).await.is_err() {
        return false;
    }
    let mut sent = first.len() as u64;
    loop {
        match chunk(&mut resp).await {
            Ok(Some(c)) => {
                if w.write_all(&c).await.is_err() {
                    return false;
                }
                sent += c.len() as u64;
            }
            Ok(None) => break,
            Err(e) => {
                // Đã gửi phần đầu: đóng kết nối để trình phát biết đoạn hỏng và tải lại
                crate::log::d("media", &format!("đoạn video đứt giữa chừng ({e}): {}", short(url)));
                return false;
            }
        }
    }
    sent == total && w.flush().await.is_ok()
}

/// Tệp video thường: chuyển tiếp yêu cầu Range (tua) và trả lời của máy chủ gốc.
async fn file<W: AsyncWrite + Unpin>(url: &str, referer: Option<&str>, range: Option<&str>, w: &mut W, head_only: bool) -> bool {
    let mut resp = match fetch(url, referer, range, true).await {
        Ok(r) => r,
        Err(e) => {
            crate::log::d("media", &format!("tệp video lỗi ({e}): {}", short(url)));
            return reply(w, 502, "text/plain", b"", head_only).await;
        }
    };
    let code = resp.status().as_u16();
    if code >= 400 {
        crate::log::d("media", &format!("tệp video HTTP {code}: {}", short(url)));
        return reply(w, if code == 416 { 416 } else { 502 }, "text/plain", b"", head_only).await;
    }
    let get = |k: reqwest::header::HeaderName| resp.headers().get(k).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let ct = get(reqwest::header::CONTENT_TYPE).filter(|c| c.starts_with("video/") || c.starts_with("audio/")).unwrap_or_else(|| "video/mp4".into());
    let mut hs = vec![("Content-Type", ct), ("Accept-Ranges", "bytes".to_string())];
    let upstream_range = get(reqwest::header::CONTENT_RANGE).and_then(|cr| parse_content_range(&cr));
    // Máy chủ chỉ trả từng khúc (k-20: 8 MB mỗi lần) dù trình phát xin tới hết tệp: nối các khúc thành một luồng liền — trình
    // phát khỏi báo «luồng kết thúc sớm» rồi kết nối lại mỗi 8 MB
    let open_ended = range.map(|r| r.trim_end().ends_with('-')).unwrap_or(true);
    let stitch = match upstream_range {
        Some((a, b, total)) if code == 206 && open_ended && b + 1 < total => Some((a, total)),
        _ => None,
    };
    let (status, len) = match (stitch, upstream_range) {
        (Some((a, total)), _) => {
            if range.is_some() {
                hs.push(("Content-Range", format!("bytes {a}-{}/{total}", total - 1)));
            }
            (if range.is_some() { 206 } else { 200 }, Some(total - a))
        }
        (None, Some((a, b, total))) => {
            hs.push(("Content-Range", format!("bytes {a}-{b}/{total}")));
            (if code == 206 { 206 } else { 200 }, resp.content_length())
        }
        (None, None) => (if code == 206 { 206 } else { 200 }, resp.content_length()),
    };
    if let Some(n) = len {
        hs.push(("Content-Length", n.to_string()));
    }
    let keep = len.is_some();
    if write_head(w, status, &hs, keep).await.is_err() {
        return false;
    }
    if head_only {
        return keep && w.flush().await.is_ok();
    }
    let mut pos = upstream_range.map(|r| r.0).unwrap_or(0);
    loop {
        match chunk(&mut resp).await {
            Ok(Some(c)) => {
                if w.write_all(&c).await.is_err() {
                    return false;
                }
                pos += c.len() as u64;
            }
            Ok(None) => {
                let Some((_, total)) = stitch else { break };
                if pos >= total {
                    break;
                }
                // Khúc sau
                let next = format!("bytes={pos}-");
                resp = match fetch(url, referer, Some(&next), true).await {
                    Ok(r) if r.status().as_u16() == 206 => r,
                    _ => {
                        crate::log::d("media", &format!("tệp video: không tải được khúc từ byte {pos}: {}", short(url)));
                        return false;
                    }
                };
                let start = resp.headers().get(reqwest::header::CONTENT_RANGE).and_then(|v| v.to_str().ok()).and_then(parse_content_range);
                if start.map(|r| r.0) != Some(pos) {
                    return false;
                }
            }
            Err(_) => return false,
        }
    }
    keep && w.flush().await.is_ok()
}

/// «bytes a-b/total» → (a, b, total).
fn parse_content_range(cr: &str) -> Option<(u64, u64, u64)> {
    let rest = cr.trim().strip_prefix("bytes")?.trim();
    let (span, total) = rest.split_once('/')?;
    let (a, b) = span.split_once('-')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?, total.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "http://127.0.0.1:1/tok";

    fn back(l: &Links, local: &str) -> String {
        l.get(id_of(local.rsplit('/').next().unwrap()).unwrap()).unwrap().clone()
    }

    #[test]
    fn referer_is_origin() {
        assert_eq!(referer_of("https://www.kkphim.com/phim/a/tap-1?sv=0").as_deref(), Some("https://www.kkphim.com/"));
        assert_eq!(referer_of("about:blank"), None);
    }

    #[test]
    fn playlist_detection() {
        assert!(is_playlist("https://a.b/x/index.m3u8"));
        assert!(is_playlist("https://a.b/x/INDEX.M3U8?t=1"));
        assert!(!is_playlist("https://a.b/x/phim.mp4"));
        assert!(is_playlist("data:application/vnd.apple.mpegurl;base64,I0VYVE0zVQ=="));
    }

    #[test]
    fn name_roundtrip_and_ext() {
        let u = "https://cdn.x/a/b/seg_001.ts?token=abc&e=1";
        let mut t = Links::default();
        let l = local(P, 's', u, &mut t);
        assert_eq!(l, "http://127.0.0.1:1/tok/s/0.ts");
        assert_eq!(back(&t, &l), u);
        assert_eq!(local(P, 's', u, &mut t), l, "cùng link → cùng số");
        // link rất dài (token lồng token) vẫn cho địa chỉ ngắn
        let long = format!("https://embed14.streamc.xyz/{}", "eyJ0".repeat(2000));
        assert!(local(P, 'p', &long, &mut t).len() < 60);
        assert!(local(P, 's', "https://cdn.x/a/seg.png", &mut t).ends_with(".ts"), "đoạn ngụy trang ảnh vẫn mang đuôi .ts");
        assert!(local(P, 's', "https://cdn.x/a/init.mp4", &mut t).ends_with(".mp4"));
        assert!(local(P, 'f', "https://cdn.x/phim", &mut t).ends_with(".mp4"));
        assert!(local(P, 'p', "https://cdn.x/a/index.m3u8", &mut t).ends_with(".m3u8"));
    }

    #[test]
    fn strip_ads_like_hls_html() {
        let ad = "https://x.y/v3/0123456789abcdef0123456789abcdef/segment_0001.ts";
        let pl = format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6,\nphim001.ts\n#EXTINF:6,\nphim002.ts\n#EXT-X-DISCONTINUITY\n#EXTINF:5,\n{ad}\n\
             #EXTINF:5,\n{ad}\n#EXT-X-DISCONTINUITY\n#EXTINF:6,\nphim003.ts\n#EXT-X-ENDLIST\n"
        );
        let out = strip_ads(&pl);
        assert!(!out.contains("segment_0001"));
        assert!(out.contains("phim001.ts") && out.contains("phim003.ts") && out.contains("#EXT-X-ENDLIST"));
        // khối quảng cáo nằm cuối: vẫn giữ dấu hết danh sách
        let tail = format!("#EXTM3U\n#EXTINF:6,\nphim001.ts\n#EXT-X-DISCONTINUITY\n#EXTINF:5,\n{ad}\n#EXT-X-ENDLIST\n");
        let out = strip_ads(&tail);
        assert!(!out.contains("segment_0001") && out.contains("#EXT-X-ENDLIST"));
        // không có DISCONTINUITY: giữ nguyên
        assert_eq!(strip_ads("#EXTM3U\n#EXTINF:6,\na.ts\n"), "#EXTM3U\n#EXTINF:6,\na.ts\n");
        // khối convertvN (phim có chữ đè) giữ lại
        let conv = "#EXTM3U\n#EXT-X-DISCONTINUITY\n#EXTINF:6,\nhttps://x.y/convertv7/abc/segment_0001.ts\n";
        assert!(strip_ads(conv).contains("convertv7"));
    }

    #[test]
    fn rewrite_master_and_media() {
        let mut t = Links::default();
        let base = url::Url::parse("https://a.kvp726.com/20261109/1Rtdidda/index.m3u8").unwrap();
        let master = "#EXTM3U\r\n#EXT-X-STREAM-INF:PROGRAM-ID=1,BANDWIDTH=3500000,RESOLUTION=2542x1080\r\n3500kb/hls/index.m3u8\r\n";
        let out = rewrite_playlist(master, Some(&base), P, &mut t);
        let line = out.lines().last().unwrap();
        assert!(line.starts_with("http://127.0.0.1:1/tok/p/") && line.ends_with(".m3u8"));
        assert_eq!(back(&t, line), "https://a.kvp726.com/20261109/1Rtdidda/3500kb/hls/index.m3u8");

        let base = url::Url::parse("https://a.kvp726.com/20261109/1Rtdidda/3500kb/hls/index.m3u8").unwrap();
        let media = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.key\",IV=0x01\n#EXT-X-MAP:URI=\"/init.mp4\"\n#EXTINF:6,\nseg1.ts\n\
                     #EXTINF:6,\nhttps://cdn2.x/seg2.png\n#EXT-X-I-FRAME-STREAM-INF:URI=\"if.m3u8\"\n#EXT-X-ENDLIST";
        let out = rewrite_playlist(media, Some(&base), P, &mut t);
        let key = out.lines().find(|l| l.starts_with("#EXT-X-KEY")).unwrap();
        assert!(key.contains("URI=\"http://127.0.0.1:1/tok/k/") && key.ends_with(".key\",IV=0x01"));
        let map = out.lines().find(|l| l.starts_with("#EXT-X-MAP")).unwrap();
        let map_url = map.split('"').nth(1).unwrap();
        assert_eq!(back(&t, map_url), "https://a.kvp726.com/init.mp4");
        let segs: Vec<&str> = out.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(segs.len(), 2);
        assert_eq!(back(&t, segs[0]), "https://a.kvp726.com/20261109/1Rtdidda/3500kb/hls/seg1.ts");
        assert!(segs[1].contains("/tok/s/") && segs[1].ends_with(".ts"));
        assert!(!out.contains("I-FRAME"));
        assert!(out.contains("#EXT-X-ENDLIST"));
    }

    #[test]
    fn inline_data_playlist() {
        let mut t = Links::default();
        let text = "#EXTM3U\n#EXTINF:4,\nhttps://s.cdn/a/1.ts\n#EXT-X-ENDLIST\n";
        let src = format!("data:application/vnd.apple.mpegurl;base64,{}", base64::engine::general_purpose::STANDARD.encode(text));
        assert_eq!(data_text(&src).as_deref(), Some(text));
        let out = rewrite_playlist(text, None, P, &mut t);
        let seg = out.lines().find(|l| !l.starts_with('#')).unwrap();
        assert_eq!(back(&t, seg), "https://s.cdn/a/1.ts");
    }

    #[test]
    fn protected_streams_stay_with_page_player() {
        let drm = "#EXTM3U\n#EXT-X-KEY:IV=0x01,KEYFORMAT=\"urn:uuid:2dfa3a2e-bb7a-447d-a7b1-3d15b81f5b62\",METHOD=SAMPLE-AES,URI=\"data:text/plain;base64,VVNLRA==\"\n#EXTINF:10,\na.ts\n";
        assert!(protected(drm));
        let src = format!("data:application/vnd.apple.mpegurl;base64,{}", base64::engine::general_purpose::STANDARD.encode(drm));
        assert!(protected(&src));
        // AES-128 thường (khoá công khai cho mọi trình phát HLS) / không mã hoá: phát native được
        assert!(!protected("#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.key\",IV=0x01\n#EXTINF:10,\na.ts\n"));
        assert!(!protected("#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.key\",KEYFORMAT=\"identity\"\n"));
        assert!(!protected("#EXTM3U\n#EXTINF:10,\na.ts\n"));
        // NguonC (streamc): cả danh sách mã hoá AES-GCM — không phải HLS thường
        assert!(!playable("#EXTM3U\n#ENC-AESGCM;iv=000102030405060708090a0b\n#EXT-X-B65:0-138\nQUJDREVGR0hJSktMTU5PUA==\n"));
        assert!(!playable(drm));
        assert!(!playable("<html>lỗi</html>"));
        assert!(playable("\u{feff}#EXTM3U\n#EXTINF:10,\na.ts\n"));
        assert!(playable("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv/index.m3u8\n"));
    }

    #[test]
    fn content_range() {
        assert_eq!(parse_content_range("bytes 8388608-16777215/939890641"), Some((8388608, 16777215, 939890641)));
        assert_eq!(parse_content_range("bytes */100"), None);
        assert_eq!(parse_content_range("items 0-1/2"), None);
    }

    #[test]
    fn png_wrapped_segment() {
        let mut ts = vec![0u8; 188 * 5];
        for i in 0..5 {
            ts[i * 188] = 0x47;
        }
        let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend(std::iter::repeat(0x11).take(70));
        let skip = png.len();
        png.extend(&ts);
        assert_eq!(png_skip(&png), skip);
        assert_eq!(png_skip(&ts), 0);
        assert_eq!(png_skip(b"\x89PNG"), 0);
    }

    #[test]
    fn open_session_urls() {
        let o = open("https://a.b/x/index.m3u8", Some("https://a.b/sub.png"), Some("https://site.vn/phim/1"), false).unwrap();
        assert!(o.url.contains(&o.token) && o.url.contains("/p/") && o.url.ends_with(".m3u8"));
        assert!(o.subtitle.as_deref().unwrap().contains("/t/"));
        let f = open("https://a.b/x/phim.mp4", None, None, false).unwrap();
        assert!(f.url.contains("/f/") && f.url.ends_with(".mp4"));
        // link token không đuôi (streamc) mà trình phát nhúng báo là HLS
        let t = open("https://embed14.streamc.xyz/eyJ0IjoiZXlK", None, None, true).unwrap();
        assert!(t.url.contains("/p/") && t.url.ends_with(".m3u8"));
        assert!(open("ftp://x/y", None, None, false).is_none());
        assert_eq!(session(&o.token).unwrap().0.as_deref(), Some("https://site.vn/"));
    }
}
