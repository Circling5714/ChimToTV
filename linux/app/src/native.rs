//! Trình phát native (thử nghiệm): libmpv vẽ hình thẳng vào ô của app (render API), thay WebView cho luồng .m3u8 / tệp video.
//! - Máy có GPU: GtkGLArea + render API OpenGL, giải mã bằng card đồ hoạ khi được (hwdec=auto-safe → VA-API).
//! - Máy không có GPU (máy ảo): render API phần mềm («sw») vẽ vào bộ nhớ đúng cỡ ô, cairo dán lên — OpenGL giả lập (llvmpipe)
//!   quá chậm (đo trên máy ảo thử: 378% CPU, rơi khung liên tục).
//! Mạng: mpv chỉ tải từ cổng phát nội bộ (cbtv::media_proxy) — DNS mã hoá, vượt chặn, proxy, bỏ quảng cáo như hls.html.
//! Trạng thái / lệnh dùng đúng giao thức của player-agent.js (t, d, paused, ended, waiting, ready, err; toggle, play, pause,
//! seekTo, rate, fit) để player.rs giữ nguyên mọi logic (xem tiếp, bỏ nhạc mở đầu, đổi server, giới hạn giờ…).

use crate::mpv::{self, GlFbo, GlInitParams, Mpv, RenderCtx, RenderParam};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_int, c_void, CString};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Trạng thái phát (giống window.__chimtoState của player-agent.js).
#[derive(Debug, Clone, Default)]
pub struct State {
    pub t: f64,
    pub d: f64,
    pub paused: bool,
    pub ended: bool,
    pub waiting: bool,
    pub ready: bool,
    pub err: Option<String>,
}

struct SwBuf {
    data: Vec<u8>,
    off: usize,
    w: i32,
    h: i32,
    stride: usize,
}

pub struct Native {
    pub widget: gtk::Widget,
    pub gl: bool,
    mpv: Mpv,
    ctx: Cell<RenderCtx>,
    sw: RefCell<SwBuf>,
    has_video: Cell<bool>,
    frame_dirty: Cell<bool>,
    loaded: Cell<bool>,
    restarted: Cell<bool>,
    err: RefCell<Option<String>>,
    pending_sub: RefCell<Option<String>>,
    log_lines: Cell<u32>,
}

thread_local! {
    static CUR: RefCell<Weak<Native>> = const { RefCell::new(Weak::new()) };
}
static WAKE: AtomicBool = AtomicBool::new(false);
static FRAME: AtomicBool = AtomicBool::new(false);

fn cur() -> Option<Rc<Native>> {
    CUR.with(|c| c.borrow().upgrade())
}

/// libmpv báo có sự kiện (gọi từ luồng của mpv) → xử lý ở luồng giao diện.
unsafe extern "C" fn on_wakeup(_: *mut c_void) {
    if !WAKE.swap(true, Ordering::SeqCst) {
        glib::idle_add_once(|| {
            WAKE.store(false, Ordering::SeqCst);
            if let Some(n) = cur() {
                n.drain_events();
            }
        });
    }
}

/// libmpv báo cần vẽ khung mới (gọi từ luồng của mpv).
unsafe extern "C" fn on_update(_: *mut c_void) {
    if !FRAME.swap(true, Ordering::SeqCst) {
        glib::idle_add_once(|| {
            FRAME.store(false, Ordering::SeqCst);
            if let Some(n) = cur() {
                n.on_update();
            }
        });
    }
}

/// Có trình phát native dùng được không (đã bật trong cài đặt và máy có libmpv).
pub fn available() -> bool {
    mpv::lib().is_some()
}

