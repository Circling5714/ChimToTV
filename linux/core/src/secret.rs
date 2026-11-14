//! Bí mật (mật khẩu proxy, khoá Shadowsocks): tệp JSON riêng `secret.json` quyền 600 trong thư mục dữ liệu, không nằm
//! trong prefs.json (bản Windows mã hoá bằng DPAPI, iPad dùng Keychain, Mac cũng là tệp 600). Không dùng kho khoá GNOME:
//! máy tự đăng nhập thì kho khoá bị khoá, app sẽ hỏi mật khẩu.

//! Bản Windows (lõi trong conbang_core.dll): DPAPI như Secret.cs của bản C# — xem `mod dpapi` cuối tệp.

#[cfg(windows)]
pub use dpapi::{get, set, works};

#[cfg(not(windows))]
use serde_json::{Map, Value};
#[cfg(not(windows))]
use std::io::Write;
#[cfg(not(windows))]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
#[cfg(not(windows))]
use std::sync::Mutex;

#[cfg(not(windows))]
static LOCK: Mutex<()> = Mutex::new(());

#[cfg(not(windows))]
fn file() -> std::path::PathBuf {
    crate::prefs::data_dir().join("secret.json")
}

#[cfg(not(windows))]
fn read() -> Map<String, Value> {
    std::fs::read(file()).ok().and_then(|b| serde_json::from_slice::<Map<String, Value>>(&b).ok()).unwrap_or_default()
}

#[cfg(not(windows))]
pub fn get(key: &str) -> String {
    let _g = LOCK.lock().unwrap();
    read().get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

#[cfg(not(windows))]
pub fn set(key: &str, value: &str) {
    let _g = LOCK.lock().unwrap();
    let mut m = read();
    if value.is_empty() {
        m.remove(key);
    } else {
        m.insert(key.to_string(), Value::String(value.to_string()));
    }
    let f = file();
    if let Some(dir) = f.parent() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let tmp = f.with_extension("json.tmp");
    let ok = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .and_then(|mut h| h.write_all(serde_json::to_string(&m).unwrap_or_default().as_bytes()));
    if ok.is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    }
}

/// Lưu / đọc thử được không (để Cài đặt báo khi thư mục dữ liệu không ghi được).
#[cfg(not(windows))]
pub fn works() -> bool {
    set("probe", "1");
    let ok = get("probe") == "1";
    set("probe", "");
    ok
}

/// Windows: mã hoá bằng DPAPI (chỉ tài khoản Windows này trên máy này giải được) — đúng cách của Secret.cs bản C# (mô tả
/// «ConBangTV», không entropy, cấm hiện hộp thoại), lưu base64 trong prefs.json ở khoá «<tên>_dpapi» (proxy_pass_dpapi,
/// ss_key_dpapi) — bản lõi Rust đọc được mật khẩu bản C# đã lưu và ngược lại.
#[cfg(windows)]
mod dpapi {
    use base64::Engine;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};

    fn run(data: &[u8], protect: bool) -> Option<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let desc: Vec<u16> = "ConBangTV\0".encode_utf16().collect();
        // SAFETY: input trỏ vào data còn sống; DPAPI cấp out (giải phóng bằng LocalFree bên dưới)
        let ok = unsafe {
            if protect {
                CryptProtectData(&input, desc.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            } else {
                CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            }
        };
        if ok == 0 || out.pbData.is_null() {
            return None;
        }
        // SAFETY: out.pbData có out.cbData byte do DPAPI cấp
        let v = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
        // SAFETY: bộ nhớ do DPAPI cấp bằng LocalAlloc
        unsafe { LocalFree(out.pbData as _) };
        Some(v)
    }

    fn key(name: &str) -> String {
        format!("{name}_dpapi")
    }

    pub fn get(name: &str) -> String {
        let stored: String = crate::prefs::get(&key(name)).unwrap_or_default();
        if stored.is_empty() {
            return String::new();
        }
        base64::engine::general_purpose::STANDARD
            .decode(stored.trim())
            .ok()
            .and_then(|b| run(&b, false))
            .and_then(|p| String::from_utf8(p).ok())
            .unwrap_or_default()
    }

    pub fn set(name: &str, value: &str) {
        let stored = if value.is_empty() {
            String::new()
        } else {
            run(value.as_bytes(), true).map(|b| base64::engine::general_purpose::STANDARD.encode(b)).unwrap_or_default()
        };
        crate::prefs::set(&key(name), Some(stored));
    }

    pub fn works() -> bool {
        run(b"probe", true).and_then(|p| run(&p, false)).as_deref() == Some(&b"probe"[..])
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn dpapi_round_trip() {
            let p = super::run("mật khẩu ✓".as_bytes(), true).unwrap();
            assert_ne!(p, "mật khẩu ✓".as_bytes());
            assert_eq!(super::run(&p, false).unwrap(), "mật khẩu ✓".as_bytes());
            assert!(super::works());
        }
    }
}
