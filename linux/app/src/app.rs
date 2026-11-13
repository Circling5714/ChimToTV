//! Cửa sổ chính: menu bên trái (như bản TV), trang nội dung bên phải, trình phát phủ cả cửa sổ, hộp thoại / thông báo trong
//! app. Đi bằng chuột hoặc phím mũi tên / remote (keynav); Esc / Backspace / phím Back = quay lại. Bản Windows:
//! MainWindow.xaml.cs.

use crate::css::px;
use crate::keynav::{self, Dir};
use crate::pages::{self, Page};
use crate::ui;
use cbtv::prefs;
use futures::channel::oneshot;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct MenuEntry {
    pub key: String,
    pub title: String,
    pub open: Rc<dyn Fn()>,
}

pub struct App {
    pub window: gtk::ApplicationWindow,
    stack: gtk::Stack,
    menu_box: gtk::Box,
    site_name: gtk::Label,
    page_host: gtk::Box,
    shell: gtk::Box,
    dialog_layer: gtk::EventBox,
    dialog_title: gtk::Label,
    dialog_text: gtk::Label,
    dialog_extra: gtk::Box,
    dialog_buttons: gtk::Box,
    dialog_box: gtk::Box,
    toast_box: gtk::Box,
    toast_label: gtk::Label,
    toast_timer: RefCell<Option<glib::SourceId>>,
    stack_pages: RefCell<Vec<Rc<dyn Page>>>,
    /// Thứ tự ô đang chọn trong trang hiện tại (ô mất vì trang dựng lại thì chọn ô cùng chỗ).
    focus_index: Cell<Option<usize>>,
    menu_buttons: RefCell<Vec<(String, gtk::Button)>>,
    pub categories: RefCell<Vec<cbtv::Category>>,
    active_menu: RefCell<String>,
    dialog_tx: RefCell<Option<oneshot::Sender<i32>>>,
    focus_before_dialog: RefCell<Option<gtk::Widget>>,
    fullscreen: Cell<bool>,
    maximized: Cell<bool>,
    pub player: RefCell<Option<Rc<crate::player::Player>>>,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Cửa sổ chính (chỉ gọi trên luồng giao diện, sau `App::build`).
pub fn app() -> Rc<App> {
    APP.with(|a| a.borrow().clone().expect("App chưa dựng"))
}

impl App {
    pub fn build(gapp: &gtk::Application) -> Rc<App> {
        let window = gtk::ApplicationWindow::new(gapp);
        window.set_title("Côn Bằng TV");
        window.set_size_request(720, 480);
        let (w, h, max) = match prefs::window_bounds() {
            Some(b) if b.len() == 3 => (b[0] as i32, b[1] as i32, b[2] > 0.0),
            _ => (1360, 820, false),
        };
        window.set_default_size(w.max(720), h.max(480));
        if max {
            window.maximize();
        }
        if let Some(p) = crate::assets::logo(128) {
            window.set_icon(Some(&p));
        }
        window.style_context().add_class("bg");

        // ---- Menu bên trái ----
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
        menu.style_context().add_class("sidebar");
        menu.set_size_request(px(250.0), -1);
        let head = ui::hbox(12.0);
        ui::margins(&head, 22.0, 16.0, 14.0, 20.0);
        if let Some(p) = crate::assets::logo(px(44.0)) {
            head.pack_start(&gtk::Image::from_pixbuf(Some(&p)), false, false, 0);
        }
        let names = ui::vbox(0.0);
        names.set_valign(gtk::Align::Center);
        names.pack_start(&ui::label("Côn Bằng TV", &["app-name"], false), false, false, 0);
        let site_name = ui::label("", &["site-name"], false);
        site_name.set_max_width_chars(20);
        names.pack_start(&site_name, false, false, 0);
        head.pack_start(&names, true, true, 0);
        menu.pack_start(&head, false, false, 0);
        let menu_scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        menu_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        menu_scroll.set_can_focus(false);
        let menu_box = ui::vbox(0.0);
        menu_box.set_margin_top(px(4.0));
        menu_box.set_margin_bottom(px(12.0));
        menu_scroll.add(&menu_box);
        menu.pack_start(&menu_scroll, true, true, 0);

        // ---- Nội dung ----
        let page_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let shell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        shell.pack_start(&menu, false, false, 0);
        shell.pack_start(&page_host, true, true, 0);

        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::None);
        stack.add_named(&shell, "shell");

