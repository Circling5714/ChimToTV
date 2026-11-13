//! Xem tiếp / lịch sử xem: Enter xem tiếp ngay; Delete (chuột phải) xoá một phim. Bản Windows: Pages.cs HistoryPage.

use super::{Page, PageBase};
use crate::app::app;
use crate::card::{card, CardOpts};
use crate::ui;
use cbtv::{prefs, Film};
use gtk::prelude::*;
use std::rc::Rc;

pub struct HistoryPage {
    base: PageBase,
}

pub fn history() -> Rc<dyn Page> {
    Rc::new(HistoryPage { base: PageBase::new("history") })
}

impl HistoryPage {
    fn build(self: &Rc<Self>) {
        let b = &self.base;
        b.clear();
        b.add(&ui::header("Xem tiếp / Lịch sử"));
        let list = prefs::history();
        if list.is_empty() {
            b.add(&ui::dim("Chưa xem phim nào."));
            return;
        }
        let me = self.clone();
        b.add(&ui::row(&[ui::pill("🗑  Xoá cả lịch sử", move || {
            let me = me.clone();
            ui::local(async move {
                if !app().confirm("Xoá cả lịch sử xem?", "Xoá danh sách Xem tiếp và chỗ đang xem của mọi tập.", "Xoá hết", "Huỷ").await {
                    return;
                }
                prefs::clear_history();
                crate::startup::sync_soon(0);
                me.build();
            });
        })
        .upcast()]));
        let note = ui::dim("Enter / bấm: xem tiếp ngay.  Delete hoặc chuột phải: xoá phim khỏi lịch sử.");
        note.set_margin_bottom(crate::css::px(16.0));
        b.add(&note);
        let mut cards = Vec::new();
        for h in list {
            let (fu, eu) = (h.film_url.clone(), h.ep_url.clone());
            let me = self.clone();
            let h2 = h.clone();
            let remove: Rc<dyn Fn()> = Rc::new(move || {
                let me = me.clone();
                let h2 = h2.clone();
                ui::local(async move {
                    if !app().confirm(&format!("Xoá «{}» khỏi lịch sử?", h2.title), "Xoá cả chỗ đang xem của các tập phim này.", "Xoá", "Huỷ").await {
                        return;
                    }
                    prefs::remove_history(&h2.film_url);
                    crate::startup::sync_soon(0);
                    me.build();
                    super::focus_first(me.as_ref());
                });
            });
            let film = Film::new(h.film_url.clone(), h.title.clone(), h.poster.clone(), cbtv::site::host_of(&h.film_url));
            cards.push(card(
                film,
                Rc::new(move || app().play(&fu, &eu, true)),
                CardOpts { sub: Some(format!("{} · {}", h.ep_title, cbtv::util::local_dm_hm(h.time))), remove: Some(remove), ..Default::default() },
            ));
        }
        b.add(&ui::grid(cards));
    }
}

impl Page for HistoryPage {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.build();
    }

    fn returned(self: Rc<Self>) {
        self.build();
    }
}
