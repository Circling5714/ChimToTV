//! Trang tạm cho mục chưa làm xong (bản Linux đang dựng dần).

use super::{Page, PageBase};
use crate::ui;
use std::rc::Rc;

pub struct Placeholder {
    base: PageBase,
    title: String,
}

impl Page for Placeholder {
    fn base(&self) -> &PageBase {
        &self.base
    }

    fn start(self: Rc<Self>) {
        self.base.clear();
        self.base.add(&ui::header(&self.title));
        self.base.add(&ui::dim("Mục này đang được làm cho bản Linux."));
    }
}

pub fn new(menu_key: &str, title: &str) -> Rc<dyn Page> {
    Rc::new(Placeholder { base: PageBase::new(menu_key), title: title.to_string() })
}

pub fn sites() -> Rc<dyn Page> {
    new("sites", "Đổi trang phim")
}