        // ---- Hộp thoại ----
        let dialog_layer = gtk::EventBox::new();
        dialog_layer.style_context().add_class("dialog-layer");
        dialog_layer.set_no_show_all(true);
        let dialog_box = ui::vbox(0.0);
        dialog_box.style_context().add_class("dialog");
        dialog_box.set_halign(gtk::Align::Center);
        dialog_box.set_valign(gtk::Align::Center);
        dialog_box.set_size_request(px(380.0), -1);
        let dialog_title = ui::label("", &["dialog-title"], true);
        dialog_title.set_max_width_chars(48);
        let dialog_text = ui::dim("");
        dialog_text.set_max_width_chars(60);
        dialog_text.set_margin_top(px(12.0));
        let dialog_extra = ui::vbox(0.0);
        dialog_extra.set_margin_top(px(12.0));
        let dialog_buttons = ui::hbox(10.0);
        dialog_buttons.set_halign(gtk::Align::End);
        dialog_buttons.set_margin_top(px(20.0));
        dialog_box.pack_start(&dialog_title, false, false, 0);
        dialog_box.pack_start(&dialog_text, false, false, 0);
        dialog_box.pack_start(&dialog_extra, false, false, 0);
        dialog_box.pack_start(&dialog_buttons, false, false, 0);
        dialog_layer.add(&dialog_box);

        // ---- Thông báo ngắn ----
        let toast_box = ui::hbox(0.0);
        toast_box.style_context().add_class("toast");
        toast_box.set_halign(gtk::Align::Center);
        toast_box.set_valign(gtk::Align::End);
        toast_box.set_margin_bottom(px(36.0));
        toast_box.set_no_show_all(true);
        let toast_label = ui::text("");
        toast_label.set_max_width_chars(70);
        toast_box.pack_start(&toast_label, false, false, 0);
        toast_label.show();

        let root = gtk::Overlay::new();
        root.add(&stack);
        root.add_overlay(&dialog_layer);
        root.add_overlay(&toast_box);
        root.set_overlay_pass_through(&toast_box, true);
        window.add(&root);

        let a = Rc::new(App {
            window,
            stack,
            menu_box,
            site_name,
            page_host,
            shell,
            dialog_layer,
            dialog_title,
            dialog_text,
            dialog_extra,
            dialog_buttons,
            dialog_box,
            toast_box,
            toast_label,
            toast_timer: RefCell::new(None),
            stack_pages: RefCell::new(Vec::new()),
            focus_index: Cell::new(None),
            menu_buttons: RefCell::new(Vec::new()),
            categories: RefCell::new(Vec::new()),
            active_menu: RefCell::new("home".into()),
            dialog_tx: RefCell::new(None),
            focus_before_dialog: RefCell::new(None),
            fullscreen: Cell::new(false),
            maximized: Cell::new(max),
            player: RefCell::new(None),
        });
        APP.with(|x| *x.borrow_mut() = Some(a.clone()));
        a.connect_events();
        a
    }

