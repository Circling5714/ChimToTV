//! Dựng nhanh các phần giao diện dùng chung (chữ, nút, lưới, dải vuốt ngang) + chạy việc nền rồi trả kết quả về luồng giao
//! diện. Bản Windows: UI/Ui.cs.

use crate::css::px;
use gtk::prelude::*;
use std::future::Future;

/// Chạy việc nền (mạng, đọc trang) trên bộ chạy của lõi, xong thì gọi `done` trên luồng giao diện.
pub fn spawn<T, F>(fut: F, done: impl FnOnce(T) + 'static)
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let h = cbtv::rt().spawn(fut);
    glib::MainContext::default().spawn_local(async move {
        if let Ok(v) = h.await {
            done(v);
        }
    });
}

/// Chạy việc nền và chờ kết quả trong một luồng giao diện async (glib::MainContext::spawn_local).
pub async fn run<T, F>(fut: F) -> Option<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    cbtv::rt().spawn(fut).await.ok()
}

/// Chạy một luồng giao diện async (hộp thoại nối tiếp…).
pub fn local(fut: impl Future<Output = ()> + 'static) {
    glib::MainContext::default().spawn_local(fut);
}

/// Hẹn giờ một lần trên luồng giao diện.
pub fn after(ms: u64, f: impl FnOnce() + 'static) -> glib::SourceId {
    glib::timeout_add_local_once(std::time::Duration::from_millis(ms), f)
}

/// Chờ (trong luồng giao diện async).
pub async fn sleep(ms: u64) {
    glib::timeout_future(std::time::Duration::from_millis(ms)).await;
}

pub fn label(text: &str, class: &[&str], wrap: bool) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.set_halign(gtk::Align::Start);
    if wrap {
        l.set_line_wrap(true);
        l.set_line_wrap_mode(pango::WrapMode::WordChar);
    } else {
        l.set_ellipsize(pango::EllipsizeMode::End);
    }
    for c in class {
        l.style_context().add_class(c);
    }
    l
}

/// Chữ thường (tự xuống dòng).
pub fn text(s: &str) -> gtk::Label {
    label(s, &[], true)
}

/// Chữ mờ (ghi chú).
pub fn dim(s: &str) -> gtk::Label {
    label(s, &["dim"], true)
}

/// Tiêu đề trang / nhóm.
pub fn header(s: &str) -> gtk::Label {
    let l = label(s, &["h1"], false);
    l.set_margin_top(px(10.0));
    l.set_margin_bottom(px(14.0));
    l
}

pub fn margins(w: &impl IsA<gtk::Widget>, top: f64, right: f64, bottom: f64, left: f64) {
    w.set_margin_top(px(top));
    w.set_margin_end(px(right));
    w.set_margin_bottom(px(bottom));
    w.set_margin_start(px(left));
}

/// Nút bo tròn (đang chọn bằng phím: nền cam). tag: "active" (đang dùng) / "watched" (đã xem).
pub fn pill(text: &str, on_click: impl Fn() + 'static) -> gtk::Button {
    let b = gtk::Button::with_label(text);
    b.style_context().add_class("pill");
    b.set_can_focus(true);
    b.set_focus_on_click(true);
    if let Some(l) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
        l.set_line_wrap(true);
        l.set_max_width_chars(48);
    }
    b.connect_clicked(move |_| on_click());
    b
}

pub fn pill_tag(text: &str, tag: Option<&str>, on_click: impl Fn() + 'static) -> gtk::Button {
    let b = pill(text, on_click);
    if let Some(t) = tag {
        b.style_context().add_class(t);
    }
    b
}

/// Đổi chữ của nút.
pub fn set_text(b: &gtk::Button, text: &str) {
    b.set_label(text);
}

/// Hàng nút tự xuống dòng.
pub fn row(items: &[gtk::Widget]) -> gtk::FlowBox {
    let f = flow(10.0, 10.0);
    for w in items {
        add_flow(&f, w);
    }
    f
}

