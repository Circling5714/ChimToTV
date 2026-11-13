//! Việc lúc mở app (sau khi cửa sổ hiện): miễn trừ trách nhiệm (Đồng ý mới vào), kiểm tra bản mới, «Ai đang xem?»; đồng bộ
//! trong nhà (máy chủ + tin báo về giao diện). Bản Windows: MainWindow.Start / ShowDisclaimer / CheckUpdate.

use crate::app::app;
use crate::css::px;
use crate::ui;
use cbtv::{disclaimer, prefs, sync_service, update_check};
use gtk::prelude::*;
use std::cell::Cell;

/// Gọi một lần sau khi cửa sổ chính hiện.
pub fn after_show() {
    // Đồng bộ trong nhà: tin báo từ luồng nền → luồng giao diện
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<sync_service::SyncEvent>();
    sync_service::add_listener(move |e| {
        let _ = tx.unbounded_send(e.clone());
    });
    ui::local(async move {
        use futures::StreamExt;
        while let Some(e) = rx.next().await {
            match e {
                sync_service::SyncEvent::DataChanged => sync_applied(),
                sync_service::SyncEvent::Message(m) => app().toast(&m),
                sync_service::SyncEvent::StateChanged => crate::pages::sync_page::state_changed(),
            }
        }
    });
    sync_service::app_started();
    update_check::cleanup();
    ui::local(async {
        // Lần đầu mở app (hoặc vừa cập nhật lên bản có hộp này): miễn trừ trách nhiệm — «Đồng ý» mới vào, «Thoát» là đóng app.
        // Hỏi cập nhật / «Ai đang xem?» chờ sau đó.
        if disclaimer::needed() && !show_disclaimer(true).await {
            app().window.close();
            return;
        }
        let cur = prefs::current_profile();
        if cur.locked() || (prefs::ask_profile() && prefs::profiles().len() > 1) {
            crate::profiles::pick_profile_async(!cur.locked()).await;
        }
        check_update(false).await;
    });
}

/// Đồng bộ trong nhà vừa đổi dữ liệu máy này: vẽ lại menu và trang đang mở (không đụng trang đang có ô nhập).
fn sync_applied() {
    let a = app();
    a.rebuild_menu();
    if a.playing().is_none() {
        if let Some(p) = a.current() {
            if !p.has_input() {
                let focus = a.window.focused_widget();
                p.clone().returned();
                if let Some(f) = focus.filter(|f| f.is_visible() && f.is_mapped()) {
                    f.grab_focus();
                }
            }
        }
    }
}

/// Đồng bộ trong nhà sau `secs` giây (gộp các lần gọi liền nhau).
pub fn sync_soon(secs: u64) {
    sync_service::sync_soon(if secs == 0 { sync_service::SYNC_SOON_SECONDS } else { secs as f64 });
}

/// Hộp miễn trừ trách nhiệm. gate: «Đồng ý» (ghi lại) / «Thoát»; không gate (Cài đặt): chỉ đọc, «Đóng». true khi bấm nút đầu.
pub async fn show_disclaimer(gate: bool) -> bool {
    let a = app();
    let body = ui::vbox(0.0);
    let intro = ui::dim(&disclaimer::intro(gate));
    intro.set_margin_bottom(px(10.0));
    body.pack_start(&intro, false, false, 0);
    let text = ui::vbox(0.0);
    text.set_margin_end(px(14.0));
    for (title, para) in disclaimer::SECTIONS {
        let t = ui::label(title, &["h2"], true);
        t.set_margin_top(px(10.0));
        t.set_margin_bottom(px(3.0));
        t.set_max_width_chars(90);
        text.pack_start(&t, false, false, 0);
        let p = ui::text(para);
        p.set_max_width_chars(90);
        text.pack_start(&p, false, false, 0);
    }
    let consent = ui::dim(&disclaimer::consent_line());
    consent.set_margin_top(px(14.0));
    text.pack_start(&consent, false, false, 0);
    // Cuộn bằng con lăn, hoặc PageUp / PageDown khi vùng chữ được chọn
    let sw = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    let (_, h) = a.window.size();
    sw.set_size_request(-1, ((h as f64) * 0.55).max(240.0) as i32);
    sw.add(&text);
    body.pack_start(&sw, true, true, 0);
    a.dialog_width(px(860.0));
    let buttons: &[&str] = if gate { &["Đồng ý", "Thoát"] } else { &["Đóng"] };
    let r = a.ask_async(if gate { "Trước khi bắt đầu" } else { disclaimer::TITLE }, "", buttons, Some(body.upcast()), 0).await;
    a.dialog_width(px(380.0));
    if gate && r == 0 {
        disclaimer::accept();
    }
    r == 0
}