impl Native {
    /// Tạo trình phát (một cái cho cả trình phát của app; tắt phim thì huỷ để trả bộ nhớ).
    pub fn new(has_gpu: bool) -> Option<Rc<Native>> {
        let gl = match std::env::var("CONBANG_NATIVE_RENDER").as_deref() {
            Ok("sw") => false,
            Ok("gl") => true,
            _ => has_gpu,
        };
        let ua = cbtv::http::user_agent();
        let mute = if std::env::var("CONBANG_MUTE").as_deref() == Ok("1") { "yes" } else { "no" };
        let opts: Vec<(&str, &str)> = vec![
            ("vo", "libmpv"),
            ("hwdec", if gl { "auto-safe" } else { "no" }),
            ("config", "no"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("terminal", "no"),
            ("idle", "yes"),
            // Hết phim: đứng ở khung cuối (eof-reached = true) để app tự chuyển tập — như <video> ended
            ("keep-open", "yes"),
            ("audio-client-name", "Côn Bằng TV"),
            ("sub-auto", "no"),
            ("osd-level", "0"),
            // Bộ đệm ~2 phút như hls.html (BUFFER = 120) — mặc định của mpv giữ tới 150 MB
            ("cache", "yes"),
            ("cache-secs", "120"),
            ("demuxer-max-bytes", "48MiB"),
            ("demuxer-max-back-bytes", "16MiB"),
            ("network-timeout", "30"),
            // Đoạn HLS lỗi: tải lại thay vì bỏ qua (cổng nội bộ đã tự thử lại 3 lần)
            ("demuxer-lavf-o", "seg_max_retry=5"),
            ("user-agent", &ua),
            ("mute", mute),
        ];
        let m = Mpv::new(&opts)?;
        // SAFETY: ghi nhật ký mức cảnh báo trở lên; gọi lại wakeup từ luồng của mpv
        unsafe {
            let lvl = CString::new("warn").unwrap();
            (m.l.request_log_messages)(m.h, lvl.as_ptr());
            (m.l.set_wakeup_callback)(m.h, Some(on_wakeup), std::ptr::null_mut());
        }
        let widget: gtk::Widget = if gl {
            let a = gtk::GLArea::new();
            a.set_auto_render(false);
            a.set_has_alpha(false);
            a.set_has_depth_buffer(false);
            a.upcast()
        } else {
            gtk::DrawingArea::new().upcast()
        };
        widget.set_hexpand(true);
        widget.set_vexpand(true);
        widget.set_can_focus(false);
        let n = Rc::new(Native {
            widget,
            gl,
            mpv: m,
            ctx: Cell::new(std::ptr::null_mut()),
            sw: RefCell::new(SwBuf { data: Vec::new(), off: 0, w: 0, h: 0, stride: 0 }),
            has_video: Cell::new(false),
            frame_dirty: Cell::new(false),
            loaded: Cell::new(false),
            restarted: Cell::new(false),
            err: RefCell::new(None),
            pending_sub: RefCell::new(None),
            log_lines: Cell::new(0),
        });
        CUR.with(|c| *c.borrow_mut() = Rc::downgrade(&n));
        if gl {
            n.connect_gl();
        } else {
            n.create_sw()?;
            n.connect_sw();
        }
        cbtv::log::d("native", &format!("mpv ({}) vẽ bằng {}", n.mpv.l.soname, if gl { "OpenGL" } else { "phần mềm" }));
        Some(n)
    }

    fn set_update_callback(&self) {
        // SAFETY: ctx vừa tạo; callback chỉ đặt cờ + idle (an toàn đa luồng)
        unsafe { (self.mpv.l.rc_set_update_callback)(self.ctx.get(), Some(on_update), std::ptr::null_mut()) };
    }

    // ---- Vẽ bằng phần mềm ----

    fn create_sw(&self) -> Option<()> {
        let api = CString::new("sw").unwrap();
        let mut params = [
            RenderParam { kind: mpv::RP_API_TYPE, data: api.as_ptr() as *mut c_void },
            RenderParam { kind: mpv::RP_INVALID, data: std::ptr::null_mut() },
        ];
        let mut ctx: RenderCtx = std::ptr::null_mut();
        // SAFETY: tham số kết thúc bằng INVALID; chuỗi sống trong lúc gọi
        let r = unsafe { (self.mpv.l.rc_create)(&mut ctx, self.mpv.h, params.as_mut_ptr()) };
        if r < 0 {
            cbtv::log::d("native", &format!("render API sw lỗi: {}", self.mpv.err(r)));
            return None;
        }
        self.ctx.set(ctx);
        self.set_update_callback();
        Some(())
    }

    fn connect_sw(self: &Rc<Self>) {
        let area = self.widget.clone().downcast::<gtk::DrawingArea>().unwrap();
        let weak = Rc::downgrade(self);
        area.connect_draw(move |w, cr| {
            cr.set_source_rgb(0.0, 0.0, 0.0);
            let _ = cr.paint();
            let Some(n) = weak.upgrade() else { return glib::Propagation::Stop };
            if !n.has_video.get() {
                return glib::Propagation::Stop;
            }
            let sf = w.scale_factor().max(1);
            let (pw, ph) = (w.allocated_width() * sf, w.allocated_height() * sf);
            if pw <= 0 || ph <= 0 {
                return glib::Propagation::Stop;
            }
            let mut b = n.sw.borrow_mut();
            let resized = b.w != pw || b.h != ph;
            if resized {
                // Hàng ảnh căn 64 byte (libmpv dùng SIMD); dư 64 byte để căn đầu bộ nhớ
                let stride = ((pw as usize * 4) + 63) & !63;
                b.data = vec![0u8; stride * ph as usize + 64];
                b.off = b.data.as_ptr().align_offset(64);
                b.w = pw;
                b.h = ph;
                b.stride = stride;
            }
            if resized || n.frame_dirty.replace(false) {
                let mut size: [c_int; 2] = [pw, ph];
                let fmt = CString::new("bgr0").unwrap();
                let mut stride = b.stride;
                let off = b.off;
                let ptr = b.data[off..].as_mut_ptr();
                let mut params = [
                    RenderParam { kind: mpv::RP_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
                    RenderParam { kind: mpv::RP_SW_FORMAT, data: fmt.as_ptr() as *mut c_void },
                    RenderParam { kind: mpv::RP_SW_STRIDE, data: &mut stride as *mut usize as *mut c_void },
                    RenderParam { kind: mpv::RP_SW_POINTER, data: ptr as *mut c_void },
                    RenderParam { kind: mpv::RP_INVALID, data: std::ptr::null_mut() },
                ];
                // SAFETY: bộ nhớ đủ stride × ph byte từ ptr; mpv vẽ đồng bộ rồi trả về
                unsafe { (n.mpv.l.rc_render)(n.ctx.get(), params.as_mut_ptr()) };
            }
            let (w2, h2, st, off) = (b.w, b.h, b.stride, b.off);
            let ptr = b.data[off..].as_mut_ptr();
            // SAFETY: bề mặt chỉ dùng trong lần vẽ này, bộ nhớ (b) còn mượn tới cuối hàm
            if let Ok(s) = unsafe { gtk::cairo::ImageSurface::create_for_data_unsafe(ptr, gtk::cairo::Format::Rgb24, w2, h2, st as i32) } {
                s.set_device_scale(sf as f64, sf as f64);
                let _ = cr.set_source_surface(&s, 0.0, 0.0);
                let _ = cr.paint();
            }
            glib::Propagation::Stop
        });
    }

    // ---- Vẽ bằng OpenGL ----

    fn connect_gl(self: &Rc<Self>) {
        let area = self.widget.clone().downcast::<gtk::GLArea>().unwrap();
        let weak = Rc::downgrade(self);
        area.connect_realize(move |a| {
            let Some(n) = weak.upgrade() else { return };
            a.make_current();
            if let Some(e) = a.error() {
                cbtv::log::d("native", &format!("GLArea lỗi: {e}"));
                return;
            }
            n.create_gl(a);
        });
        let weak = Rc::downgrade(self);
        area.connect_unrealize(move |a| {
            let Some(n) = weak.upgrade() else { return };
            a.make_current();
            n.free_ctx();
        });
        let weak = Rc::downgrade(self);
        area.connect_render(move |a, _| {
            let Some(n) = weak.upgrade() else { return glib::Propagation::Stop };
            if n.ctx.get().is_null() {
                return glib::Propagation::Stop;
            }
            let sf = a.scale_factor().max(1);
            let mut fbo: c_int = 0;
            if let Some(get) = gl_get_integerv() {
                // SAFETY: GL_FRAMEBUFFER_BINDING — FBO GTK đang cho vẽ vào
                unsafe { get(0x8CA6, &mut fbo) };
            }
            let mut f = GlFbo { fbo, w: a.allocated_width() * sf, h: a.allocated_height() * sf, internal_format: 0 };
            let mut flip: c_int = 1;
            let mut params = [
                RenderParam { kind: mpv::RP_OPENGL_FBO, data: &mut f as *mut GlFbo as *mut c_void },
                RenderParam { kind: mpv::RP_FLIP_Y, data: &mut flip as *mut c_int as *mut c_void },
                RenderParam { kind: mpv::RP_INVALID, data: std::ptr::null_mut() },
            ];
            // SAFETY: ngữ cảnh GL của GLArea đang hiện hành trong tín hiệu render
            unsafe { (n.mpv.l.rc_render)(n.ctx.get(), params.as_mut_ptr()) };
            glib::Propagation::Stop
        });
    }

    fn create_gl(&self, a: &gtk::GLArea) {
        let api = CString::new("opengl").unwrap();
        let mut init = GlInitParams { get_proc_address: Some(get_proc), get_proc_address_ctx: std::ptr::null_mut(), extra_exts: std::ptr::null() };
        let mut params = vec![
            RenderParam { kind: mpv::RP_API_TYPE, data: api.as_ptr() as *mut c_void },
            RenderParam { kind: mpv::RP_OPENGL_INIT_PARAMS, data: &mut init as *mut GlInitParams as *mut c_void },
        ];
        // Màn hình của GTK — mpv cần để giải mã bằng card đồ hoạ rồi đưa thẳng vào OpenGL (VA-API)
        let (kind, disp) = native_display(&a.display());
        if !disp.is_null() {
            params.push(RenderParam { kind, data: disp });
        }
        params.push(RenderParam { kind: mpv::RP_INVALID, data: std::ptr::null_mut() });
        let mut ctx: RenderCtx = std::ptr::null_mut();
        // SAFETY: ngữ cảnh GL hiện hành (vừa make_current); tham số kết thúc bằng INVALID
        let r = unsafe { (self.mpv.l.rc_create)(&mut ctx, self.mpv.h, params.as_mut_ptr()) };
        if r < 0 {
            cbtv::log::d("native", &format!("render API OpenGL lỗi: {}", self.mpv.err(r)));
            return;
        }
        self.ctx.set(ctx);
        self.set_update_callback();
    }

    fn free_ctx(&self) {
        let c = self.ctx.replace(std::ptr::null_mut());
        if !c.is_null() {
            // SAFETY: ctx của mpv này, không còn dùng ở đâu (GL: ngữ cảnh đã make_current)
            unsafe { (self.mpv.l.rc_free)(c) };
        }
    }

    fn on_update(&self) {
        let c = self.ctx.get();
        if c.is_null() {
            return;
        }
        // SAFETY: phải gọi sau mỗi lần mpv báo update
        let flags = unsafe { (self.mpv.l.rc_update)(c) };
        if flags & mpv::RENDER_UPDATE_FRAME != 0 {
            self.frame_dirty.set(true);
            if self.gl {
                if let Some(a) = self.widget.downcast_ref::<gtk::GLArea>() {
                    a.queue_render();
                }
            } else {
                self.widget.queue_draw();
            }
        }
    }

    // ---- Sự kiện ----

    fn drain_events(&self) {
        loop {
            // SAFETY: mpv_wait_event(0) không chờ; sự kiện sống tới lần gọi sau
            let ev = unsafe { &*(self.mpv.l.wait_event)(self.mpv.h, 0.0) };
            match ev.event_id {
                mpv::EVENT_NONE => break,
                mpv::EVENT_START_FILE => {
                    self.loaded.set(false);
                    self.restarted.set(false);
                }
                mpv::EVENT_FILE_LOADED => {
                    self.loaded.set(true);
                    self.has_video.set(true);
                    if let Some(sub) = self.pending_sub.borrow_mut().take() {
                        self.mpv.command(&["sub-add", &sub, "select", "Tiếng Việt"]);
                    }
                }
                mpv::EVENT_PLAYBACK_RESTART => {
                    if !self.restarted.replace(true) {
                        cbtv::log::d("native", "native: chạy");
                    }
                }
                mpv::EVENT_END_FILE if !ev.data.is_null() => {
                    // SAFETY: data của END_FILE là mpv_event_end_file
                    let e = unsafe { &*(ev.data as *const mpv::EventEndFile) };
                    if e.reason == mpv::END_ERROR {
                        let m = self.mpv.err(e.error);
                        cbtv::log::d("native", &format!("mpv không phát được: {m}"));
                        *self.err.borrow_mut() = Some(m);
                    }
                }
                mpv::EVENT_LOG_MESSAGE if !ev.data.is_null() => {
                    let n = self.log_lines.get();
                    if n < 60 {
                        self.log_lines.set(n + 1);
                        // SAFETY: data của LOG_MESSAGE là mpv_event_log_message
                        let m = unsafe { &*(ev.data as *const mpv::EventLogMessage) };
                        cbtv::log::d("mpv", &format!("[{}] {}: {}", mpv::cstr(m.prefix), mpv::cstr(m.level), mpv::cstr(m.text).trim_end()));
                    }
                }
                mpv::EVENT_SHUTDOWN => break,
                _ => {}
            }
        }
    }

    // ---- Điều khiển ----

    /// Phát địa chỉ (của cổng phát nội bộ) từ giây `start`, kèm phụ đề rời nếu có.
    pub fn load(&self, url: &str, start: f64, sub: Option<String>) {
        *self.err.borrow_mut() = None;
        *self.pending_sub.borrow_mut() = sub;
        self.log_lines.set(0);
        self.restarted.set(false);
        self.loaded.set(false);
        // «start» áp cho tệp nạp sau đó (cùng cách ở libmpv.so.1 và .so.2 — cú pháp loadfile có tuỳ chọn đã đổi giữa hai bản)
        self.mpv.set_str("start", &if start > 0.0 { format!("{start}") } else { "none".into() });
        self.mpv.set_flag("pause", false);
        self.mpv.command(&["loadfile", url, "replace"]);
    }

    pub fn stop(&self) {
        self.mpv.command(&["stop"]);
        self.has_video.set(false);
        self.restarted.set(false);
        self.loaded.set(false);
        self.widget.queue_draw();
    }

    pub fn state(&self) -> State {
        let t = self.mpv.get_double("time-pos");
        let seeking = self.mpv.get_flag("seeking").unwrap_or(false);
        State {
            t: t.unwrap_or(0.0).max(0.0),
            d: self.mpv.get_double("duration").unwrap_or(0.0),
            paused: self.mpv.get_flag("pause").unwrap_or(false),
            ended: self.mpv.get_flag("eof-reached").unwrap_or(false),
            waiting: self.mpv.get_flag("paused-for-cache").unwrap_or(false) || seeking,
            ready: self.restarted.get() && t.is_some(),
            err: self.err.borrow().clone(),
        }
    }

    /// Lệnh như chimtoCmd của player-agent.js.
    pub fn cmd(&self, name: &str, arg: Option<f64>) {
        match name {
            "toggle" => {
                let p = self.mpv.get_flag("pause").unwrap_or(false);
                self.mpv.set_flag("pause", !p);
            }
            "play" => {
                self.mpv.set_flag("pause", false);
            }
            "pause" => {
                self.mpv.set_flag("pause", true);
            }
            "seekTo" => {
                if let Some(t) = arg {
                    self.mpv.command(&["seek", &format!("{t}"), "absolute"]);
                }
            }
            "rate" => {
                if let Some(r) = arg {
                    self.mpv.set_double("speed", r);
                }
            }
            // Lấp đầy màn hình: phóng hình cho hết viền đen (cắt bớt hai bên)
            "fit" => {
                self.mpv.set_double("panscan", if arg.unwrap_or(0.0) > 0.5 { 1.0 } else { 0.0 });
            }
            _ => {}
        }
    }

    /// Cổng gỡ lỗi: khung rơi, cỡ hình, bộ giải mã.
    pub fn debug(&self) -> String {
        format!(
            "render={} dropped={} decoderDropped={} video={}x{} hwdec={} cache={:.0}s",
            if self.gl { "gl" } else { "sw" },
            self.mpv.get_i64("frame-drop-count").unwrap_or(-1),
            self.mpv.get_i64("decoder-frame-drop-count").unwrap_or(-1),
            self.mpv.get_i64("width").unwrap_or(0),
            self.mpv.get_i64("height").unwrap_or(0),
            self.mpv.get_str("hwdec-current").unwrap_or_default(),
            self.mpv.get_double("demuxer-cache-duration").unwrap_or(0.0)
        )
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        if self.gl {
            if let Some(a) = self.widget.downcast_ref::<gtk::GLArea>() {
                if a.is_realized() {
                    a.make_current();
                }
            }
        }
        self.free_ctx();
        // self.mpv tự huỷ sau (mpv_terminate_destroy)
    }
}

// ---- OpenGL: lấy địa chỉ hàm theo kiểu màn hình của GTK (Wayland → EGL, X11 → GLX) ----

type GetProc = unsafe extern "C" fn(*const c_char) -> *mut c_void;

fn loader() -> Option<GetProc> {
    static L: OnceLock<Option<usize>> = OnceLock::new();
    let p = L.get_or_init(|| {
        let wayland = gdk::Display::default().map(|d| d.type_().name().contains("Wayland")).unwrap_or(false);
        let (lib, name) = if wayland { ("libEGL.so.1", "eglGetProcAddress") } else { ("libGL.so.1", "glXGetProcAddressARB") };
        // SAFETY: nạp thư viện GL của hệ thống (GTK đã nạp sẵn), lấy hàm tra địa chỉ
        unsafe {
            let l = CString::new(lib).unwrap();
            let h = libc::dlopen(l.as_ptr(), libc::RTLD_NOW | libc::RTLD_NOLOAD);
            let h = if h.is_null() { libc::dlopen(l.as_ptr(), libc::RTLD_NOW) } else { h };
            if h.is_null() {
                return None;
            }
            let n = CString::new(name).unwrap();
            let f = libc::dlsym(h, n.as_ptr());
            (!f.is_null()).then_some(f as usize)
        }
    });
    // SAFETY: địa chỉ hàm eglGetProcAddress / glXGetProcAddressARB
    p.map(|a| unsafe { std::mem::transmute::<usize, GetProc>(a) })
}

unsafe extern "C" fn get_proc(_: *mut c_void, name: *const c_char) -> *mut c_void {
    if let Some(f) = loader() {
        let p = f(name);
        if !p.is_null() {
            return p;
        }
    }
    libc::dlsym(libc::RTLD_DEFAULT, name)
}

type GetIntegerv = unsafe extern "C" fn(u32, *mut c_int);

fn gl_get_integerv() -> Option<GetIntegerv> {
    static F: OnceLock<Option<usize>> = OnceLock::new();
    let p = F.get_or_init(|| {
        let n = CString::new("glGetIntegerv").unwrap();
        // SAFETY: tra địa chỉ hàm GL
        let p = unsafe { get_proc(std::ptr::null_mut(), n.as_ptr()) };
        (!p.is_null()).then_some(p as usize)
    });
    // SAFETY: địa chỉ glGetIntegerv
    p.map(|a| unsafe { std::mem::transmute::<usize, GetIntegerv>(a) })
}

/// wl_display* / Display* của GTK (cho VA-API của mpv). Tra hàm GDK lúc chạy — GDK dựng thiếu một kiểu màn hình vẫn chạy.
fn native_display(d: &gdk::Display) -> (c_int, *mut c_void) {
    use glib::translate::ToGlibPtr;
    let wayland = d.type_().name().contains("Wayland");
    let (kind, name) = if wayland {
        (mpv::RP_WL_DISPLAY, "gdk_wayland_display_get_wl_display")
    } else {
        (mpv::RP_X11_DISPLAY, "gdk_x11_display_get_xdisplay")
    };
    let n = CString::new(name).unwrap();
    // SAFETY: hàm GDK nhận GdkDisplay* đúng kiểu (đã kiểm tên kiểu ở trên)
    unsafe {
        let f = libc::dlsym(libc::RTLD_DEFAULT, n.as_ptr());
        if f.is_null() {
            return (kind, std::ptr::null_mut());
        }
        let f: unsafe extern "C" fn(*mut gdk::ffi::GdkDisplay) -> *mut c_void = std::mem::transmute(f);
        let p: *mut gdk::ffi::GdkDisplay = d.to_glib_none().0;
        (kind, f(p))
    }
}
