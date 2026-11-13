//! Chặn tên miền quảng cáo / theo dõi (bản Android: AdBlock.kt, Windows: AdBlock.cs — cùng danh sách). Không chặn thứ trình
//! phát cần: JW Player tải từ ssl.p.jwpcdn.com và hỏi giấy phép ở entitlements.jwplayer.com, video nằm trên CDN.
//! Trên Linux: trình phát (WebKitGTK) hỏi `is_ad` cho từng yêu cầu; `filters()` là mẫu địa chỉ như bộ lọc của WebView2.

pub const HOSTS: &[&str] = &[
    "doubleclick.net", "googlesyndication.com", "googletagmanager.com", "google-analytics.com",
    "adservice.google.com", "popads.net", "popcash.net", "propellerads.com", "adsterra.com",
    "exoclick.com", "juicyads.com", "hilltopads.net", "onclickads.net", "clickadu.com",
    "a-ads.com", "histats.com", "adnxs.com", "taboola.com", "outbrain.com",
    // Đo trên 5 trang (11/2026): thống kê Cloudflare, JW Player, Ahrefs; quảng cáo Adcash, cửa sổ quảng cáo hoathinh3d
    "cloudflareinsights.com", "jwpltx.com", "analytics.ahrefs.com", "acscdn.com", "excavatenearbywand.com",
    // Gặp khi thử bản iPad (11/2026): iframe quảng cáo trong trang tập hoathinh3d.pw
    "atroblxac.in",
    // Thống kê / theo dõi phổ biến khác
    "connect.facebook.net", "clarity.ms", "mc.yandex.ru", "statcounter.com", "hotjar.com",
];

pub fn is_ad(host: Option<&str>) -> bool {
    let Some(host) = host else { return false };
    let h = host.to_lowercase();
    HOSTS.iter().any(|x| h == *x || h.ends_with(&format!(".{x}")))
}

/// Mẫu địa chỉ (tên miền và mọi tên miền con), như AddWebResourceRequestedFilter của bản Windows.
pub fn filters() -> Vec<String> {
    HOSTS.iter().flat_map(|h| [format!("*://{h}/*"), format!("*://*.{h}/*")]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SiteTests.AdBlockHosts
    #[test]
    fn ad_block_hosts() {
        assert!(is_ad(Some("static.cloudflareinsights.com")));
        assert!(!is_ad(Some("ssl.p.jwpcdn.com")));
        assert!(!is_ad(None));
        assert!(filters().contains(&"*://*.doubleclick.net/*".to_string()));
    }
}
