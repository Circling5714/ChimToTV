//! Tải ảnh bìa: bộ nhớ (400 ảnh gần nhất) → ổ đĩa (~/.cache/conbangtv/img, giữ 14 ngày) → mạng (6 ảnh cùng lúc, qua cổng
//! nội bộ như mọi kết nối). Giải mã bằng crate image trên luồng nền (WebP cũng được), thu về đúng cỡ hiện để đỡ tốn RAM.
//! Bản Windows: UI/ImageLoader.cs.

use gdk_pixbuf::Pixbuf;
use sha1::{Digest, Sha1};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::OnceLock;

type Cb = Box<dyn FnOnce(Option<Pixbuf>)>;

thread_local! {
    static MEMORY: RefCell<HashMap<String, Pixbuf>> = RefCell::new(HashMap::new());
    static ORDER: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
    static PENDING: RefCell<HashMap<String, Vec<Cb>>> = RefCell::new(HashMap::new());
}

const MAX_MEMORY: usize = 400;

fn dir() -> PathBuf {
    dirs_cache().join("img")
}

pub fn dirs_cache() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("conbangtv")
}

fn gate() -> &'static tokio::sync::Semaphore {
    static G: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    G.get_or_init(|| tokio::sync::Semaphore::new(6))
}

/// Dọn ảnh cũ trên ổ đĩa (lúc mở app, chạy nền).
pub fn cleanup() {
    std::thread::spawn(|| {
        let Ok(rd) = std::fs::read_dir(dir()) else { return };
        let cut = std::time::SystemTime::now() - std::time::Duration::from_secs(14 * 86400);
        for e in rd.flatten() {
            if let Ok(m) = e.metadata() {
                if m.modified().map(|t| t < cut).unwrap_or(false) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    });
}

/// Xoá hết ảnh đã lưu (Cài đặt → xoá dữ liệu duyệt web).
pub fn clear_disk() {
    let _ = std::fs::remove_dir_all(dir());
    MEMORY.with(|m| m.borrow_mut().clear());
    ORDER.with(|o| o.borrow_mut().clear());
}

fn hash(url: &str) -> String {
    Sha1::digest(url.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Ảnh (đã cắt / thu đúng w×h kiểu «lấp đầy») — gọi cb trên luồng giao diện (None = lỗi).
pub fn load(url: &str, w: i32, h: i32, cb: impl FnOnce(Option<Pixbuf>) + 'static) {
    if url.is_empty() || w <= 0 || h <= 0 {
        cb(None);
        return;
    }
    let key = format!("{w}x{h}|{url}");
    if let Some(p) = MEMORY.with(|m| m.borrow().get(&key).cloned()) {
        ORDER.with(|o| {
            let mut o = o.borrow_mut();
            o.retain(|k| k != &key);
            o.push_front(key.clone());
        });
        cb(Some(p));
        return;
    }
    let first = PENDING.with(|p| {
        let mut p = p.borrow_mut();
        let list = p.entry(key.clone()).or_default();
        list.push(Box::new(cb));
        list.len() == 1
    });
    if !first {
        return;
    }
    let url = url.to_string();
    let k2 = key.clone();
    crate::ui::spawn(fetch(url, w as u32, h as u32), move |r: Option<(Vec<u8>, i32, i32)>| {
        let pix = r.map(|(data, pw, ph)| {
            Pixbuf::from_bytes(&glib::Bytes::from_owned(data), gdk_pixbuf::Colorspace::Rgb, true, 8, pw, ph, pw * 4)
        });
        if let Some(p) = &pix {
            MEMORY.with(|m| m.borrow_mut().insert(k2.clone(), p.clone()));
            ORDER.with(|o| {
                let mut o = o.borrow_mut();
                o.push_front(k2.clone());
                while o.len() > MAX_MEMORY {
                    if let Some(old) = o.pop_back() {
                        MEMORY.with(|m| m.borrow_mut().remove(&old));
                    }
                }
            });
        }
        let cbs = PENDING.with(|p| p.borrow_mut().remove(&k2)).unwrap_or_default();
        for c in cbs {
            c(pix.clone());
        }
    });
}

async fn fetch(url: String, w: u32, h: u32) -> Option<(Vec<u8>, i32, i32)> {
    let file = dir().join(hash(&url));
    let bytes = match tokio::fs::read(&file).await {
        Ok(b) if b.len() > 12 => b,
        _ => {
            let _permit = gate().acquire().await.ok()?;
            let referer = format!("{}/", cbtv::prefs::base_url());
            let r = cbtv::http::get(
                &url,
                &[("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8"), ("Referer", referer.as_str())],
            )
            .await
            .ok()?;
            let _ = tokio::fs::create_dir_all(dir()).await;
            let _ = tokio::fs::write(&file, &r.data).await;
            r.data
        }
    };
    tokio::task::spawn_blocking(move || decode_cover(&bytes, w, h)).await.ok().flatten()
}

/// Giải mã + cắt giữa cho vừa khung w×h (như Stretch="UniformToFill"); trả RGBA.
pub fn decode_cover(bytes: &[u8], w: u32, h: u32) -> Option<(Vec<u8>, i32, i32)> {
    let img = image::load_from_memory(bytes).ok()?;
    let (iw, ih) = (img.width().max(1), img.height().max(1));
    let s = (w as f64 / iw as f64).max(h as f64 / ih as f64);
    let (sw, sh) = (((iw as f64) * s).ceil().max(1.0) as u32, ((ih as f64) * s).ceil().max(1.0) as u32);
    let resized = img.resize_exact(sw, sh, image::imageops::FilterType::Triangle);
    let x = (sw.saturating_sub(w)) / 2;
    let y = (sh.saturating_sub(h)) / 2;
    let cropped = resized.crop_imm(x, y, w.min(sw), h.min(sh)).to_rgba8();
    let (cw, ch) = (cropped.width() as i32, cropped.height() as i32);
    Some((cropped.into_raw(), cw, ch))
}
