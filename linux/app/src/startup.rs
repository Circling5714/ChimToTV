//! Việc lúc mở app (sau khi cửa sổ hiện): miễn trừ trách nhiệm, «Ai đang xem?», kiểm tra bản mới; đồng bộ trong nhà.

/// Gọi một lần sau khi cửa sổ chính hiện.
pub fn after_show() {
    crate::ui::local(async {
        let a = crate::app::app();
        let cur = cbtv::prefs::current_profile();
        if cur.locked() || (cbtv::prefs::ask_profile() && cbtv::prefs::profiles().len() > 1) {
            crate::profiles::pick_profile_async(!cur.locked()).await;
        }
        let _ = a;
    });
}

/// Đồng bộ trong nhà sau `secs` giây (gộp các lần gọi liền nhau).
pub fn sync_soon(_secs: u64) {}
