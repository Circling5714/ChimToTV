import Network
import UIKit
import WebKit

/// Các tệp trình phát dùng chung với bản Android (app/src/main/assets, chép vào bundle khi dựng).
enum Assets {
    private static func text(_ name: String, _ ext: String) -> String {
        guard let u = Bundle.main.url(forResource: name, withExtension: ext), let s = try? String(contentsOf: u, encoding: .utf8) else {
            Log.d("Assets", "thiếu \(name).\(ext)")
            return ""
        }
        return s
    }

    static let agent = text("player-agent", "js")
    static let wrapper = text("player", "html")
    static let hls = text("hls", "html")
    static let hlsJs = text("hls.light.min", "js")
    static let pageFocus = text("page-focus", "js")

    /// Trang bọc: iframe trình phát của trang, origin = địa chỉ tập (xem player.html).
    static func wrapperPage(src: String) -> String {
        wrapper.replacingOccurrences(of: "{{SRC}}", with: htmlAttr(src))
    }

    /// Trang phát HLS của app (hls.html). hls.js và agent được WKWebView chèn sẵn (WKUserScript), nên bỏ thẻ nạp
    /// /__chimto/hls.js (bản Android trả tệp cho đường dẫn đó) và chỗ chèn agent.
    static func hlsPage(src: String, start: Int, sub: String?) -> String {
        hls.replacingOccurrences(of: "<script src=\"/__chimto/hls.js\"></script>", with: "")
            .replacingOccurrences(of: "{{AGENT}}", with: "")
            .replacingOccurrences(of: "{{SRC}}", with: jsString(src))
            .replacingOccurrences(of: "{{START}}", with: String(start))
            .replacingOccurrences(of: "{{SUB}}", with: sub.map { jsString($0) } ?? "null")
    }

    static func htmlAttr(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "<", with: "&lt;").replacingOccurrences(of: ">", with: "&gt;")
    }

    /// Chuỗi JS an toàn (JSON) — kể cả ký tự "</" để không đóng thẻ script.
    static func jsString(_ s: String) -> String {
        let data = (try? JSONSerialization.data(withJSONObject: [s])) ?? Data("[\"\"]".utf8)
        let arr = String(decoding: data, as: UTF8.self)
        return String(arr.dropFirst().dropLast()).replacingOccurrences(of: "</", with: "<\\/")
    }
}

/// Cấu hình WKWebView chung: User-Agent của app, chặn quảng cáo, cho phát video ngay trong trang.
enum WebSetup {
    private(set) static var adRules: WKContentRuleList?
    private static var pending: [WeakController] = []

    private final class WeakController {
        weak var value: WKUserContentController?
        init(_ v: WKUserContentController) { value = v }
    }

    /// Dịch luật chặn quảng cáo một lần lúc mở app; WebView tạo trước khi xong thì gắn sau.
    static func prepare() {
        WKContentRuleListStore.default().compileContentRuleList(forIdentifier: "adblock-1", encodedContentRuleList: AdBlock.ruleListJSON) { list, error in
            DispatchQueue.main.async {
                if let error = error { Log.d("AdBlock", "lỗi dịch luật chặn: \(error)") }
                adRules = list
                if let list = list { for c in pending { c.value?.add(list) } }
                pending.removeAll()
            }
        }
    }

    static func attachAdRules(_ ucc: WKUserContentController) {
        if let r = adRules { ucc.add(r) } else { pending.append(WeakController(ucc)) }
    }

    static func configuration() -> WKWebViewConfiguration {
        let c = WKWebViewConfiguration()
        c.allowsInlineMediaPlayback = true
        c.mediaTypesRequiringUserActionForPlayback = []
        c.allowsPictureInPictureMediaPlayback = true
        c.allowsAirPlayForMediaPlayback = true
        c.websiteDataStore = dataStore()
        c.preferences.javaScriptCanOpenWindowsAutomatically = false
        c.applicationNameForUserAgent = "Version/\(ProcessInfo.processInfo.operatingSystemVersion.majorVersion) Safari/605.1.15"
        attachAdRules(c.userContentController)
        return c
    }

    /// Kho dữ liệu web theo proxy (AppProxy): tắt proxy → kho mặc định (lưu cookie như cũ). Bật: iPadOS 17+ đặt proxy
    /// cho kho mặc định (API công khai); iPadOS 15–16 dùng kho riêng qua SPI của WebKit (WebKitSPI.m — không lưu xuống
    /// máy: tắt app là quên cookie). Không đặt được → kho mặc định, trình phát đi thẳng (proxySupported = false).
    private static var proxied: (generation: Int, store: WKWebsiteDataStore?)?
    private static var defaultProxied = false

    /// false: iPadOS này không cho WebView đi qua proxy (chỉ phần đọc trang của app đi qua proxy).
    private(set) static var proxySupported = true

