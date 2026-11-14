//! Trình phát (bản Android: PlayerActivity.kt, Windows: Player/PlayerView.cs). Video phát trong WebKitGTK bằng đúng cách
//! của bản TV:
//! - trang có server (player.php / Next.js / API): trang bọc (player.html) chứa iframe trình phát của trang, hoặc hls.html tự
//!   phát luồng .m3u8 (bỏ quảng cáo chèn giữa phim). Trang bọc được nạp tại chính địa chỉ trang tập (load_alternate_html —
//!   như loadDataWithBaseURL của Android) để trình phát nhúng thấy Referer của trang phim.
//! - trang không chia server (page mode): mở thẳng trang tập, page-focus.js phóng trình phát ra cả màn hình.
//! player-agent.js chạy trong mọi khung (UserScript, mọi khung, lúc tài liệu vừa tạo), điều khiển <video> và báo trạng thái;
//! app đọc trạng thái mỗi giây (window.__chimtoState) và gửi lệnh qua window.chimtoCmd.
//! Mọi kết nối của trình phát đi qua cổng SOCKS nội bộ (DNS mã hoá, vượt chặn, proxy, chặn tên miền quảng cáo).

use crate::app::app;
use crate::css::px;
use crate::keynav;
use crate::ui;
use cbtv::util::format_time;
use cbtv::{prefs, EpisodeGroup, EpisodeSource, FilmDetail, HistoryItem, Server};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use webkit2gtk::{
    NetworkProxyMode, NetworkProxySettings, SettingsExt, UserContentInjectedFrames, UserContentManager,
    UserContentManagerExt, UserScript, UserScriptInjectionTime, WebContext, WebContextExt, WebView, WebViewExt,
    WebsiteDataManagerExt, WebsiteDataManagerExtManual,
};

const RATES: [f64; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];
/// Trình phát native quá chừng này (ms) chưa chạy được mà cũng không báo lỗi → chuyển sang WebView. Đo trên máy ảo thử: mpv
/// thường chạy sau 1–4 s kể từ lúc nạp (VSMOV chậm nhất ~4 s).
const NATIVE_STALL_MS: i64 = 20_000;

/// CSS của agent (agent thêm lúc tài liệu vừa tạo có thể chưa có <head> → thêm lại khi trang dựng xong).
const STYLE_SHIM: &str = "(function(){var css='.jw-controlbar,.jw-title{display:none!important}.chimto-pin{position:fixed!important;left:0!important;\
top:0!important;right:auto!important;bottom:auto!important;width:100vw!important;height:100vh!important;max-width:none!important;\
max-height:none!important;margin:0!important;transform:none!important;z-index:2147483646!important;background:#000!important}';\
function add(){try{if(document.getElementById('chimto-css'))return;var s=document.createElement('style');s.id='chimto-css';\
s.textContent=css;(document.head||document.documentElement).appendChild(s);}catch(e){}}\
if(document.documentElement)add();document.addEventListener('DOMContentLoaded',add);})();\n";

/// Bản thử (CONBANG_DEBUG): chẩn đoán khung trình phát — khung trên cùng gửi {chimto:'diag'} xuống, mỗi khung báo trạng thái
/// JW Player / <video> lên window.__diag (cổng gỡ lỗi /diag).
const DIAG_JS: &str = r#"
(function(){
  if (window.__chimtoDiag) return; window.__chimtoDiag = 1;
  if (window.top === window) {
    window.__diag = [];
    window.addEventListener('message', function (e) { if (e.data && e.data.chimtoDiag) window.__diag.push(e.data.chimtoDiag); });
  }
  var errs = [];
  var hook = setInterval(function () {
    try { var p = window.jwplayer && window.jwplayer(); if (p && p.on && !p.__diag) { p.__diag = 1;
      p.on('error', function (e) { errs.push('error ' + e.code + ' ' + e.message); });
      p.on('setupError', function (e) { errs.push('setupError ' + e.code + ' ' + e.message); });
      p.on('warning', function (e) { errs.push('warning ' + e.code + ' ' + e.message); });
      clearInterval(hook); } } catch (x) {}
  }, 300);
  window.addEventListener('message', function (e) {
    if (!e.data || e.data.chimto !== 'diag') return;
    var o = { href: location.href.slice(0, 120), errs: errs };
    try { var p = window.jwplayer && window.jwplayer(); if (p && p.getState) o.jw = { state: p.getState(), provider: (p.getProvider && p.getProvider() || {}).name,
      levels: p.getQualityLevels ? p.getQualityLevels().length : null, file: (p.getPlaylistItem && (p.getPlaylistItem() || {}).file || '').slice(0, 120) }; } catch (x) { o.jwErr = String(x); }
    var v = document.querySelector('video');
    if (v) o.v = { src: (v.currentSrc || v.src || '').slice(0, 80), rs: v.readyState, ns: v.networkState, err: v.error && (v.error.code + ' ' + v.error.message), buf: v.buffered.length, w: v.videoWidth, h: v.videoHeight,
      q: v.getVideoPlaybackQuality ? (function (q) { return q.droppedVideoFrames + '/' + q.totalVideoFrames; })(v.getVideoPlaybackQuality()) : null };
    try { window.top.postMessage({ chimtoDiag: o }, '*'); } catch (x) {}
    var fs = document.getElementsByTagName('iframe');
    for (var i = 0; i < fs.length; i++) try { fs[i].contentWindow.postMessage({ chimto: 'diag' }, '*'); } catch (x) {}
  });
})();
"#;

