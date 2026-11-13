//! Trình phát — TẠM (đang viết).

use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct Player {
    root: gtk::Box,
    label: gtk::Label,
    open: Cell<bool>,
}

impl Player {
    pub fn new() -> Rc<Player> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.style_context().add_class("player");
        let label = gtk::Label::new(None);
        root.pack_start(&label, true, true, 0);
        Rc::new(Player { root, label, open: Cell::new(false) })
    }

    pub fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn open(&self, film_url: &str, ep_url: &str, _resume: bool) {
        self.open.set(true);
        self.label.set_text(&format!("{film_url}\n{ep_url}\n(Esc để thoát)"));
    }

    pub fn close(&self, notify: bool) {
        if !self.open.replace(false) {
            return;
        }
        if notify {
            crate::app::app().player_closed();
        }
    }

    pub fn handle_key(&self, ev: &gdk::EventKey) -> bool {
        use gdk::keys::constants as k;
        if ev.keyval() == k::Escape || ev.keyval() == k::BackSpace {
            self.close(true);
            return true;
        }
        false
    }

    pub fn notice(&self, _text: &str, _ms: u64) {}
}
