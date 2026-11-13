//! Bí mật (mật khẩu proxy, khoá Shadowsocks): tệp JSON riêng `secret.json` quyền 600 trong thư mục dữ liệu, không nằm
//! trong prefs.json (bản Windows mã hoá bằng DPAPI, iPad dùng Keychain, Mac cũng là tệp 600). Không dùng kho khoá GNOME:
//! máy tự đăng nhập thì kho khoá bị khoá, app sẽ hỏi mật khẩu.

use serde_json::{Map, Value};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

fn file() -> std::path::PathBuf {
    crate::prefs::data_dir().join("secret.json")
}

fn read() -> Map<String, Value> {
    std::fs::read(file()).ok().and_then(|b| serde_json::from_slice::<Map<String, Value>>(&b).ok()).unwrap_or_default()
}

pub fn get(key: &str) -> String {
    let _g = LOCK.lock().unwrap();
    read().get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

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
pub fn works() -> bool {
    set("probe", "1");
    let ok = get("probe") == "1";
    set("probe", "");
    ok
}
