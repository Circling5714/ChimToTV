import UIKit
import WebKit

/// Đọc hộ trang bật lớp chống bot của Cloudflare (Core/WebFetch — hoathinh3d.pw từ 14/11/2026). Mỗi trang bị chặn một WKWebView ẩn
/// (`WebSetup.make()` — cùng kho cookie / proxy / luật chặn quảng cáo / User-Agent với trình phát) gắn vào cửa sổ app ở ngoài màn
/// hình (trang chạy như thật, không hiện ra): mở trang chủ cho trình duyệt tự qua lớp chặn, đứng ở robots.txt rồi mỗi yêu cầu là
/// `fetch()` trong trang (web-fetch.js — dùng chung với bản Windows / Linux) qua `callAsyncJavaScript`. Không giả / không giải hộ:
/// trang đòi bấm xác nhận hay chặn hẳn WebKit thì báo lỗi. 5 phút không dùng thì đóng WebView.
@MainActor
final class WebFetcher {
    static let shared = WebFetcher()

    /// Gắn vào Core (gọi một lần lúc mở app).
    static func install() {
        WebFetch.provider = { r in try await WebFetcher.shared.fetch(r) }
    }

    private static let fetchJS: String = {
        guard let u = Bundle.main.url(forResource: "web-fetch", withExtension: "js"), let s = try? String(contentsOf: u) else {
            Log.d("Web", "thiếu web-fetch.js trong gói app")
            return ""
        }
        return s
    }()

    private static let probeJS = "JSON.stringify([location.host, document.readyState, document.title || '', typeof window._cf_chl_opt, " +
        "!!document.querySelector('#challenge-form,#challenge-stage,#cf-challenge-running,.cf-turnstile'), " +
        "!!document.querySelector('#cf-error-details,.cf-error-details') || /you have been blocked/i.test(document.body ? document.body.innerText.slice(0, 400) : '')])"

    private static let passTimeout: TimeInterval = 30
    private static let callTimeout: Double = 45
    private static let idleClose: UInt64 = 300_000_000_000
    private static let blockedFor: TimeInterval = 1800

    /// Một trang bị chặn = một WebView ẩn.
    @MainActor
    private final class Session: NSObject, WKNavigationDelegate {
        let host: String
        var web: WKWebView?
        /// AppProxy.generation lúc tạo WebView (đổi proxy → tạo lại theo kho dữ liệu mới)
        var gen = 0
        var ready = false
        var opening: Task<Void, Error>?
        var blockedUntil: Date?
        var idle: Task<Void, Never>?
        var status = "mới"
        private var finished: CheckedContinuation<Void, Never>?
        private var waitingFor: String?

        init(host: String) { self.host = host }

        /// Mở `url`, chờ trang có đường dẫn `waitPath` nạp xong (tối đa `seconds` giây).
        func load(_ url: URL, waitPath: String, seconds: Double) async {
            guard let web = web else { return }
            await withCheckedContinuation { (c: CheckedContinuation<Void, Never>) in
                finished?.resume()
                finished = c
                waitingFor = waitPath
                web.load(URLRequest(url: url))
                Task { @MainActor [weak self] in
                    try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
                    self?.resumeFinished()
                }
            }
        }

