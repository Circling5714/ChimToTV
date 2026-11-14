//! libmpv nạp lúc chạy (dlopen), không cần lúc dựng: Ubuntu 22.04 / Mint 21 có libmpv.so.1 (mpv 0.34), Ubuntu 24.04+ /
//! Debian 12 có libmpv.so.2 — các hàm dùng ở đây (client.h + render.h) giống nhau ở cả hai. Không có libmpv thì app phát
//! bằng WebView như cũ.

use std::ffi::{c_char, c_double, c_int, c_void, CStr, CString};
use std::sync::OnceLock;

pub type Handle = *mut c_void;
pub type RenderCtx = *mut c_void;
pub type Callback = unsafe extern "C" fn(*mut c_void);

// mpv_format
pub const FORMAT_STRING: c_int = 1;
pub const FORMAT_FLAG: c_int = 3;
pub const FORMAT_INT64: c_int = 4;
pub const FORMAT_DOUBLE: c_int = 5;

// mpv_event_id
pub const EVENT_NONE: c_int = 0;
pub const EVENT_SHUTDOWN: c_int = 1;
pub const EVENT_LOG_MESSAGE: c_int = 2;
pub const EVENT_START_FILE: c_int = 6;
pub const EVENT_END_FILE: c_int = 7;
pub const EVENT_FILE_LOADED: c_int = 8;
pub const EVENT_PLAYBACK_RESTART: c_int = 21;

// mpv_end_file_reason (0 = hết tệp — keep-open=yes nên không gặp, 2 = dừng / thay tệp)
pub const END_ERROR: c_int = 4;

// mpv_render_param_type
pub const RP_INVALID: c_int = 0;
pub const RP_API_TYPE: c_int = 1;
pub const RP_OPENGL_INIT_PARAMS: c_int = 2;
pub const RP_OPENGL_FBO: c_int = 3;
pub const RP_FLIP_Y: c_int = 4;
pub const RP_X11_DISPLAY: c_int = 8;
pub const RP_WL_DISPLAY: c_int = 9;
pub const RP_SW_SIZE: c_int = 17;
pub const RP_SW_FORMAT: c_int = 18;
pub const RP_SW_STRIDE: c_int = 19;
pub const RP_SW_POINTER: c_int = 20;
pub const RENDER_UPDATE_FRAME: u64 = 1;

