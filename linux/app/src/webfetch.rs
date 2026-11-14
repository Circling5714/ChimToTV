//! Trang bật lớp chống bot của Cloudflare (hoathinh3d.pw từ 14/11/2026: 403 «Just a moment…» với mọi trình tải không phải trình
//! duyệt — cả trang, API, ảnh). Lõi (`cbtv::web_fetch`) nhận ra lớp chặn và nhờ app đọc hộ: mỗi trang bị chặn một WebKitGTK ẩn
//! (`OffscreenWindow`, cùng WebContext / cookie / cổng SOCKS nội bộ với trình phát) mở trang chủ cho trình duyệt tự qua lớp chặn,
//! đứng ở robots.txt rồi mỗi yêu cầu là `fetch()` ngay trong trang (web-fetch.js — dùng chung với bản Windows, gửi kết quả về qua
//! `window.webkit.messageHandlers.cbReply`). Ảnh bìa đi qua lõi nên tự được đọc hộ. Không giả / không giải hộ lớp chặn: trang đòi
//! bấm xác nhận hay chặn hẳn WebKit thì báo lỗi. 5 phút không dùng thì đóng WebView cho nhẹ máy.

use cbtv::web_fetch::{self, WebReply, WebRequest};
use gtk::prelude::*;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use webkit2gtk::{
    LoadEvent, SettingsExt, UserContentManager, UserContentManagerExt, WebView, WebViewExt,
};

const FETCH_JS: &str = include_str!("../../../app/src/main/assets/web-fetch.js");
const REPLY_JS: &str =
    "window.__cbReply=function(m){try{window.webkit.messageHandlers.cbReply.postMessage(JSON.stringify(m))}catch(e){}};";
const PROBE_JS: &str = "JSON.stringify([location.host, document.readyState, document.title || '', typeof window._cf_chl_opt, \
    !!document.querySelector('#challenge-form,#challenge-stage,#cf-challenge-running,.cf-turnstile'), \
    !!document.querySelector('#cf-error-details,.cf-error-details') || /you have been blocked/i.test(document.body ? document.body.innerText.slice(0, 400) : '')])";

/// Chờ trình duyệt qua lớp chặn tối đa
const PASS: Duration = Duration::from_secs(30);
/// Chờ một yêu cầu tối đa
const CALL: Duration = Duration::from_secs(45);
/// Không dùng bao lâu thì đóng WebView
const IDLE: Duration = Duration::from_secs(300);
/// Cloudflare chặn hẳn WebKit thì thôi mở lại trong 30 phút
const BLOCKED_FOR: Duration = Duration::from_secs(1800);

type Done = oneshot::Sender<Result<WebReply, String>>;
type Waiter = Box<dyn FnOnce(Result<(), String>)>;

/// Đăng ký với lõi (gọi một lần, ở luồng giao diện — lõi gọi từ luồng của tokio, việc được chuyển về luồng giao diện).
pub fn install() {
    web_fetch::set_provider(Some(Arc::new(|req: WebRequest| {
        let (tx, rx) = oneshot::channel();
        glib::MainContext::default().invoke(move || handle(req, tx));
        Box::pin(async move { rx.await.unwrap_or_else(|_| Err("WebKit thôi đọc trang".into())) })
    })));
}

thread_local! {
    static SESSIONS: RefCell<HashMap<String, Rc<Session>>> = RefCell::new(HashMap::new());
}

fn handle(req: WebRequest, tx: Done) {
    let host = web_fetch::host_of(&req.url);
    let s = SESSIONS.with(|m| m.borrow_mut().entry(host.clone()).or_insert_with(|| Session::new(&host)).clone());
    s.run(req, tx, 0);
}

/// Gỡ lỗi: tình trạng các phiên.
pub fn describe() -> String {
    SESSIONS.with(|m| {
        let m = m.borrow();
        if m.is_empty() {
            return "chưa mở phiên nào".into();
        }
        m.values().map(|s| format!("{}: {}", s.host, s.status.borrow())).collect::<Vec<_>>().join("; ")
    })
}

enum State {
    Closed,
    Opening(Vec<Waiter>),
    Ready,
    Blocked(Instant),
}

