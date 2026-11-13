//! Đồng bộ trong nhà với TV / iPad / iPhone / Windows: mã ghép của máy này, máy trong mạng, máy đã ghép, ghép bằng địa chỉ. Mở
//! trang thì cho máy khác ghép (mã 4 số); rời trang thì thôi. Bản Windows: UI/SyncPage.cs.

use super::{Page, PageBase};
use crate::app::app;
use crate::css::px;
use crate::ui;
use cbtv::{prefs, sync_service as ss};
use gtk::prelude::*;
use std::rc::Rc;

pub struct SyncPage {
    base: PageBase,
    ip: gtk::Entry,
    code: gtk::Entry,
}

thread_local! {
    static OPEN: std::cell::RefCell<Option<std::rc::Weak<SyncPage>>> = const { std::cell::RefCell::new(None) };
}

pub fn new() -> Rc<dyn Page> {
    let ip = crate::ui::entry();
    ip.set_width_chars(22);
    ip.set_placeholder_text(Some("Địa chỉ IP máy kia"));
    let code = crate::ui::entry();
    code.set_width_chars(8);
    code.set_max_length(4);
    code.set_placeholder_text(Some("Mã 4 số"));
    let p = Rc::new(SyncPage { base: PageBase::new("sync"), ip, code });
    let weak = Rc::downgrade(&p);
    p.code.connect_activate(move |_| {
        if let Some(p) = weak.upgrade() {
            p.pair_manual();
        }
    });
    // Rời trang (trang bị gỡ khỏi cửa sổ) → thôi cho ghép
    p.base.scroller.connect_unmap(|_| {
        ss::close_pairing();
    });
    p
}

/// Tình trạng đồng bộ đổi (luồng giao diện): vẽ lại trang Đồng bộ nếu đang mở.
pub fn state_changed() {
    if let Some(p) = OPEN.with(|o| o.borrow().as_ref().and_then(|w| w.upgrade())) {
        if p.base.scroller.is_mapped() {
            p.rebuild();
        }
    }
}

impl SyncPage {
    fn rebuild(self: &Rc<Self>) {
        let text = ui::focused_label();
        self.build();
        ui::refocus_label(self.base.body.upcast_ref(), text);
    }