#[repr(C)]
pub struct Event {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct EventEndFile {
    pub reason: c_int,
    pub error: c_int,
}

#[repr(C)]
pub struct EventLogMessage {
    pub prefix: *const c_char,
    pub level: *const c_char,
    pub text: *const c_char,
    pub log_level: c_int,
}

#[repr(C)]
pub struct RenderParam {
    pub kind: c_int,
    pub data: *mut c_void,
}

/// mpv_opengl_init_params. libmpv.so.1 còn trường thứ ba (extra_exts, bỏ qua, phải NULL); libmpv.so.2 không đọc tới nó.
#[repr(C)]
pub struct GlInitParams {
    pub get_proc_address: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
    pub get_proc_address_ctx: *mut c_void,
    pub extra_exts: *const c_char,
}

#[repr(C)]
pub struct GlFbo {
    pub fbo: c_int,
    pub w: c_int,
    pub h: c_int,
    pub internal_format: c_int,
}

pub struct Lib {
    pub soname: &'static str,
    pub create: unsafe extern "C" fn() -> Handle,
    pub initialize: unsafe extern "C" fn(Handle) -> c_int,
    pub terminate_destroy: unsafe extern "C" fn(Handle),
    pub set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    pub set_property: unsafe extern "C" fn(Handle, *const c_char, c_int, *mut c_void) -> c_int,
    pub get_property: unsafe extern "C" fn(Handle, *const c_char, c_int, *mut c_void) -> c_int,
    pub command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    pub wait_event: unsafe extern "C" fn(Handle, c_double) -> *mut Event,
    pub set_wakeup_callback: unsafe extern "C" fn(Handle, Option<Callback>, *mut c_void),
    pub request_log_messages: unsafe extern "C" fn(Handle, *const c_char) -> c_int,
    pub error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    pub free: unsafe extern "C" fn(*mut c_void),
    pub rc_create: unsafe extern "C" fn(*mut RenderCtx, Handle, *mut RenderParam) -> c_int,
    pub rc_set_update_callback: unsafe extern "C" fn(RenderCtx, Option<Callback>, *mut c_void),
    pub rc_update: unsafe extern "C" fn(RenderCtx) -> u64,
    pub rc_render: unsafe extern "C" fn(RenderCtx, *mut RenderParam) -> c_int,
    pub rc_free: unsafe extern "C" fn(RenderCtx),
}

// SAFETY: chỉ là bảng con trỏ hàm của thư viện đã nạp (không bao giờ dlclose)
unsafe impl Send for Lib {}
unsafe impl Sync for Lib {}

unsafe fn sym<T: Copy>(h: *mut c_void, name: &str) -> Option<T> {
    let c = CString::new(name).ok()?;
    let p = libc::dlsym(h, c.as_ptr());
    if p.is_null() {
        None
    } else {
        Some(std::mem::transmute_copy::<*mut c_void, T>(&p))
    }
}

/// Thư viện libmpv (nạp một lần). None = máy không có libmpv.
pub fn lib() -> Option<&'static Lib> {
    static LIB: OnceLock<Option<Lib>> = OnceLock::new();
    LIB.get_or_init(|| {
        for soname in ["libmpv.so.2", "libmpv.so.1", "libmpv.so"] {
            let c = CString::new(soname).unwrap();
            // SAFETY: nạp thư viện hệ thống; các ký hiệu đọc theo đúng khai báo của client.h / render.h
            unsafe {
                let h = libc::dlopen(c.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
                if h.is_null() {
                    continue;
                }
                let l = (|| {
                    Some(Lib {
                        soname,
                        create: sym(h, "mpv_create")?,
                        initialize: sym(h, "mpv_initialize")?,
                        terminate_destroy: sym(h, "mpv_terminate_destroy")?,
                        set_option_string: sym(h, "mpv_set_option_string")?,
                        set_property: sym(h, "mpv_set_property")?,
                        get_property: sym(h, "mpv_get_property")?,
                        command: sym(h, "mpv_command")?,
                        wait_event: sym(h, "mpv_wait_event")?,
                        set_wakeup_callback: sym(h, "mpv_set_wakeup_callback")?,
                        request_log_messages: sym(h, "mpv_request_log_messages")?,
                        error_string: sym(h, "mpv_error_string")?,
                        free: sym(h, "mpv_free")?,
                        rc_create: sym(h, "mpv_render_context_create")?,
                        rc_set_update_callback: sym(h, "mpv_render_context_set_update_callback")?,
                        rc_update: sym(h, "mpv_render_context_update")?,
                        rc_render: sym(h, "mpv_render_context_render")?,
                        rc_free: sym(h, "mpv_render_context_free")?,
                    })
                })();
                match l {
                    Some(l) => {
                        cbtv::log::d("mpv", &format!("Đã nạp {soname}"));
                        return Some(l);
                    }
                    None => cbtv::log::d("mpv", &format!("{soname} thiếu hàm cần dùng — bỏ qua")),
                }
            }
        }
        cbtv::log::d("mpv", "Máy không có libmpv — phát bằng WebView");
        None
    })
    .as_ref()
}

/// Một trình phát mpv (vo=libmpv: app tự vẽ hình qua render API).
pub struct Mpv {
    pub l: &'static Lib,
    pub h: Handle,
}

impl Mpv {
    /// Tạo + khởi động với các tuỳ chọn (tên, giá trị). None khi libmpv không có / không khởi động được.
    pub fn new(opts: &[(&str, &str)]) -> Option<Mpv> {
        let l = lib()?;
        // libmpv từ chối chạy khi LC_NUMERIC không phải "C" (GTK đã đặt theo ngôn ngữ máy, vd vi_VN dùng dấu phẩy thập phân)
        // SAFETY: đổi một mục locale của tiến trình; GTK không dựa vào LC_NUMERIC
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }
        // SAFETY: gọi hàm libmpv theo đúng chữ ký; h được giải phóng trong Drop
        unsafe {
            let h = (l.create)();
            if h.is_null() {
                cbtv::log::d("mpv", "mpv_create lỗi");
                return None;
            }
            let m = Mpv { l, h };
            for (k, v) in opts {
                let r = m.set_option(k, v);
                if r < 0 {
                    cbtv::log::d("mpv", &format!("tuỳ chọn {k}={v}: {}", m.err(r)));
                }
            }
            let r = (l.initialize)(h);
            if r < 0 {
                cbtv::log::d("mpv", &format!("mpv_initialize lỗi: {}", m.err(r)));
                return None;
            }
            Some(m)
        }
    }

