import Foundation

/// Trang bật lớp chống bot của Cloudflare (hoathinh3d.pw từ 14/11/2026: 403 «Just a moment…» + `cf-mitigated: challenge` với mọi
/// trình tải không phải trình duyệt — cả trang, API, ảnh). Trình duyệt thật (WKWebView) tự qua lớp chặn, nên app đọc hộ bằng một
/// WKWebView ẩn (Player/WebFetcher.swift gán `provider`). Http.get / post tự chuyển khi gặp lớp chặn; trang bị chặn được nhớ 12 giờ
/// (các lần sau đi thẳng qua WebView). Không giả / không giải hộ lớp chặn: trang đòi bấm xác nhận / chặn hẳn thì báo lỗi.
/// Cùng luật với lõi Rust (web_fetch.rs) và bản TV (WebFetch.kt).
enum WebFetch {
    struct Request {
        let url: String
        let method: String
        let body: String?
        let headers: [String: String]
    }

    /// Trang bật chống bot mà app chưa đọc hộ được.
    struct ChallengeError: LocalizedError {
        let host: String
        var errorDescription: String? {
            "Trang \(host) đang bật lớp chống bot của Cloudflare — app chưa đọc được (thử lại sau, hoặc xem phim này ở nguồn khác)."
        }
    }

    /// Cloudflare của trang chặn hẳn WebKit («Attention Required! … you have been blocked») — quyết định của chủ trang.
    struct BlockedError: LocalizedError {
        let host: String
        var errorDescription: String? {
            "Trang \(host) chặn app này (Cloudflare «you have been blocked») — xem phim này ở nguồn khác, vd KKPhim."
        }
    }

    /// Hàm đọc hộ của app (WKWebView ẩn); nil = chưa có (test, widget).
    static var provider: ((Request) async throws -> Http.Response)?

    static let keepSeconds: TimeInterval = 12 * 3600
    private static let lock = NSLock()
    private static var hosts: [String: Date] = [:]

    static func hostOf(_ url: String) -> String { URL(string: url)?.host?.lowercased() ?? "" }

    static func remember(_ host: String, now: Date = Date()) {
        guard !host.isEmpty else { return }
        lock.lock()
        hosts[host.lowercased()] = now
        lock.unlock()
    }

    /// Trang của địa chỉ này đang bị nhớ là chặn (trong 12 giờ).
    static func needs(_ url: String, now: Date = Date()) -> Bool {
        let h = hostOf(url)
        lock.lock()
        defer { lock.unlock() }
        guard let t = hosts[h] else { return false }
        let age = now.timeIntervalSince(t)
        return age >= 0 && age < keepSeconds
    }

    static func forgetAll() {
        lock.lock()
        hosts.removeAll()
        lock.unlock()
    }

    /// Các trang đang bị nhớ là chặn — gỡ lỗi.
    static var rememberedHosts: [String] {
        lock.lock()
        defer { lock.unlock() }
        return hosts.keys.sorted()
    }

    /// Trả lời này là lớp chặn của Cloudflare (không phải lỗi thật của trang): đầu thư `cf-mitigated: challenge`, hoặc 403 / 503 mà
    /// nội dung là trang «Just a moment…» / mã của trang chặn (`_cf_chl_opt`).
    static func isChallenge(code: Int, cfMitigated: String?, body: Data?) -> Bool {
        if cfMitigated?.trimmingCharacters(in: .whitespaces).lowercased() == "challenge" { return true }
        guard code == 403 || code == 503, let body = body else { return false }
        let head = String(decoding: body.prefix(8192), as: UTF8.self).lowercased()
        return head.contains("<title>just a moment") || head.contains("_cf_chl_opt") || head.contains("/cdn-cgi/challenge-platform/h/")
    }

    /// Nhờ app đọc hộ (chưa có app đăng ký → ChallengeError). Mã ≥ 400 → Http.HttpError như Http.get.
    static func fetch(_ r: Request) async throws -> Http.Response {
        let host = hostOf(r.url)
        guard let p = provider else { throw ChallengeError(host: host) }
        let t0 = Date()
        let resp = try await p(r)
        Log.d("Web", "\(r.url) → \(resp.code) qua WebView, \(resp.data.count) byte, \(Int(Date().timeIntervalSince(t0) * 1000)) ms")
        if isChallenge(code: resp.code, cfMitigated: nil, body: resp.data) { throw ChallengeError(host: host) }
        if resp.code >= 400 { throw Http.HttpError(code: resp.code) }
        return resp
    }
}
