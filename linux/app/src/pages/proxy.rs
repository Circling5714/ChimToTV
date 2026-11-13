//! Proxy (SOCKS5 hoặc Shadowsocks): bật / tắt, địa chỉ, cổng, đăng nhập / khoá, DNS của proxy hay của máy, toàn app hay chỉ
//! trang phim. Proxy lỗi thì báo lỗi, không lặng lẽ đi thẳng. Đổi xong có hiệu lực ngay (cả trình phát — qua cổng nội bộ).
//! Bản Windows: UI/ProxyPage.cs.

use super::{Page, PageBase};
use crate::app::app;
use crate::css::px;
use crate::ui;
use cbtv::net::SocksConfig;
use cbtv::shadowsocks::{self as ss, SsConfig, SsMethod};
use cbtv::{prefs, Error};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct ProxyPage {
    base: PageBase,
    host: gtk::Entry,
    port: gtk::Entry,
    user: gtk::Entry,
    pass: gtk::Entry,
    ss_link: gtk::Entry,
    ss_host: gtk::Entry,
    ss_port: gtk::Entry,
    ss_key: gtk::Entry,
    ss_method: RefCell<SsMethod>,
    kind: RefCell<String>,
    remote_dns: Cell<bool>,
    sites_only: Cell<bool>,
    result: RefCell<String>,
}

fn entry(chars: i32, text: &str, secret: bool) -> gtk::Entry {
    let e = gtk::Entry::new();
    e.set_width_chars(chars);
    e.set_text(text);
    if secret {
        e.set_visibility(false);
        e.set_input_purpose(gtk::InputPurpose::Password);
    }
    e
}

pub fn new() -> Rc<dyn Page> {
    let p = Rc::new(ProxyPage {
        base: PageBase::new("settings"),
        host: entry(30, &prefs::proxy_host(), false),
        port: entry(8, &prefs::proxy_port().to_string(), false),
        user: entry(30, &prefs::proxy_user(), false),
        pass: entry(30, &prefs::proxy_pass(), true),
        ss_link: entry(52, "", false),
        ss_host: entry(30, &prefs::ss_host(), false),
        ss_port: entry(8, &if prefs::ss_port() > 0 { prefs::ss_port().to_string() } else { String::new() }, false),
        ss_key: entry(52, &prefs::ss_key(), true),
        ss_method: RefCell::new(SsMethod::of(&prefs::ss_method_id()).unwrap_or(SsMethod::DEFAULT)),
        kind: RefCell::new(prefs::proxy_kind()),
        remote_dns: Cell::new(prefs::proxy_remote_dns()),
        sites_only: Cell::new(prefs::proxy_sites_only()),
        result: RefCell::new(String::new()),
    });
    p.ss_link.set_placeholder_text(Some("ss://…"));
    // Dán link ss:// (3X-UI: nút Copy / QR của người dùng) là tự điền các ô
    let weak = Rc::downgrade(&p);
    p.ss_link.connect_changed(move |e| {
        if e.text().trim().to_lowercase().starts_with("ss://") {
            if let Some(p) = weak.upgrade() {
                let p2 = p.clone();
                ui::after(10, move || p2.read_link());
            }
        }
    });
    p
}

fn label(t: &str) -> gtk::Label {
    let l = ui::label(t, &["h2"], true);
    ui::margins(&l, 12.0, 0.0, 6.0, 0.0);
    l
}

fn detach(e: &gtk::Entry) {
    if let Some(p) = e.parent().and_then(|p| p.downcast::<gtk::Container>().ok()) {
        p.remove(e);
    }
}

fn hrow(items: &[&gtk::Widget]) -> gtk::Box {
    let r = ui::hbox(10.0);
    r.set_margin_bottom(px(10.0));
    for i in items {
        r.pack_start(*i, false, false, 0);
    }
    r
}

