import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Tải trang bằng URLSession (bản Android: Http.kt). Tự đi theo chuyển hướng (kể cả http → https, đổi tên miền).
enum Http {
    struct Response {
        /// Địa chỉ cuối cùng (sau chuyển hướng)
        let url: String
        let code: Int
        let data: Data
        let contentType: String?
        var body: String { Http.text(data, contentType: contentType) }
    }

    /// Trang trả lời bằng mã lỗi (4xx / 5xx): thử lại cũng vậy.
    struct HttpError: LocalizedError {
        let code: Int
        var errorDescription: String? { "Trang trả về lỗi HTTP \(code)" }
    }

    /// User-Agent chung cho cả app (WKWebView đặt cùng chuỗi này — xem WebSetup): Safari trên iPad, đúng bản iOS.
    static var userAgent: String = defaultUserAgent()

    static func defaultUserAgent() -> String {
        let v = ProcessInfo.processInfo.operatingSystemVersion
        return "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) " +
            "Version/\(safariMajor(v.majorVersion)).\(v.minorVersion) Safari/605.1.15"
    }

    /// Số bản Safari đi kèm hệ điều hành: iOS / iPadOS cùng số với Safari. Bản Mac (Mac Catalyst) ProcessInfo trả số macOS:
    /// macOS 12–15 đi kèm Safari 15–18 (cộng 3), từ macOS 26 cùng số (Safari 26) — không đổi thì trang web tưởng Safari 14 cũ.
    static func safariMajor(_ osMajor: Int, mac: Bool = isMac) -> Int {
        guard mac else { return osMajor }
        return osMajor >= 26 ? osMajor : osMajor + 3
    }

    static var isMac: Bool {
        #if targetEnvironment(macCatalyst)
        return true
        #else
        return false
        #endif
    }

    private static let sessionLock = NSLock()
    private static var cached: (generation: Int, session: URLSession)?

    /// Phiên tải chung; bật / tắt / đổi proxy (AppProxy.generation đổi) thì mở phiên mới đi theo cài đặt mới.
    static var session: URLSession {
        let gen = AppProxy.generation
        sessionLock.lock()
        defer { sessionLock.unlock() }
        if let c = cached, c.generation == gen { return c.session }
        cached?.session.finishTasksAndInvalidate()
        let c = URLSessionConfiguration.default
        c.timeoutIntervalForRequest = 20
        c.timeoutIntervalForResource = 60
        c.requestCachePolicy = .reloadIgnoringLocalCacheData
        c.urlCache = nil
        c.httpAdditionalHeaders = ["Accept-Language": "vi-VN,vi;q=0.9,en;q=0.8"]
        c.httpMaximumConnectionsPerHost = 6
        if let p = AppProxy.cfDictionary { c.connectionProxyDictionary = p }
        let s = URLSession(configuration: c)
        cached = (gen, s)
        return s
    }

    static func get(_ url: String, headers: [String: String] = [:]) async throws -> Response {
        let web = WebFetch.Request(url: url, method: "GET", body: nil, headers: headers)
        // Trang bật chống bot của Cloudflare (đã nhớ): đọc thẳng qua WebView ẩn
        if WebFetch.provider != nil && WebFetch.needs(url) { return try await WebFetch.fetch(web) }
        do {
            do {
                return try await send(request(url, headers: headers))
            } catch let e as URLError where e.code == .networkConnectionLost {
                // Kết nối giữ lại từ lần trước bị ngắt ngầm → thử lại một lần (bản Android: "unexpected end of stream")
                var h = headers
                h["Connection"] = "close"
                return try await send(request(url, headers: h))
            }
        } catch let e as WebFetch.ChallengeError {
            return try await viaWeb(web, e)
        }
    }

    /// POST dạng form (admin-ajax.php của WordPress).
    static func post(_ url: String, form: [String: String], headers: [String: String] = [:]) async throws -> Response {
        let body = form.map { "\($0.key.formEncoded)=\($0.value.formEncoded)" }.joined(separator: "&")
        var webHeaders = headers
        webHeaders["Content-Type"] = "application/x-www-form-urlencoded; charset=UTF-8"
        let web = WebFetch.Request(url: url, method: "POST", body: body, headers: webHeaders)
        if WebFetch.provider != nil && WebFetch.needs(url) { return try await WebFetch.fetch(web) }
        var r = try request(url, headers: headers)
        r.httpMethod = "POST"
        r.setValue("application/x-www-form-urlencoded; charset=UTF-8", forHTTPHeaderField: "Content-Type")
        r.httpBody = body.data(using: .utf8)
        do {
            return try await send(r)
        } catch let e as WebFetch.ChallengeError {
            return try await viaWeb(web, e)
        }
    }

    /// Vừa gặp lớp chống bot của Cloudflare: nhớ trang, app có WebView thì đọc hộ (không thì báo lỗi «chống bot»).
    private static func viaWeb(_ web: WebFetch.Request, _ e: WebFetch.ChallengeError) async throws -> Response {
        WebFetch.remember(e.host)
        guard WebFetch.provider != nil else { throw e }
        Log.d("Web", "\(e.host) bật chống bot của Cloudflare — đọc qua WebView")
        return try await WebFetch.fetch(web)
    }

    static func request(_ url: String, headers: [String: String] = [:]) throws -> URLRequest {
        guard let u = URL(string: url) ?? URL(string: url.addingPercentEncoding(withAllowedCharacters: .urlFragmentAllowed) ?? "") else {
            throw AppError("Địa chỉ không hợp lệ: \(url)")
        }
        var r = URLRequest(url: u)
        r.setValue(userAgent, forHTTPHeaderField: "User-Agent")
        for (k, v) in headers { r.setValue(v, forHTTPHeaderField: k) }
        return r
    }

    private static func send(_ r: URLRequest) async throws -> Response {
        let (data, resp) = try await session.data(for: r)
        let h = resp as? HTTPURLResponse
        let code = h?.statusCode ?? 200
        if code >= 400 {
            // Lớp chống bot của Cloudflare (WebFetch): báo riêng để bên gọi nhờ WebView đọc hộ
            if (code == 403 || code == 503),
               WebFetch.isChallenge(code: code, cfMitigated: h?.value(forHTTPHeaderField: "cf-mitigated"), body: data) {
                throw WebFetch.ChallengeError(host: (resp.url?.host ?? r.url?.host ?? "").lowercased())
            }
            throw HttpError(code: code)
        }
        return Response(url: resp.url?.absoluteString ?? r.url?.absoluteString ?? "", code: code, data: data,
                        contentType: h?.value(forHTTPHeaderField: "Content-Type"))
    }

    /// Giải mã theo charset trong Content-Type (mặc định UTF-8; sai bảng mã thì đọc kiểu Latin-1 cho khỏi mất trang).
    static func text(_ data: Data, contentType: String?) -> String {
        if let ct = contentType?.lowercased(), let r = ct.range(of: "charset=") {
            let name = ct[r.upperBound...].trimmingCharacters(in: CharacterSet(charactersIn: "\"' ")).before(";")
            let cf = CFStringConvertIANACharSetNameToEncoding(name as CFString)
            if cf != kCFStringEncodingInvalidId {
                let enc = String.Encoding(rawValue: CFStringConvertEncodingToNSStringEncoding(cf))
                if let s = String(data: data, encoding: enc) { return s }
            }
        }
        if let s = String(data: data, encoding: .utf8) { return s }
        return String(decoding: data, as: UTF8.self)
    }
}
