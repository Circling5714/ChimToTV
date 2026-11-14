//! Tự cập nhật từ GitHub Releases (bản Android: Updater.kt, iPad: UpdateCheck.swift, Windows: UpdateCheck.cs). Bản Linux phát
//! hành bằng tag riêng "linux-v…" và không bao giờ là bản "latest" (app TV đọc releases/latest và lấy tệp .apk) → duyệt danh
//! sách release, lấy tag linux-v* có tệp .deb (không lấy release của nền tảng khác: TV `v…`, Windows `win-v`, iPad `ios-v`,
//! Mac `mac-v`). Cài: tải tệp .deb vào ~/.cache/conbangtv, kiểm đúng cỡ + SHA-256 GitHub báo + đúng gói `conbangtv` và số bản
//! (`dpkg-deb -f`), rồi `pkexec apt-get install -y <tệp>` (không có thì `pkexec dpkg -i`) và mở lại app (`relaunch`).

use crate::json::{self, Obj};
use crate::util::{now_ms, Error, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const RELEASES_API: &str = "https://api.github.com/repos/Circling5714/ChimToTV/releases?per_page=30";
pub const RELEASES_PAGE: &str = "https://github.com/Circling5714/ChimToTV/releases";
pub const PREFIX: &str = "linux-v";
/// Tự kiểm tối đa 2 giờ một lần.
pub const CHECK_EVERY: i64 = 2 * 60 * 60 * 1000;
/// «Để sau»: không hỏi lại bản đó trong 24 giờ.
pub const SNOOZE: i64 = 24 * 60 * 60 * 1000;
/// Tên gói .deb và nơi cài tệp chạy.
pub const PACKAGE: &str = "conbangtv";
pub const INSTALLED_BIN: &str = "/usr/bin/conbangtv";

/// `api_url`: link tải cùng tệp qua API (api.github.com/…/releases/assets/<id>) — dùng khi link github.com lỗi.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub deb_url: String,
    pub notes: String,
    pub tag: String,
    pub sha256: Option<String>,
    pub size: i64,
    pub api_url: Option<String>,
}

/// Kết quả cài bản mới.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    /// Đã cài xong — gọi `relaunch()` rồi thoát app.
    Done,
    /// Người dùng đóng hộp hỏi mật khẩu (không cài).
    Cancelled,
}

/// "1.0.0" → "1.0" (bỏ số 0 cuối cho gọn, khớp tên tag).
pub fn current_version() -> String {
    short_version(crate::version())
}

fn short_version(v: &str) -> String {
    let p = parts(v);
    let at = |i: usize| p.get(i).copied().unwrap_or(0);
    if at(2) > 0 {
        format!("{}.{}.{}", at(0), at(1), at(2))
    } else {
        format!("{}.{}", at(0), at(1))
    }
}

/// Bản Linux mới nhất (bỏ bản nháp / thử nghiệm); None = chưa có bản nào.
pub async fn latest(api: Option<&str>) -> Result<Option<Release>> {
    // Thử cập nhật trên máy: biến môi trường CONBANG_UPDATE_API trỏ tới danh sách release giả
    let api = match api {
        Some(a) => a.to_string(),
        None => std::env::var("CONBANG_UPDATE_API").unwrap_or_else(|_| RELEASES_API.to_string()),
    };
    let r = crate::http::get(&api, &[("Accept", "application/vnd.github+json")]).await?;
    Ok(pick_latest(&r.data))
}

/// Chọn bản Linux mới nhất trong danh sách release (JSON của GitHub API).
pub fn pick_latest(data: &[u8]) -> Option<Release> {
    let list = json::parse(data)?;
    let list = list.as_array()?;
    let mut best: Option<Release> = None;
    for o in list.iter().filter_map(|v| v.as_object()) {
        let Some(r) = release_of(o) else { continue };
        if best.as_ref().map(|b| is_newer(&r.version, Some(&b.version))).unwrap_or(true) {
            best = Some(r);
        }
    }
    best
}

