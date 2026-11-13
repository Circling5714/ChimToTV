import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Proxy cho toàn app (Cài đặt → Proxy), như bản Android (AppProxy.kt): SOCKS5, hoặc Shadowsocks (mã hoá — nhà mạng không
/// đọc được tên trang / nội dung, vd máy chủ 3X-UI; xem Shadowsocks.swift).
///
/// URLSession và WebKit không đăng nhập được SOCKS5 có tên / mật khẩu, nên app mở một cổng SOCKS5 nội bộ
/// (Socks5Bridge, chỉ nghe 127.0.0.1): URLSession (Http) và WKWebView (WebSetup) đều đi qua cổng này; cổng nội bộ tự
/// đăng nhập proxy thật, chọn DNS (của proxy / của iPad) và ở chế độ "chỉ trang phim" thì cho kết nối tới nơi khác đi
/// thẳng. Tắt proxy → app đi mạng mặc định như trước.
enum AppProxy {
    private static let lock = NSLock()
    private static var bridge: Socks5Bridge?
    private static var config: SocksConfig?
    private static var ss: SsConfig?
    private static var hosts: [String] = []
    private static var gen = 0
    /// Cổng gỡ lỗi: mở cổng nội bộ không có proxy thật (mọi kết nối đi thẳng) — để xem WebKit có đi qua cổng không.
    private static var direct = false
    /// Vượt chặn nhà mạng (Cài đặt): cổng nội bộ chạy cả khi không bật proxy, chia nhỏ gói bắt tay HTTPS.
    private static var split = false
    private static var disorder = false
    /// DNS mã hoá (Cài đặt, bật sẵn): cổng nội bộ chạy cả khi không bật proxy / vượt chặn.
    private static var dns = false

    private static func locked<T>(_ f: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return f()
    }

    /// Đang bật proxy (SOCKS5 hoặc Shadowsocks).
    static var active: Bool { locked { config != nil || ss != nil } }

    /// Tăng mỗi lần đổi cài đặt: Http mở URLSession mới, WebSetup dùng kho dữ liệu web mới.
    static var generation: Int { locked { gen } }

    /// Cổng nội bộ; nil = tắt. Bật mà không mở được cổng (rất hiếm) thì trỏ vào cổng 1 (đóng): kết nối báo lỗi chứ không
    /// lén đi thẳng.
    static var localPort: Int? { locked { bridge?.port ?? (config != nil || ss != nil || split || dns ? 1 : nil) } }

    /// Từ điển proxy CFNetwork (URLSession.connectionProxyDictionary, WebKit); nil = đi thẳng.
    static var cfDictionary: [AnyHashable: Any]? { localPort.map { dictionary(port: $0) } }

    static func dictionary(port: Int) -> [AnyHashable: Any] {
        [
            "SOCKSEnable": 1,
            kCFStreamPropertySOCKSProxyHost as String: "127.0.0.1",
            kCFStreamPropertySOCKSProxyPort as String: port,
            kCFStreamPropertySOCKSVersion as String: kCFStreamSocketSOCKSVersion5 as String,
        ]
    }

    /// Áp dụng cài đặt trong Prefs — lúc mở app, mỗi lần đổi trong Cài đặt, đổi / thêm trang phim. Cài đặt khác đi thì mở
    /// cổng nội bộ mới và đóng cổng cũ cùng mọi kết nối đang giữ: URLSession / WebKit buộc phải kết nối lại theo cài đặt
    /// mới (không dùng lại kết nối keep-alive đi theo cài đặt cũ).
    static func apply() {
        if Level2Memo.save == nil {
            // Trang đã tự lên Vượt chặn Mức 2 (các lần mở app trước): nạp lại, ghi xuống máy mỗi lần nhớ thêm
            Level2Memo.load(Prefs.level2Hosts)
            Level2Memo.save = { Prefs.level2Hosts = $0 }
        }
        let c = Prefs.proxyConfig()
        let q = Prefs.ssConfig()
        let h = (Prefs.sites + [Prefs.baseUrl]).compactMap { Site.hostOf($0)?.lowercased() }
        let s = Prefs.unblock
        let d = s && Prefs.unblockLevel >= 2
        let n = Prefs.secureDns
        lock.lock()
        defer { lock.unlock() }
        hosts = h
        let want = c != nil || q != nil || direct || s || n
        if c == config && q == ss && s == split && d == disorder && n == dns && want == (bridge != nil) { return }
        bridge?.close()
        Doh.clearCache()
        bridge = want ? try? Socks5Bridge(socks: c, ss: q, split: s, disorder: d, dns: n, siteHosts: { AppProxy.siteHosts }) : nil
        config = c
        ss = q
        split = s
        disorder = d
        dns = n
        gen += 1
    }

    /// App quay lại màn hình: cổng nội bộ bị iOS thu hồi lúc nằm nền (bật vượt chặn / proxy, vào lại một lúc sau thì mọi
    /// trang báo mất kết nối tới khi tắt app) → mở lại, cố giữ đúng số cổng cũ để WebKit / URLSession khỏi đổi gì.
    static func revive() {
        lock.lock()
        defer { lock.unlock() }
        guard let b = bridge, !b.listening else { return }
        let old = b.port
        b.close()
        bridge = try? Socks5Bridge(socks: config, ss: ss, split: split, disorder: disorder, dns: dns, port: old, siteHosts: { AppProxy.siteHosts })
        if bridge?.port != old { gen += 1 } // cổng khác: Http mở URLSession mới, WebSetup dùng cấu hình mới
        Log.d("Proxy", "mở lại cổng nội bộ: \(old) → \(bridge.map { String($0.port) } ?? "không mở được")")
    }

    /// Cổng gỡ lỗi: làm hỏng ổ cắm nghe của cổng nội bộ (như iOS thu hồi lúc app nằm nền) để thử revive.
    static func debugBreak() {
        locked { bridge }?.debugBreak()
    }

    /// Cổng gỡ lỗi: bật / tắt cổng nội bộ đi thẳng (khi không bật proxy thật).
    static func setDebugDirect(_ on: Bool) {
        locked { direct = on }
        apply()
    }

    private static var siteHosts: [String] { locked { hosts } }

    /// Lỗi proxy trong 30 giây gần đây, để ghép vào thông báo lỗi (vd "proxy báo sai tên đăng nhập / mật khẩu").
    static var recentError: String? {
        locked { (config != nil || ss != nil) && bridge == nil ? "không mở được cổng nội bộ" : bridge?.recentError }
    }

    /// Câu nhắc thêm vào thông báo lỗi khi đang bật proxy.
    static func note() -> String {
        guard active else { return "" }
        return "\nĐang bật proxy" + (recentError.map { ": \($0)" } ?? "") + " — kiểm tra Cài đặt → Proxy, hoặc tắt proxy."
    }

    /// Video cũng đi qua proxy (chế độ "toàn bộ app").
    static var videoViaProxy: Bool { locked { (config?.sitesOnly ?? ss?.sitesOnly).map { !$0 } ?? false } }

    /// Câu nhắc cho lỗi video: máy chủ video hay chặn IP của proxy → gợi ý cho video đi thẳng.
    static func videoNote() -> String {
        guard videoViaProxy, recentError == nil else { return note() }
        return "\nĐang bật proxy cho toàn bộ app: máy chủ video có thể chặn IP của proxy — thử Cài đặt → Proxy → " +
            "«Dùng cho: chỉ trang phim», hoặc tắt proxy."
    }

    /// Khoá chấm điểm server (PlayStats): video đi qua proxy thì chấm riêng.
    static func statsHost(_ host: String?) -> String? { videoViaProxy ? "\(host ?? "")·proxy" : host }

    /// Cho cổng gỡ lỗi.
    static func debugState() -> [String: Any] {
        lock.lock()
        defer { lock.unlock() }
        var o: [String: Any] = [:]
        o["active"] = config != nil || ss != nil
        o["direct"] = direct
        o["unblock"] = split
        o["dns"] = dns
        o["port"] = bridge?.port ?? 0
        o["generation"] = gen
        o["proxy"] = config.map { c -> String in "\(c.host):\(c.port)" + (c.user.isEmpty ? "" : " (có đăng nhập)") }
            ?? ss.map { "Shadowsocks \($0.host):\($0.port) \($0.method)" } ?? ""
        o["remoteDns"] = config?.remoteDns ?? ss?.remoteDns ?? true
        o["sitesOnly"] = config?.sitesOnly ?? ss?.sitesOnly ?? false
        o["lastError"] = bridge?.recentError ?? ""
        o["hosts"] = hosts
        let stats: [String: [Int]] = bridge?.statsCopy ?? [:]
        o["stats"] = stats.map { k, v -> String in "\(k) proxy=\(v[0]) thẳng=\(v[1]) lỗi=\(v[2])" }.sorted()
        return o
    }

    /// Nút "Kiểm tra proxy": tải https://www.cloudflare.com/cdn-cgi/trace qua proxy c (mở cổng nội bộ tạm, thử được trước
    /// khi lưu) → IP ra ngoài kèm nơi đặt máy, vd "103.1.2.3 (SG)".
    static func check(_ c: SocksConfig) async throws -> String {
        var t = c
        t.sitesOnly = false
        return try await trace(Socks5Bridge(socks: t, siteHosts: { [] }))
    }

    /// Nút "Kiểm tra proxy" với Shadowsocks: như check(SocksConfig). Máy chủ SS 2022 không trả lời thì xem giờ trên máy có
    /// lệch không (máy chủ bỏ qua yêu cầu lệch quá 30 giây).
    static func check(_ c: SsConfig) async throws -> String {
        var t = c
        t.sitesOnly = false
        do {
            return try await trace(Socks5Bridge(socks: nil, ss: t, siteHosts: { [] }))
        } catch {
            if SsMethod.of(c.method)?.is2022 == true, let skew = await clockSkew() {
                throw AppError("giờ trên máy lệch \(skew) giây — Shadowsocks 2022 cần giờ đúng (lệch tối đa 30 giây): chỉnh ngày giờ của máy")
            }
            throw error
        }
    }

    /// Giờ trên máy lệch bao nhiêu giây so với Cloudflare (đi thẳng); nil = không lệch quá 30 giây / không hỏi được.
    private static func clockSkew() async -> Int64? {
        let cfg = URLSessionConfiguration.ephemeral
        cfg.connectionProxyDictionary = [:]
        cfg.timeoutIntervalForRequest = 8
        let s = URLSession(configuration: cfg)
        defer { s.invalidateAndCancel() }
        guard let r = try? await s.data(from: URL(string: "https://www.cloudflare.com/cdn-cgi/trace")!) else { return nil }
        let text = String(decoding: r.0, as: UTF8.self)
        guard let line = text.split(separator: "\n").first(where: { $0.hasPrefix("ts=") }),
              let ts = Double(line.dropFirst(3)) else { return nil }
        let skew = abs(Int64(Date().timeIntervalSince1970 - ts))
        return skew > 30 ? skew : nil
    }

    /// Tải https://www.cloudflare.com/cdn-cgi/trace qua cổng nội bộ tạm b → IP ra ngoài kèm nơi đặt máy.
    private static func trace(_ b: Socks5Bridge) async throws -> String {
        defer { b.close() }
        let cfg = URLSessionConfiguration.ephemeral
        cfg.connectionProxyDictionary = dictionary(port: b.port)
        cfg.timeoutIntervalForRequest = 20
        cfg.timeoutIntervalForResource = 30
        let s = URLSession(configuration: cfg)
        defer { s.invalidateAndCancel() }
        var r = URLRequest(url: URL(string: "https://www.cloudflare.com/cdn-cgi/trace")!)
        r.setValue(Http.userAgent, forHTTPHeaderField: "User-Agent")
        let text: String
        do {
            let (data, _) = try await s.data(for: r)
            text = String(decoding: data, as: UTF8.self)
        } catch {
            // Lỗi ở phía proxy (sai mật khẩu, không tới được proxy…) rõ hơn lỗi chung của URLSession
            if let e = b.recentError { throw AppError(e) }
            throw error
        }
        func field(_ name: String) -> String? {
            text.split(separator: "\n").first { $0.hasPrefix("\(name)=") }.map { String($0.dropFirst(name.count + 1)) }
        }
        guard let ip = field("ip") else { throw AppError("không đọc được kết quả từ www.cloudflare.com") }
        return ip + (field("loc").map { " (\($0))" } ?? "")
    }

    /// Đọc chữ người xem nhập; nhận cả dạng dán sẵn "socks5://user:pass@host:cổng" hoặc "host:cổng".
    /// Trả về cấu hình, hoặc câu báo lỗi.
    static func parse(host rawHost: String, port rawPort: String, user rawUser: String, pass rawPass: String) -> Result<SocksConfig, AppError> {
        var host = rawHost.trimmed
        if let r = host.range(of: "://") { host = String(host[r.upperBound...]) }
        host = host.trimmingSuffix("/")
        var port = rawPort.trimmed, user = rawUser, pass = rawPass
        if let at = host.lastIndex(of: "@") {
            let login = String(host[..<at])
            host = String(host[host.index(after: at)...])
            if let colon = login.firstIndex(of: ":") {
                user = String(login[..<colon])
                pass = String(login[login.index(after: colon)...])
            } else {
                user = login
                pass = ""
            }
        }
        if host.filter({ $0 == ":" }).count == 1, let colon = host.firstIndex(of: ":") {
            port = String(host[host.index(after: colon)...])
            host = String(host[..<colon])
        }
        guard !host.isEmpty, !host.contains(" ") else { return .failure(AppError("Nhập máy chủ proxy (vd 103.1.2.3).")) }
        guard let p = Int(port), (1...65535).contains(p) else { return .failure(AppError("Cổng proxy không hợp lệ (1–65535, vd 1080).")) }
        guard user.utf8.count <= 255, pass.utf8.count <= 255 else {
            return .failure(AppError("Tên đăng nhập / mật khẩu tối đa 255 ký tự."))
        }
        return .success(SocksConfig(host: host, port: p, user: user, pass: pass, remoteDns: Prefs.proxyRemoteDns, sitesOnly: Prefs.proxySitesOnly))
    }
}
