import Foundation

/// Chặn tên miền quảng cáo / theo dõi (bản Android: AdBlock.kt — cùng danh sách). Không chặn thứ trình phát cần:
/// JW Player tải từ ssl.p.jwpcdn.com và hỏi giấy phép ở entitlements.jwplayer.com, video nằm trên CDN.
/// Trên iOS dùng luật chặn sẵn có của WebKit (WKContentRuleList) — chặn ngay trong trình duyệt, không tốn mạng.
enum AdBlock {
    static let hosts = [
        "doubleclick.net", "googlesyndication.com", "googletagmanager.com", "google-analytics.com",
        "adservice.google.com", "popads.net", "popcash.net", "propellerads.com", "adsterra.com",
        "exoclick.com", "juicyads.com", "hilltopads.net", "onclickads.net", "clickadu.com",
        "a-ads.com", "histats.com", "adnxs.com", "taboola.com", "outbrain.com",
        // Đo trên 5 trang (11/2026): thống kê Cloudflare, JW Player, Ahrefs; quảng cáo Adcash, cửa sổ quảng cáo hoathinh3d
        "cloudflareinsights.com", "jwpltx.com", "analytics.ahrefs.com", "acscdn.com", "excavatenearbywand.com",
        // Thống kê / theo dõi phổ biến khác
        "connect.facebook.net", "clarity.ms", "mc.yandex.ru", "statcounter.com", "hotjar.com",
    ]

    static func isAd(_ host: String?) -> Bool {
        guard let h = host?.lowercased() else { return false }
        return hosts.contains { h == $0 || h.hasSuffix("." + $0) }
    }

    /// Luật chặn cho WKContentRuleListStore (biểu thức chính quy kiểu WebKit: không có |, chỉ . * + ? [] () ^ $).
    static var ruleListJSON: String {
        let rules: [[String: Any]] = hosts.map { host in
            let escaped = host.replacingOccurrences(of: ".", with: "\\.")
            return ["trigger": ["url-filter": "^https?://([a-z0-9.-]*\\.)?\(escaped)[:/]"], "action": ["type": "block"]]
        }
        let data = (try? JSONSerialization.data(withJSONObject: rules)) ?? Data("[]".utf8)
        return String(decoding: data, as: UTF8.self)
    }
}
