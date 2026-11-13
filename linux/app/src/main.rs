//! Côn Bằng TV cho Linux — GTK3 + WebKitGTK. Giao diện như bản TV / Windows: menu bên trái, thẻ phim, đi bằng phím mũi tên /
//! remote, trình phát dùng chung trang phát của bản Android (app/src/main/assets).

mod app;
mod assets;
mod card;
mod css;
mod debug;
mod hooks;
mod images;
mod keynav;
mod limits;
mod menu;
mod pages;
mod player;
mod profiles;
mod startup;
mod ui;

use gtk::prelude::*;

fn main() {
    cbtv::prefs::load(None);
    cbtv::log::start();
    cbtv::log::d("app", &format!("Côn Bằng TV {} (Linux) mở", cbtv::version()));
    cbtv::prefs::apply_net_defaults();
    cbtv::net::start_bridge();
    glib::set_prgname(Some("conbangtv"));
    glib::set_application_name("Côn Bằng TV");

    // Một cửa sổ cho cả máy: mở lần nữa thì đưa cửa sổ đang có lên trước
    let gapp = gtk::Application::new(Some("vn.chimto.tv"), gio::ApplicationFlags::default());
    gapp.connect_activate(|g| {
        if let Some(w) = g.windows().first() {
            w.present();
            return;
        }
        css::install();
        let a = app::App::build(g);
        a.start();
    });
    gapp.run_with_args(&Vec::<String>::new());
    cbtv::prefs::flush();
}