fn release_of(o: &Obj) -> Option<Release> {
    let tag = json::str(Some(o), "tag_name");
    if !tag.starts_with(PREFIX) || json::bool(Some(o), "draft", false) || json::bool(Some(o), "prerelease", false) {
        return None;
    }
    let debs: Vec<&Obj> = json::objects(json::arr(Some(o), "assets"))
        .into_iter()
        .filter(|a| json::str(Some(*a), "name").to_lowercase().ends_with(".deb"))
        .collect();
    // Có nhiều tệp .deb (amd64 / arm64): lấy tệp đúng kiến trúc máy này
    let arch = deb_arch();
    let deb: &Obj = debs
        .iter()
        .copied()
        .find(|a| json::str(Some(*a), "name").to_lowercase().contains(arch))
        .or_else(|| debs.first().copied())?;
    let digest = json::str(Some(deb), "digest");
    let sha = digest.strip_prefix("sha256:").map(|s| s.to_lowercase());
    let size = json::int(Some(deb), "size", 0);
    let url = json::str(Some(deb), "url");
    let api = if url.starts_with("https://api.github.com/") { Some(url) } else { None };
    Some(Release {
        version: tag[PREFIX.len()..].to_string(),
        deb_url: json::str(Some(deb), "browser_download_url"),
        notes: json::str(Some(o), "body"),
        tag,
        sha256: sha,
        size,
        api_url: api,
    })
}

/// Tên kiến trúc kiểu Debian của máy này.
pub fn deb_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "arm" => "armhf",
        "x86" => "i386",
        a => a,
    }
}

