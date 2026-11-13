//! Cài bản TV lên tivi bằng app Downloader (Cài đặt → Giới thiệu): mã thật to, ba bước, nút chép hướng dẫn để gửi người quen.
//! Bản Windows: UI/InvitePage.cs.

use super::{Page, PageBase};
use crate::app::app;
use crate::css::{px, FOCUS};
use crate::ui;
use cbtv::invite;
use gtk::prelude::*;
use std::rc::Rc;

pub struct InvitePage {
    base: PageBase,
}

pub fn new() -> Rc<dyn Page> {
    Rc::new(InvitePage { base: PageBase::new("settings") })
}

impl Page for InvitePage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Cài Côn Bằng TV lên tivi"));
        let n = ui::dim("Tivi / box Android nào cũng cài được bằng app Downloader — không cần USB hay máy tính.");
        n.set_margin_bottom(px(16.0));
        b.add(&n);
        b.add(&ui::label("1.  Trên tivi, mở CH Play và cài app Downloader (của AFTVnews).", &["h2"], true));
        let s2 = ui::label("2.  Mở Downloader, nhập mã này rồi bấm Go:", &["h2"], true);
        s2.set_margin_top(px(12.0));
        b.add(&s2);
        let code = gtk::Label::new(None);
        code.set_markup(&format!(
            "<span font_size='{}pt' weight='bold' foreground='{FOCUS}'>{}</span>",
            (42.0 * crate::css::scale()).round(),
            glib::markup_escape_text(&invite::spaced_code())
        ));
        code.set_halign(gtk::Align::Start);
        ui::margins(&code, 0.0, 0.0, 4.0, 26.0);
        b.add(&code);
        b.add(&ui::label("3.  Tải xong chọn Install. Lần đầu tivi hỏi cho phép Downloader cài ứng dụng thì cho phép.", &["h2"], true));
        let n = ui::dim("Cài xong, các bản sau app tự cập nhật.");
        ui::margins(&n, 12.0, 0.0, 18.0, 0.0);
        b.add(&n);
        b.add(&ui::row(&[
            ui::pill("Chép hướng dẫn để gửi bạn bè", || {
                ui::copy_text(&invite::message());
                app().toast("Đã chép hướng dẫn — dán vào Zalo, tin nhắn… để gửi.");
            })
            .upcast(),
            ui::pill("Chép mã", || {
                ui::copy_text(invite::CODE);
                app().toast(&format!("Đã chép mã {}.", invite::CODE));
            })
            .upcast(),
            ui::pill("Mở trang hướng dẫn", || ui::open_browser(invite::GUIDE_URL)).upcast(),
        ]));
        let l = ui::dim(&invite::links_line());
        l.set_margin_top(px(10.0));
        b.add(&l);
    }
}