impl ProxyPage {
    fn build(self: &Rc<Self>) {
        let text = ui::focused_label();
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Proxy (SOCKS5 / Shadowsocks)"));
        let n = ui::dim("Cho app đi qua proxy của anh. Proxy lỗi thì app báo lỗi chứ không lặng lẽ đi thẳng. Mật khẩu / khoá lưu trong tệp riêng chỉ tài khoản này đọc được.");
        n.set_margin_bottom(px(8.0));
        b.add(&n);
        let on = prefs::proxy_on();
        let (m1, m2) = (self.clone(), self.clone());
        b.add(&ui::row(&[
            ui::pill_tag("Bật", on.then_some("active"), move || m1.toggle(true)).upcast(),
            ui::pill_tag("Tắt", (!on).then_some("active"), move || m2.toggle(false)).upcast(),
        ]));
        b.add(&label("Kiểu proxy"));
        let kind = self.kind.borrow().clone();
        let (m1, m2) = (self.clone(), self.clone());
        b.add(&ui::row(&[
            ui::pill_tag("SOCKS5", (kind == "socks5").then_some("active"), move || m1.set_kind("socks5")).upcast(),
            ui::pill_tag("Shadowsocks (mã hoá)", (kind == "ss").then_some("active"), move || m2.set_kind("ss")).upcast(),
        ]));
        for e in [&self.host, &self.port, &self.user, &self.pass, &self.ss_link, &self.ss_host, &self.ss_port, &self.ss_key] {
            detach(e);
        }
        if kind == "ss" {
            b.add(&label("Dán link ss:// (3X-UI: nút Copy / QR của người dùng) — tự điền các ô dưới"));
            b.add(&hrow(&[self.ss_link.upcast_ref()]));
            b.add(&label("Máy chủ và cổng"));
            b.add(&hrow(&[self.ss_host.upcast_ref(), self.ss_port.upcast_ref()]));
            b.add(&label("Kiểu mã hoá"));
            let row = ui::flow(10.0, 10.0);
            let cur = self.ss_method.borrow().clone();
            for m in SsMethod::ALL {
                let me = self.clone();
                let m2 = m.clone();
                let p = ui::pill_tag(
                    &format!("{}{}", m.id, if m.supported() { "" } else { " (máy này không có)" }),
                    (m.id == cur.id).then_some("active"),
                    move || {
                        *me.ss_method.borrow_mut() = m2.clone();
                        me.build();
                    },
                );
                ui::add_flow(&row, p.upcast_ref());
            }
            b.add(&row);
            b.add(&label("Khoá (3X-UI: khoá-máy-chủ:khoá-người-dùng) / mật khẩu"));
            b.add(&hrow(&[self.ss_key.upcast_ref()]));
            b.add(&ui::dim(
                "Shadowsocks mã hoá cả đường tới máy chủ (vd lối vào Shadowsocks của 3X-UI, TCP, kiểu 2022-blake3-aes-256-gcm): nhà mạng \
                 chỉ thấy dữ liệu ngẫu nhiên. SS 2022 cần giờ trên máy đúng (lệch tối đa 30 giây).",
            ));
        } else {
            b.add(&label("Địa chỉ proxy và cổng"));
            b.add(&hrow(&[self.host.upcast_ref(), self.port.upcast_ref()]));
            b.add(&label("Tên đăng nhập / mật khẩu (để trống nếu proxy không cần)"));
            b.add(&hrow(&[self.user.upcast_ref(), self.pass.upcast_ref()]));
            b.add(&ui::dim("SOCKS5 không mã hoá: nhà mạng thấy tên trang, tên đăng nhập / mật khẩu proxy. Cần giấu thì dùng Shadowsocks."));
        }
        b.add(&label("Tên miền phân giải ở"));
        let rd = self.remote_dns.get();
        let (m1, m2) = (self.clone(), self.clone());
        b.add(&ui::row(&[
            ui::pill_tag("DNS của proxy", rd.then_some("active"), move || {
                m1.remote_dns.set(true);
                m1.build();
            })
            .upcast(),
            ui::pill_tag("DNS của máy này", (!rd).then_some("active"), move || {
                m2.remote_dns.set(false);
                m2.build();
            })
            .upcast(),
        ]));
        b.add(&label("Đi qua proxy"));
        let so = self.sites_only.get();
        let (m1, m2) = (self.clone(), self.clone());
        b.add(&ui::row(&[
            ui::pill_tag("Toàn bộ app", (!so).then_some("active"), move || {
                m1.sites_only.set(false);
                m1.build();
            })
            .upcast(),
            ui::pill_tag("Chỉ trang phim", so.then_some("active"), move || {
                m2.sites_only.set(true);
                m2.build();
            })
            .upcast(),
        ]));
        let n = ui::dim("«Chỉ trang phim»: chỉ các trang trong danh sách đi qua proxy; video, ảnh bìa, GitHub… đi thẳng mạng nhà (dùng khi video chậm / bị chặn qua proxy).");
        n.set_margin_bottom(px(8.0));
        b.add(&n);
        let (m1, m2) = (self.clone(), self.clone());
        b.add(&ui::row(&[
            ui::pill("💾  Lưu", move || {
                m1.save(true);
            })
            .upcast(),
            ui::pill("Thử kết nối", move || m2.test()).upcast(),
        ]));
        let r = self.result.borrow().clone();
        if !r.is_empty() {
            let l = ui::text(&r);
            l.set_margin_top(px(6.0));
            b.add(&l);
        }
        ui::refocus_label(b.body.upcast_ref(), text);
    }