/// Lưới tự xuống hàng (thẻ phim / nút).
pub fn flow(col: f64, row: f64) -> gtk::FlowBox {
    let f = gtk::FlowBox::new();
    f.set_selection_mode(gtk::SelectionMode::None);
    f.set_homogeneous(false);
    f.set_column_spacing(px(col) as u32);
    f.set_row_spacing(px(row) as u32);
    f.set_max_children_per_line(200);
    f.set_min_children_per_line(1);
    f.set_valign(gtk::Align::Start);
    f.set_halign(gtk::Align::Fill);
    f.set_can_focus(false);
    f.set_activate_on_single_click(false);
    f
}

/// Thêm vào lưới; ô bọc ngoài (FlowBoxChild) không nhận phím — phím đi thẳng vào nút / thẻ bên trong.
pub fn add_flow(f: &gtk::FlowBox, w: &gtk::Widget) {
    f.insert(w, -1);
    if let Some(c) = w.parent() {
        c.set_can_focus(false);
    }
}

/// Lưới thẻ phim.
pub fn grid(cards: Vec<gtk::Widget>) -> gtk::FlowBox {
    let f = flow(18.0, 22.0);
    for c in cards {
        add_flow(&f, &c);
    }
    f
}

pub fn vbox(spacing: f64) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, px(spacing))
}

pub fn hbox(spacing: f64) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, px(spacing))
}

/// Một nhóm phim: tiêu đề (+ ghi chú) + lưới thẻ (tự xuống hàng).
pub fn section(title: &str, note: Option<&str>, cards: Vec<gtk::Widget>) -> gtk::Box {
    let p = vbox(0.0);
    p.set_margin_bottom(px(12.0));
    let head = hbox(8.0);
    head.set_margin_top(px(6.0));
    head.set_margin_bottom(px(14.0));
    head.pack_start(&label(title, &["h1"], false), false, false, 0);
    if let Some(n) = note {
        let l = dim(n);
        l.set_valign(gtk::Align::End);
        head.pack_start(&l, false, false, 0);
    }
    p.pack_start(&head, false, false, 0);
    p.pack_start(&grid(cards), false, false, 0);
    p
}

/// Hàng vuốt ngang của trang chủ tổng hợp: tiêu đề (+ «Xem thêm ›» bên phải cho chuột), dải cuộn ngang các thẻ (phím ◄ ►
/// cuộn theo ô đang chọn — keynav; Shift + lăn chuột cuộn dải, lăn chuột thường cuộn trang).
pub fn strip(title: &gtk::Widget, cards: Vec<gtk::Widget>, more: Option<std::rc::Rc<dyn Fn()>>) -> gtk::Box {
    let p = vbox(0.0);
    p.set_margin_bottom(px(2.0));
    let head = hbox(0.0);
    head.set_margin_top(px(6.0));
    head.set_margin_bottom(px(10.0));
    head.pack_start(title, true, true, 0);
    if let Some(m) = more {
        let link = gtk::Button::with_label("Xem thêm ›");
        link.style_context().add_class("link");
        link.set_can_focus(false); // chỉ để bấm chuột — phím đã có ô «Xem thêm» cuối dải
        link.set_valign(gtk::Align::End);
        link.connect_clicked(move |_| m());
        head.pack_end(&link, false, false, 0);
    }
    p.pack_start(&head, false, false, 0);
    let row = hbox(18.0);
    for c in cards {
        c.set_valign(gtk::Align::Start);
        row.pack_start(&c, false, false, 0);
    }
    row.set_margin_bottom(px(18.0));
    let sw = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    sw.set_policy(gtk::PolicyType::External, gtk::PolicyType::Never);
    sw.set_widget_name("strip");
    sw.set_can_focus(false);
    sw.add(&row);
    // Lăn chuột dọc: cuộn trang (dải chỉ cuộn bằng Shift + lăn / vuốt ngang)
    sw.connect_scroll_event(|sw, ev| {
        let shift = ev.state().contains(gdk::ModifierType::SHIFT_MASK);
        let (dx, dy) = ev.delta();
        let dir = ev.direction();
        let horizontal = shift || dx.abs() > dy.abs() || matches!(dir, gdk::ScrollDirection::Left | gdk::ScrollDirection::Right);
        if horizontal {
            return glib::Propagation::Proceed;
        }
        if let Some(page) = sw.parent().and_then(|p| find_ancestor_scroller(&p)) {
            let adj = page.vadjustment();
            let step = match dir {
                gdk::ScrollDirection::Up => -120.0,
                gdk::ScrollDirection::Down => 120.0,
                _ => dy * 60.0,
            };
            adj.set_value((adj.value() + step).clamp(adj.lower(), (adj.upper() - adj.page_size()).max(adj.lower())));
        }
        glib::Propagation::Stop
    });
    p.pack_start(&sw, false, false, 0);
    p
}