    fn connect_events(self: &Rc<Self>) {
        self.window.connect_key_press_event(|_, ev| app().on_key(ev));
        self.window.connect_window_state_event(|_, ev| {
            let s = ev.new_window_state();
            let a = app();
            a.fullscreen.set(s.contains(gdk::WindowState::FULLSCREEN));
            a.maximized.set(s.contains(gdk::WindowState::MAXIMIZED));
            glib::Propagation::Proceed
        });
        self.window.connect_button_press_event(|_, ev| {
            // nút lùi của chuột = quay lại
            if ev.button() == 8 && app().playing().is_none() && !app().dialog_open() {
                app().back();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        // Rời app hơn 30 phút (không xem phim): hỏi lại ai đang xem; quay lại app (vd vừa xem trên tivi): đồng bộ, tối đa 1 phút
        // một lần
        self.window.connect_is_active_notify(|w| {
            thread_local! {
                static AWAY: Cell<i64> = const { Cell::new(0) };
                static LAST_SYNC: Cell<i64> = const { Cell::new(0) };
            }
            let now = cbtv::util::now_ms();
            if !w.is_active() {
                AWAY.with(|a| a.set(now));
                return;
            }
            let away = AWAY.with(|a| a.get());
            let a = app();
            // Ô đang chọn mất lúc cửa sổ ở dưới (trang dựng lại khi đồng bộ…) thì lúc quay lại chọn lại
            ui::after(150, || app().refocus_if_lost());
            if away > 0 && now - away > 30 * 60_000 && a.playing().is_none() && prefs::profiles().len() > 1 {
                let cur = prefs::current_profile();
                if prefs::ask_profile() || cur.locked() {
                    crate::profiles::pick_profile(!cur.locked());
                }
            }
            if now - LAST_SYNC.with(|l| l.get()) > 60_000 {
                LAST_SYNC.with(|l| l.set(now));
                crate::startup::sync_soon(1);
            }
        });
        // Lưới an toàn: ô đang chọn bị huỷ (trang dựng lại sau khi bấm) thì GTK để trống focus — người dùng remote / bàn phím không
        // thấy ô nào sáng, phím Delete / Enter không có đích. Đợi một nhịp (trang có thể tự chọn ô sau 30–60 ms) rồi chọn ô đầu trang
        // (trang chưa có ô nào thì chọn menu).
        self.window.connect_set_focus(|_, f| {
            let a = app();
            let cur = a.stack_pages.try_borrow().ok().and_then(|s| s.last().cloned());
            if let Some(f) = f {
                // Nhớ thứ tự ô trong trang (ô trong hộp thoại / menu thì giữ số cũ)
                if let Some(p) = cur.filter(|p| f.is_ancestor(&p.base().body)) {
                    if let Some(i) = keynav::targets(p.base().body.upcast_ref()).iter().position(|t| t == f) {
                        a.focus_index.set(Some(i));
                    }
                }
                return;
            }
            ui::after(150, || app().refocus_if_lost());
        });
        self.window.connect_delete_event(|_, _| {
            app().closing();
            glib::Propagation::Proceed
        });
    }

    /// Lưu cỡ cửa sổ, đóng phim, ghi cài đặt (lúc thoát).
    fn closing(&self) {
        if !self.fullscreen.get() {
            let (w, h) = self.window.size();
            prefs::set_window_bounds(&[w as f64, h as f64, if self.maximized.get() { 1.0 } else { 0.0 }]);
        }
        if let Some(p) = self.player.borrow().clone() {
            p.close(false);
        }
        cbtv::sync_service::stop();
        prefs::flush();
    }

    pub fn start(self: &Rc<Self>) {
        self.window.show_all();
        if prefs::start_fullscreen() {
            self.set_fullscreen(true);
        }
        crate::images::cleanup();
        self.rebuild_menu();
        self.show_root(crate::menu::home_page());
        crate::debug::start();
        crate::startup::after_show();
    }

    // ---- Toàn màn hình ----

    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen.get()
    }

    pub fn set_fullscreen(&self, on: bool) {
        if on == self.fullscreen.get() {
            return;
        }
        if on {
            self.window.fullscreen();
        } else {
            self.window.unfullscreen();
        }
        self.fullscreen.set(on);
        // Toàn màn hình không có thanh tiêu đề: menu có thêm mục thoát toàn màn hình
        self.rebuild_menu();
    }

    pub fn toggle_fullscreen(&self) {
        self.set_fullscreen(!self.fullscreen.get());
    }

    // ---- Menu ----

    fn menu_entries(&self) -> Vec<MenuEntry> {
        let mut v: Vec<MenuEntry> = Vec::new();
        let mut add = |key: &str, title: String, open: Rc<dyn Fn()>| v.push(MenuEntry { key: key.into(), title, open });
        if self.fullscreen.get() {
            add("windowed", "⤡  Thoát toàn màn hình (F11)".into(), Rc::new(|| app().set_fullscreen(false)));
        }
        let me = prefs::current_profile();
        add(
            "profile",
            format!("👤  Người xem: {}{}", me.name, if me.locked() { " 🔒" } else { "" }),
            Rc::new(|| crate::profiles::pick_profile(true)),
        );
        for e in crate::menu::entries(self) {
            v.push(e);
        }
        v
    }

    pub fn set_categories(&self, list: Vec<cbtv::Category>) {
        *self.categories.borrow_mut() = list;
        self.rebuild_menu();
    }

    pub fn rebuild_menu(&self) {
        let focused = self.window.focused_widget().and_then(|f| {
            self.menu_buttons.borrow().iter().find(|(_, b)| b.upcast_ref::<gtk::Widget>() == &f).map(|(k, _)| k.clone())
        });
        ui::clear(&self.menu_box);
        self.menu_buttons.borrow_mut().clear();
        let base = prefs::base_url();
        self.site_name.set_text(&crate::menu::site_display(&base));
        let mut cats_started = false;
        for m in self.menu_entries() {
            if (m.key.starts_with("cat:") && !cats_started) || m.key == "sites" {
                if m.key.starts_with("cat:") {
                    cats_started = true;
                }
                let sep = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                sep.style_context().add_class("separator-line");
                ui::margins(&sep, 8.0, 16.0, 8.0, 16.0);
                self.menu_box.pack_start(&sep, false, false, 0);
            }
            let b = gtk::Button::with_label(&m.title);
            b.style_context().add_class("menu-item");
            if let Some(l) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
                l.set_xalign(0.0);
                l.set_ellipsize(pango::EllipsizeMode::End);
            }
            if m.key == *self.active_menu.borrow() {
                b.style_context().add_class("active");
            }
            let open = m.open.clone();
            b.connect_clicked(move |_| open());
            self.menu_box.pack_start(&b, false, false, 0);
            self.menu_buttons.borrow_mut().push((m.key.clone(), b));
        }
        self.menu_box.show_all();
        if let Some(k) = focused {
            if let Some((_, b)) = self.menu_buttons.borrow().iter().find(|(x, _)| *x == k) {
                b.grab_focus();
            }
        }
    }

    fn set_active_menu(&self, key: &str) {
        *self.active_menu.borrow_mut() = key.to_string();
        for (k, b) in self.menu_buttons.borrow().iter() {
            if k == key {
                b.style_context().add_class("active");
            } else {
                b.style_context().remove_class("active");
            }
        }
    }

    /// Không ô nào được chọn (ô cũ bị huỷ khi trang dựng lại) → chọn ô cùng thứ tự trong trang / ô đầu / menu. Người dùng remote /
    /// bàn phím phải luôn thấy một ô sáng.
    pub fn refocus_if_lost(&self) {
        if self.window.focused_widget().is_some() || !self.window.is_active() || self.playing().is_some() || self.dialog_open() {
            return;
        }
        let Some(p) = self.current() else {
            self.focus_menu();
            return;
        };
        let targets = keynav::targets(p.base().body.upcast_ref());
        match self.focus_index.get().and_then(|i| targets.get(i.min(targets.len().saturating_sub(1)))) {
            Some(t) => keynav::focus_and_show(t),
            None if p.first_target().is_some() => pages::focus_first(p.as_ref()),
            None => self.focus_menu(),
        }
    }

    pub fn focus_menu(&self) {
        let list = self.menu_buttons.borrow();
        let active = self.active_menu.borrow().clone();
        if let Some((_, b)) = list.iter().find(|(k, _)| *k == active).or(list.first()) {
            keynav::focus_and_show(b.upcast_ref());
        }
    }

    fn menu_has_focus(&self) -> bool {
        match self.window.focused_widget() {
            Some(f) => self.menu_buttons.borrow().iter().any(|(_, b)| b.upcast_ref::<gtk::Widget>() == &f),
            None => false,
        }
    }

    // ---- Trang ----

    pub fn current(&self) -> Option<Rc<dyn Page>> {
        self.stack_pages.borrow().last().cloned()
    }

    fn host(&self, p: &Rc<dyn Page>) {
        self.focus_index.set(None);
        ui::clear(&self.page_host);
        self.page_host.pack_start(&p.base().scroller, true, true, 0);
        p.base().scroller.show_all();
    }

    /// Mở trang gốc (từ menu): bỏ các trang đang mở.
    pub fn show_root(&self, p: Rc<dyn Page>) {
        self.stack_pages.borrow_mut().clear();
        self.push(p);
    }

    pub fn push(&self, p: Rc<dyn Page>) {
        if let Some(cur) = self.current() {
            let f = self.window.focused_widget();
            let idx = f.as_ref().and_then(|f| keynav::targets(cur.base().body.upcast_ref()).iter().position(|w| w == f));
            cur.base().last_index.set(idx);
            *cur.base().last_focus.borrow_mut() = f;
        }
        self.stack_pages.borrow_mut().push(p.clone());
        self.set_active_menu(&p.base().menu_key.borrow());
        self.host(&p);
        p.start();
    }

    /// Thay trang trên cùng (vd đổi nguồn ở trang phim): Esc vẫn về trang trước đó.
    pub fn replace(&self, p: Rc<dyn Page>) {
        if self.stack_pages.borrow().is_empty() {
            self.push(p);
            return;
        }
        if let Some(last) = self.stack_pages.borrow_mut().last_mut() {
            *last = p.clone();
        }
        self.set_active_menu(&p.base().menu_key.borrow());
        self.host(&p);
        p.start();
    }

    pub fn back(&self) {
        if self.dialog_open() {
            self.close_dialog(-1);
            return;
        }
        if self.stack_pages.borrow().len() > 1 {
            self.stack_pages.borrow_mut().pop();
            let p = self.current().unwrap();
            self.set_active_menu(&p.base().menu_key.borrow());
            self.host(&p);
            p.clone().returned();
            restore_focus(p);
            return;
        }
        // Trang gốc: Back về menu (như bản TV); đã ở menu mà đang toàn màn hình → thoát toàn màn hình
        if !self.menu_has_focus() {
            self.focus_menu();
        } else if self.fullscreen.get() {
            self.set_fullscreen(false);
            self.toast("Đã thoát toàn màn hình (F11 để bật lại).");
        }
    }

    // ---- Trình phát ----

    /// Phim đang xem (không phải ở cửa sổ nhỏ).
    pub fn playing(&self) -> Option<Rc<crate::player::Player>> {
        self.player.borrow().clone().filter(|p| p.is_open())
    }

    /// Mở phim ở tập ep_url. resume: mở từ "Xem tiếp" (tập đã xem hết thì sang tập sau).
    pub fn play(&self, film_url: &str, ep_url: &str, resume: bool) {
        if let Some(cur) = self.current() {
            *cur.base().last_focus.borrow_mut() = self.window.focused_widget();
        }
        let p = {
            let mut slot = self.player.borrow_mut();
            if slot.is_none() {
                let p = crate::player::Player::new();
                self.stack.add_named(&p.widget(), "player");
                *slot = Some(p);
            }
            slot.clone().unwrap()
        };
        p.widget().show_all();
        self.stack.set_visible_child_name("player");
        p.open(film_url, ep_url, resume);
        self.rebuild_menu();
    }

    /// Trình phát đã đóng: về trang trước, cập nhật tập đã xem / xem tiếp.
    pub fn player_closed(&self) {
        crate::startup::sync_soon(1);
        self.stack.set_visible_child_name("shell");
        self.window.set_title("Côn Bằng TV");
        if let Some(p) = self.current() {
            p.clone().returned();
            restore_focus(p);
        }
        self.rebuild_menu();
    }

    // ---- Phím ----

    fn on_key(&self, ev: &gdk::EventKey) -> glib::Propagation {
        use gdk::keys::constants as k;
        let key = ev.keyval();
        let st = ev.state();
        let alt = st.contains(gdk::ModifierType::MOD1_MASK);
        let ctrl = st.contains(gdk::ModifierType::CONTROL_MASK);
        let focused = self.window.focused_widget();
        let entry = focused.as_ref().and_then(|f| f.clone().downcast::<gtk::Entry>().ok());

        if self.dialog_open() {
            if key == k::Escape || key == k::Back || (key == k::BackSpace && entry.is_none()) {
                self.close_dialog(-1);
                return glib::Propagation::Stop;
            }
            if let Some(dir) = keynav::dir_of(&key) {
                if entry.is_some() && matches!(dir, Dir::Left | Dir::Right) {
                    return glib::Propagation::Proceed;
                }
                let root: gtk::Widget = self.dialog_box.clone().upcast();
                match focused.filter(|f| keynav::is_inside(f, &root)) {
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
                return glib::Propagation::Stop;
            }
            return glib::Propagation::Proceed;
        }
        if let Some(p) = self.playing() {
            if p.handle_key(ev) {
                return glib::Propagation::Stop;
            }
            return glib::Propagation::Proceed;
        }
        if key == k::F11 || (key == k::Return && alt) {
            self.toggle_fullscreen();
            return glib::Propagation::Stop;
        }
        if key == k::Escape || key == k::Back || (key == k::BackSpace && entry.is_none()) {
            self.back();
            return glib::Propagation::Stop;
        }
        if key == k::Search || (ctrl && (key == k::f || key == k::F)) {
            self.show_root(crate::menu::search_page());
            return glib::Propagation::Stop;
        }
        if key == k::HomePage {
            self.show_root(crate::menu::home_page());
            return glib::Propagation::Stop;
        }
        let Some(dir) = keynav::dir_of(&key) else { return glib::Propagation::Proceed };
        if let Some(e) = &entry {
            if matches!(dir, Dir::Left | Dir::Right) {
                let pos = e.position();
                let len = e.text().chars().count() as i32;
                let sel = e.selection_bounds().is_some();
                let at_edge = if dir == Dir::Left { pos == 0 && !sel } else { pos >= len };
                if !at_edge {
                    return glib::Propagation::Proceed;
                }
            }
        }
        let root: gtk::Widget = self.shell.clone().upcast();
        let page_host: gtk::Widget = self.page_host.clone().upcast();
        let Some(f) = focused.filter(|f| keynav::is_inside(f, &root) && f.is_visible() && f.is_mapped()) else {
            let first = self.current().and_then(|p| p.first_target()).or_else(|| keynav::first(&root));
            if let Some(f) = first {
                keynav::focus_and_show(&f);
            }
            return glib::Propagation::Stop;
        };
        // ▲ / ▼ trong trang chỉ đi trong trang (không nhảy sang menu bên trái)
        let area = if matches!(dir, Dir::Up | Dir::Down) && keynav::is_inside(&f, &page_host) { page_host } else { root };
        match keynav::next(&area, &f, dir) {
            Some(n) => keynav::focus_and_show(&n),
            None => {
                if matches!(dir, Dir::Up | Dir::Down) {
                    if let Some(p) = self.current() {
                        p.scroll_edge(dir);
                    }
                }
            }
        }
        glib::Propagation::Stop
    }

    // ---- Thông báo / hộp thoại ----

    pub fn toast(&self, text: &str) {
        self.toast_ms(text, 4000);
    }

    pub fn toast_ms(&self, text: &str, ms: u64) {
        self.toast_label.set_text(text);
        self.toast_box.show();
        if let Some(id) = self.toast_timer.borrow_mut().take() {
            id.remove();
        }
        let id = ui::after(ms, || {
            let a = app();
            a.toast_timer.borrow_mut().take();
            a.toast_box.hide();
        });
        *self.toast_timer.borrow_mut() = Some(id);
        if let Some(p) = self.playing() {
            p.notice(text, ms);
        }
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog_layer.is_visible()
    }

    /// Hộp thoại: trả chỉ số nút được bấm (-1 = huỷ / Back). Nút `focus` được chọn sẵn.
    pub fn ask(&self, title: &str, text: &str, buttons: &[&str], extra: Option<gtk::Widget>, focus: usize) -> oneshot::Receiver<i32> {
        if let Some(tx) = self.dialog_tx.borrow_mut().take() {
            let _ = tx.send(-1);
        }
        if !self.dialog_open() {
            *self.focus_before_dialog.borrow_mut() = self.window.focused_widget();
        }
        self.dialog_title.set_text(title);
        self.dialog_text.set_text(text);
        self.dialog_text.set_visible(!text.is_empty());
        ui::clear(&self.dialog_extra);
        self.dialog_extra.set_visible(extra.is_some());
        if let Some(e) = &extra {
            self.dialog_extra.pack_start(e, false, false, 0);
        }
        ui::clear(&self.dialog_buttons);
        let mut list = Vec::new();
        for (i, t) in buttons.iter().enumerate() {
            let b = ui::pill(t, move || app().close_dialog(i as i32));
            b.set_size_request(px(110.0), -1);
            self.dialog_buttons.pack_start(&b, false, false, 0);
            list.push(b);
        }
        let (tx, rx) = oneshot::channel();
        *self.dialog_tx.borrow_mut() = Some(tx);
        self.dialog_layer.show();
        self.dialog_box.show_all();
        self.dialog_text.set_visible(!text.is_empty());
        self.dialog_extra.set_visible(extra.is_some());
        if let Some(b) = list.get(focus) {
            let b = b.clone();
            ui::after(10, move || b.grab_focus());
        }
        rx
    }

    pub async fn ask_async(&self, title: &str, text: &str, buttons: &[&str], extra: Option<gtk::Widget>, focus: usize) -> i32 {
        self.ask(title, text, buttons, extra, focus).await.unwrap_or(-1)
    }

    pub async fn confirm(&self, title: &str, text: &str, ok: &str, cancel: &str) -> bool {
        self.ask_async(title, text, &[ok, cancel], None, 1).await == 0
    }

    pub fn close_dialog(&self, result: i32) {
        self.dialog_layer.hide();
        ui::clear(&self.dialog_extra);
        if let Some(f) = self.focus_before_dialog.borrow_mut().take() {
            if f.is_visible() && f.is_mapped() {
                f.grab_focus();
            }
        }
        if let Some(tx) = self.dialog_tx.borrow_mut().take() {
            let _ = tx.send(result);
        }
    }

    /// Hộp thoại đang mở: rộng tối đa (miễn trừ trách nhiệm…).
    pub fn dialog_width(&self, w: i32) {
        self.dialog_box.set_size_request(w, -1);
    }

    /// Nền hộp thoại đen kín (không lộ trang phía sau — «Ai đang xem?» bắt buộc).
    pub fn dialog_solid(&self, on: bool) {
        if on {
            self.dialog_layer.style_context().add_class("solid");
        } else {
            self.dialog_layer.style_context().remove_class("solid");
        }
    }

    /// Nhập mã PIN 4 số (đủ 4 số là tự xong). None = huỷ.
    pub async fn ask_pin(&self, title: &str, text: &str) -> Option<String> {
        let entry = crate::ui::entry();
        entry.set_visibility(false);
        entry.set_max_length(prefs::PIN_LENGTH as i32);
        entry.set_input_purpose(gtk::InputPurpose::Pin);
        entry.set_width_chars(8);
        entry.set_halign(gtk::Align::Start);
        entry.connect_changed(|e| {
            let t = e.text();
            if t.chars().count() == prefs::PIN_LENGTH && t.chars().all(|c| c.is_ascii_digit()) {
                app().close_dialog(0);
            }
        });
        // Enter khi chưa đủ số thì bỏ qua (đóng luôn là huỷ cả việc đặt mã — bấm nhầm Enter rất dễ); Esc / «Huỷ» mới là huỷ
        entry.connect_activate(|e| {
            if e.text().chars().count() == prefs::PIN_LENGTH {
                app().close_dialog(0);
            }
        });
        let rx = self.ask(title, text, &["OK", "Huỷ"], Some(entry.clone().upcast()), 0);
        let e2 = entry.clone();
        ui::after(30, move || e2.grab_focus());
        let r = rx.await.unwrap_or(-1);
        let pin = entry.text().to_string();
        (r == 0 && pin.chars().count() == prefs::PIN_LENGTH && pin.chars().all(|c| c.is_ascii_digit())).then_some(pin)
    }

    /// Đặt mã PIN mới (nhập hai lần). None = huỷ / không khớp.
    pub async fn ask_new_pin(&self, title: &str) -> Option<String> {
        let a = self.ask_pin(title, "Nhập mã PIN mới (4 số).").await?;
        let b = self.ask_pin(title, "Nhập lại mã PIN để xác nhận.").await;
        if b.as_deref() == Some(a.as_str()) {
            return Some(a);
        }
        if b.is_some() {
            self.toast("Hai lần nhập không khớp.");
        }
        None
    }

    /// Hỏi mã PIN phụ huynh (chưa có thì đặt mới). true = đúng / vừa đặt.
    pub async fn check_parent(&self, why: &str) -> bool {
        if !prefs::has_parent_pin() {
            let Some(np) = self.ask_new_pin("Đặt mã PIN phụ huynh").await else { return false };
            prefs::set_parent_pin(Some(&np));
            self.toast("Đã đặt mã PIN phụ huynh — nhớ mã này, quên thì phải xoá dữ liệu app.");
            return true;
        }
        let Some(pin) = self.ask_pin("Mã PIN phụ huynh", why).await else { return false };
        if prefs::check_parent_pin(&pin) {
            return true;
        }
        self.toast("Sai mã PIN phụ huynh.");
        false
    }

    pub fn stack(&self) -> &gtk::Stack {
        &self.stack
    }
}

/// Quay lại một trang: chọn lại ô cũ (còn hiện) hoặc ô đầu tiên.
pub fn restore_focus(p: Rc<dyn Page>) {
    ui::after(30, move || {
        let last = p.base().last_focus.borrow().clone();
        if let Some(f) = last.filter(|f| f.is_visible() && f.is_mapped() && f.is_ancestor(&p.base().scroller)) {
            keynav::focus_and_show(&f);
            return;
        }
        // Trang đã dựng lại: chọn ô cùng thứ tự (vd nút «Giới hạn» của đúng người xem vừa sửa)
        let targets = keynav::targets(p.base().body.upcast_ref());
        match p.base().last_index.get().and_then(|i| targets.get(i.min(targets.len().saturating_sub(1)))) {
            Some(f) => keynav::focus_and_show(f),
            None => pages::focus_first(p.as_ref()),
        }
    });
}
