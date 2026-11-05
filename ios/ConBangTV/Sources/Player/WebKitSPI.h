// API ẩn (SPI) của WebKit để WKWebView đi qua proxy trên iPadOS 15–16 — iPadOS 17 mới có API công khai
// (WKWebsiteDataStore.proxyConfigurations). App cài bằng TrollStore, không lên App Store, nên dùng được; iPadOS nào
// không có các SPI này thì hàm trả nil và app báo "trình phát không đi qua proxy được" (như WebView quá cũ trên TV).
// Tệp này cũng là bridging header của target ConBangTV (project.yml: SWIFT_OBJC_BRIDGING_HEADER).
#import <WebKit/WebKit.h>

NS_ASSUME_NONNULL_BEGIN

/// Kho dữ liệu web riêng, không lưu xuống máy; mọi kết nối của WebView dùng kho này đi theo từ điển proxy CFNetwork
/// (vd SOCKSProxy / SOCKSPort). nil nếu iPadOS không có SPI.
WKWebsiteDataStore *_Nullable CBTProxiedDataStore(NSDictionary *proxy);

/// Các SPI cần dùng có mặt không (cho cổng gỡ lỗi).
NSDictionary<NSString *, NSNumber *> *CBTWebKitSPIInfo(void);

NS_ASSUME_NONNULL_END