    /// Đổi kiểu: mỗi kiểu giữ máy chủ / khoá riêng (đang bật thì đổi luôn đường đi).
    fn set_kind(self: &Rc<Self>, k: &str) {
        *self.kind.borrow_mut() = k.to_string();
        prefs::set_proxy_kind(k);
        cbtv::net::settings_changed();
        self.build();
    }

    fn read_link(self: &Rc<Self>) {
        match ss::parse_link(&self.ss_link.text(), self.remote_dns.get(), self.sites_only.get()) {
            Ok(c) => {
                self.ss_host.set_text(&c.host);
                self.ss_port.set_text(&c.port.to_string());
                *self.ss_method.borrow_mut() = SsMethod::of(&c.method).unwrap_or(SsMethod::DEFAULT);
                self.ss_key.set_text(&c.password);
                self.ss_link.set_text("");
                *self.result.borrow_mut() = format!("Đã đọc link: {}:{} · {} — bấm «Thử kết nối» rồi «Lưu».", c.host, c.port, c.method);
            }
            Err(e) => *self.result.borrow_mut() = format!("✗ {}", err_text(&e)),
        }
        if *self.kind.borrow() != "ss" {
            self.set_kind("ss");
        } else {
            self.build();
        }
    }

    fn toggle(self: &Rc<Self>, on: bool) {
        if on && !self.save(false) {
            return;
        }
        prefs::set_proxy_on(on);
        cbtv::net::settings_changed();
        app().toast(&if on {
            format!("Đã bật proxy {}.", if *self.kind.borrow() == "ss" { "Shadowsocks" } else { "SOCKS5" })
        } else {
            "Đã tắt proxy — đi thẳng mạng nhà.".into()
        });
        self.build();
    }

    /// Đọc ô Shadowsocks; sai thì báo, trả None.
    fn read_ss(&self) -> Option<SsConfig> {
        let mut host = self.ss_host.text().trim().to_string();
        let mut port_text = self.ss_port.text().trim().to_string();
        if host.matches(':').count() == 1 || host.starts_with('[') {
            if let Some((h, p)) = ss::split_host_port(&host) {
                host = h;
                port_text = p.to_string();
            }
        }
        if host.is_empty() || host.contains(' ') || host.contains('/') {
            app().toast("Nhập máy chủ Shadowsocks (IP hoặc tên miền), hoặc dán link ss://…");
            return None;
        }
        let Some(port) = port_text.parse::<u16>().ok().filter(|p| *p > 0) else {
            app().toast("Cổng không hợp lệ (1–65535).");
            return None;
        };
        let m = self.ss_method.borrow().clone();
        if !m.supported() {
            app().toast(&format!("Máy này không có {} — đổi sang {}.", m.id, SsMethod::DEFAULT.id));
            return None;
        }
        let key = self.ss_key.text().trim().to_string();
        if let Err(e) = ss::keys(&m, &key) {
            app().toast(&err_text(&e));
            return None;
        }
        Some(SsConfig { host, port, method: m.id.to_string(), password: key, remote_dns: self.remote_dns.get(), sites_only: self.sites_only.get() })
    }