/// remote mới hơn local (None = bản đang chạy)?
pub fn is_newer(remote: &str, local: Option<&str>) -> bool {
    let a = parts(remote);
    let cur;
    let local = match local {
        Some(l) => l,
        None => {
            cur = current_version();
            &cur
        }
    };
    let b = parts(local);
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

fn parts(v: &str) -> Vec<i64> {
    let v = crate::util::before(crate::util::before(v, "-"), "+");
    v.split('.').map(|p| p.trim().parse::<i64>().unwrap_or(0)).collect()
}

/// Ghi chú phát hành (markdown) → vài dòng chữ thường cho hộp thoại; có mục "Có gì mới" thì chỉ lấy mục đó.
pub fn summary(notes: &str) -> String {
    let lines: Vec<String> = notes.replace('\r', "").split('\n').map(|l| l.trim().to_string()).collect();
    let start = lines.iter().position(|l| l.starts_with('#') && l.to_lowercase().contains("có gì mới"));
    let mut section: &[String] = &lines;
    if let Some(s) = start {
        section = &lines[s + 1..];
        if let Some(end) = section.iter().position(|l| l.starts_with("## ")) {
            section = &section[..end];
        }
    }
    let text = section
        .iter()
        .filter(|l| !l.is_empty() && !l.starts_with("![") && !l.starts_with('|') && !l.contains("SHA-256"))
        .map(|l| l.trim_matches(|c| matches!(c, '#' | '-' | '*' | ' ')).replace("**", "").replace('`', ""))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if text.chars().count() > 600 {
        text.chars().take(600).collect()
    } else {
        text
    }
}

// ---- Kiểm tra (luồng của hộp «Có bản mới», như MainWindow.CheckUpdate bản Windows) ----

/// Có nên tự kiểm lúc này không (bật tự cập nhật và đã quá 2 giờ từ lần trước); bấm tay thì luôn kiểm.
pub fn should_check(manual: bool) -> bool {
    manual || (crate::prefs::auto_update() && now_ms() - crate::prefs::last_update_check() >= CHECK_EVERY)
}

/// Kiểm bản mới: Some = có bản mới hơn cần hỏi người dùng (tự kiểm thì bỏ bản đã «Để sau»). Ghi lại lúc kiểm.
pub async fn check(manual: bool) -> Result<Option<Release>> {
    if !should_check(manual) {
        return Ok(None);
    }
    let rel = latest(None).await?;
    crate::prefs::set_last_update_check(now_ms());
    let Some(rel) = rel else { return Ok(None) };
    if !is_newer(&rel.version, None) {
        return Ok(None);
    }
    if !manual && crate::prefs::is_update_snoozed(&rel.version) {
        return Ok(None);
    }
    Ok(Some(rel))
}

/// «Để sau»: không hỏi lại bản này trong 24 giờ.
pub fn snooze(rel: &Release) {
    crate::prefs::snooze_update(&rel.version, now_ms() + SNOOZE);
}

/// "Đang dùng bản mới nhất (1.0)." — bấm kiểm tay mà không có bản mới.
pub fn up_to_date_text() -> String {
    format!("Đang dùng bản mới nhất ({}).", current_version())
}

pub fn dialog_title(rel: &Release) -> String {
    format!("Có bản mới: Côn Bằng TV {}", rel.version)
}

pub fn dialog_text(rel: &Release) -> String {
    let s = summary(&rel.notes);
    format!("{}Bản đang dùng: {}.", if s.is_empty() { String::new() } else { s + "\n\n" }, current_version())
}

// ---- Tải + cài ----

/// Thư mục tải bản cập nhật (~/.cache/conbangtv).
pub fn download_dir() -> PathBuf {
    dirs::cache_dir().map(|d| d.join("conbangtv")).unwrap_or_else(|| crate::prefs::data_dir().join("update"))
}

fn is_update_file(name: &str) -> bool {
    name.starts_with("ConBangTV-Linux-") && name.ends_with(".deb")
}

/// Lúc mở app: dọn tệp .deb của lần cập nhật trước.
pub fn cleanup() {
    if let Ok(rd) = std::fs::read_dir(download_dir()) {
        for e in rd.flatten() {
            if is_update_file(&e.file_name().to_string_lossy()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Tải và kiểm bản mới (đúng cỡ, đúng mã SHA-256 GitHub báo, đúng gói + số hiệu trong tệp) → đường dẫn tệp đã tải.
/// `progress` nhận 0..1 (gọi từ luồng nền).
pub async fn download(rel: &Release, progress: impl Fn(f64) + Send + Sync) -> Result<PathBuf> {
    let dir = download_dir();
    tokio::fs::create_dir_all(&dir).await?;
    cleanup();
    let file = dir.join(format!("ConBangTV-Linux-{}.deb", rel.version));
    // Link github.com lỗi (11/2026: máy chủ GitHub vùng Đông Nam Á trả 504 hàng giờ liền trong khi API vẫn chạy) → tải cùng
    // tệp qua API (http::download gửi Accept: application/octet-stream). Tệp từ đường nào cũng phải khớp cỡ + SHA-256 + số bản.
    let sources: Vec<&str> = [Some(rel.deb_url.as_str()), rel.api_url.as_deref()].into_iter().flatten().collect();
    let mut last: Option<Error> = None;
    for attempt in 0..3 {
        if attempt > 0 {
            if last.is_none() {
                break;
            }
            // GitHub lỗi thường chỉ vài chục giây: nghỉ 5 s rồi 15 s trước khi thử lại cả hai đường
            tokio::time::sleep(Duration::from_secs(if attempt == 1 { 5 } else { 15 })).await;
        }
        for url in &sources {
            match crate::http::download(url, &file, |p| progress(p)).await {
                Ok(()) => {
                    last = None;
                    break;
                }
                Err(e) => {
                    crate::log::d("update", &format!("Tải lần {} ({url}) lỗi: {e}", attempt + 1));
                    last = Some(e);
                }
            }
        }
    }
    if let Some(e) = last {
        return Err(e);
    }
    let data = tokio::fs::read(&file).await?;
    if rel.size > 0 && data.len() as i64 != rel.size {
        return Err(Error::app("Tải bản cập nhật không đủ (thiếu dữ liệu) — thử lại sau."));
    }
    if let Some(sha) = &rel.sha256 {
        let hash: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
        if &hash != sha {
            return Err(Error::app("Bản cập nhật tải về bị hỏng (sai mã kiểm tra) — thử lại sau."));
        }
    }
    drop(data);
    let f = file.clone();
    let out = tokio::task::spawn_blocking(move || {
        Command::new("dpkg-deb").arg("-f").arg(&f).args(["Package", "Version", "Architecture"]).stdin(Stdio::null()).output()
    })
    .await
    .map_err(|e| Error::app(e.to_string()))?;
    let out = match out {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::app(format!("Máy này không có dpkg (không phải Ubuntu / Debian) — tải bản mới ở {RELEASES_PAGE}.")))
        }
        Err(e) => return Err(e.into()),
    };
    if !out.status.success() {
        return Err(Error::app(format!("Tệp tải về không phải bản Côn Bằng TV {}.", rel.version)));
    }
    check_deb_fields(&String::from_utf8_lossy(&out.stdout), &rel.version)?;
    Ok(file)
}

/// Kiểm thông tin gói (`dpkg-deb -f <tệp> Package Version Architecture`): đúng gói conbangtv, đúng số bản, đúng kiến trúc.
pub fn check_deb_fields(output: &str, version: &str) -> Result<()> {
    let field = |name: &str| -> String {
        output
            .lines()
            .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_string()))
            .unwrap_or_default()
    };
    let (pkg, ver, arch) = (field("Package"), field("Version"), field("Architecture"));
    if pkg != PACKAGE || ver.is_empty() || is_newer(&ver, Some(version)) || is_newer(version, Some(&ver)) {
        return Err(Error::app(format!("Tệp tải về không phải bản Côn Bằng TV {version}.")));
    }
    if !arch.is_empty() && arch != "all" && arch != deb_arch() {
        return Err(Error::app(format!("Tệp tải về dành cho máy {arch}, không cài được trên máy này ({}).", deb_arch())));
    }
    Ok(())
}

/// Cài tệp .deb đã tải: `pkexec apt-get install -y <tệp>` (hệ thống hỏi mật khẩu); không được thì `pkexec dpkg -i <tệp>`.
pub async fn install(file: &Path) -> Result<Installed> {
    crate::prefs::flush();
    let f = file.to_path_buf();
    tokio::task::spawn_blocking(move || install_blocking(&f)).await.map_err(|e| Error::app(e.to_string()))?
}

fn install_blocking(file: &Path) -> Result<Installed> {
    let run = |args: &[&str]| -> std::io::Result<std::process::Output> {
        Command::new("pkexec").args(args).arg(file).stdin(Stdio::null()).output()
    };
    let mut last = String::new();
    for args in [&["apt-get", "install", "-y"][..], &["dpkg", "-i"][..]] {
        let out = match run(args) {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::app(format!(
                    "Máy này không có pkexec — cài bằng tay: sudo apt install {}",
                    file.display()
                )))
            }
            Err(e) => return Err(e.into()),
        };
        let code = out.status.code().unwrap_or(-1);
        let err = String::from_utf8_lossy(&out.stderr);
        crate::log::d("update", &format!("pkexec {} → mã {code} {}", args.join(" "), tail(&err)));
        match code {
            0 => return Ok(Installed::Done),
            // pkexec: 126 = người dùng đóng hộp hỏi mật khẩu
            126 => return Ok(Installed::Cancelled),
            _ => last = format!("mã {code}{}", if err.trim().is_empty() { String::new() } else { format!(": {}", tail(&err)) }),
        }
    }
    Err(Error::app(format!("Không cài được bản mới ({last}).")))
}