    fn build(self: &Rc<Self>) {
        let st = ss::status();
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Đồng bộ trong nhà"));
        let n = ui::dim(
            "Máy này, tivi (Côn Bằng TV 2.2+), iPad / iPhone (1.3+) và máy Windows cùng mạng Wi‑Fi đồng bộ với nhau: Xem tiếp / lịch \
             sử, chỗ đang xem từng tập, phim theo dõi và danh sách trang phim. Xoá ở máy này thì máy kia cũng xoá.",
        );
        n.set_margin_bottom(px(14.0));
        b.add(&n);

        let h = ui::label("Máy này", &["h2"], true);
        ui::margins(&h, 4.0, 0.0, 6.0, 0.0);
        b.add(&h);
        b.add(&ui::dim(&format!("{} · địa chỉ: {}", st.device_name, if st.address.is_empty() { "chưa có mạng".into() } else { st.address.clone() })));
        if let Some(code) = &st.pair_code {
            let row = ui::hbox(8.0);
            ui::margins(&row, 8.0, 0.0, 4.0, 0.0);
            row.pack_start(&ui::text("Mã ghép:"), false, false, 0);
            let c = gtk::Label::new(None);
            c.set_markup(&format!("<span size='xx-large' weight='bold' foreground='{}'>{}</span>", crate::css::FOCUS, code));
            row.pack_start(&c, false, false, 0);
            b.add(&row);
            b.add(&ui::dim(
                "Trên tivi: Cài đặt → Đồng bộ với iPad / iPhone… → chọn máy này (hoặc nhập địa chỉ ở trên) → nhập mã. iPad / iPhone / \
                 Windows: mục Đồng bộ trong nhà. Mã đổi sau mỗi lần ghép.",
            ));
        }
        if !st.listening {
            b.add(&ui::dim("Chưa mở được cổng đồng bộ (cổng đang bị app khác dùng?) — máy khác không gọi sang được, máy này vẫn tự gọi sang máy kia."));
        }

        let h = ui::label("Máy đã ghép", &["h2"], true);
        ui::margins(&h, 12.0, 0.0, 6.0, 0.0);
        b.add(&h);
        if st.peers.is_empty() {
            b.add(&ui::dim("Chưa ghép máy nào."));
        } else {
            for p in &st.peers {
                let row = ui::hbox(14.0);
                row.set_margin_bottom(px(8.0));
                row.pack_start(&ui::label(&p.line(), &[], false), false, false, 0);
                let (me, p2) = (self.clone(), p.clone());
                row.pack_start(
                    &ui::pill("Bỏ ghép", move || {
                        let (me, p2) = (me.clone(), p2.clone());
                        ui::local(async move {
                            if !app().confirm(&format!("Bỏ ghép với {}?", p2.name), "Hai máy thôi đồng bộ (dữ liệu đã có vẫn giữ).", "Bỏ ghép", "Huỷ").await {
                                return;
                            }
                            ss::unpair(&p2.id);
                            me.rebuild();
                        });
                    }),
                    false,
                    false,
                    0,
                );
                b.add(&row);
            }
            b.add(&ui::row(&[ui::pill(if st.busy { "Đang đồng bộ…" } else { "⇄  Đồng bộ ngay" }, ss::sync_now).upcast()]));
        }
        let me = self.clone();
        b.add(&ui::row(&[ui::pill(if prefs::sync_auto() { "Tự đồng bộ: Bật" } else { "Tự đồng bộ: Tắt" }, move || {
            prefs::set_sync_auto(!prefs::sync_auto());
            me.rebuild();
        })
        .upcast()]));
        b.add(&ui::dim("Tự đồng bộ: lúc mở app, thoát phim, theo dõi / bỏ theo dõi, xoá lịch sử."));

        let h = ui::label("Máy trong mạng", &["h2"], true);
        ui::margins(&h, 12.0, 0.0, 6.0, 0.0);
        b.add(&h);
        let found: Vec<_> = st.found.iter().filter(|f| st.peers.iter().all(|p| p.id != f.id)).cloned().collect();
        if found.is_empty() {
            b.add(&ui::dim(if st.browsing {
                "Đang tìm…"
            } else {
                "Không thấy máy nào chưa ghép (máy kia phải đang mở Côn Bằng TV; khác lớp mạng thì nhập địa chỉ bên dưới)."
            }));
        }
        let mut items: Vec<gtk::Widget> = Vec::new();
        for f in found {
            let me = self.clone();
            items.push(
                ui::pill(&format!("{}  Ghép với {}", ss::platform_icon(&f.platform), f.name), move || me.ask_code(f.host.clone(), f.port, f.name.clone()))
                    .upcast(),
            );
        }
        items.push(ui::pill(if st.browsing { "Đang tìm…" } else { "↻  Tìm lại" }, ss::browse_now).upcast());
        b.add(&ui::row(&items));

        let h = ui::label("Ghép bằng địa chỉ máy kia", &["h2"], true);
        ui::margins(&h, 14.0, 0.0, 6.0, 0.0);
        b.add(&h);
        for e in [&self.ip, &self.code] {
            if let Some(p) = e.parent().and_then(|p| p.downcast::<gtk::Container>().ok()) {
                p.remove(e);
            }
        }
        let row = ui::hbox(10.0);
        row.set_margin_bottom(px(10.0));
        row.pack_start(&self.ip, false, false, 0);
        row.pack_start(&self.code, false, false, 0);
        let me = self.clone();
        row.pack_start(&ui::pill("Ghép", move || me.pair_manual()), false, false, 0);
        b.add(&row);
        b.add(&ui::dim("Ô đầu: địa chỉ IP của tivi / iPad (hiện trên màn Đồng bộ của máy đó); ô sau: mã ghép 4 số đang hiện ở máy đó."));
    }

    fn ask_code(self: &Rc<Self>, host: String, port: u16, name: String) {
        let me = self.clone();
        ui::local(async move {
            let e = crate::ui::entry();
            e.set_max_length(4);
            e.set_width_chars(8);
            e.set_halign(gtk::Align::Start);
            e.connect_activate(|_| app().close_dialog(0));
            let rx = app().ask(&format!("Ghép với {name}"), "Nhập mã ghép 4 số đang hiện trên màn Đồng bộ của máy đó.", &["Ghép", "Huỷ"], Some(e.clone().upcast()), 0);
            let e2 = e.clone();
            ui::after(30, move || e2.grab_focus());
            if rx.await.unwrap_or(-1) != 0 {
                return;
            }
            let code = e.text().trim().to_string();
            if let Some(err) = ss::code_error(&code) {
                app().toast(err);
                return;
            }
            app().toast(&ss::pairing_text(&host));
            let msg = ui::run(async move { ss::pair_checked(&host, port, &code).await }).await.unwrap_or_default();
            app().toast(&msg);
            me.rebuild();
        });
    }

    fn pair_manual(self: &Rc<Self>) {
        let (addr, code) = (self.ip.text().to_string(), self.code.text().trim().to_string());
        let me = self.clone();
        ui::local(async move {
            if let Some((host, _)) = ss::parse_address(&addr) {
                app().toast(&ss::pairing_text(&host));
            }
            let msg = ui::run(async move { ss::pair_from_input(&addr, &code).await }).await.unwrap_or_default();
            app().toast(&msg);
            me.rebuild();
        });
    }
}

impl Page for SyncPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        OPEN.with(|o| *o.borrow_mut() = Some(Rc::downgrade(&self)));
        ss::open_pairing();
        ss::browse_now();
        self.build();
    }

    fn returned(self: Rc<Self>) {
        ss::open_pairing();
        self.build();
    }

    fn has_input(&self) -> bool {
        true
    }
}