    /// Cách WebView đang đi qua proxy (cho cổng gỡ lỗi): "tắt" | "ios17" | "spi" | "không hỗ trợ".
    private(set) static var proxyMode = "tắt"

    static func dataStore() -> WKWebsiteDataStore {
        let gen = AppProxy.generation
        guard let port = AppProxy.localPort, let dict = AppProxy.cfDictionary else {
            if #available(iOS 17.0, *), defaultProxied {
                WKWebsiteDataStore.default().proxyConfigurations = []
                defaultProxied = false
            }
            proxied = nil
            proxyMode = "tắt"
            return .default()
        }
        if #available(iOS 17.0, *) {
            if let p = NWEndpoint.Port(rawValue: UInt16(truncatingIfNeeded: port)) {
                WKWebsiteDataStore.default().proxyConfigurations = [ProxyConfiguration(socksv5Proxy: .hostPort(host: "127.0.0.1", port: p))]
                defaultProxied = true
                proxySupported = true
                proxyMode = "ios17"
                return .default()
            }
        }
        if let p = proxied, p.generation == gen { return p.store ?? .default() }
        let store = CBTProxiedDataStore(dict)
        proxied = (gen, store)
        proxySupported = store != nil
        proxyMode = store != nil ? "spi" : "không hỗ trợ"
        if store == nil { Log.d("Proxy", "iPadOS không có SPI đặt proxy cho WebView — trình phát đi thẳng") }
        return store ?? .default()
    }

    /// iPadOS này cho WebView đi qua proxy không (iPadOS 17+, hoặc có SPI của WebKit).
    static var canProxy: Bool {
        if #available(iOS 17.0, *) { return true }
        return CBTWebKitSPIInfo().values.allSatisfy { $0.boolValue }
    }

    /// Kho riêng đang dùng cho proxy (để xoá dữ liệu duyệt web cả ở đó).
    static var proxiedStore: WKWebsiteDataStore? { proxied?.store }

    static func make(_ config: WKWebViewConfiguration = configuration()) -> WKWebView {
        let w = WKWebView(frame: .zero, configuration: config)
        w.customUserAgent = Http.userAgent
        w.isOpaque = false
        w.backgroundColor = .black
        w.scrollView.backgroundColor = .black
        w.scrollView.contentInsetAdjustmentBehavior = .never
        w.allowsBackForwardNavigationGestures = false
        Prefs.webDirty = true
        return w
    }

    /// Trình phát nhúng (JW Player của streamfree / NguonC) tự giữ khung 16:9 theo bề ngang: trên TV 16:9 vừa khít, trên
    /// iPad (≈ 4:3) lộ dải nền xám của trang nhúng ở dưới (đo: khung 1194×672 trong iframe 1194×834). Phóng trình phát
    /// ra đủ khung, nền trang đen → phim nằm giữa, viền đen trên dưới như thường.
    static let fillCss = """
    (function(){ if (window.__chimtoFill) return; window.__chimtoFill = 1;
      var s = document.createElement('style');
      s.textContent = 'html,body{background:#000!important}' +
        '.jwplayer{position:fixed!important;left:0!important;top:0!important;width:100vw!important;height:100vh!important;' +
        'max-width:none!important;max-height:none!important;margin:0!important}';
      (document.head || document.documentElement).appendChild(s);
    })();
    """

    /// Bản thử: chép console.log / lỗi JS của trang phát vào nhật ký (chỉ giá trị đơn giản — không đụng getter của
    /// đối tượng, khỏi làm trình phát tưởng đang mở DevTools).
    static let consoleHook = """
    (function(){ if (window.__chimtoLog) return; window.__chimtoLog = 1;
      var h = window.webkit && webkit.messageHandlers && webkit.messageHandlers.chimtoLog; if (!h) return;
      function fmt(a){ var o=[]; for (var i=0;i<a.length;i++){ var x=a[i], t=typeof x;
        o.push(t==='string'||t==='number'||t==='boolean' ? String(x) : '['+t+']'); } return o.join(' '); }
      ['log','warn','error','info'].forEach(function(k){ var f=console[k]; console[k]=function(){
        try{ h.postMessage(k+': '+fmt(arguments)); }catch(e){} return f.apply(console, arguments); }; });
      window.addEventListener('error', function(e){ try{ h.postMessage('onerror: '+e.message+' @'+(e.filename||'')+':'+(e.lineno||0)); }catch(x){} });
    })();
    """
}

/// Nhận tin từ trang (console của bản thử) mà không giữ chặt đối tượng nhận (WKUserContentController giữ mạnh handler).
final class ScriptRelay: NSObject, WKScriptMessageHandler {
    private let onMessage: (WKScriptMessage) -> Void
    init(_ onMessage: @escaping (WKScriptMessage) -> Void) { self.onMessage = onMessage }
    func userContentController(_ c: WKUserContentController, didReceive message: WKScriptMessage) { onMessage(message) }
}
