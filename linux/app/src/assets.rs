//! Tệp dùng chung với bản Android / iPad / Windows (một nguồn duy nhất để sửa): trang phát, agent điều khiển video, hls.js.
//! Gói thẳng vào tệp chạy lúc dựng (như EmbeddedResource của bản Windows).

pub const PLAYER_HTML: &str = include_str!("../../../app/src/main/assets/player.html");
pub const HLS_HTML: &str = include_str!("../../../app/src/main/assets/hls.html");
pub const PLAYER_AGENT_JS: &str = include_str!("../../../app/src/main/assets/player-agent.js");
pub const PAGE_FOCUS_JS: &str = include_str!("../../../app/src/main/assets/page-focus.js");
pub const HLS_JS: &str = include_str!("../../../app/src/main/assets/hls.light.min.js");
/// Logo (鵬) — biểu tượng cửa sổ và góc menu.
pub const LOGO_PNG: &[u8] = include_bytes!("../../../design/logo-512.png");

#[allow(dead_code)]
pub fn get(name: &str) -> Option<String> {
    Some(
        match name {
            "player.html" => PLAYER_HTML,
            "hls.html" => HLS_HTML,
            "player-agent.js" => PLAYER_AGENT_JS,
            "page-focus.js" => PAGE_FOCUS_JS,
            "hls.light.min.js" => HLS_JS,
            _ => return None,
        }
        .to_string(),
    )
}

use gdk_pixbuf::prelude::*;

/// Logo cỡ size×size.
pub fn logo(size: i32) -> Option<gdk_pixbuf::Pixbuf> {
    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.write(LOGO_PNG).ok()?;
    loader.close().ok()?;
    loader.pixbuf()?.scale_simple(size, size, gdk_pixbuf::InterpType::Bilinear)
}
