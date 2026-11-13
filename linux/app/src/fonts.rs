//! Font chữ của app: Inter 4.1 — trong các font mở, gần giống San Francisco (font của iOS / iPad / Mac) nhất; đủ dấu tiếng Việt.
//! SF của Apple không được dùng ngoài máy Apple nên không nhúng được. Giấy phép Inter: SIL Open Font License 1.1 (fonts/OFL.txt,
//! được nhúng lại / phát hành kèm phần mềm).
//! Như iOS: chữ thường dùng Inter (≈ SF Pro Text), tiêu đề cỡ lớn dùng Inter Display (≈ SF Pro Display) — css.rs chọn theo cỡ.
//! Font nằm sẵn trong tệp chạy (máy nào cũng hiện giống nhau, không cần gói font của hệ thống); lúc mở app chép ra
//! ~/.cache/conbangtv/fonts-inter-4.1 (fontconfig chỉ đọc font từ tệp) rồi thêm làm font riêng của app — không cài vào hệ thống.

use std::ffi::{c_char, c_int, c_void, CString};
use std::os::unix::ffi::OsStrExt;

const FONTS: &[(&str, &[u8])] = &[
    ("Inter-Regular.ttf", include_bytes!("../fonts/Inter-Regular.ttf")),
    ("Inter-Medium.ttf", include_bytes!("../fonts/Inter-Medium.ttf")),
    ("Inter-SemiBold.ttf", include_bytes!("../fonts/Inter-SemiBold.ttf")),
    ("Inter-Bold.ttf", include_bytes!("../fonts/Inter-Bold.ttf")),
    ("InterDisplay-SemiBold.ttf", include_bytes!("../fonts/InterDisplay-SemiBold.ttf")),
    ("InterDisplay-Bold.ttf", include_bytes!("../fonts/InterDisplay-Bold.ttf")),
];

/// Họ font cho chữ thường / tiêu đề lớn (máy thiếu font thì lùi về font hệ thống).
pub const TEXT: &str = "Inter, \"Ubuntu Sans\", Ubuntu, Cantarell, \"Noto Sans\", sans-serif";
pub const DISPLAY: &str = "\"Inter Display\", Inter, \"Ubuntu Sans\", Ubuntu, Cantarell, \"Noto Sans\", sans-serif";
/// Cho pango::FontDescription (danh sách cách nhau bằng dấu phẩy, không ngoặc kép).
pub const DISPLAY_PANGO: &str = "Inter Display,Inter,Ubuntu Sans,sans-serif";

#[link(name = "fontconfig")]
extern "C" {
    fn FcConfigAppFontAddDir(config: *mut c_void, dir: *const c_char) -> c_int;
}

/// Gọi trước khi tạo cửa sổ (trước khi Pango nạp danh sách font).
pub fn install() {
    let dir = crate::images::dirs_cache().join("fonts-inter-4.1");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        cbtv::log::d("ui", &format!("font: không tạo được thư mục font ({e}) — dùng font hệ thống"));
        return;
    }
    for (name, data) in FONTS {
        let p = dir.join(name);
        let same = std::fs::metadata(&p).map(|m| m.len() == data.len() as u64).unwrap_or(false);
        if !same {
            // Ghi tệp tạm rồi đổi tên: hai cửa sổ app mở cùng lúc không đọc phải tệp ghi dở
            let tmp = dir.join(format!(".{name}.{}", std::process::id()));
            if std::fs::write(&tmp, data).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
            let _ = std::fs::remove_file(&tmp);
        }
    }
    let Ok(c) = CString::new(dir.as_os_str().as_bytes()) else { return };
    // NULL = cấu hình fontconfig hiện tại (cái Pango sẽ dùng)
    let ok = unsafe { FcConfigAppFontAddDir(std::ptr::null_mut(), c.as_ptr()) };
    if ok == 0 {
        cbtv::log::d("ui", "font: fontconfig không nạp được Inter — dùng font hệ thống");
    }
}
