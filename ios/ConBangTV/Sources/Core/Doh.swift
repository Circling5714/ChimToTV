import Foundation

/// Vượt chặn bằng DNS (bản TV: Doh.resolveAny + Socks5Bridge.deviceDns, Windows: Doh.cs). DNS chặn trang — của nhà mạng,
/// hay bộ lọc quảng cáo trong mạng nhà — thường trả 0.0.0.0 / :: (có nơi 127.x / ::1), vd ảnh bìa CLB Phim Xưa
/// (clbphimxua.com): chia nhỏ gói bắt tay thôi thì không đủ, không có địa chỉ thật để kết nối. Đang bật Vượt chặn mà DNS
/// của máy không trả lời / trả địa chỉ chặn thì cổng nội bộ hỏi lại qua DNS mã hoá (Cloudflare 1.1.1.1, lỗi thì Google
/// 8.8.8.8 — gọi bằng IP, đi thẳng không qua cổng nội bộ).
enum Doh {
    private static let servers = [("Cloudflare 1.1.1.1", "https://1.1.1.1/dns-query"), ("Google 8.8.8.8", "https://8.8.8.8/resolve")]
    private static let lock = NSLock()
    private static var cache: [String: (addr: IPAddr, until: Date)] = [:]

    /// Đi thẳng: không proxy (từ điển rỗng = không dùng cả proxy của hệ thống lẫn cổng nội bộ).
    private static let session: URLSession = {
        let c = URLSessionConfiguration.ephemeral
        c.connectionProxyDictionary = [:]
        c.timeoutIntervalForRequest = 6
        c.timeoutIntervalForResource = 8
        return URLSession(configuration: c)
    }()

    #if DEVTOOLS
    /// Chỉ bản thử (cổng gỡ lỗi /dnsblock): coi DNS của máy trả 0.0.0.0 cho các tên miền này, để thử Vượt chặn.
    static var debugBlock: Set<String> = []
    #endif

    /// Địa chỉ DNS chặn trả: trỏ về chính máy chứ không phải trang thật.
    static func blocked(_ a: IPAddr) -> Bool {
        if a.isV4 { return a.bytes.allSatisfy { $0 == 0 } || a.bytes[0] == 127 }
        return a.bytes.allSatisfy { $0 == 0 } || a.bytes == [UInt8](repeating: 0, count: 15) + [1]
    }

    /// Vượt chặn: DNS của máy không trả lời / chỉ trả địa chỉ chặn → địa chỉ thật qua DNS mã hoá; nil = cứ dùng DNS của máy.
    static func unblocked(_ host: String) -> IPAddr? {
        let name = host.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "[]")).trimmingSuffix(".")
        if IPAddr(text: name) != nil || name == "localhost" { return nil }
        let device = debugBlocked(name) ? [IPAddr([0, 0, 0, 0])!] : ((try? Socks5.resolveAll(name)) ?? [])
        if device.contains(where: { !blocked($0) }) { return nil }
        return resolve(name)
    }

    /// Hỏi bản ghi A qua DNS mã hoá (nhớ theo TTL, 10 giây – 5 phút); nil nếu cả hai máy chủ đều lỗi. Chạy ở luồng nền.
    static func resolve(_ name: String) -> IPAddr? {
        lock.lock()
        if let hit = cache[name], hit.until > Date() {
            lock.unlock()
            return hit.addr
        }
        lock.unlock()
        for (label, base) in servers {
            var q = URLComponents(string: base)!
            q.queryItems = [URLQueryItem(name: "name", value: name), URLQueryItem(name: "type", value: "A")]
            var req = URLRequest(url: q.url!)
            req.setValue("application/dns-json", forHTTPHeaderField: "Accept")
            let done = DispatchSemaphore(value: 0)
            var body: Data?
            session.dataTask(with: req) { data, res, _ in
                if (res as? HTTPURLResponse)?.statusCode == 200 { body = data }
                done.signal()
            }.resume()
            _ = done.wait(timeout: .now() + 9)
            guard let data = body, let (addr, ttl) = parse(data) else { continue }
            lock.lock()
            cache[name] = (addr, Date().addingTimeInterval(TimeInterval(min(max(ttl, 10), 300))))
            lock.unlock()
            Log.d("dns", "DNS mã hoá (vượt chặn): \(name) → \(addr.text) (\(label))")
            return addr
        }
        return nil
    }

    /// {"Answer":[{"type":5,…CNAME},{"type":1,"TTL":75,"data":"104.21.35.220"}]} → bản ghi A đầu tiên (kèm TTL).
    static func parse(_ data: Data) -> (IPAddr, Int)? {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let answers = o["Answer"] as? [[String: Any]] else { return nil }
        for a in answers where (a["type"] as? Int) == 1 {
            guard let text = a["data"] as? String, let ip = IPAddr(text: text), ip.isV4, !blocked(ip) else { continue }
            return (ip, a["TTL"] as? Int ?? 60)
        }
        return nil
    }

    private static func debugBlocked(_ name: String) -> Bool {
        #if DEVTOOLS
        lock.lock()
        defer { lock.unlock() }
        return debugBlock.contains(name)
        #else
        return false
        #endif
    }

    #if DEVTOOLS
    static func setDebugBlock(_ hosts: Set<String>) {
        lock.lock()
        debugBlock = hosts
        cache = [:]
        lock.unlock()
    }
    #endif
}
