//! Trang chủ tổng hợp (mọi nguồn, xếp theo lịch sử xem) — TẠM: chờ lõi feed / home_feed, dùng trang chủ theo trang.

use super::Page;
use std::rc::Rc;

pub fn new() -> Rc<dyn Page> {
    super::home::new_site()
}