/// Trình phát native: trình phát nhúng (JW Player của streamfree, NguonC…) tự dựng link luồng (streamfree giải mã ra
/// `data:application/vnd.apple.mpegurl;base64,…`) — đọc link đó từ JW Player trong khung nhúng rồi báo lên khung trên cùng
/// (window.__chimtoSrc = {u: link, p: trang của khung, s: phụ đề}); app tắt trang, phát bằng mpv.
const EXTRACT_JS: &str = r#"
(function(){
  if (window.__chimtoExtract) return; window.__chimtoExtract = 1;
  if (window.top === window) {
    window.addEventListener('message', function (e) { if (e.data && e.data.chimtoSrc && !window.__chimtoSrc) window.__chimtoSrc = e.data.chimtoSrc; });
  }
  var n = 0, ok = /^data:application\/(vnd\.apple\.mpegurl|x-mpegurl)[;,]/i, file = /\.(m3u8|mp4)(\?|#|$)/i;
  var t = setInterval(function () {
    if (++n > 150) { clearInterval(t); return; }
    try {
      var p = window.jwplayer && window.jwplayer(), it = p && p.getPlaylistItem && p.getPlaylistItem();
      var f = it && (it.file || (it.sources && it.sources[0] && it.sources[0].file));
      // JW Player tự biết luồng HLS dù link không có đuôi .m3u8 (NguonC / streamc: link là token mã hoá) — kiểu ghi trong
      // playlist (type: 'hls') hoặc bộ phát đang dùng hls.js
      var hls = !!(it && /hls|m3u8/i.test(String(it.type || ((it.sources && it.sources[0]) || {}).type || ''))) ||
        !!(p && p.getProvider && /hls/i.test((p.getProvider() || {}).name || ''));
      if (!f || typeof f !== 'string') {
        var v = document.querySelector('video'), s = v && (v.currentSrc || v.src);
        if (s && /^https?:/.test(s) && file.test(s)) f = s; else return;
      }
      if (!ok.test(f) && !file.test(f) && !(hls && /^https?:/.test(f))) return;
      clearInterval(t);
      var msg = { u: /^data:/.test(f) ? f : new URL(f, location.href).href, p: location.href, h: hls ? 1 : 0 };
      try { var tr = ((it && it.tracks) || []).filter(function (x) { return x.kind === 'captions' || x.kind === 'subtitles'; })[0];
        if (tr && tr.file) msg.s = new URL(tr.file, location.href).href; } catch (x) {}
      if (window.top === window) window.__chimtoSrc = msg; else window.top.postMessage({ chimtoSrc: msg }, '*');
    } catch (x) {}
  }, 200);
})();
"#;

#[derive(Debug, Clone, Copy, Default)]
struct PlayState {
    t: f64,
    d: f64,
    paused: bool,
    ended: bool,
    waiting: bool,
    ready: bool,
}

#[derive(Default)]
struct St {
    film_url: String,
    detail: Option<FilmDetail>,
    group: Option<EpisodeGroup>,
    index: usize,
    resume_open: bool,
    open_ep_url: String,
    load_token: u64,
    page_mode: bool,
    source: Option<EpisodeSource>,
    server: Option<Server>,
    token: u64,
    page_load: u64,
    embed_url: Option<String>,
    hls_url: Option<String>,
    episode_url: String,
    err_shown: bool,
    last_ad_at: i64,
    state: Option<PlayState>,
    started: bool,
    resume_at: i64,
    resume_pending: bool,
    resume_issued_at: i64,
    ended_handled: bool,
    hint_shown: bool,
    load_started_at: i64,
    last_save: i64,
    seek_target: Option<f64>,
    rate: f64,
    start_hint: i64,
    intro_done: bool,
    repeat: u32,
    dragging: bool,
    click_through: bool,
    polling: bool,
    limit_warned: bool,
    expect_nav: Option<String>,
    osd_hover: bool,
    /// Số lần app tự gửi lệnh phát (JW Player trên WebKitGTK không tự phát — đứng «idle» chờ bấm)
    nudges: u32,
    last_nudge: i64,
    /// Đang phát bằng trình phát native (libmpv)
    native_on: bool,
    /// Đang mở trình phát nhúng chỉ để lấy link luồng (rồi phát native)
    extract: bool,
    /// Trình phát native lỗi với server này → phát bằng WebView tới khi đổi tập / server
    force_web: bool,
}

pub struct Player {
    root: gtk::Overlay,
    /// Khung hình: WebView (trang phát) hoặc ô của trình phát native — cửa sổ phim nhỏ mang cả khung này đi
    stage: gtk::Stack,
    web: WebView,
    /// Trình phát native (libmpv) — tạo khi cần, tắt phim thì huỷ
    native: RefCell<Option<Rc<crate::native::Native>>>,
    ucm: UserContentManager,
    hit: gtk::EventBox,
    topbar: gtk::EventBox,
    title: gtk::Label,
    loading_box: gtk::Box,
    loading_bar: gtk::ProgressBar,
    loading_text: gtk::Label,
    notice_box: gtk::Box,
    notice_text: gtk::Label,
    osd: gtk::EventBox,
    time_text: gtk::Label,
    state_text: gtk::Label,
    seek: gtk::Scale,
    btn_play: gtk::Button,
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    panel: gtk::Box,
    panel_title: gtk::Label,
    panel_stack: gtk::Box,
    btn_done: gtk::Button,
    open: Cell<bool>,
    s: RefCell<St>,
    poll_timer: RefCell<Option<glib::SourceId>>,
    osd_timer: RefCell<Option<glib::SourceId>>,
    notice_timer: RefCell<Option<glib::SourceId>>,
    seek_timer: RefCell<Option<glib::SourceId>>,
    click_timer: RefCell<Option<glib::SourceId>>,
    /// Cửa sổ phim nhỏ (đang thu nhỏ) — xem enter_mini().
    mini: RefCell<Option<Mini>>,
}

/// Cửa sổ phim nhỏ: WebView chuyển sang đây, cửa sổ app trở lại các trang.
struct Mini {
    win: gtk::Window,
    holder: gtk::Overlay,
    play: gtk::Button,
}

thread_local! {
    static PLAYER: RefCell<Option<std::rc::Weak<Player>>> = const { RefCell::new(None) };
}

fn me() -> Option<Rc<Player>> {
    PLAYER.with(|p| p.borrow().as_ref().and_then(|w| w.upgrade()))
}

fn osd_button(text: &str, f: impl Fn(&Rc<Player>) + 'static) -> gtk::Button {
    let b = ui::pill(text, move || {
        if let Some(p) = me() {
            f(&p);
        }
    });
    b.set_can_focus(false);
    b
}

/// Cổng nội bộ cho mọi trang của WebKit (gọi một lần trước khi tạo WebView).
pub fn setup_web_context() {
    thread_local! { static DONE: Cell<bool> = const { Cell::new(false) }; }
    if DONE.with(|d| d.replace(true)) {
        return;
    }
    let ctx = WebContext::default().expect("WebContext");
    let port = cbtv::net::start_bridge();
    if let Some(dm) = ctx.website_data_manager() {
        let uri = format!("socks5://127.0.0.1:{port}");
        let mut ps = NetworkProxySettings::new(Some(&uri), &[]);
        dm.set_network_proxy_settings(NetworkProxyMode::Custom, Some(&mut ps));
    }
    ctx.set_cache_model(webkit2gtk::CacheModel::WebBrowser);
    cbtv::log::d("player", &format!("WebKit đi qua cổng nội bộ 127.0.0.1:{port}"));
}

impl Player {
    pub fn new() -> Rc<Player> {
        setup_web_context();
        let ucm = UserContentManager::new();
        // Cho trang phát tự phát video có tiếng (mặc định WebKitGTK chỉ cho video không tiếng tự phát)
        let policies = webkit2gtk::WebsitePolicies::builder().autoplay(webkit2gtk::AutoplayPolicy::Allow).build();
        let web = WebView::builder().user_content_manager(&ucm).website_policies(&policies).build();
        web.set_background_color(&gdk::RGBA::new(0.0, 0.0, 0.0, 1.0));
        web.set_can_focus(false);
        if let Some(st) = WebViewExt::settings(&web) {
            st.set_media_playback_requires_user_gesture(false);
            st.set_javascript_can_open_windows_automatically(false);
            st.set_enable_fullscreen(true);
            // Chỉ bật khi cần soi (CONBANG_CONSOLE=1): trang phát như streamfree.vip dò công cụ lập trình bằng cách đo thời gian
            // ghi console — ghi ra ngoài làm chậm → trang tưởng đang mở công cụ lập trình, không phát
            let console = std::env::var_os("CONBANG_CONSOLE").is_some();
            st.set_enable_developer_extras(console);
            st.set_enable_write_console_messages_to_stdout(console);
            st.set_user_agent(Some(&web_user_agent()));
            // Vẽ trang phát. Máy không có GPU (máy ảo, không có /dev/dri/renderD*) mà để WebKit dựng lớp bằng OpenGL thì nó chạy
            // OpenGL giả lập (llvmpipe): đo trên máy ảo thử, phim 1080p ăn 240% CPU + 338 MB; tắt hẳn thì còn 85% + 203 MB.
            // Máy có GPU giữ mặc định của WebKit. CONBANG_HWACCEL=never | always | ondemand để ép.
            use webkit2gtk::HardwareAccelerationPolicy as Hw;
            match std::env::var("CONBANG_HWACCEL").as_deref() {
                Ok("never") => st.set_hardware_acceleration_policy(Hw::Never),
                Ok("always") => st.set_hardware_acceleration_policy(Hw::Always),
                Ok("ondemand") => st.set_hardware_acceleration_policy(Hw::OnDemand),
                _ if !has_gpu() => st.set_hardware_acceleration_policy(Hw::Never),
                _ => {}
            }
        }
        if std::env::var("CONBANG_MUTE").as_deref() == Ok("1") {
            web.set_is_muted(true);
        }

        // ---- Lớp phủ ----
        let root = gtk::Overlay::new();
        root.style_context().add_class("player");
        let stage = gtk::Stack::new();
        stage.add_named(&web, "web");
        root.add(&stage);

        let hit = gtk::EventBox::new();
        hit.set_visible_window(false);
        hit.set_above_child(false);
        hit.add_events(gdk::EventMask::POINTER_MOTION_MASK | gdk::EventMask::BUTTON_PRESS_MASK | gdk::EventMask::BUTTON_RELEASE_MASK);
        root.add_overlay(&hit);

        // Thanh trên: quay lại + tên phim + thu nhỏ / toàn màn hình
        let topbar = gtk::EventBox::new();
        topbar.set_visible_window(false);
        topbar.set_valign(gtk::Align::Start);
        let tb = ui::hbox(12.0);
        tb.style_context().add_class("topbar");
        tb.pack_start(&osd_button("←  Quay lại", |p| p.close(true)), false, false, 0);
        let title = ui::label("", &["osd-title"], false);
        tb.pack_start(&title, true, true, 0);
        let fs = osd_button("⛶  Toàn màn hình", |_| app().toggle_fullscreen());
        // Nhãn theo trạng thái (bật / tắt bằng F11, phím F, menu… đều đổi theo); trình phát chỉ tạo một lần nên nối một lần
        let fs2 = fs.clone();
        app().window.connect_window_state_event(move |_, ev| {
            let on = ev.new_window_state().contains(gdk::WindowState::FULLSCREEN);
            fs2.set_label(if on { "⤡  Thoát toàn màn hình" } else { "⛶  Toàn màn hình" });
            glib::Propagation::Proceed
        });
        tb.pack_end(&fs, false, false, 0);
        tb.pack_end(&osd_button("▣  Thu nhỏ", |p| p.enter_mini()), false, false, 0);
        topbar.add(&tb);
        root.add_overlay(&topbar);

        // Đang tải / lỗi
        let loading_box = ui::vbox(14.0);
        loading_box.style_context().add_class("box-dark");
        loading_box.set_halign(gtk::Align::Center);
        loading_box.set_valign(gtk::Align::Center);
        let loading_bar = gtk::ProgressBar::new();
        loading_bar.set_size_request(px(240.0), -1);
        loading_bar.set_halign(gtk::Align::Center);
        let loading_text = ui::text("");
        loading_text.set_justify(gtk::Justification::Center);
        loading_text.set_halign(gtk::Align::Center);
        loading_text.set_max_width_chars(70);
        loading_box.pack_start(&loading_bar, false, false, 0);
        loading_box.pack_start(&loading_text, false, false, 0);
        root.add_overlay(&loading_box);
        root.set_overlay_pass_through(&loading_box, true);

        // Thông báo ngắn
        let notice_box = ui::hbox(0.0);
        notice_box.style_context().add_class("notice");
        notice_box.set_halign(gtk::Align::Center);
        notice_box.set_valign(gtk::Align::Start);
        notice_box.set_margin_top(px(90.0));
        let notice_text = ui::text("");
        notice_text.set_justify(gtk::Justification::Center);
        notice_text.set_max_width_chars(80);
        notice_box.pack_start(&notice_text, false, false, 0);
        root.add_overlay(&notice_box);
        root.set_overlay_pass_through(&notice_box, true);

        // Thanh dưới: thời gian, thanh tua, nút
        let osd = gtk::EventBox::new();
        osd.set_visible_window(false);
        osd.set_valign(gtk::Align::End);
        let ob = ui::vbox(0.0);
        ob.style_context().add_class("osd");
        let line = ui::hbox(12.0);
        let time_text = ui::label("", &["osd-time"], false);
        let state_text = ui::label("", &["osd-state"], false);
        line.pack_start(&time_text, false, false, 0);
        line.pack_end(&state_text, false, false, 0);
        ob.pack_start(&line, false, false, 0);
        let seek = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
        seek.style_context().add_class("seek");
        seek.set_draw_value(false);
        seek.set_can_focus(false);
        seek.set_margin_top(px(2.0));
        seek.set_margin_bottom(px(8.0));
        ob.pack_start(&seek, false, false, 0);
        let btns = ui::hbox(10.0);
        btns.set_halign(gtk::Align::Center);
        let btn_prev = osd_button("⏮  Tập trước", |p| p.prev());
        let btn_play = osd_button("⏯  Phát / Dừng", |p| p.toggle());
        btn_play.set_size_request(px(150.0), -1);
        let btn_next = osd_button("Tập sau  ⏭", |p| p.next());
        btns.pack_start(&btn_prev, false, false, 0);
        btns.pack_start(&osd_button("↺ 10s", |p| p.seek_by(-10.0)), false, false, 0);
        btns.pack_start(&btn_play, false, false, 0);
        btns.pack_start(&osd_button("10s ↻", |p| p.seek_by(10.0)), false, false, 0);
        btns.pack_start(&btn_next, false, false, 0);
        btns.pack_start(&osd_button("☰  Server · Tập · Tốc độ", |p| p.show_panel()), false, false, 0);
        ob.pack_start(&btns, false, false, 0);
        osd.add(&ob);
        root.add_overlay(&osd);

        // Bảng tuỳ chọn bên phải (▼ / M / nút ☰)
        let panel = ui::vbox(0.0);
        panel.style_context().add_class("panel");
        panel.set_halign(gtk::Align::End);
        panel.set_size_request(px(470.0), -1);
        let ph = ui::hbox(8.0);
        ui::margins(&ph, 18.0, 16.0, 6.0, 22.0);
        let panel_title = ui::label("", &["panel-title"], true);
        ph.pack_start(&panel_title, true, true, 0);
        let close = ui::pill("✕", || {
            if let Some(p) = me() {
                p.hide_panel();
            }
        });
        close.style_context().add_class("small");
        ph.pack_end(&close, false, false, 0);
        panel.pack_start(&ph, false, false, 0);
        let panel_scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        panel_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        panel_scroll.set_can_focus(false);
        let panel_stack = ui::vbox(0.0);
        ui::margins(&panel_stack, 4.0, 14.0, 22.0, 22.0);
        panel_scroll.add(&panel_stack);
        panel.pack_start(&panel_scroll, true, true, 0);
        root.add_overlay(&panel);

        // Chế độ bấm thẳng vào trang
        let btn_done = osd_button("✓  Xong — quay lại điều khiển bằng phím", |p| p.exit_click_through());
        btn_done.set_halign(gtk::Align::Center);
        btn_done.set_valign(gtk::Align::Start);
        btn_done.set_margin_top(px(14.0));
        root.add_overlay(&btn_done);

        for w in [loading_box.upcast_ref::<gtk::Widget>(), notice_box.upcast_ref(), panel.upcast_ref(), btn_done.upcast_ref()] {
            w.set_no_show_all(true);
        }
        for w in [&loading_box, &notice_box, &panel] {
            for c in w.children() {
                c.show_all();
            }
        }

        let p = Rc::new(Player {
            root,
            stage,
            web,
            native: RefCell::new(None),
            ucm,
            hit,
            topbar,
            title,
            loading_box,
            loading_bar,
            loading_text,
            notice_box,
            notice_text,
            osd,
            time_text,
            state_text,
            seek,
            btn_play,
            btn_prev,
            btn_next,
            panel,
            panel_title,
            panel_stack,
            btn_done,
            open: Cell::new(false),
            s: RefCell::new(St { rate: 1.0, ..Default::default() }),
            poll_timer: RefCell::new(None),
            osd_timer: RefCell::new(None),
            notice_timer: RefCell::new(None),
            seek_timer: RefCell::new(None),
            click_timer: RefCell::new(None),
            mini: RefCell::new(None),
        });
        PLAYER.with(|x| *x.borrow_mut() = Some(Rc::downgrade(&p)));
        p.connect();
        p
    }

    fn connect(self: &Rc<Self>) {
        // Cửa sổ quảng cáo / trang tự mở cửa sổ mới: chặn
        self.web.connect_create(|_, _| None);
        self.web.connect_context_menu(|_, _, _, _| true);
        self.web.connect_load_changed(|_, ev| {
            if let Some(p) = me() {
                if ev == webkit2gtk::LoadEvent::Started {
                    p.on_main_load_started();
                }
                if matches!(ev, webkit2gtk::LoadEvent::Committed | webkit2gtk::LoadEvent::Finished) {
                    p.inject_page();
                }
            }
        });
        self.web.connect_web_process_terminated(|_, r| {
            use webkit2gtk::WebProcessTerminationReason as R;
            // App tự tắt (terminate_web — trả bộ nhớ) thì không phải lỗi
            if !matches!(r, R::Crashed | R::ExceededMemoryLimit) {
                cbtv::log::d("player", "Đã tắt tiến trình WebKit của trình phát (trả bộ nhớ)");
                return;
            }
            cbtv::log::d("player", &format!("Trình duyệt lỗi: {r:?}"));
            if let Some(p) = me() {
                if p.is_open() {
                    p.show_error("Trình duyệt bị lỗi khi phát.\nNhấn ▼ → «Tải lại».");
                }
            }
        });
        // Chuột: rê để hiện thanh điều khiển, bấm phát / dừng, bấm đúp toàn màn hình
        self.hit.connect_motion_notify_event(|_, _| {
            if let Some(p) = me() {
                if !p.s.borrow().click_through {
                    p.set_cursor(true);
                    p.show_osd(3000);
                }
            }
            glib::Propagation::Proceed
        });
        self.hit.connect_button_press_event(|_, ev| {
            let Some(p) = me() else { return glib::Propagation::Proceed };
            if p.s.borrow().click_through || ev.button() != 1 {
                return glib::Propagation::Proceed;
            }
            if p.panel.is_visible() {
                p.hide_panel();
                return glib::Propagation::Stop;
            }
            if ev.event_type() == gdk::EventType::DoubleButtonPress {
                if let Some(id) = p.click_timer.borrow_mut().take() {
                    id.remove();
                }
                app().toggle_fullscreen();
                return glib::Propagation::Stop;
            }
            if ev.event_type() == gdk::EventType::ButtonPress {
                if let Some(id) = p.click_timer.borrow_mut().take() {
                    id.remove();
                }
                let id = ui::after(260, || {
                    if let Some(p) = me() {
                        p.click_timer.borrow_mut().take();
                        p.toggle();
                    }
                });
                *p.click_timer.borrow_mut() = Some(id);
            }
            glib::Propagation::Stop
        });
        for w in [&self.osd, &self.topbar] {
            w.connect_enter_notify_event(|_, _| {
                if let Some(p) = me() {
                    p.s.borrow_mut().osd_hover = true;
                }
                glib::Propagation::Proceed
            });
            w.connect_leave_notify_event(|_, ev| {
                if let Some(p) = me() {
                    if ev.detail() != gdk::NotifyType::Inferior {
                        p.s.borrow_mut().osd_hover = false;
                    }
                }
                glib::Propagation::Proceed
            });
        }
        // Thanh tua: kéo / bấm nhảy tới chỗ đó
        self.seek.connect_button_press_event(|_, _| {
            if let Some(p) = me() {
                p.s.borrow_mut().dragging = true;
            }
            glib::Propagation::Proceed
        });
        self.seek.connect_button_release_event(|s, _| {
            if let Some(p) = me() {
                let was = std::mem::replace(&mut p.s.borrow_mut().dragging, false);
                let has_d = p.s.borrow().state.map(|x| x.d > 0.0).unwrap_or(false);
                if was && has_d {
                    p.seek_to(s.value());
                }
            }
            glib::Propagation::Proceed
        });
        self.seek.connect_value_changed(|s| {
            if let Some(p) = me() {
                let st = p.s.borrow();
                if st.dragging {
                    if let Some(x) = st.state.filter(|x| x.d > 0.0) {
                        p.time_text.set_text(&format!("{} / {}", format_time(s.value()), format_time(x.d)));
                    }
                }
            }
        });
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    // ---- Cửa sổ phim nhỏ (bản Windows: Player/MiniWindow.cs, bản TV: MiniPlayer) ----

    pub fn is_mini(&self) -> bool {
        self.mini.borrow().is_some()
    }

    /// Tiêu đề cửa sổ đang hiện phim (cửa sổ nhỏ hoặc cửa sổ app).
    fn set_window_title(&self, t: &str) {
        match self.mini.borrow().as_ref() {
            Some(m) => m.win.set_title(&format!("{t} · Côn Bằng TV")),
            None => app().window.set_title(&format!("{t} · Côn Bằng TV")),
        }
    }

    /// Thu nhỏ (nút «▣ Thu nhỏ», phím W): phim chạy tiếp ở cửa sổ nhỏ, cửa sổ app trở lại các trang. Thanh dưới cửa sổ nhỏ:
    /// phát / dừng, phóng to, tắt; bấm vào phim = phát / dừng, bấm đúp = phóng to. X11: cửa sổ luôn nổi trên cùng ở góc dưới
    /// phải; GNOME Wayland không cho app tự đặt chỗ / tự nổi trên cùng — người dùng chọn «Luôn ở trên» (Alt+Space) nếu muốn.
    pub fn enter_mini(self: &Rc<Self>) {
        if !self.is_open() || self.is_mini() {
            return;
        }
        self.hide_panel();
        self.exit_click_through();
        self.osd.hide();
        self.topbar.hide();
        self.set_cursor(true);
        let a = app();
        if a.is_fullscreen() {
            a.set_fullscreen(false);
        }
        let win = gtk::Window::new(gtk::WindowType::Toplevel);
        win.set_icon_name(Some("vn.chimto.tv"));
        let (w, h) = (px(480.0), px(270.0) + px(46.0));
        win.set_default_size(w, h);
        win.set_keep_above(true);
        win.style_context().add_class("bg");

        let holder = gtk::Overlay::new();
        self.root.remove(&self.stage);
        holder.add(&self.stage);
        // Lớp bắt chuột phủ trên phim (không để chuột chạm vào trang — quảng cáo): bấm = phát / dừng, bấm đúp = phóng to
        let hit = gtk::EventBox::new();
        hit.set_visible_window(false);
        hit.add_events(gdk::EventMask::BUTTON_PRESS_MASK);
        hit.connect_button_press_event(|_, ev| {
            if let Some(p) = me() {
                if ev.event_type() == gdk::EventType::DoubleButtonPress {
                    p.exit_mini(true);
                } else if ev.event_type() == gdk::EventType::ButtonPress && ev.button() == 1 {
                    p.toggle();
                }
            }
            glib::Propagation::Stop
        });
        holder.add_overlay(&hit);

        let bar = ui::hbox(6.0);
        bar.style_context().add_class("mini-bar");
        // Tên phim đã ở thanh tiêu đề cửa sổ — thanh dưới chỉ có nút, dồn sang phải
        bar.pack_start(&ui::hbox(0.0), true, true, 0);
        let small = |text: &str, tip: &str, f: fn(&Rc<Player>)| {
            let b = ui::pill(text, move || {
                if let Some(p) = me() {
                    f(&p);
                }
            });
            b.set_can_focus(false);
            b.set_tooltip_text(Some(tip));
            b
        };
        let paused = self.s.borrow().state.map(|x| x.paused).unwrap_or(false);
        let play = small(if paused { "▶" } else { "⏸" }, "Phát / dừng (Space)", |p| p.toggle());
        bar.pack_start(&play, false, false, 0);
        bar.pack_start(&small("⤢", "Phóng to (Enter / bấm đúp vào phim)", |p| p.exit_mini(true)), false, false, 0);
        bar.pack_start(&small("✕", "Tắt phim (Esc)", |p| p.close(true)), false, false, 0);
        // Bấm đúp thanh dưới cũng phóng to (như bản Windows)
        let bar_ev = gtk::EventBox::new();
        bar_ev.add(&bar);
        bar_ev.connect_button_press_event(|_, ev| {
            if ev.event_type() == gdk::EventType::DoubleButtonPress {
                if let Some(p) = me() {
                    p.exit_mini(true);
                }
            }
            glib::Propagation::Proceed
        });

        let vb = ui::vbox(0.0);
        vb.pack_start(&holder, true, true, 0);
        vb.pack_start(&bar_ev, false, false, 0);
        win.add(&vb);
        win.connect_key_press_event(|_, ev| {
            use gdk::keys::constants as k;
            let Some(p) = me() else { return glib::Propagation::Proceed };
            let key = ev.keyval();
            if key == k::space || key == k::k || key == k::K || key == k::AudioPlay || key == k::AudioPause {
                p.toggle();
            } else if key == k::Return || key == k::KP_Enter || key == k::w || key == k::W {
                p.exit_mini(true);
            } else if key == k::Escape {
                p.close(true);
            } else if key == k::Left || key == k::Right {
                let d = p.step(0);
                p.seek_by(if key == k::Left { -d } else { d });
            } else {
                return glib::Propagation::Proceed;
            }
            glib::Propagation::Stop
        });
        // Đóng bằng nút của cửa sổ / Alt+F4: tắt phim (lấy trình phát về trước)
        win.connect_delete_event(|_, _| {
            if let Some(p) = me() {
                p.close(true);
            }
            glib::Propagation::Stop
        });
        // X11: góc dưới phải vùng làm việc của màn hình chính (Wayland bỏ qua)
        if let Some(mon) = gdk::Display::default().and_then(|d| d.primary_monitor().or_else(|| d.monitor(0))) {
            let wa = mon.workarea();
            win.move_(wa.x() + wa.width() - w - px(24.0), wa.y() + wa.height() - h - px(24.0));
        }
        *self.mini.borrow_mut() = Some(Mini { win: win.clone(), holder, play });
        self.set_window_title(&self.title.text());
        win.show_all();
        a.player_minimized();
    }

    /// Phóng to lại (show = hiện trình phát trong cửa sổ app); false khi tắt phim / mở phim khác.
    pub fn exit_mini(&self, show: bool) {
        let Some(m) = self.mini.borrow_mut().take() else { return };
        m.holder.remove(&self.stage);
        // Con chính của Overlay luôn vẽ dưới các lớp phủ (thanh điều khiển, bảng…)
        self.root.add(&self.stage);
        // SAFETY: cửa sổ nhỏ chỉ được giữ ở đây; khung hình đã gỡ ra trước nên không bị huỷ theo
        unsafe { m.win.destroy() };
        if !show {
            return;
        }
        self.set_window_title(&self.title.text());
        app().player_restored();
        self.show_osd(4000);
    }

    // ---- WebKit ----

    /// Chặn mọi điều hướng cả trang do trang tự gây ra (quảng cáo); app tự nạp trang. Chỉ khung trên cùng mới báo «bắt đầu tải»
    /// (khung con — iframe trình phát — không), chuyển hướng phía máy chủ báo «Redirected» nên không bị chặn. Dừng tải thì
    /// trang đang hiện vẫn giữ nguyên.
    fn on_main_load_started(&self) {
        let uri = self.web.uri().map(|u| u.to_string()).unwrap_or_default();
        {
            let mut s = self.s.borrow_mut();
            if let Some(e) = &s.expect_nav {
                if same_url(&uri, e) {
                    s.expect_nav = None;
                    return;
                }
            }
        }
        if uri.is_empty() || uri == "about:blank" || !self.is_open() {
            return;
        }
        cbtv::log::d("player", &format!("Chặn điều hướng: {uri}"));
        self.web.stop_loading();
    }
    fn inject_page(&self) {
        if !self.s.borrow().page_mode {
            return;
        }
        self.js(crate::assets::PAGE_FOCUS_JS);
    }

    fn js(&self, code: &str) {
        #[allow(deprecated)]
        self.web.run_javascript(code, None::<&gio::Cancellable>, |_| {});
    }

    /// Nạp trang (html = Some: trả lời bằng trang bọc của app tại địa chỉ url).
    fn navigate(&self, url: &str, html: Option<String>, start: i64) {
        let load = self.s.borrow().page_load;
        self.ucm.remove_all_scripts();
        let mut js = format!(
            "window.__chimtoStart={start};if(window.top===window)window.__chimtoLoad={load};\n{STYLE_SHIM}{}",
            crate::assets::PLAYER_AGENT_JS
        );
        if std::env::var_os("CONBANG_DEBUG").is_some() {
            js.push_str(DIAG_JS);
        }
        if self.s.borrow().extract {
            js.push_str(EXTRACT_JS);
        }
        let script = UserScript::new(&js, UserContentInjectedFrames::AllFrames, UserScriptInjectionTime::Start, &[], &[]);
        self.ucm.add_script(&script);
        self.s.borrow_mut().expect_nav = Some(url.to_string());
        match html {
            Some(h) => self.web.load_alternate_html(&h, url, Some(url)),
            None => self.web.load_uri(url),
        }
    }

    fn blank(&self) {
        // WebView chưa từng mở trang (chỉ phát native) / đã trống: khỏi nạp — nạp about:blank cũng sinh tiến trình WebKit (~45 MB)
        if self.web.uri().map(|u| u.is_empty() || u == "about:blank").unwrap_or(true) {
            return;
        }
        self.s.borrow_mut().expect_nav = Some("about:blank".into());
        self.web.load_uri("about:blank");
    }

    fn cmd(&self, name: &str, arg: Option<f64>) {
        if self.s.borrow().native_on {
            if let Some(n) = self.native.borrow().as_ref() {
                n.cmd(name, arg);
            }
            return;
        }
        let a = arg.map(|x| format!("{x}")).unwrap_or_else(|| "null".into());
        self.js(&format!("window.chimtoCmd&&chimtoCmd('{name}',{a})"));
    }

    // ---- Mở / đóng ----

    pub fn open(self: &Rc<Self>, film_url: &str, ep_url: &str, resume: bool) {
        let was_open = self.open.replace(true);
        if was_open {
            self.s.borrow_mut().token += 1;
            self.stop_video();
        }
        self.hide_panel();
        {
            let mut s = self.s.borrow_mut();
            s.film_url = film_url.to_string();
            s.open_ep_url = ep_url.to_string();
            s.resume_open = resume;
            s.detail = None;
            s.source = None;
            s.group = None;
            s.index = 0;
            s.limit_warned = false;
            s.load_token += 1;
        }
        self.start_poll();
        let t = self.s.borrow().load_token;
        if let Some(cached) = cbtv::site::cached_detail(film_url) {
            self.start(cached, ep_url);
            return;
        }
        self.show_loading("Đang tải thông tin phim…");
        self.title.set_text("");
        app().window.set_title("Côn Bằng TV");
        let (fu, eu) = (film_url.to_string(), ep_url.to_string());
        let me = self.clone();
        ui::spawn(async move { cbtv::site::detail(&fu).await }, move |r| {
            if t != me.s.borrow().load_token || !me.is_open() {
                return;
            }
            match r {
                Ok(d) => me.start(d, &eu),
                Err(e) => me.show_error(&format!("Không tải được phim.\n{}", cbtv::util::friendly(&e))),
            }
        });
    }

    fn start(self: &Rc<Self>, d: FilmDetail, ep_url: &str) {
        // Phim đang theo dõi: đã vào xem thì không còn là "tập mới"
        if let Some(last) = d.latest_episode() {
            prefs::update_follow(&d.url, d.episode_count() as i64, &last.title, true);
        }
        let g = d.group_of(ep_url).cloned().or_else(|| d.groups.first().cloned());
        let Some(g) = g else {
            self.s.borrow_mut().detail = Some(d);
            self.show_error("Phim này chưa có tập nào.");
            return;
        };
        let i = g.episodes.iter().position(|e| e.url == ep_url).unwrap_or(0);
        let resume_open = self.s.borrow().resume_open;
        let next = resume_open && prefs::finished(ep_url) && i + 1 < g.episodes.len();
        {
            let mut s = self.s.borrow_mut();
            s.detail = Some(d);
            s.group = Some(g);
        }
        self.open_episode(if next { i + 1 } else { i });
    }

    /// Thoát trình phát (lưu chỗ xem, về trang trước).
    pub fn close(&self, notify: bool) {
        if !self.open.replace(false) {
            return;
        }
        self.exit_mini(false);
        {
            let mut s = self.s.borrow_mut();
            s.token += 1;
            s.load_token += 1;
        }
        self.stop_video();
        // Trả bộ nhớ khi không xem phim: trình phát native (mpv) và tiến trình WebKit của trình phát; lần xem sau tạo lại
        self.drop_native();
        self.terminate_web();
        crate::limits::flush();
        self.stop_poll();
        if let Some(id) = self.click_timer.borrow_mut().take() {
            id.remove();
        }
        self.exit_click_through();
        self.hide_panel();
        self.set_cursor(true);
        app().window.set_title("Côn Bằng TV");
        if notify {
            app().player_closed();
        }
    }

    fn start_poll(self: &Rc<Self>) {
        if self.poll_timer.borrow().is_some() {
            return;
        }
        let id = glib::timeout_add_local(std::time::Duration::from_millis(1000), || match me() {
            Some(p) if p.is_open() => {
                p.poll();
                glib::ControlFlow::Continue
            }
            Some(p) => {
                p.poll_timer.borrow_mut().take();
                glib::ControlFlow::Break
            }
            None => glib::ControlFlow::Break,
        });
        *self.poll_timer.borrow_mut() = Some(id);
    }

    fn stop_poll(&self) {
        if let Some(id) = self.poll_timer.borrow_mut().take() {
            id.remove();
        }
    }

    // ---- Mở tập / server ----

    fn episodes_len(&self) -> usize {
        self.s.borrow().group.as_ref().map(|g| g.episodes.len()).unwrap_or(0)
    }

    fn open_episode(self: &Rc<Self>, i: usize) {
        let (d, g) = {
            let s = self.s.borrow();
            match (&s.detail, &s.group) {
                (Some(d), Some(g)) if i < g.episodes.len() => (d.clone(), g.clone()),
                _ => return,
            }
        };
        let ep = g.episodes[i].clone();
        // Người đang xem đã hết giờ xem: chưa mở tập, hỏi mã phụ huynh trước
        if let Some(b) = crate::limits::blocked() {
            self.limit_block(b, &d.url, &ep.url);
            return;
        }
        self.stop_video();
        let t = {
            let mut s = self.s.borrow_mut();
            s.index = i;
            s.token += 1;
            s.episode_url = ep.url.clone();
            s.source = None;
            s.force_web = false;
            s.resume_at = prefs::position_of(&ep.url);
            s.intro_done = false;
            s.token
        };
        let group_name = if d.groups.len() > 1 { format!(" ({})", g.name) } else { String::new() };
        self.title.set_text(&format!("{} — {}{}", d.title, ep.title, group_name));
        self.set_window_title(&format!("{} — {}", d.title, ep.title));
        prefs::add_history(HistoryItem {
            film_url: d.url.clone(),
            title: d.title.clone(),
            poster: d.poster.clone(),
            ep_title: ep.title.clone(),
            ep_url: ep.url.clone(),
            time: cbtv::util::now_ms(),
        });
        // Trang chủ tổng hợp: hết báo «tập mới» tới khi có tập sau; người hay xem bản thuyết minh / lồng tiếng thì có hàng riêng
        crate::hooks::mark_seen(&d);
        if let Some(lang) = cbtv::kk_phim::lang(&g.name) {
            prefs::record_watch_lang(&lang);
        }
        self.show_loading(&format!("Đang mở {}…", ep.title));
        self.show_osd(4000);
        let me = self.clone();
        let ep2 = ep.clone();
        ui::spawn(async move { cbtv::kk_phim::source(&d, Some(&g), &ep2).await }, move |r| {
            if t != me.s.borrow().token || !me.is_open() {
                return;
            }
            match r {
                Ok(src) => {
                    let page_only = src.page_only();
                    me.s.borrow_mut().source = Some(src.clone());
                    if page_only {
                        me.play_page(&src, None);
                        return;
                    }
                    let host = cbtv::site::host_of(&src.ep_url);
                    let want = prefs::server_type(host.as_deref());
                    let usual = src.servers.iter().find(|s| Some(&s.kind) == want.as_ref()).or(src.servers.first()).cloned();
                    me.play_server(usual);
                }
                Err(e) => me.show_error(&format!("Không mở được {}.\n{}", ep.title, cbtv::util::friendly(&e))),
            }
        });
    }

    /// Mở thẳng trang tập: trang tự nạp trình phát, page-focus.js phóng nó ra toàn màn hình.
    fn play_page(self: &Rc<Self>, src: &EpisodeSource, as_server: Option<Server>) {
        self.stop_video();
        let start = {
            let mut s = self.s.borrow_mut();
            s.server = as_server;
            s.page_mode = true;
            s.load_started_at = cbtv::util::now_ms();
            drop(s);
            self.start_for()
        };
        self.s.borrow_mut().start_hint = start;
        self.show_loading("Đang mở trang tập phim…");
        self.navigate(&src.ep_url, None, start);
    }

    fn play_server(self: &Rc<Self>, s: Option<Server>) {
        let Some(src) = self.s.borrow().source.clone() else { return };
        // "Trình phát của trang" (trang mở thẳng trang tập, có thêm server KKPhim)
        if s.as_ref().map(|x| x.kind == EpisodeSource::PAGE_SERVER).unwrap_or(false) {
            self.s.borrow_mut().token += 1;
            self.play_page(&src, s);
            return;
        }
        let t = {
            let mut st = self.s.borrow_mut();
            st.server = s.clone();
            st.token += 1;
            st.token
        };
        self.stop_video();
        self.show_loading(&format!("Đang tải {}…", s.as_ref().map(|x| x.label.as_str()).unwrap_or("trình phát")));
        let me = self.clone();
        let s2 = s.clone();
        ui::spawn(async move { cbtv::site::video_of(&src, s2.as_ref()).await.map(|v| (v, src)) }, move |r| {
            if t != me.s.borrow().token || !me.is_open() {
                return;
            }
            match r {
                Ok((v, src)) => {
                    me.s.borrow_mut().load_started_at = cbtv::util::now_ms();
                    let start = me.start_for();
                    me.s.borrow_mut().start_hint = start;
                    let is_hls = v.file || url::Url::parse(&v.url).map(|u| u.path().ends_with(".m3u8")).unwrap_or(false);
                    let native = me.native_allowed();
                    // Trình phát native: luồng .m3u8 / tệp video phát thẳng bằng mpv
                    if is_hls && native {
                        me.s.borrow_mut().hls_url = Some(v.url.clone());
                        if me.play_native(&v.url, v.subtitle.as_deref(), &src.ep_url, start, false) {
                            return;
                        }
                        me.s.borrow_mut().hls_url = None;
                    }
                    // Trình phát nhúng: mở như cũ, đọc link luồng trong JW Player rồi chuyển sang mpv (EXTRACT_JS)
                    me.s.borrow_mut().extract = native && !is_hls;
                    let page = if is_hls {
                        me.s.borrow_mut().hls_url = Some(v.url.clone());
                        hls_page(&v.url, v.subtitle.as_deref(), start)
                    } else {
                        me.s.borrow_mut().embed_url = Some(v.url.clone());
                        crate::assets::PLAYER_HTML.replace("{{SRC}}", &html_escape(&v.url))
                    };
                    cbtv::log::d(
                        "player",
                        &format!("Phát {} {} (server {}, bắt đầu {start}s)", if is_hls { "HLS" } else { "iframe" }, v.url, s.as_ref().map(|x| x.label.as_str()).unwrap_or("")),
                    );
                    me.navigate(&src.ep_url, Some(page), start);
                }
                Err(e) => {
                    me.show_error(&format!("{}\nNhấn ▼ (hoặc ☰) để chọn server khác.", cbtv::util::friendly(&e)));
                    me.show_panel();
                }
            }
        });
    }

    // ---- Trình phát native (libmpv) ----

    /// Được dùng trình phát native cho lần phát này: đã bật trong cài đặt, máy có libmpv, server này chưa lỗi với mpv.
    fn native_allowed(&self) -> bool {
        prefs::native_player() && !self.s.borrow().force_web && crate::native::available()
    }

    /// Tắt hẳn tiến trình WebKit của trình phát: trang đã đóng mà tiến trình vẫn giữ 200–300 MB (đo trên máy ảo thử); lần nạp
    /// trang sau WebKit tự mở tiến trình mới. Hàm có từ WebKitGTK 2.34 — tra lúc chạy để bản dựng vẫn chạy trên bản cũ hơn.
    fn terminate_web(&self) {
        use glib::object::ObjectType;
        static SYM: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        // SAFETY: tra ký hiệu trong các thư viện đã nạp
        let f = *SYM.get_or_init(|| unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"webkit_web_view_terminate_web_process".as_ptr()) as usize });
        if f == 0 || self.web.uri().is_none() {
            return;
        }
        // SAFETY: void webkit_web_view_terminate_web_process(WebKitWebView*) — đúng kiểu, WebView còn sống
        unsafe {
            let f: unsafe extern "C" fn(*mut std::ffi::c_void) = std::mem::transmute(f);
            f(self.web.as_ptr() as *mut std::ffi::c_void);
        }
    }

    fn native_video(&self) -> Option<Rc<crate::native::Native>> {
        if let Some(n) = self.native.borrow().as_ref() {
            return Some(n.clone());
        }
        let n = crate::native::Native::new(has_gpu())?;
        self.stage.add_named(&n.widget, "native");
        n.widget.show();
        *self.native.borrow_mut() = Some(n.clone());
        Some(n)
    }

    fn drop_native(&self) {
        if let Some(n) = self.native.borrow_mut().take() {
            self.stage.remove(&n.widget);
            drop(n);
            cbtv::media_proxy::close_all();
            cbtv::log::d("native", "Đã tắt trình phát native");
        }
    }

    /// Phát `src` (luồng .m3u8 / tệp video / data:…mpegurl) bằng mpv qua cổng phát nội bộ; `hls` = biết chắc là luồng HLS dù
    /// link không có đuôi .m3u8. false = không dùng được (bên gọi tự phát bằng WebView).
    fn play_native(self: &Rc<Self>, src: &str, sub: Option<&str>, page: &str, start: i64, hls: bool) -> bool {
        let Some(n) = self.native_video() else { return false };
        let Some(o) = cbtv::media_proxy::open(src, sub, Some(page), hls) else { return false };
        let shown = if src.starts_with("data:") { "data:… (link do trình phát nhúng giải mã)".to_string() } else { src.to_string() };
        cbtv::log::d("native", &format!("Phát native {shown} (trang {page}, bắt đầu {start}s)"));
        {
            let mut s = self.s.borrow_mut();
            s.native_on = true;
            s.extract = false;
            s.load_started_at = cbtv::util::now_ms();
        }
        // Trang nhúng (nếu đang mở để lấy link) / trang của lần xem trước không cần nữa: tắt cả tiến trình WebKit để trả bộ nhớ
        self.blank();
        self.terminate_web();
        self.stage.set_visible_child_name("native");
        n.load(&o.url, start as f64, o.subtitle);
        true
    }

    /// Đọc trạng thái mpv, đưa vào cùng đường xử lý với trạng thái của player-agent.js.
    fn poll_native(self: &Rc<Self>) {
        let Some(n) = self.native.borrow().clone() else { return };
        let st = n.state();
        // mpv hỏng thì phát lại bằng WebView (hls.html / trình phát nhúng) như trước khi có mpv, cùng server, tiếp đúng chỗ:
        // - lỗi trước khi phát được giây nào (không mở được luồng)
        // - lỗi giữa chừng (luồng đứt mà mpv đã thử lại hết — cổng nội bộ tự thử lại 3 lần, FFmpeg thêm 5 lần mỗi đoạn)
        // - treo: quá NATIVE_STALL_MS mà chưa chạy được, cũng không báo lỗi
        let (started, waited) = {
            let s = self.s.borrow();
            (s.started, cbtv::util::now_ms() - s.load_started_at)
        };
        if let Some(e) = st.err.as_ref() {
            self.native_fallback(&format!("mpv lỗi ({e}){}", if started { " giữa chừng" } else { "" }));
            return;
        }
        if !started && waited > NATIVE_STALL_MS {
            self.native_fallback(&format!("mpv chưa chạy được sau {} giây", waited / 1000));
            return;
        }
        let load = self.s.borrow().page_load;
        let o = serde_json::json!({
            "t": st.t, "d": st.d, "paused": st.paused, "ended": st.ended, "waiting": st.waiting, "ready": st.ready,
            "err": st.err.unwrap_or_default(),
        });
        self.on_state(Some(serde_json::json!([load, o]).to_string()));
    }

    /// Trình phát native hỏng: phát lại server này bằng WebView (tới khi đổi tập / đổi server), tiếp đúng chỗ đang xem.
    fn native_fallback(self: &Rc<Self>, why: &str) {
        cbtv::log::d("native", &format!("{why} — chuyển sang trình phát web"));
        self.notice("Trình phát native không phát được — chuyển sang trình phát web.", 5000);
        if let Some(t) = self.current_time() {
            self.s.borrow_mut().resume_at = t;
        }
        self.s.borrow_mut().force_web = true;
        let server = self.s.borrow().server.clone();
        self.play_server(server);
    }

    /// Cổng gỡ lỗi (bản thử): giả lỗi giữa chừng của mpv / làm cổng phát nội bộ treo — thử đường tự chuyển sang WebView.
    #[cfg(debug_assertions)]
    pub fn debug_native(&self, fail: bool, hang: Option<bool>) -> String {
        if let Some(h) = hang {
            cbtv::media_proxy::debug_hang(h);
        }
        if fail {
            match self.native.borrow().as_ref() {
                Some(n) if self.s.borrow().native_on => n.debug_fail(),
                _ => return "không đang phát native".into(),
            }
        }
        "ok".into()
    }

    fn stop_video(&self) {
        self.save_position();
        if self.s.borrow().native_on {
            if let Some(n) = self.native.borrow().as_ref() {
                n.stop();
            }
            self.stage.set_visible_child_name("web");
        }
        {
            let mut s = self.s.borrow_mut();
            s.native_on = false;
            s.extract = false;
            s.page_mode = false;
            s.embed_url = None;
            s.hls_url = None;
            s.err_shown = false;
            s.last_ad_at = 0;
            s.state = None;
            s.started = false;
            s.resume_pending = false;
            s.ended_handled = false;
            s.hint_shown = false;
            s.seek_target = None;
            s.nudges = 0;
            s.last_nudge = 0;
            s.page_load += 1;
        }
        if let Some(id) = self.seek_timer.borrow_mut().take() {
            id.remove();
        }
        self.blank();
    }

    /// Đổi server (nút trong bảng tuỳ chọn): nhớ lựa chọn, xem tiếp từ chỗ đang xem.
    fn switch_server(self: &Rc<Self>, sv: Server) {
        let Some(src) = self.s.borrow().source.clone() else { return };
        prefs::set_server(cbtv::site::host_of(&src.ep_url).as_deref(), Some(&sv.kind), Some(&sv.label));
        if let Some(t) = self.current_time() {
            self.s.borrow_mut().resume_at = t;
        }
        self.s.borrow_mut().force_web = false;
        self.hide_panel();
        self.play_server(Some(sv));
    }

    fn reload(self: &Rc<Self>) {
        self.hide_panel();
        if let Some(t) = self.current_time() {
            self.s.borrow_mut().resume_at = t;
        }
        let (has_detail, src, film, ep, resume, index, server) = {
            let s = self.s.borrow();
            (s.detail.is_some(), s.source.clone(), s.film_url.clone(), s.open_ep_url.clone(), s.resume_open, s.index, s.server.clone())
        };
        if !has_detail {
            self.open(&film, &ep, resume); // chưa tải được thông tin phim: tải lại
        } else if let Some(src) = src {
            if src.page_only() {
                self.play_page(&src, None);
            } else {
                self.play_server(server);
            }
        } else {
            self.open_episode(index);
        }
    }

    fn next(self: &Rc<Self>) {
        let i = self.s.borrow().index;
        if i + 1 < self.episodes_len() {
            self.open_episode(i + 1);
        } else {
            self.notice("Đây là tập mới nhất.", 4000);
        }
    }

    fn prev(self: &Rc<Self>) {
        let i = self.s.borrow().index;
        if i > 0 {
            self.open_episode(i - 1);
        } else {
            self.notice("Đây là tập đầu tiên.", 4000);
        }
    }

    /// Sang bản khác (Vietsub / Thuyết Minh…): mở tập cùng tên và xem tiếp đúng chỗ; bản đó chưa có tập này thì mở tập cùng vị trí.
    fn switch_group(self: &Rc<Self>, g: EpisodeGroup) {
        self.hide_panel();
        let (cur, idx, film) = {
            let s = self.s.borrow();
            if s.group.as_ref() == Some(&g) || s.detail.is_none() || g.episodes.is_empty() {
                return;
            }
            (s.group.as_ref().and_then(|x| x.episodes.get(s.index)).cloned(), s.index, s.detail.as_ref().map(|d| d.url.clone()).unwrap_or_default())
        };
        let same = g.episodes.iter().position(|e| Some(&e.title) == cur.as_ref().map(|c| &c.title));
        let target = same.unwrap_or(idx.min(g.episodes.len() - 1));
        if same.is_some() {
            if let Some(t) = self.current_time() {
                prefs::save_position(&g.episodes[target].url, t, &film);
            }
        } else {
            self.notice(
                &format!("{} chưa có {} — mở {}.", g.name, cur.map(|c| c.title).unwrap_or_default(), g.episodes[target].title),
                6000,
            );
        }
        self.s.borrow_mut().group = Some(g);
        self.open_episode(target);
    }

    // ---- Trạng thái phát ----

    fn poll(self: &Rc<Self>) {
        if !self.is_open() || self.s.borrow().polling {
            return;
        }
        if self.s.borrow().native_on {
            self.poll_native();
            return;
        }
        self.s.borrow_mut().polling = true;
        let me = self.clone();
        #[allow(deprecated)]
        self.web.run_javascript(
            "JSON.stringify([window.__chimtoLoad||0,window.__chimtoState||null,window.__chimtoSrc||null])",
            None::<&gio::Cancellable>,
            move |r| {
                me.s.borrow_mut().polling = false;
                let raw = r.ok().and_then(|x| x.js_value()).map(|v| {
                    use javascriptcore::ValueExt;
                    v.to_str().to_string()
                });
                me.on_state(raw);
            },
        );
    }

    fn on_state(self: &Rc<Self>, raw: Option<String>) {
        let page_load = self.s.borrow().page_load;
        let v = raw.and_then(|r| serde_json::from_str::<serde_json::Value>(&r).ok());
        let mine = v.as_ref().and_then(|v| v.as_array()).filter(|a| a.len() >= 2 && a[0].as_u64() == Some(page_load));
        // Trình phát nhúng đã lộ link luồng (EXTRACT_JS): chuyển sang trình phát native
        if self.s.borrow().extract {
            if let Some(src) = mine.and_then(|a| a.get(2)).and_then(|x| x.as_object()) {
                let u = cbtv::json::str(Some(src), "u");
                let page = cbtv::json::str(Some(src), "p");
                let sub = cbtv::json::str(Some(src), "s");
                let hls = cbtv::json::num(Some(src), "h").unwrap_or(0.0) > 0.0;
                if !u.is_empty() && !page.is_empty() {
                    self.s.borrow_mut().extract = false;
                    // Kiểm trước (trang vẫn mở, phát tiếp được nếu không dùng mpv): luồng có lớp bảo vệ riêng của trang —
                    // streamfree: SAMPLE-AES + khoá gói kín, NguonC: cả danh sách mã hoá AES-GCM — thì không vượt, để trình phát
                    // của trang phát như trước
                    let t = self.s.borrow().token;
                    let me = self.clone();
                    let (u2, p2) = (u.clone(), page.clone());
                    ui::spawn(async move { cbtv::media_proxy::probe(&u2, Some(&p2), hls).await }, move |ok| {
                        if t != me.s.borrow().token || !me.is_open() {
                            return;
                        }
                        if !ok {
                            cbtv::log::d("native", "Luồng của trình phát nhúng có lớp bảo vệ riêng — giữ trình phát của trang");
                            return;
                        }
                        // Trang đã kịp phát thì mpv bắt đầu đúng chỗ đang xem
                        let start = me.current_time().unwrap_or(me.s.borrow().start_hint);
                        me.play_native(&u, (!sub.is_empty()).then_some(sub.as_str()), &page, start, hls);
                    });
                }
            }
        }
        let o = mine.and_then(|a| a[1].as_object().cloned());
        let now = cbtv::util::now_ms();
        let in_ad = o.as_ref().map(|o| cbtv::json::bool(Some(o), "ad", false)).unwrap_or(false);
        if in_ad {
            self.s.borrow_mut().last_ad_at = now;
        }
        {
            let s = self.s.borrow();
            let waited = now - s.load_started_at.max(s.last_ad_at);
            let show_hint = !s.started && !s.err_shown && !s.hint_shown && !in_ad && (s.embed_url.is_some() || s.hls_url.is_some() || s.page_mode) && waited > 25_000;
            let page_only = s.source.as_ref().map(|x| x.page_only()).unwrap_or(true);
            drop(s);
            if show_hint {
                self.s.borrow_mut().hint_shown = true;
                self.hide_loading();
                let other = if page_only { "▼ → «Tải lại» / «Bấm thẳng vào trang»" } else { "▼ để đổi server" };
                self.notice(&format!("Video chưa chạy? Nhấn Enter để thử phát, hoặc {other}."), 10_000);
            }
        }
        let Some(o) = o else { return };
        if in_ad {
            // Trình phát đang chạy quảng cáo (agent tự bỏ qua khi được phép) — chưa phải phim
            if !self.s.borrow().started {
                self.show_loading("Trình phát đang chạy quảng cáo — tự bỏ qua khi được phép…");
            }
            return;
        }
        let num = |k: &str| cbtv::json::num(Some(&o), k).unwrap_or(0.0);
        let b = |k: &str| cbtv::json::bool(Some(&o), k, false);
        let st = PlayState { t: num("t"), d: num("d"), paused: b("paused"), ended: b("ended"), waiting: b("waiting"), ready: b("ready") };
        let was_paused = self.s.borrow().state.map(|x| x.paused);
        self.s.borrow_mut().state = Some(st);
        let err = cbtv::json::str(Some(&o), "err");
        if !err.is_empty() && !self.s.borrow().err_shown {
            self.s.borrow_mut().err_shown = true;
            if self.s.borrow().started {
                self.notice(&format!("Mất kết nối video ({err}).\nNhấn ▼ → «Tải lại», hoặc chọn server khác."), 10_000);
            } else {
                let label = self.s.borrow().server.as_ref().map(|x| x.label.clone()).unwrap_or_else(|| "Server".into());
                self.show_error(&format!("{label} không phát được video ({err}).\nNhấn ▼ để chọn server khác."));
                self.show_panel();
            }
        }
        self.nudge(st, now);
        let started = self.s.borrow().started;
        crate::limits::tick(started && !st.paused && !st.waiting, now);
        if self.check_limit() {
            return;
        }
        // Bắt đầu khi video thật sự chạy (không phải lúc mới nạp xong mà còn dừng)
        if !started && st.ready && !st.paused {
            self.s.borrow_mut().started = true;
            self.hide_loading();
            let (resume_at, hint, rate) = {
                let s = self.s.borrow();
                (s.resume_at, s.start_hint, s.rate)
            };
            cbtv::log::d("player", &format!("Video chạy t={}, báo trước {hint}", st.t as i64));
            if resume_at > 30 && (st.d <= 0.0 || (resume_at as f64) < st.d - 30.0) {
                self.s.borrow_mut().intro_done = true;
                // Trình phát đã bắt đầu đúng chỗ (app báo trước) thì khỏi tua lại
                if (st.t - resume_at as f64).abs() > 10.0 {
                    let mut s = self.s.borrow_mut();
                    s.resume_pending = true;
                    s.resume_issued_at = now;
                    drop(s);
                    self.cmd("seekTo", Some(resume_at as f64));
                }
                self.notice(&format!("Xem tiếp từ {}", format_time(resume_at as f64)), 4000);
            } else if hint > 0 && st.t >= (hint - 10) as f64 {
                self.s.borrow_mut().intro_done = true;
                self.notice(&format!("Đã bỏ qua nhạc mở đầu (tới {}) — ◀ để xem lại.", format_time(hint as f64)), 4000);
            }
            if (rate - 1.0).abs() > 0.01 {
                self.cmd("rate", Some(rate));
            }
            if prefs::video_fill() {
                self.cmd("fit", Some(1.0));
            }
            self.show_osd(4000);
        }
        {
            let mut s = self.s.borrow_mut();
            if s.resume_pending && (st.t >= (s.resume_at - 5) as f64 || now - s.resume_issued_at > 30_000) {
                s.resume_pending = false;
            }
        }
        let (started, intro_done) = {
            let s = self.s.borrow();
            (s.started, s.intro_done)
        };
        if started && !intro_done && !st.paused {
            self.skip_intro(st);
        }
        if self.osd.is_visible() {
            self.update_osd();
        }
        if let Some(m) = self.mini.borrow().as_ref() {
            m.play.set_label(if st.paused { "▶" } else { "⏸" });
        }
        if let Some(wp) = was_paused {
            if wp != st.paused {
                self.show_osd(4000);
            }
        }
        if started && now - self.s.borrow().last_save > 5000 {
            self.s.borrow_mut().last_save = now;
            self.save_position();
        }
        if st.ended && !self.s.borrow().ended_handled {
            self.s.borrow_mut().ended_handled = true;
            let i = self.s.borrow().index;
            if prefs::auto_next() && i + 1 < self.episodes_len() {
                let t = self.s.borrow().group.as_ref().map(|g| g.episodes[i + 1].title.clone()).unwrap_or_default();
                self.notice(&format!("Hết tập — chuyển sang {t}"), 4000);
                self.open_episode(i + 1);
            }
        }
    }

    /// Trình phát đã nạp mà video chưa có dữ liệu, đang dừng (JW Player trên WebKitGTK đứng «idle», không tự phát như trên
    /// Chromium): sau 2,5 giây tự gửi lệnh phát — như người xem bấm Enter; tối đa 4 lần, cách nhau 3 giây.
    fn nudge(&self, st: PlayState, now: i64) {
        let go = {
            let s = self.s.borrow();
            !s.started && st.paused && !st.ready && s.nudges < 4 && now - s.load_started_at > 2_500 && now - s.last_nudge > 3_000
        };
        if go {
            {
                let mut s = self.s.borrow_mut();
                s.nudges += 1;
                s.last_nudge = now;
            }
            cbtv::log::d("player", "Trình phát chưa tự phát — gửi lệnh phát");
            self.cmd("play", None);
        }
    }

    // ---- Giới hạn giờ xem ----

    /// Mỗi lần đọc trạng thái: sắp hết giờ thì nhắc, hết giờ thì dừng phim, thoát trình phát và hỏi mã PIN phụ huynh.
    fn check_limit(self: &Rc<Self>) -> bool {
        match crate::limits::blocked() {
            None => {
                let (started, warned) = {
                    let s = self.s.borrow();
                    (s.started, s.limit_warned)
                };
                if started && !warned {
                    if let Some(r) = crate::limits::remaining().filter(|r| *r > 0 && *r <= 300) {
                        self.s.borrow_mut().limit_warned = true;
                        self.notice(&format!("Còn {} xem hôm nay ({}).", crate::limits::minutes_text(r), prefs::current_profile().name), 8000);
                    }
                }
                false
            }
            Some(b) => {
                self.cmd("pause", None);
                let (film, ep) = {
                    let s = self.s.borrow();
                    (s.detail.as_ref().map(|d| d.url.clone()).unwrap_or_else(|| s.film_url.clone()), s.episode_url.clone())
                };
                self.limit_block(b, &film, &ep);
                true
            }
        }
    }

    fn limit_block(&self, b: crate::limits::Block, film: &str, ep: &str) {
        cbtv::log::d("player", &format!("Hết giờ xem — {}", prefs::current_profile().name));
        self.close(true);
        crate::limits::limit_reached(b, film.to_string(), ep.to_string());
    }

    fn intro_key(&self) -> String {
        let s = self.s.borrow();
        s.detail.as_ref().map(|d| d.url.clone()).unwrap_or_else(|| s.film_url.clone())
    }

    /// Phim đã đánh dấu nhạc mở đầu: video vừa chạy tới mốc đầu (trễ tối đa 8 giây) thì tua tới mốc hết, một lần mỗi tập.
    fn skip_intro(&self, st: PlayState) {
        let Some((start, end)) = prefs::intro(&self.intro_key()) else { return };
        if end <= start || self.s.borrow().seek_target.is_some() {
            return;
        }
        let (start, end) = (start as f64, end as f64);
        if st.t >= start + 8.0 || st.t >= end - 3.0 {
            self.s.borrow_mut().intro_done = true;
            return;
        }
        if st.t < start - 1.0 || (st.d > 0.0 && end > st.d - 30.0) {
            return;
        }
        self.s.borrow_mut().intro_done = true;
        self.cmd("seekTo", Some(end));
        self.notice(&format!("Đã bỏ qua nhạc mở đầu (tới {}) — ◀ để xem lại.", format_time(end)), 4000);
    }

    /// Chỗ bắt đầu báo trước cho lần nạp trang này: chỗ xem dở; hoặc hết nhạc mở đầu nếu nhạc nằm ngay đầu tập.
    fn start_for(&self) -> i64 {
        let (resume_at, intro_done) = {
            let s = self.s.borrow();
            (s.resume_at, s.intro_done)
        };
        if resume_at > 30 {
            return resume_at;
        }
        match prefs::intro(&self.intro_key()) {
            Some((start, end)) if !intro_done && start <= 1 && end > start => end,
            _ => 0,
        }
    }

    /// Chỗ đang xem để mang sang (đổi server / tải lại / đổi bản).
    fn current_time(&self) -> Option<i64> {
        let s = self.s.borrow();
        if s.resume_pending {
            return Some(s.resume_at);
        }
        match s.state {
            Some(st) if s.started && st.t > 5.0 => Some(st.t as i64),
            _ => None,
        }
    }

    fn save_position(&self) {
        let s = self.s.borrow();
        let Some(st) = s.state else { return };
        if !s.started || s.episode_url.is_empty() || s.resume_pending {
            return;
        }
        // Xem gần hết thì coi như xong (lần sau mở lại từ đầu)
        let done = st.ended || (st.d > 0.0 && st.t > st.d - 90.0);
        let film = s.detail.as_ref().map(|d| d.url.clone()).unwrap_or_else(|| s.film_url.clone());
        prefs::save_position(&s.episode_url, if done { 0 } else { st.t as i64 }, &film);
    }

    // ---- Điều khiển ----

    fn toggle(&self) {
        let started = self.s.borrow().started;
        self.cmd(if started { "toggle" } else { "play" }, None);
        self.show_osd(4000);
    }

    fn step(&self, repeat: u32) -> f64 {
        prefs::seek_step() as f64 * (1.0 + repeat as f64 / 8.0).min(4.0)
    }

    /// Gộp các lần bấm tua liên tiếp thành một lệnh tua (đỡ giật khi giữ phím).
    fn seek_by(&self, delta: f64) {
        let Some(st) = self.s.borrow().state else { return };
        if !self.s.borrow().started {
            return;
        }
        let max = if st.d > 0.0 { st.d - 1.0 } else { f64::MAX };
        {
            let mut s = self.s.borrow_mut();
            s.seek_target = Some((s.seek_target.unwrap_or(st.t) + delta).clamp(0.0, max));
        }
        self.show_osd(4000);
        if let Some(id) = self.seek_timer.borrow_mut().take() {
            id.remove();
        }
        let id = ui::after(500, || {
            if let Some(p) = me() {
                p.seek_timer.borrow_mut().take();
                p.flush_seek();
            }
        });
        *self.seek_timer.borrow_mut() = Some(id);
    }

    fn seek_to(&self, target: f64) {
        self.s.borrow_mut().seek_target = Some(target);
        self.show_osd(4000);
        if let Some(id) = self.seek_timer.borrow_mut().take() {
            id.remove();
        }
        self.flush_seek();
    }

    fn flush_seek(&self) {
        let Some(target) = self.s.borrow_mut().seek_target.take() else { return };
        self.s.borrow_mut().resume_pending = false; // người xem tự tua: từ giờ lấy theo trình phát
        self.cmd("seekTo", Some(target));
        if let Some(st) = self.s.borrow_mut().state.as_mut() {
            st.t = target;
        }
        self.update_osd();
    }

    /// Phím khi đang xem phim. true = đã xử lý.
    pub fn handle_key(self: &Rc<Self>, ev: &gdk::EventKey) -> bool {
        use gdk::keys::constants as k;
        let key = ev.keyval();
        let alt = ev.state().contains(gdk::ModifierType::MOD1_MASK);
        // GTK3 không báo phím lặp: coi là lặp khi cùng phím bấm liền trong 150 ms
        let repeat = is_repeat(&key, ev.time());
        {
            let mut s = self.s.borrow_mut();
            s.repeat = if repeat { s.repeat + 1 } else { 0 };
        }
        if self.s.borrow().click_through {
            if key == k::Escape {
                self.exit_click_through();
                return true;
            }
            return false;
        }
        if key == k::F11 || (key == k::Return && alt) || key == k::f || key == k::F {
            if !repeat {
                app().toggle_fullscreen();
            }
            return true;
        }
        if self.panel.is_visible() {
            if key == k::Escape || key == k::Back || key == k::BackSpace || key == k::m || key == k::M || key == k::Menu {
                self.hide_panel();
                return true;
            }
            if let Some(dir) = keynav::dir_of(&key) {
                let root: gtk::Widget = self.panel_stack.clone().upcast();
                match app().window.focused_widget().filter(|f| keynav::is_inside(f, &root)) {
                    Some(f) => {
                        if let Some(n) = keynav::next(&root, &f, dir) {
                            keynav::focus_and_show(&n);
                        }
                    }
                    None => {
                        if let Some(f) = keynav::first(&root) {
                            keynav::focus_and_show(&f);
                        }
                    }
                }
                return true;
            }
            return false; // Enter / Space: nút đang chọn tự xử lý
        }
        let first = !repeat;
        let rep = self.s.borrow().repeat;
        match key {
            k::Escape | k::Back | k::BackSpace => {
                if first {
                    self.close(true);
                }
            }
            k::space | k::Return | k::KP_Enter | k::k | k::K | k::AudioPlay | k::AudioPause => {
                if first {
                    self.toggle();
                }
            }
            k::AudioStop => {
                if first {
                    self.cmd("pause", None);
                }
            }
            k::Left | k::KP_Left | k::j | k::J | k::AudioRewind => self.seek_by(-self.step(rep)),
            k::Right | k::KP_Right | k::l | k::L | k::AudioForward => self.seek_by(self.step(rep)),
            k::Up | k::KP_Up | k::i | k::I => self.show_osd(6000),
            k::w | k::W => {
                if first {
                    self.enter_mini();
                }
            }
            k::Down | k::KP_Down | k::m | k::M | k::Menu => {
                if first {
                    self.show_panel();
                }
            }
            k::n | k::N | k::Page_Down | k::AudioNext => {
                if first {
                    self.next();
                }
            }
            k::p | k::P | k::Page_Up | k::AudioPrev => {
                if first {
                    self.prev();
                }
            }
            _ => {
                let digit = key.to_unicode().and_then(|c| c.to_digit(10));
                match digit {
                    Some(n) => {
                        let st = self.s.borrow().state;
                        if let (true, Some(st)) = (first, st) {
                            if st.d > 0.0 {
                                self.seek_to(st.d * n as f64 / 10.0);
                            }
                        }
                    }
                    None => return false,
                }
            }
        }
        true
    }

    // ---- Giao diện phủ ----

    fn set_cursor(&self, visible: bool) {
        if let Some(w) = self.root.window() {
            let c = if visible { None } else { gdk::Cursor::for_display(&w.display(), gdk::CursorType::BlankCursor) };
            w.set_cursor(c.as_ref());
        }
    }

    fn show_osd(&self, ms: u64) {
        if !self.is_open() || self.s.borrow().click_through {
            return;
        }
        self.osd.show_all();
        self.topbar.show_all();
        self.update_osd();
        if let Some(id) = self.osd_timer.borrow_mut().take() {
            id.remove();
        }
        let id = ui::after(ms, || {
            if let Some(p) = me() {
                p.osd_timer.borrow_mut().take();
                p.hide_osd();
            }
        });
        *self.osd_timer.borrow_mut() = Some(id);
    }

    fn hide_osd(&self) {
        // Đang dừng / chuột đang ở trên thanh / đang tua: giữ thanh
        let keep = {
            let s = self.s.borrow();
            s.state.map(|x| x.paused).unwrap_or(false) || s.seek_target.is_some() || s.dragging || s.osd_hover || !s.started
        } || self.panel.is_visible();
        if keep {
            self.show_osd(2000);
            return;
        }
        self.osd.hide();
        self.topbar.hide();
        self.set_cursor(false);
    }

    fn update_osd(&self) {
        let s = self.s.borrow();
        let st = s.state;
        let t = s.seek_target.or(st.map(|x| x.t)).unwrap_or(0.0);
        let d = st.map(|x| x.d).unwrap_or(0.0);
        if !s.dragging {
            self.time_text.set_text(&if d > 0.0 { format!("{} / {}", format_time(t), format_time(d)) } else { format_time(t) });
            let adj = self.seek.adjustment();
            adj.set_upper(d.max(1.0));
            adj.set_value(t.min(d.max(1.0)));
        }
        let state = match st {
            None => "Đang tải".to_string(),
            Some(x) => match s.seek_target {
                Some(tg) => if tg >= x.t { "Tua tới" } else { "Tua lùi" }.to_string(),
                None if x.waiting => "Đang tải".into(),
                None if x.paused => "Tạm dừng".into(),
                None if (s.rate - 1.0).abs() > 0.01 => format!("Đang phát · {}", rate_text(s.rate)),
                None => "Đang phát".into(),
            },
        };
        self.state_text.set_text(&state);
        self.btn_play.set_label(if st.map(|x| !x.paused).unwrap_or(false) { "⏸  Dừng" } else { "▶  Phát" });
        let n = s.group.as_ref().map(|g| g.episodes.len()).unwrap_or(0);
        self.btn_prev.set_sensitive(s.index > 0);
        self.btn_next.set_sensitive(s.index + 1 < n);
    }

    pub fn notice(&self, text: &str, ms: u64) {
        if !self.is_open() {
            return;
        }
        self.notice_text.set_text(text);
        self.notice_box.show();
        if let Some(id) = self.notice_timer.borrow_mut().take() {
            id.remove();
        }
        let id = ui::after(ms, || {
            if let Some(p) = me() {
                p.notice_timer.borrow_mut().take();
                p.notice_box.hide();
            }
        });
        *self.notice_timer.borrow_mut() = Some(id);
    }

    fn show_loading(&self, text: &str) {
        self.loading_box.show();
        self.loading_bar.show();
        self.loading_text.set_text(text);
        let weak = self.loading_bar.downgrade();
        glib::timeout_add_local(std::time::Duration::from_millis(120), move || match weak.upgrade() {
            Some(b) if b.is_visible() && b.parent().map(|p| p.is_visible()).unwrap_or(false) => {
                b.pulse();
                glib::ControlFlow::Continue
            }
            _ => glib::ControlFlow::Break,
        });
    }

    fn show_error(&self, text: &str) {
        self.loading_box.show();
        self.loading_bar.hide();
        self.loading_text.set_text(text);
        self.show_osd(8000);
    }

    fn hide_loading(&self) {
        self.loading_box.hide();
    }

    // ---- Bảng tuỳ chọn ----

    fn section(&self, title: &str, note: Option<&str>) {
        let t = ui::label(title, &["h2"], true);
        t.set_margin_top(px(14.0));
        t.set_margin_bottom(px(8.0));
        self.panel_stack.pack_start(&t, false, false, 0);
        if let Some(n) = note {
            let l = ui::dim(n);
            l.set_margin_bottom(px(8.0));
            self.panel_stack.pack_start(&l, false, false, 0);
        }
    }

    fn panel_row(&self) -> gtk::FlowBox {
        let r = ui::flow(10.0, 10.0);
        self.panel_stack.pack_start(&r, false, false, 0);
        r
    }

    fn show_panel(self: &Rc<Self>) {
        if !self.is_open() {
            return;
        }
        ui::clear(&self.panel_stack);
        self.panel_title.set_text(&self.title.text());
        let mut focus: Option<gtk::Button> = None;
        let (src, server, detail, group, index, started, rate, t) = {
            let s = self.s.borrow();
            (s.source.clone(), s.server.clone(), s.detail.clone(), s.group.clone(), s.index, s.started, s.rate, s.state.map(|x| x.t).unwrap_or(0.0) as i64)
        };
        match &src {
            Some(src) if !src.page_only() && !src.servers.is_empty() => {
                self.section("Server", Some("Video giật / không chạy: chọn server khác (app nhớ cho lần sau)."));
                let row = self.panel_row();
                for sv in &src.servers {
                    let active = server.as_ref().map(|x| x.kind == sv.kind).unwrap_or(false);
                    let me = self.clone();
                    let sv2 = sv.clone();
                    let b = ui::pill_tag(&sv.label, active.then_some("active"), move || me.switch_server(sv2.clone()));
                    ui::add_flow(&row, b.upcast_ref());
                    if active {
                        focus = Some(b);
                    }
                }
            }
            Some(src) if src.page_only() => self.section("Server", Some("Trang này không chia server: app mở thẳng trang tập phim.")),
            _ => {}
        }
        if let Some(d) = detail.as_ref().filter(|d| d.groups.len() > 1) {
            self.section("Bản", None);
            let row = self.panel_row();
            for g in &d.groups {
                let me = self.clone();
                let g2 = g.clone();
                let b = ui::pill_tag(&g.name, (Some(g) == group.as_ref()).then_some("active"), move || me.switch_group(g2.clone()));
                ui::add_flow(&row, b.upcast_ref());
            }
        }
        let n = group.as_ref().map(|g| g.episodes.len()).unwrap_or(0);
        self.section("Xem", None);
        let view = self.panel_row();
        let me = self.clone();
        let prev = ui::pill("⏮  Tập trước", move || {
            me.hide_panel();
            me.prev();
        });
        prev.set_sensitive(index > 0);
        let me = self.clone();
        let next = ui::pill("Tập sau  ⏭", move || {
            me.hide_panel();
            me.next();
        });
        next.set_sensitive(index + 1 < n);
        let me = self.clone();
        let reload = ui::pill("↻  Tải lại", move || me.reload());
        for b in [&prev, &next, &reload] {
            ui::add_flow(&view, b.upcast_ref());
        }
        if focus.is_none() {
            focus = Some(if index + 1 < n { next.clone() } else { prev.clone() });
        }
        let rates = self.panel_row();
        for r in RATES {
            let me = self.clone();
            let b = ui::pill_tag(&rate_text(r), ((rate - r).abs() < 0.01).then_some("active"), move || {
                me.s.borrow_mut().rate = r;
                me.cmd("rate", Some(r));
                me.notice(&format!("Tốc độ {}", rate_text(r)), 4000);
                me.show_panel();
            });
            b.set_size_request(px(64.0), -1);
            ui::add_flow(&rates, b.upcast_ref());
        }
        let me = self.clone();
        let fit = ui::pill(if prefs::video_fill() { "Khung hình: Lấp đầy" } else { "Khung hình: Vừa màn hình" }, move || {
            prefs::set_video_fill(!prefs::video_fill());
            me.cmd("fit", Some(if prefs::video_fill() { 1.0 } else { 0.0 }));
            me.show_panel();
        });
        ui::add_flow(&self.panel_row(), fit.upcast_ref());

        // Nhạc mở đầu
        let key = self.intro_key();
        let mark = prefs::intro(&key);
        let note = match mark {
            None => "Chưa đánh dấu — đang ở đầu / cuối nhạc thì bấm nút tương ứng.".to_string(),
            Some((s0, e)) if e < 0 => format!("Bắt đầu {}, chưa đánh dấu chỗ hết.", format_time(s0 as f64)),
            Some((s0, e)) => format!("{} → {}, tự bỏ qua ở các tập.", format_time(s0 as f64), format_time(e as f64)),
        };
        self.section("Nhạc mở đầu (cả phim)", Some(&note));
        let intro = self.panel_row();
        let (me, k2) = (self.clone(), key.clone());
        let b_start = ui::pill(&format!("Bắt đầu ở {}", format_time(t as f64)), move || {
            me.hide_panel();
            let end = prefs::intro(&k2).filter(|m| m.1 > t).map(|m| m.1).unwrap_or(-1);
            prefs::set_intro(&k2, t, end);
            me.notice(
                &if end > t {
                    format!("Nhạc mở đầu: {} → {}, các tập sẽ tự bỏ qua.", format_time(t as f64), format_time(end as f64))
                } else {
                    "Đã đánh dấu đầu nhạc mở đầu. Hết nhạc thì nhấn ▼ → «Hết nhạc ở …».".to_string()
                },
                6000,
            );
        });
        let (me, k2) = (self.clone(), key.clone());
        let b_end = ui::pill(&format!("Hết nhạc ở {}", format_time(t as f64)), move || {
            me.hide_panel();
            let start = prefs::intro(&k2).filter(|m| m.0 < t).map(|m| m.0).unwrap_or(0);
            prefs::set_intro(&k2, start, t);
            me.s.borrow_mut().intro_done = true;
            me.notice(&format!("Nhạc mở đầu: {} → {}, các tập sẽ tự bỏ qua.", format_time(start as f64), format_time(t as f64)), 6000);
        });
        b_start.set_sensitive(started);
        b_end.set_sensitive(started);
        ui::add_flow(&intro, b_start.upcast_ref());
        ui::add_flow(&intro, b_end.upcast_ref());
        if mark.is_some() {
            let (me, k2) = (self.clone(), key.clone());
            let b = ui::pill("Bỏ mốc", move || {
                me.hide_panel();
                prefs::clear_intro(&k2);
                me.notice("Đã bỏ mốc nhạc mở đầu của phim này.", 4000);
            });
            ui::add_flow(&intro, b.upcast_ref());
        }

        // Danh sách tập
        if let Some(g) = group.as_ref().filter(|g| g.episodes.len() > 1) {
            self.section(&format!("Tập ({})", g.episodes.len()), None);
            let eps = self.panel_row();
            let watched = prefs::watched_episodes();
            for (i, e) in g.episodes.iter().enumerate() {
                let tag = if i == index {
                    Some("active")
                } else if watched.contains(&e.url) {
                    Some("watched")
                } else {
                    None
                };
                let me = self.clone();
                let b = ui::pill_tag(&e.title, tag, move || {
                    me.hide_panel();
                    me.open_episode(i);
                });
                b.style_context().add_class("small");
                b.set_size_request(px(84.0), -1);
                ui::add_flow(&eps, b.upcast_ref());
            }
        }

        self.section("Khác", None);
        let other = self.panel_row();
        let me = self.clone();
        ui::add_flow(&other, ui::pill("🖱  Bấm thẳng vào trang", move || me.enter_click_through()).upcast_ref());
        let me = self.clone();
        ui::add_flow(
            &other,
            ui::pill("↗  Mở trang tập trên trình duyệt", move || {
                me.hide_panel();
                let u = me.s.borrow().episode_url.clone();
                ui::open_browser(&u);
            })
            .upcast_ref(),
        );
        let me = self.clone();
        ui::add_flow(&other, ui::pill("▣  Thu nhỏ (cửa sổ nhỏ)", move || me.enter_mini()).upcast_ref());
        let me = self.clone();
        ui::add_flow(&other, ui::pill("✕  Thoát phim", move || me.close(true)).upcast_ref());

        // bảng đặt no_show_all (ẩn sẵn) → show_all() không có tác dụng: hiện con trước, rồi bảng
        self.panel_stack.show_all();
        self.panel.show();
        self.set_cursor(true);
        self.show_osd(4000);
        if let Some(f) = focus {
            ui::after(30, move || keynav::focus_and_show(f.upcast_ref()));
        }
    }

    fn hide_panel(&self) {
        self.panel.hide();
        // phím trở về trình phát (không để nút trong bảng giữ phím)
        app().window.set_focus(None::<&gtk::Widget>);
    }

    /// Chế độ bấm thẳng vào trang (trình phát cần bấm nút riêng, vd «Bấm để xem»): bỏ lớp nhận chuột để chuột đi xuyên.
    fn enter_click_through(&self) {
        self.hide_panel();
        self.s.borrow_mut().click_through = true;
        self.osd.hide();
        self.topbar.hide();
        self.hit.hide();
        self.set_cursor(true);
        self.btn_done.show();
        self.web.set_can_focus(true);
        self.notice("Dùng chuột bấm vào trang. Xong thì bấm nút «Xong» ở trên cùng.", 6000);
    }

    fn exit_click_through(&self) {
        if !std::mem::replace(&mut self.s.borrow_mut().click_through, false) {
            return;
        }
        self.hit.show();
        self.btn_done.hide();
        self.web.set_can_focus(false);
        app().window.set_focus(None::<&gtk::Widget>);
        self.show_osd(4000);
    }

    /// Cổng gỡ lỗi: tình trạng trình phát.
    #[cfg(debug_assertions)]
    pub fn debug(&self) -> String {
        let s = self.s.borrow();
        let native = match (s.native_on, self.native.borrow().as_ref()) {
            (true, Some(n)) => format!("on {}", n.debug()),
            _ if s.extract => "đang lấy link từ trình phát nhúng".to_string(),
            _ => format!("off (force_web={})", s.force_web),
        };
        format!(
            "native={native} open={} pageMode={} started={} server={:?} ep={} t={:?} d={:?} paused={:?} embed={:?} hls={:?} load={}",
            self.is_open(),
            s.page_mode,
            s.started,
            s.server.as_ref().map(|x| x.label.clone()),
            s.episode_url,
            s.state.map(|x| x.t),
            s.state.map(|x| x.d),
            s.state.map(|x| x.paused),
            s.embed_url,
            s.hls_url,
            s.page_load
        )
    }

    #[cfg(debug_assertions)]
    /// Cổng gỡ lỗi: chạy JS trong trình phát, trả kết quả dạng chữ.
    pub fn eval(&self, code: &str, done: impl FnOnce(String) + 'static) {
        #[allow(deprecated)]
        self.web.run_javascript(code, None::<&gio::Cancellable>, move |r| {
            let s = match r {
                Ok(x) => x
                    .js_value()
                    .map(|v| {
                        use javascriptcore::ValueExt;
                        v.to_str().to_string()
                    })
                    .unwrap_or_default(),
                Err(e) => format!("lỗi: {e}"),
            };
            done(s);
        });
    }
}