    /// Lưu ô nhập; sai thì báo và trả false.
    fn save(&self, notify: bool) -> bool {
        if *self.kind.borrow() == "ss" {
            let Some(c) = self.read_ss() else { return false };
            prefs::set_ss_host(&c.host);
            prefs::set_ss_port(c.port as i64);
            prefs::set_ss_method_id(&c.method);
            prefs::set_ss_key(&c.password);
        } else {
            let host = self.host.text().trim().to_string();
            let Some(port) = self.port.text().trim().parse::<u16>().ok().filter(|p| *p > 0) else {
                app().toast("Cổng không hợp lệ (1–65535).");
                return false;
            };
            if host.is_empty() || host.contains(' ') || host.contains('/') {
                app().toast("Nhập địa chỉ proxy (IP hoặc tên miền, không có http://).");
                return false;
            }
            prefs::set_proxy_host(&host);
            prefs::set_proxy_port(port as i64);
            prefs::set_proxy_user(self.user.text().trim());
            prefs::set_proxy_pass(&self.pass.text());
        }
        prefs::set_proxy_remote_dns(self.remote_dns.get());
        prefs::set_proxy_sites_only(self.sites_only.get());
        cbtv::net::settings_changed();
        if notify {
            app().toast(if prefs::proxy_on() { "Đã lưu — app đi qua proxy mới." } else { "Đã lưu (proxy đang tắt)." });
        }
        true
    }

    /// Thử theo ô đang nhập (chưa cần lưu): SOCKS5 — mở kết nối tới trang phim đang dùng; Shadowsocks — hỏi Cloudflare qua máy
    /// chủ, báo IP ra ngoài.
    fn test(self: &Rc<Self>) {
        let t0 = cbtv::util::now_ms();
        let me = self.clone();
        if *self.kind.borrow() == "ss" {
            let Some(c) = self.read_ss() else { return };
            *self.result.borrow_mut() = format!("Đang thử Shadowsocks {}:{}…", c.host, c.port);
            self.build();
            ui::spawn(async move { ss::check(&c).await }, move |r| {
                *me.result.borrow_mut() = match r {
                    Ok(ip) => format!("✓ Shadowsocks chạy — IP ra ngoài {ip} ({} ms).", cbtv::util::now_ms() - t0),
                    Err(e) => format!("✗ Lỗi Shadowsocks: {}", err_text(&e)),
                };
                me.build();
            });
            return;
        }
        let Some(port) = self.port.text().trim().parse::<u16>().ok().filter(|_| !self.host.text().trim().is_empty()) else {
            app().toast("Nhập địa chỉ và cổng proxy trước.");
            return;
        };
        let sc = SocksConfig {
            host: self.host.text().trim().to_string(),
            port,
            user: self.user.text().trim().to_string(),
            pass: self.pass.text().to_string(),
            remote_dns: self.remote_dns.get(),
            sites_only: false,
        };
        let target = cbtv::net::proxy_test_target();
        *self.result.borrow_mut() = format!("Đang thử qua proxy tới {target}…");
        self.build();
        ui::spawn(
            {
                let t = target.clone();
                async move { cbtv::net::socks5_connect(&sc, &t, 443).await.map(|_| ()) }
            },
            move |r| {
                *me.result.borrow_mut() = match r {
                    Ok(()) => format!("✓ Kết nối được qua proxy tới {target} ({} ms).", cbtv::util::now_ms() - t0),
                    Err(e) => format!("✗ {}", err_text(&e)),
                };
                me.build();
            },
        );
    }
}

/// Câu báo lỗi (lỗi proxy / Shadowsocks: câu gốc, như e.Message của bản Windows).
fn err_text(e: &Error) -> String {
    match e {
        Error::Socks(_, m) => m.clone(),
        other => cbtv::util::friendly(other),
    }
}

impl Page for ProxyPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build();
    }

    fn has_input(&self) -> bool {
        true
    }
}