/// Dòng cuối (khác rỗng) của lời báo lỗi.
fn tail(s: &str) -> String {
    s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
}

/// Mở lại app bằng bản vừa cài (`--updated`), tách hẳn khỏi app đang chạy; giao diện thoát ngay sau đó. Chờ 1 giây cho app
/// cũ đóng cổng đồng bộ.
#[cfg(unix)]
pub fn relaunch() -> Result<()> {
    use std::os::unix::process::CommandExt;
    crate::prefs::flush();
    Command::new("/bin/sh")
        .args(["-c", "sleep 1; exec \"$0\" --updated", INSTALLED_BIN])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    Ok(())
}

/// Bản Windows tự cập nhật bằng UpdateCheck.cs của app WPF (tải .exe) — lõi không mở lại app.
#[cfg(windows)]
pub fn relaunch() -> Result<()> {
    Err(Error::app("Bản Windows mở lại app ở phần giao diện."))
}

/// App cài bằng gói .deb (tự cập nhật được); chạy từ thư mục dựng / AppImage… thì mở trang Releases để tải tay.
pub fn can_self_update() -> bool {
    Path::new("/var/lib/dpkg/info/conbangtv.list").exists() && Path::new(INSTALLED_BIN).exists() && which("pkexec")
}

fn which(prog: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(prog).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(tag: &str, assets: &str, extra: &str) -> String {
        format!(r#"{{"tag_name":"{tag}","draft":false,"prerelease":false,"body":"b {tag}","assets":[{assets}]{extra}}}"#)
    }

    fn asset(name: &str) -> String {
        format!(
            r#"{{"name":"{name}","size":1234,"digest":"sha256:ABCDEF","url":"https://api.github.com/repos/x/y/releases/assets/9","browser_download_url":"https://github.com/x/y/releases/download/t/{name}"}}"#
        )
    }

    #[test]
    fn picks_only_linux_releases_with_deb() {
        let json = format!(
            "[{},{},{},{},{},{},{},{}]",
            rel("v2.17", &asset("ConBangTV.apk"), ""),
            rel("win-v1.15", &asset("ConBangTV-Windows-1.15.exe"), ""),
            rel("ios-v1.17", &asset("ConBangTV-iPad-1.17.ipa"), ""),
            rel("mac-v1.17", &asset("ConBangTV-Mac-1.17.zip"), ""),
            rel("linux-v1.2", &asset("ConBangTV-Linux-1.2.deb"), ""),
            rel("linux-v1.10", &asset("ConBangTV-Linux-1.10.deb"), ""),
            // không có .deb, bản nháp, bản thử: bỏ
            rel("linux-v9.0", &asset("ConBangTV-Linux-9.0.tar.gz"), ""),
            rel("linux-v8.0", &asset("ConBangTV-Linux-8.0.deb"), r#","draft":true"#).replace(r#""draft":false,"#, ""),
        );
        let r = pick_latest(json.as_bytes()).unwrap();
        assert_eq!(r.version, "1.10");
        assert_eq!(r.tag, "linux-v1.10");
        assert_eq!(r.deb_url, "https://github.com/x/y/releases/download/t/ConBangTV-Linux-1.10.deb");
        assert_eq!(r.sha256.as_deref(), Some("abcdef"));
        assert_eq!(r.size, 1234);
        assert_eq!(r.api_url.as_deref(), Some("https://api.github.com/repos/x/y/releases/assets/9"));
        assert_eq!(r.notes, "b linux-v1.10");

        let pre = format!("[{}]", rel("linux-v3.0", &asset("a.deb"), "").replace(r#""prerelease":false"#, r#""prerelease":true"#));
        assert_eq!(pick_latest(pre.as_bytes()), None);
        let others = format!("[{},{}]", rel("v2.17", &asset("ConBangTV.apk"), ""), rel("win-v1.15", &asset("x.deb"), ""));
        assert_eq!(pick_latest(others.as_bytes()), None);
        assert_eq!(pick_latest(b"{\"message\":\"API rate limit\"}"), None);
        assert_eq!(pick_latest(b"<html>"), None);
    }

    #[test]
    fn prefers_deb_for_this_arch() {
        let other = if deb_arch() == "arm64" { "amd64" } else { "arm64" };
        let assets = format!("{},{}", asset(&format!("ConBangTV-Linux-1.0_{other}.deb")), asset(&format!("ConBangTV-Linux-1.0_{}.deb", deb_arch())));
        let r = pick_latest(format!("[{}]", rel("linux-v1.0", &assets, "")).as_bytes()).unwrap();
        assert!(r.deb_url.ends_with(&format!("_{}.deb", deb_arch())));
        // không có digest / url API
        let bare = r#"[{"tag_name":"linux-v1.1","assets":[{"name":"a.DEB","browser_download_url":"https://x/a.DEB"}]}]"#;
        let r = pick_latest(bare.as_bytes()).unwrap();
        assert_eq!((r.sha256, r.api_url, r.size), (None, None, 0));
    }

    #[test]
    fn versions() {
        assert!(is_newer("1.10", Some("1.9")));
        assert!(is_newer("1.0.1", Some("1.0")));
        assert!(!is_newer("1.0", Some("1.0.0")));
        assert!(!is_newer("1.0.0-1", Some("1.0")));
        assert!(!is_newer("0.9", Some("1.0")));
        assert_eq!(short_version("1.0.0"), "1.0");
        assert_eq!(short_version("1.2.3"), "1.2.3");
        assert!(!is_newer(&current_version(), None));
    }

    #[test]
    fn summary_takes_whats_new_section() {
        let notes = "# Côn Bằng TV cho Linux 1.1\r\n\r\n## Có gì mới\r\n- **Đồng bộ** trong nhà\r\n* Sửa `lỗi` phát\r\n![ảnh](x.png)\r\n| a | b |\r\nSHA-256: abc\r\n## Cài đặt\r\n- tải .deb";
        assert_eq!(summary(notes), "Đồng bộ trong nhà\nSửa lỗi phát");
        assert_eq!(summary("- một\n- hai"), "một\nhai");
        assert_eq!(summary(&"á".repeat(700)).chars().count(), 600);
    }

    #[test]
    fn deb_fields() {
        let ok = format!("Package: conbangtv\nVersion: 1.1.0\nArchitecture: {}\n", deb_arch());
        assert!(check_deb_fields(&ok, "1.1").is_ok());
        assert!(check_deb_fields("Package: conbangtv\nVersion: 1.1\nArchitecture: all\n", "1.1").is_ok());
        assert!(check_deb_fields("Package: conbangtv\nVersion: 1.0\n", "1.1").is_err());
        assert!(check_deb_fields("Package: khac\nVersion: 1.1\n", "1.1").is_err());
        assert!(check_deb_fields("Package: conbangtv\nVersion: 1.1\nArchitecture: s390x\n", "1.1").is_err());
        assert!(check_deb_fields("", "1.1").is_err());
    }

    #[test]
    fn texts() {
        let r = Release {
            version: "1.1".into(),
            deb_url: String::new(),
            notes: "## Có gì mới\n- Mới".into(),
            tag: "linux-v1.1".into(),
            sha256: None,
            size: 0,
            api_url: None,
        };
        assert_eq!(dialog_title(&r), "Có bản mới: Côn Bằng TV 1.1");
        assert_eq!(dialog_text(&r), format!("Mới\n\nBản đang dùng: {}.", current_version()));
        assert!(is_update_file("ConBangTV-Linux-1.1.deb"));
        assert!(!is_update_file("anh.jpg"));
    }
}