/// Xoá cookie / bộ nhớ đệm của trình duyệt trong app và ảnh bìa đã lưu.
pub fn clear_browsing_data() {
    setup_web_context();
    if let Some(dm) = WebContext::default().and_then(|c| c.website_data_manager()) {
        dm.clear(webkit2gtk::WebsiteDataTypes::ALL, glib::TimeSpan::from_seconds(0), None::<&gio::Cancellable>, |r| {
            if let Err(e) = r {
                cbtv::log::d("player", &format!("Xoá dữ liệu lỗi: {e}"));
            }
        });
    }
    crate::images::clear_disk();
}

/// Trang hls.html với hls.js đóng kèm (gắn thẳng vào trang thay cho /__chimto/hls.js — WebKitGTK không chặn được yêu cầu
/// https để trả tệp như WebView của Android / WebView2).
fn hls_page(src: &str, sub: Option<&str>, start: i64) -> String {
    let js = crate::assets::HLS_JS.replace("</script", "<\\/script");
    crate::assets::HLS_HTML
        .replace("<script src=\"/__chimto/hls.js\"></script>", &format!("<script>{js}</script>"))
        .replace("{{SRC}}", &serde_json::to_string(src).unwrap_or_else(|_| "\"\"".into()))
        .replace("{{START}}", &start.to_string())
        .replace("{{SUB}}", &sub.map(|s| serde_json::to_string(s).unwrap_or_else(|_| "null".into())).unwrap_or_else(|| "null".into()))
        .replace("{{AGENT}}", "")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn same_url(a: &str, b: &str) -> bool {
    let strip = |u: &str| -> Option<String> {
        let mut x = url::Url::parse(u).ok()?;
        x.set_fragment(None);
        Some(x.to_string().to_lowercase())
    };
    match (strip(a), strip(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

fn rate_text(r: f64) -> String {
    if r.fract() == 0.0 {
        format!("{}×", r as i64)
    } else {
        format!("{}×", format!("{r}").replace('.', ","))
    }
}

/// User-Agent của trình phát: WebKitGTK mặc định ghi «Version/60.5 Safari» lạ — đặt như Safari trên Mac (cùng nhân WebKit),
/// trang phát chọn đường MSE / hls.js như trên trình duyệt thường.
/// Có GPU vẽ được không: có nút render DRM (/dev/dri/renderD*) và không bị ép vẽ bằng phần mềm (LIBGL_ALWAYS_SOFTWARE).
fn has_gpu() -> bool {
    if std::env::var("LIBGL_ALWAYS_SOFTWARE").map(|v| v == "1" || v == "true").unwrap_or(false) {
        return false;
    }
    std::fs::read_dir("/dev/dri")
        .map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("renderD")))
        .unwrap_or(false)
}

fn web_user_agent() -> String {
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15".to_string()
}

fn is_repeat(key: &gdk::keys::Key, time: u32) -> bool {
    thread_local! { static LAST: Cell<(u32, u32)> = const { Cell::new((0, 0)) }; }
    let k = **key;
    let (lk, lt) = LAST.with(|l| l.get());
    LAST.with(|l| l.set((k, time)));
    lk == k && time.wrapping_sub(lt) < 150
}