    pub fn err(&self, code: c_int) -> String {
        // SAFETY: mpv_error_string trả chuỗi tĩnh
        unsafe { CStr::from_ptr((self.l.error_string)(code)).to_string_lossy().into_owned() }
    }

    pub fn set_option(&self, k: &str, v: &str) -> c_int {
        let (k, v) = (CString::new(k).unwrap(), CString::new(v).unwrap());
        // SAFETY: chuỗi C hợp lệ trong lúc gọi
        unsafe { (self.l.set_option_string)(self.h, k.as_ptr(), v.as_ptr()) }
    }

    pub fn command(&self, args: &[&str]) -> c_int {
        let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
        let mut ps: Vec<*const c_char> = cs.iter().map(|c| c.as_ptr()).collect();
        ps.push(std::ptr::null());
        // SAFETY: mảng chuỗi C kết thúc bằng NULL, sống tới hết lệnh
        let r = unsafe { (self.l.command)(self.h, ps.as_mut_ptr()) };
        if r < 0 {
            cbtv::log::d("mpv", &format!("lệnh {:?}: {}", args.first(), self.err(r)));
        }
        r
    }

    pub fn set_str(&self, k: &str, v: &str) -> c_int {
        let (k, v) = (CString::new(k).unwrap(), CString::new(v).unwrap());
        let mut p = v.as_ptr();
        // SAFETY: MPV_FORMAT_STRING nhận con trỏ tới char*
        unsafe { (self.l.set_property)(self.h, k.as_ptr(), FORMAT_STRING, &mut p as *mut *const c_char as *mut c_void) }
    }

    pub fn set_flag(&self, k: &str, v: bool) -> c_int {
        let k = CString::new(k).unwrap();
        let mut f: c_int = v as c_int;
        // SAFETY: MPV_FORMAT_FLAG nhận int*
        unsafe { (self.l.set_property)(self.h, k.as_ptr(), FORMAT_FLAG, &mut f as *mut c_int as *mut c_void) }
    }

    pub fn set_double(&self, k: &str, v: f64) -> c_int {
        let k = CString::new(k).unwrap();
        let mut d: c_double = v;
        // SAFETY: MPV_FORMAT_DOUBLE nhận double*
        unsafe { (self.l.set_property)(self.h, k.as_ptr(), FORMAT_DOUBLE, &mut d as *mut c_double as *mut c_void) }
    }

    pub fn get_double(&self, k: &str) -> Option<f64> {
        let k = CString::new(k).unwrap();
        let mut d: c_double = 0.0;
        // SAFETY: như trên
        let r = unsafe { (self.l.get_property)(self.h, k.as_ptr(), FORMAT_DOUBLE, &mut d as *mut c_double as *mut c_void) };
        (r >= 0).then_some(d)
    }

    pub fn get_flag(&self, k: &str) -> Option<bool> {
        let k = CString::new(k).unwrap();
        let mut f: c_int = 0;
        // SAFETY: như trên
        let r = unsafe { (self.l.get_property)(self.h, k.as_ptr(), FORMAT_FLAG, &mut f as *mut c_int as *mut c_void) };
        (r >= 0).then_some(f != 0)
    }

    pub fn get_i64(&self, k: &str) -> Option<i64> {
        let k = CString::new(k).unwrap();
        let mut v: i64 = 0;
        // SAFETY: như trên
        let r = unsafe { (self.l.get_property)(self.h, k.as_ptr(), FORMAT_INT64, &mut v as *mut i64 as *mut c_void) };
        (r >= 0).then_some(v)
    }

    pub fn get_str(&self, k: &str) -> Option<String> {
        let k = CString::new(k).unwrap();
        let mut p: *mut c_char = std::ptr::null_mut();
        // SAFETY: mpv cấp chuỗi, giải phóng bằng mpv_free
        unsafe {
            let r = (self.l.get_property)(self.h, k.as_ptr(), FORMAT_STRING, &mut p as *mut *mut c_char as *mut c_void);
            if r < 0 || p.is_null() {
                return None;
            }
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            (self.l.free)(p as *mut c_void);
            Some(s)
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: h hợp lệ tới đây; render context (nếu có) phải được giải phóng trước — native.rs lo việc đó
        unsafe {
            (self.l.set_wakeup_callback)(self.h, None, std::ptr::null_mut());
            (self.l.terminate_destroy)(self.h);
        }
    }
}

/// Đọc chuỗi C (NULL → "").
pub fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        // SAFETY: chuỗi C do libmpv cấp, còn sống trong lúc xử lý sự kiện
        unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() }
    }
}