struct Session {
    host: String,
    web: RefCell<Option<(gtk::OffscreenWindow, WebView)>>,
    state: RefCell<State>,
    calls: RefCell<HashMap<u64, (WebRequest, Done, u8)>>,
    next: Cell<u64>,
    idle: RefCell<Option<glib::SourceId>>,
    status: RefCell<String>,
    /// Đã thấy trang thật, đang chuyển sang robots.txt — bộ dò thôi chạy
    passed: Cell<bool>,
}

impl Session {
    fn new(host: &str) -> Rc<Session> {
        Rc::new(Session {
            host: host.to_string(),
            web: RefCell::new(None),
            state: RefCell::new(State::Closed),
            calls: RefCell::new(HashMap::new()),
            next: Cell::new(0),
            idle: RefCell::new(None),
            status: RefCell::new("mới".into()),
            passed: Cell::new(false),
        })
    }

    fn blocked_msg(&self) -> String {
        format!("Trang {} chặn app này (Cloudflare «you have been blocked») — xem phim này ở nguồn khác, vd KKPhim.", self.host)
    }

    /// Một yêu cầu: chờ qua lớp chặn rồi fetch() trong trang. attempt = 1: lần thử lại sau khi lớp chặn quay lại.
    fn run(self: &Rc<Self>, req: WebRequest, tx: Done, attempt: u8) {
        self.touch();
        let me = self.clone();
        self.when_ready(Box::new(move |r| match r {
            Err(e) => {
                let _ = tx.send(Err(e));
            }
            Ok(()) => me.call(req, tx, attempt),
        }));
    }

    /// Hẹn đóng WebView sau IDLE (mỗi yêu cầu hẹn lại).
    fn touch(self: &Rc<Self>) {
        if let Some(id) = self.idle.borrow_mut().take() {
            id.remove();
        }
        let me = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(IDLE, move || {
            if let Some(s) = me.upgrade() {
                s.idle.borrow_mut().take();
                if s.calls.borrow().is_empty() && !matches!(*s.state.borrow(), State::Opening(_)) {
                    s.close("không dùng 5 phút");
                }
            }
        });
        *self.idle.borrow_mut() = Some(id);
    }

    fn when_ready(self: &Rc<Self>, cb: Waiter) {
        enum Next {
            Go,
            Blocked,
            Queued,
            Open,
        }
        let has_web = self.web.borrow().is_some();
        let next = {
            let mut st = self.state.borrow_mut();
            match &mut *st {
                State::Ready if has_web => Next::Go,
                State::Blocked(until) if Instant::now() < *until => Next::Blocked,
                State::Opening(_) => Next::Queued,
                _ => Next::Open,
            }
        };
        match next {
            Next::Go => cb(Ok(())),
            Next::Blocked => cb(Err(self.blocked_msg())),
            Next::Queued => {
                if let State::Opening(list) = &mut *self.state.borrow_mut() {
                    list.push(cb);
                }
            }
            Next::Open => {
                *self.state.borrow_mut() = State::Opening(vec![cb]);
                self.open();
            }
        }
    }

    fn finish(&self, r: Result<(), String>) {
        let list = match std::mem::replace(&mut *self.state.borrow_mut(), if r.is_ok() { State::Ready } else { State::Closed }) {
            State::Opening(l) => l,
            _ => Vec::new(),
        };
        for cb in list {
            cb(r.clone());
        }
    }

    fn create(self: &Rc<Self>) -> WebView {
        if let Some((_, w)) = self.web.borrow().as_ref() {
            return w.clone();
        }
        crate::player::setup_web_context();
        let ucm = UserContentManager::new();
        ucm.register_script_message_handler("cbReply");
        let me = Rc::downgrade(self);
        ucm.connect_script_message_received(Some("cbReply"), move |_, r| {
            use javascriptcore::ValueExt;
            if let (Some(s), Some(v)) = (me.upgrade(), r.js_value()) {
                s.on_reply(&v.to_str());
            }
        });
        let web = WebView::builder().user_content_manager(&ucm).build();
        if let Some(st) = WebViewExt::settings(&web) {
            st.set_auto_load_images(false); // trang chủ mở để qua lớp chặn: khỏi tải ảnh
            st.set_media_playback_requires_user_gesture(true);
            st.set_javascript_can_open_windows_automatically(false);
            st.set_user_agent(Some(&crate::player::web_user_agent()));
        }
        web.set_is_muted(true);
        // Cửa sổ ngoài màn hình: WebView được dựng như thật (trang chạy bình thường) nhưng không hiện ra
        let win = gtk::OffscreenWindow::new();
        win.set_default_size(1024, 700);
        win.add(&web);
        win.show_all();
        *self.web.borrow_mut() = Some((win, web.clone()));
        web
    }

