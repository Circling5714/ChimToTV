//! conbang_core.dll — lõi Rust (linux/core, crate `cbtv`) cho app WPF bản Windows: thay lõi C# windows/ConBangTV/Core, để
//! Linux và Windows dùng chung một lõi (đọc trang, trang chủ tổng hợp, cài đặt, mạng, đồng bộ, cổng phát nội bộ…).
//!
//! Giao tiếp bằng JSON (UTF-8), phía C#: windows/ConBangTV/Core/Native.cs.
//! - `cbtv_init(thư mục dữ liệu, callback)` — một lần lúc mở app.
//! - `cbtv_call(id, "nhóm.hàm", args)` — chạy nền (tokio); xong gọi `callback(id, {"ok": …} | {"err": "…"})`. Hàm trả từng
//!   phần (trang chủ tổng hợp) gọi thêm `callback(id, {"part": …})` trước kết quả cuối.
//! - `cbtv_call_sync("nhóm.hàm", args)` → chuỗi JSON như trên (giải phóng bằng `cbtv_free`) — hàm nhanh (cài đặt…).
//! - Sự kiện của lõi (đồng bộ xong, có thay đổi…): `callback(0, {"event": "…", "data": …})`.
//! Panic trong lõi không làm sập app: trả {"err": …}.

mod dispatch;

use serde_json::{json, Value};
use std::ffi::{c_char, CStr, CString};
use std::panic::AssertUnwindSafe;
use std::sync::OnceLock;

/// Hàm phía C# nhận kết quả / sự kiện (gọi từ luồng của lõi; chuỗi chỉ dùng được trong lúc gọi).
pub type Callback = extern "C" fn(id: u64, json: *const c_char);

static CALLBACK: OnceLock<Callback> = OnceLock::new();

pub(crate) type Res = Result<Value, String>;

/// Gửi kết quả / sự kiện sang C#.
pub(crate) fn emit(id: u64, v: &Value) {
    if let Some(cb) = CALLBACK.get() {
        let s = CString::new(v.to_string()).unwrap_or_else(|_| CString::new("{\"err\":\"chuỗi có ký tự 0\"}").unwrap());
        cb(id, s.as_ptr());
    }
}

/// Sự kiện của lõi (id 0).
pub(crate) fn event(name: &str, data: Value) {
    emit(0, &json!({ "event": name, "data": data }));
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: chuỗi C (UTF-8) do C# cấp, còn sống trong lúc gọi
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn envelope(r: std::thread::Result<Res>) -> Value {
    match r {
        Ok(Ok(v)) => json!({ "ok": v }),
        Ok(Err(e)) => json!({ "err": e }),
        Err(p) => {
            let m = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
            cbtv::log::d("ffi", &format!("panic: {m}"));
            json!({ "err": format!("Lỗi trong lõi app ({m})") })
        }
    }
}

/// Mở lõi: thư mục dữ liệu (%LOCALAPPDATA%\ConBangTV hoặc CONBANG_DATA_DIR) + hàm nhận kết quả. 0 = được.
#[no_mangle]
pub extern "C" fn cbtv_init(data_dir: *const c_char, cb: Callback) -> i32 {
    let dir = cstr(data_dir);
    let _ = CALLBACK.set(cb);
    let r = std::panic::catch_unwind(|| {
        cbtv::prefs::load(if dir.is_empty() { None } else { Some(std::path::PathBuf::from(&dir)) });
        cbtv::log::start();
        dispatch::init();
    });
    if r.is_ok() {
        0
    } else {
        1
    }
}

/// Gọi một hàm của lõi, chạy nền; kết quả qua callback(id, …).
#[no_mangle]
pub extern "C" fn cbtv_call(id: u64, method: *const c_char, args: *const c_char) {
    let m = cstr(method);
    let a: Value = serde_json::from_str(&cstr(args)).unwrap_or(Value::Null);
    cbtv::rt().spawn(async move {
        use futures::FutureExt;
        let r = AssertUnwindSafe(dispatch::call(id, &m, a)).catch_unwind().await;
        emit(id, &envelope(r));
    });
}

/// Gọi một hàm nhanh của lõi, chờ kết quả ngay (chuỗi JSON, giải phóng bằng cbtv_free).
#[no_mangle]
pub extern "C" fn cbtv_call_sync(method: *const c_char, args: *const c_char) -> *mut c_char {
    let m = cstr(method);
    let a: Value = serde_json::from_str(&cstr(args)).unwrap_or(Value::Null);
    let r = std::panic::catch_unwind(AssertUnwindSafe(|| dispatch::sync(&m, &a).unwrap_or_else(|| Err(format!("Lõi không có hàm «{m}»")))));
    CString::new(envelope(r).to_string()).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// Trả bộ nhớ của chuỗi cbtv_call_sync.
#[no_mangle]
pub extern "C" fn cbtv_free(p: *mut c_char) {
    if !p.is_null() {
        // SAFETY: p do cbtv_call_sync cấp bằng CString::into_raw
        unsafe { drop(CString::from_raw(p)) };
    }
}
