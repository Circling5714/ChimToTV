//! Các trang nội dung (bên phải menu). Trang tự tải dữ liệu (start), hiện "Đang tải…" / lỗi kèm nút thử lại.
//! Bản Windows: UI/Page.cs + UI/Pages.cs…

use crate::app::app;
use crate::keynav::{self, Dir};
use crate::ui;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub mod detail;
pub mod history;
pub mod home;
pub mod home_mix;
pub mod home_rows;
pub mod invite;
pub mod lists;
pub mod profiles_page;
pub mod proxy;
pub mod settings;
pub mod sites;
pub mod sync_page;

pub struct PageBase {
    pub scroller: gtk::ScrolledWindow,
    pub body: gtk::Box,
    pub menu_key: RefCell<String>,
    /// Mục đang chọn lúc rời trang (quay lại thì chọn lại).
    pub last_focus: RefCell<Option<gtk::Widget>>,
    token: Cell<u64>,
}

impl PageBase {
    pub fn new(menu_key: &str) -> PageBase {
        let (scroller, body) = ui::scroller();
        PageBase { scroller, body, menu_key: RefCell::new(menu_key.to_string()), last_focus: RefCell::new(None), token: Cell::new(0) }
    }

    /// Lần tải mới (kết quả của lần tải cũ về muộn thì bỏ).
    pub fn next_token(&self) -> u64 {
        self.token.set(self.token.get() + 1);
        self.token.get()
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    pub fn clear(&self) {
        ui::clear(&self.body);
    }

    pub fn add(&self, w: &impl IsA<gtk::Widget>) {
        self.body.pack_start(w, false, false, 0);
        w.show_all();
    }

    pub fn loading(&self, text: &str) {
        self.clear();
        let p = ui::vbox(14.0);
        p.set_halign(gtk::Align::Center);
        p.set_margin_top(crate::css::px(140.0));
        let bar = gtk::ProgressBar::new();
        bar.set_size_request(crate::css::px(260.0), -1);
        p.pack_start(&bar, false, false, 0);
        let l = ui::dim(text);
        l.set_halign(gtk::Align::Center);
        p.pack_start(&l, false, false, 0);
        // thanh chạy qua lại (không biết trước thời gian)
        let weak = bar.downgrade();
        glib::timeout_add_local(std::time::Duration::from_millis(120), move || match weak.upgrade() {
            Some(b) if b.is_visible() || b.parent().is_some() => {
                b.pulse();
                glib::ControlFlow::Continue
            }
            _ => glib::ControlFlow::Break,
        });
        self.add(&p);
    }

    pub fn show_error(&self, message: &str, retry: Rc<dyn Fn()>) {
        self.clear();
        let t = ui::label("Không tải được", &["big"], true);
        t.set_margin_top(crate::css::px(60.0));
        t.set_margin_bottom(crate::css::px(10.0));
        self.add(&t);
        let m = ui::dim(message);
        m.set_margin_bottom(crate::css::px(20.0));
        self.add(&m);
        let r = retry.clone();
        let row = ui::row(&[
            ui::pill("↻  Thử lại", move || r()).upcast(),
            ui::pill("🌐  Đổi trang phim", || app().show_root(crate::pages::sites::new())).upcast(),
        ]);
        self.add(&row);
        let body = self.body.clone();
        ui::after(50, move || {
            if let Some(f) = keynav::first(body.upcast_ref()) {
                keynav::focus_and_show(&f);
            }
        });
    }
}

pub trait Page {
    fn base(&self) -> &PageBase;

    /// Gọi khi trang được mở (tải dữ liệu).
    fn start(self: Rc<Self>);

    /// Quay lại trang này (từ trang sau / trình phát).
    fn returned(self: Rc<Self>) {}

    /// Mục chọn đầu tiên khi bấm phím mũi tên lần đầu.
    fn first_target(&self) -> Option<gtk::Widget> {
        keynav::first(self.base().body.upcast_ref())
    }

    /// Không còn mục nào theo hướng ▲ / ▼: cuộn hết lên / xuống.
    fn scroll_edge(&self, dir: Dir) {
        let adj = self.base().scroller.vadjustment();
        if dir == Dir::Up {
            adj.set_value(adj.lower());
        } else {
            adj.set_value(adj.upper() - adj.page_size());
        }
    }

    /// Trang có ô nhập (đồng bộ xong thì không dựng lại kẻo mất chữ đang gõ).
    fn has_input(&self) -> bool {
        false
    }
}

pub fn focus_first(p: &dyn Page) {
    if let Some(f) = p.first_target() {
        keynav::focus_and_show(&f);
    }
}