/// ScrolledWindow gần nhất bọc ngoài (trang).
pub fn find_ancestor_scroller(w: &gtk::Widget) -> Option<gtk::ScrolledWindow> {
    let mut cur = Some(w.clone());
    while let Some(x) = cur {
        if let Ok(s) = x.clone().downcast::<gtk::ScrolledWindow>() {
            if s.widget_name() != "strip" {
                return Some(s);
            }
        }
        cur = x.parent();
    }
    None
}

/// Ô «Xem thêm» hẹp cuối dải vuốt ngang (cao bằng ảnh bìa); count: số phim (None = không ghi).
pub fn more_tile(on_click: impl Fn() + 'static, count: Option<&str>) -> gtk::Button {
    let b = gtk::Button::new();
    b.style_context().add_class("more-tile");
    let l = gtk::Label::new(None);
    let extra = count.map(|c| format!("\n<span size='smaller' weight='normal'>{}</span>", glib::markup_escape_text(c))).unwrap_or_default();
    l.set_markup(&format!("<span size='xx-large'>›</span>\n<b>Xem thêm</b>{extra}"));
    l.set_justify(gtk::Justification::Center);
    b.add(&l);
    b.set_size_request(px(112.0), px(248.0));
    b.set_valign(gtk::Align::Start);
    b.connect_clicked(move |_| on_click());
    b
}

/// Trang cuộn dọc: ScrolledWindow + thân (Box dọc, lề như bản Windows).
pub fn scroller() -> (gtk::ScrolledWindow, gtk::Box) {
    let sw = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sw.set_can_focus(false);
    let body = vbox(0.0);
    margins(&body, 26.0, 28.0, 30.0, 36.0);
    sw.add(&body);
    (sw, body)
}

/// Bỏ hết con của một hộp.
pub fn clear(b: &impl IsA<gtk::Container>) {
    for c in b.children() {
        b.remove(&c);
    }
}

/// Chữ của nút đang chọn (để chọn lại nút cùng chữ sau khi dựng lại trang).
pub fn focused_label() -> Option<String> {
    crate::app::app().window.focused_widget().and_then(|w| w.downcast::<gtk::Button>().ok()).and_then(|b| b.label()).map(|s| s.to_string())
}

/// Chọn lại nút có chữ `text` trong root (sau khi dựng lại).
pub fn refocus_label(root: &gtk::Widget, text: Option<String>) {
    let Some(text) = text else { return };
    let root = root.clone();
    after(30, move || {
        if let Some(w) = crate::keynav::targets(&root)
            .into_iter()
            .find(|w| w.clone().downcast::<gtk::Button>().ok().and_then(|b| b.label()).map(|l| l == text.as_str()).unwrap_or(false))
        {
            w.grab_focus();
        }
    });
}

/// Mở địa chỉ bằng trình duyệt của máy.
pub fn open_browser(url: &str) {
    if let Err(e) = gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>) {
        cbtv::log::d("ui", &format!("Mở trình duyệt lỗi: {e}"));
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Chép chữ vào clipboard.
pub fn copy_text(text: &str) {
    let cb = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    cb.set_text(text);
    cb.store();
}

/// Đường dẫn tới tệp giao diện dùng chung (trang phát…): cạnh tệp chạy (bản dựng thử) hoặc /usr/share/conbangtv.
pub fn asset(name: &str) -> Option<String> {
    crate::assets::get(name)
}