        func resumeFinished() {
            finished?.resume()
            finished = nil
            waitingFor = nil
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            if let p = waitingFor, webView.url?.path == p { resumeFinished() }
        }

        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
            if let p = waitingFor, webView.url?.path == p { resumeFinished() }
        }
    }

    /// Trả lời một lần (hết giờ hay có kết quả, cái nào trước).
    private final class Once {
        var done = false
    }

    private var sessions: [String: Session] = [:]

    /// Gỡ lỗi: tình trạng các phiên.
    var describe: String {
        sessions.isEmpty ? "chưa mở phiên nào" : sessions.values.map { "\($0.host): \($0.status)" }.joined(separator: "; ")
    }

    func fetch(_ r: WebFetch.Request) async throws -> Http.Response {
        let host = WebFetch.hostOf(r.url)
        let s: Session
        if let old = sessions[host] { s = old } else { s = Session(host: host); sessions[host] = s }
        touch(s)
        defer { touch(s) }
        for attempt in 0 ..< 2 {
            try await ready(s)
            let (resp, cf) = try await call(s, r)
            if cf?.lowercased() != "challenge" { return resp }
            // Lớp chặn quay lại (cookie hết hạn): qua lại lớp chặn, thử thêm một lần
            if attempt == 0 { s.ready = false }
        }
        throw WebFetch.ChallengeError(host: host)
    }

    /// Hẹn đóng WebView sau 5 phút không dùng.
    private func touch(_ s: Session) {
        s.idle?.cancel()
        s.idle = Task { @MainActor [weak self, weak s] in
            try? await Task.sleep(nanoseconds: WebFetcher.idleClose)
            guard !Task.isCancelled, let self = self, let s = s, s.opening == nil else { return }
            self.close(s, why: "không dùng 5 phút")
            s.status = "đã đóng (không dùng 5 phút)"
        }
    }

    private func ready(_ s: Session) async throws {
        if let until = s.blockedUntil, Date() < until { throw WebFetch.BlockedError(host: s.host) }
        if s.web != nil, s.gen != AppProxy.generation { close(s, why: "đổi proxy") }
        if s.ready, s.web != nil { return }
        if let t = s.opening { return try await t.value }
        let t = Task { @MainActor in try await self.open(s) }
        s.opening = t
        defer { s.opening = nil }
        try await t.value
    }

    private func open(_ s: Session) async throws {
        let web = s.web ?? create(s)
        let t0 = Date()
        s.status = "đang qua lớp chặn"
        web.load(URLRequest(url: URL(string: "https://\(s.host)/")!))
        // Trang chặn («Just a moment…», window._cf_chl_opt) tự giải rồi nạp lại trang thật
        while true {
            try await Task.sleep(nanoseconds: 500_000_000)
            if Date().timeIntervalSince(t0) > WebFetcher.passTimeout {
                s.status = "chưa qua lớp chặn"
                close(s, why: "chưa qua lớp chặn")
                throw AppError("Trang \(s.host) đòi bấm xác nhận chống bot (Cloudflare) — app chưa đọc được trang này.")
            }
            guard case .success(let v) = await run(web, WebFetcher.probeJS, args: nil, seconds: 5), let raw = v as? String,
                  let a = (try? JSONSerialization.jsonObject(with: Data(raw.utf8))) as? [Any] else { continue }
            let str = { (i: Int) in a.count > i ? (a[i] as? String ?? "") : "" }
            let bool = { (i: Int) in a.count > i ? (a[i] as? Bool ?? false) : false }
            let same = str(0).lowercased() == s.host && str(1) != "loading"
            if same && (bool(5) || str(2).hasPrefix("Attention Required")) {
                Log.d("Web", "\(s.host): Cloudflare chặn hẳn WebKit — không đọc được")
                s.blockedUntil = Date().addingTimeInterval(WebFetcher.blockedFor)
                s.status = "trang chặn hẳn WebKit"
                close(s, why: "trang chặn app")
                throw WebFetch.BlockedError(host: s.host)
            }
            if same && str(3) == "undefined" && !bool(4) && !str(2).hasPrefix("Just a moment") { break }
        }
        let ms = Int(Date().timeIntervalSince(t0) * 1000)
        Log.d("Web", "\(s.host): WebKit đã qua lớp chống bot (\(ms) ms)")
        // Đứng ở trang nhẹ cùng gốc (khỏi chạy quảng cáo / mã của trang chủ) — fetch() vẫn mang cookie của trang
        await s.load(URL(string: "https://\(s.host)/robots.txt")!, waitPath: "/robots.txt", seconds: 15)
        s.ready = true
        s.status = "sẵn sàng (qua lớp chặn \(ms) ms)"
    }

    private func create(_ s: Session) -> WKWebView {
        let c = WebSetup.configuration()
        c.mediaTypesRequiringUserActionForPlayback = .all
        c.allowsPictureInPictureMediaPlayback = false
        let web = WebSetup.make(c)
        web.frame = CGRect(x: -10_000, y: 0, width: 800, height: 600)
        web.navigationDelegate = s
        web.alpha = 0
        web.isUserInteractionEnabled = false
        // Gắn vào cửa sổ (ngoài vùng nhìn thấy): WebView không nằm trong cửa sổ thì WebKit giảm nhịp chạy mã của trang
        let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows)
        if let win = windows.first(where: \.isKeyWindow) ?? windows.first { win.addSubview(web) }
        s.web = web
        s.gen = AppProxy.generation
        return web
    }

    /// Chạy mã trong trang (bản completion handler — bản async của SDK cũ đổ vỡ khi mã trả undefined), chờ tối đa `seconds`.
    /// `args` khác nil → callAsyncJavaScript (thân hàm, được `await`), nil → evaluateJavaScript.
    private func run(_ web: WKWebView, _ js: String, args: [String: Any]?, seconds: Double) async -> Result<Any?, Error> {
        await withCheckedContinuation { (c: CheckedContinuation<Result<Any?, Error>, Never>) in
            let once = Once()
            let finish = { (r: Result<Any?, Error>) in
                if once.done { return }
                once.done = true
                c.resume(returning: r)
            }
            if let args = args {
                web.callAsyncJavaScript(js, arguments: args, in: nil, in: .page) { r in
                    switch r {
                    case .success(let v): finish(.success(v))
                    case .failure(let e): finish(.failure(e))
                    }
                }
            } else {
                web.evaluateJavaScript(js) { v, e in
                    if let e = e { finish(.failure(e)) } else { finish(.success(v)) }
                }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + seconds) {
                finish(.failure(AppError("WebView không trả lời sau \(Int(seconds)) giây.")))
            }
        }
    }

    /// fetch() trong trang; trả (trả lời, đầu thư cf-mitigated).
    private func call(_ s: Session, _ r: WebFetch.Request) async throws -> (Http.Response, String?) {
        guard let web = s.web else { throw AppError("WebView của \(s.host) đã đóng.") }
        let args: [String: Any] = ["id": 0, "url": r.url, "method": r.method, "body": r.body ?? NSNull(), "headers": r.headers]
        let body = WebFetcher.fetchJS + "\nreturn await window.__cbFetch(id, url, method, body, headers);"
        let any: Any?
        switch await run(web, body, args: args, seconds: WebFetcher.callTimeout) {
        case .success(let v): any = v
        case .failure(let e): throw AppError("WebView đọc \(r.url) lỗi: \(e.localizedDescription)")
        }
        guard let m = any as? [String: Any] else { throw AppError("WebView trả lời rỗng (\(r.url)).") }
        if let err = m["err"] as? String { throw AppError("WebView đọc \(r.url) lỗi: \(err)") }
        let data = Data(base64Encoded: m["b64"] as? String ?? "") ?? Data()
        let code = (m["code"] as? NSNumber)?.intValue ?? 0
        let resp = Http.Response(url: m["url"] as? String ?? r.url, code: code, data: data, contentType: m["type"] as? String)
        return (resp, m["cf"] as? String)
    }

    private func close(_ s: Session, why: String) {
        guard let web = s.web else { return }
        web.stopLoading()
        web.navigationDelegate = nil
        web.removeFromSuperview()
        s.web = nil
        s.ready = false
        s.resumeFinished()
        Log.d("Web", "\(s.host): đóng WebView ẩn (\(why))")
    }
}