    fn open(self: &Rc<Self>) {
        let web = self.create();
        let t0 = Instant::now();
        self.passed.set(false);
        *self.status.borrow_mut() = "đang qua lớp chặn".into();
        web.load_uri(&format!("https://{}/", self.host));
        // Trang chặn («Just a moment…», window._cf_chl_opt) tự giải rồi nạp lại trang thật
        let me = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(500), move || {
            let Some(s) = me.upgrade() else { return glib::ControlFlow::Break };
            if !matches!(*s.state.borrow(), State::Opening(_)) || s.passed.get() {
                return glib::ControlFlow::Break;
            }
            let Some(web) = s.web.borrow().as_ref().map(|(_, w)| w.clone()) else { return glib::ControlFlow::Break };
            if t0.elapsed() > PASS {
                cbtv::log::d("web", &format!("{}: WebKit chưa qua lớp chặn sau {} giây", s.host, PASS.as_secs()));
                *s.status.borrow_mut() = "chưa qua lớp chặn".into();
                s.close("chưa qua lớp chặn");
                s.finish(Err(format!(
                    "Trang {} đòi bấm xác nhận chống bot (Cloudflare) — app chưa đọc được trang này.",
                    s.host
                )));
                return glib::ControlFlow::Break;
            }
            let s2 = s.clone();
            #[allow(deprecated)]
            web.run_javascript(PROBE_JS, None::<&gio::Cancellable>, move |r| {
                use javascriptcore::ValueExt;
                let raw = r.ok().and_then(|x| x.js_value()).map(|v| v.to_str().to_string()).unwrap_or_default();
                s2.on_probe(&raw, t0);
            });
            glib::ControlFlow::Continue
        });
    }

    /// Kết quả dò trang: đã qua → đứng ở robots.txt (trang nhẹ cùng gốc) rồi báo sẵn sàng; chặn hẳn → báo lỗi.
    fn on_probe(self: &Rc<Self>, raw: &str, t0: Instant) {
        if !matches!(*self.state.borrow(), State::Opening(_)) || self.passed.get() {
            return;
        }
        let Ok(Value::Array(a)) = serde_json::from_str::<Value>(raw) else { return };
        let s = |i: usize| a.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let b = |i: usize| a.get(i).and_then(|v| v.as_bool()).unwrap_or(false);
        let same = s(0).eq_ignore_ascii_case(&self.host) && s(1) != "loading";
        if same && (b(5) || s(2).starts_with("Attention Required")) {
            cbtv::log::d("web", &format!("{}: Cloudflare chặn hẳn WebKit — không đọc được", self.host));
            let msg = self.blocked_msg();
            let old = std::mem::replace(&mut *self.state.borrow_mut(), State::Blocked(Instant::now() + BLOCKED_FOR));
            let list = match old {
                State::Opening(l) => l,
                _ => Vec::new(),
            };
            self.close("trang chặn app");
            *self.status.borrow_mut() = "trang chặn hẳn WebKit".into();
            for cb in list {
                cb(Err(msg.clone()));
            }
            return;
        }
        let passed = same && s(3) == "undefined" && !b(4) && !s(2).starts_with("Just a moment");
        if !passed {
            return;
        }
        self.passed.set(true);
        let ms = t0.elapsed().as_millis();
        cbtv::log::d("web", &format!("{}: WebKit đã qua lớp chống bot ({ms} ms)", self.host));
        *self.status.borrow_mut() = format!("sẵn sàng (qua lớp chặn {ms} ms)");
        // Báo sẵn sàng ngay (đang chuyển hẳn sang robots.txt thì chặn bằng cờ Ready khi trang nhẹ đã nạp)
        let Some(web) = self.web.borrow().as_ref().map(|(_, w)| w.clone()) else { return };
        let me = Rc::downgrade(self);
        let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
        let h2 = handler.clone();
        let id = web.connect_load_changed(move |w, ev| {
            // Chỉ khi chính robots.txt đã nạp xong (lần nạp trang chủ bị huỷ giữa chừng cũng báo Finished — fetch() chạy lúc tài
            // liệu đang bị thay thì «Load failed»)
            if ev != LoadEvent::Finished || !w.uri().is_some_and(|u| u.ends_with("/robots.txt")) {
                return;
            }
            if let Some(id) = h2.borrow_mut().take() {
                w.disconnect(id);
            }
            if let Some(s) = me.upgrade() {
                s.finish(Ok(()));
            }
        });
        *handler.borrow_mut() = Some(id);
        web.load_uri(&format!("https://{}/robots.txt", self.host));
        // Phòng hờ: robots.txt không nạp xong thì sau 15 giây vẫn báo sẵn sàng (tài liệu đang mở cùng gốc, fetch() vẫn chạy)
        let me = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_secs(15), move || {
            if let Some(s) = me.upgrade() {
                if matches!(*s.state.borrow(), State::Opening(_)) && s.passed.get() {
                    s.finish(Ok(()));
                }
            }
        });
    }

    fn call(self: &Rc<Self>, req: WebRequest, tx: Done, attempt: u8) {
        let Some(web) = self.web.borrow().as_ref().map(|(_, w)| w.clone()) else {
            let _ = tx.send(Err(format!("WebKit của {} đã đóng", self.host)));
            return;
        };
        let id = self.next.get() + 1;
        self.next.set(id);
        let headers: serde_json::Map<String, Value> = req.headers.iter().map(|(k, v)| (k.clone(), Value::from(v.clone()))).collect();
        let js = format!(
            "{REPLY_JS}{FETCH_JS}\nwindow.__cbFetch({id},{},{},{},{});0",
            serde_json::to_string(&req.url).unwrap_or_default(),
            serde_json::to_string(&req.method).unwrap_or_default(),
            serde_json::to_string(&req.body).unwrap_or_else(|_| "null".into()),
            Value::Object(headers)
        );
        self.calls.borrow_mut().insert(id, (req, tx, attempt));
        let me = Rc::downgrade(self);
        glib::timeout_add_local_once(CALL, move || {
            if let Some(s) = me.upgrade() {
                if let Some((req, tx, _)) = s.calls.borrow_mut().remove(&id) {
                    let _ = tx.send(Err(format!("WebKit không trả lời ({})", req.url)));
                }
            }
        });
        #[allow(deprecated)]
        web.run_javascript(&js, None::<&gio::Cancellable>, |_| {});
    }

    /// Trả lời của web-fetch.js: {id, code, url, type, cf, b64} hoặc {id, err}.
    fn on_reply(self: &Rc<Self>, json: &str) {
        let Ok(m) = serde_json::from_str::<Value>(json) else { return };
        let id = m.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
        let Some((req, tx, attempt)) = self.calls.borrow_mut().remove(&id) else { return };
        if let Some(e) = m.get("err").and_then(|v| v.as_str()) {
            let _ = tx.send(Err(format!("WebKit đọc {} lỗi: {e}", req.url)));
            return;
        }
        // Lớp chặn quay lại (cookie hết hạn): qua lại lớp chặn, thử thêm một lần
        if m.get("cf").and_then(|v| v.as_str()).is_some_and(|c| c.eq_ignore_ascii_case("challenge")) && attempt == 0 {
            *self.state.borrow_mut() = State::Closed;
            self.run(req, tx, 1);
            return;
        }
        let data = match web_fetch::decode_b64(m.get("b64").and_then(|v| v.as_str()).unwrap_or("")) {
            Ok(d) => d,
            Err(e) => {
                let _ = tx.send(Err(e));
                return;
            }
        };
        let _ = tx.send(Ok(WebReply {
            code: m.get("code").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
            url: m.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            data,
            content_type: m.get("type").and_then(|v| v.as_str()).map(|t| t.to_string()),
        }));
    }

    fn close(&self, why: &str) {
        if let Some((win, web)) = self.web.borrow_mut().take() {
            web.stop_loading();
            unsafe { win.destroy() };
            cbtv::log::d("web", &format!("{}: đóng WebKit ẩn ({why})", self.host));
        }
        if matches!(*self.state.borrow(), State::Ready) {
            *self.state.borrow_mut() = State::Closed;
        }
    }
}
