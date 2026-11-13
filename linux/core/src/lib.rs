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

pub use models::*;
pub use util::{Error, Result};

use std::sync::OnceLock;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
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
