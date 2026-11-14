//! Lõi Côn Bằng TV cho Linux — cùng tên hàm với bản Windows (windows/ConBangTV/Core) / Android / iPad, để sửa song song.
//! Không dính giao diện: app GTK (linux/app) gọi vào đây; việc mạng chạy trên `rt()` (tokio).

pub mod util;
pub mod log;
pub mod json;
pub mod html;
pub mod models;
pub mod prefs;
pub mod secret;
pub mod http;
pub mod net;
pub mod doh;
pub mod unblock;
pub mod shadowsocks;
pub mod site;
pub mod next_site;
pub mod api_site;
pub mod kk_phim;
pub mod nguon_c;
pub mod stremio;
pub mod ad_block;
pub mod feed;
pub mod home_feed;
pub mod related;
pub mod imdb_top;
pub mod sync_data;
pub mod sync_service;
pub mod watch_limit;
pub mod update_check;
pub mod invite;
pub mod disclaimer;
pub mod media_proxy;
pub mod web_fetch;

pub use models::*;
pub use util::{Error, Result};

use std::sync::OnceLock;

static APP_VERSION: OnceLock<String> = OnceLock::new();

/// Số bản của app đang dùng lõi (bản Windows đặt số bản của app WPF lúc mở — lời chào đồng bộ, User-Agent…). Không đặt thì là
/// số bản Linux (workspace).
pub fn set_app_version(v: &str) {
    let _ = APP_VERSION.set(v.to_string());
}

pub fn version() -> &'static str {
    APP_VERSION.get().map(|s| s.as_str()).unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Bộ chạy việc nền (mạng, đọc trang) dùng chung cả app.
pub fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("cbtv-net")
            .enable_all()
            .build()
            .expect("tokio")
    })
}