thread_local! {
    static UPDATING: Cell<bool> = const { Cell::new(false) };
}

/// Kiểm tra bản mới (tự động: 2 giờ một lần, bỏ bản người xem đã hoãn; manual: bấm trong Cài đặt).
pub async fn check_update(manual: bool) {
    if UPDATING.with(|u| u.get()) {
        return;
    }
    let a = app();
    if manual {
        a.toast("Đang kiểm tra bản mới…");
    }
    let rel = match ui::run(async move { update_check::check(manual).await }).await {
        Some(Ok(Some(r))) => r,
        Some(Ok(None)) => {
            if manual {
                a.toast(&update_check::up_to_date_text());
            }
            return;
        }
        Some(Err(e)) => {
            cbtv::log::d("update", &format!("Lỗi: {e}"));
            if manual {
                a.toast_ms(&format!("Không kiểm tra được bản mới: {}", cbtv::util::friendly(&e)), 8000);
            }
            return;
        }
        None => return,
    };
    // Đang xem phim thì để lúc khác (lần kiểm sau)
    if !manual && a.playing().is_some() {
        return;
    }
    let choice = a.ask_async(&update_check::dialog_title(&rel), &update_check::dialog_text(&rel), &["Cập nhật ngay", "Để sau"], None, 0).await;
    if choice != 0 {
        update_check::snooze(&rel);
        return;
    }
    if !update_check::can_self_update() {
        a.toast("Máy này không tự cài được (thiếu pkexec / apt) — mở trang tải bản mới.");
        ui::open_browser(update_check::RELEASES_PAGE);
        return;
    }
    UPDATING.with(|u| u.set(true));
    a.toast_ms("Đang tải bản cập nhật… 0%", 600_000);
    let (ptx, mut prx) = futures::channel::mpsc::unbounded::<i32>();
    ui::local(async move {
        use futures::StreamExt;
        while let Some(p) = prx.next().await {
            app().toast_ms(&format!("Đang tải bản cập nhật… {p}%"), 600_000);
        }
    });
    let rel2 = rel.clone();
    let last = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(-1));
    let file = ui::run(async move {
        update_check::download(&rel2, move |p| {
            let pct = (p * 100.0) as i32;
            if last.swap(pct, std::sync::atomic::Ordering::Relaxed) != pct {
                let _ = ptx.unbounded_send(pct);
            }
        })
        .await
    })
    .await;
    let file = match file {
        Some(Ok(f)) => f,
        Some(Err(e)) => {
            UPDATING.with(|u| u.set(false));
            a.toast_ms(&format!("Không tải được bản cập nhật: {}", cbtv::util::friendly(&e)), 8000);
            return;
        }
        None => {
            UPDATING.with(|u| u.set(false));
            return;
        }
    };
    a.toast_ms("Đã tải xong — nhập mật khẩu máy để cài bản mới…", 600_000);
    cbtv::log::d("update", &format!("Cài bản {} từ {}", rel.version, file.display()));
    let f2 = file.clone();
    match ui::run(async move { update_check::install(&f2).await }).await {
        Some(Ok(update_check::Installed::Done)) => {
            a.toast("Đã cài xong — mở lại app với bản mới…");
            prefs::flush();
            if let Err(e) = update_check::relaunch() {
                a.toast_ms(&format!("Đã cài bản mới — mở lại app để dùng ({e})."), 8000);
                UPDATING.with(|u| u.set(false));
                return;
            }
            a.window.close();
        }
        Some(Ok(update_check::Installed::Cancelled)) => {
            UPDATING.with(|u| u.set(false));
            a.toast("Chưa cài bản mới (đã huỷ hỏi mật khẩu).");
        }
        Some(Err(e)) => {
            UPDATING.with(|u| u.set(false));
            a.toast_ms(&format!("Không cài được bản mới: {}", cbtv::util::friendly(&e)), 8000);
        }
        None => UPDATING.with(|u| u.set(false)),
    }
}
