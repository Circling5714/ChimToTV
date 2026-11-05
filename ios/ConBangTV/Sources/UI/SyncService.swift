import CryptoKit
import Foundation
import Network
import UIKit

/// Máy đã ghép để đồng bộ trong nhà.
struct SyncPeer: Codable, Identifiable, Equatable {
    var id: String
    var name: String
    var platform: String
    var token: String
    /// Địa chỉ lần cuối kết nối được (khi Bonjour không tìm thấy).
    var host: String?
    var port: Int?
    /// Lần đồng bộ được gần nhất (ms) và lời báo lần thử gần nhất.
    var last: Int64?
    var status: String?
}

/// Máy tìm thấy trong mạng (Bonjour), chưa chắc đã ghép.
struct SyncFound: Identifiable, Equatable {
    let id: String
    let name: String
    let platform: String
    let endpoint: NWEndpoint
}

/// Đồng bộ trong nhà (bản Android: Sync.kt + SyncServer.kt). Mỗi máy mở cổng 8791 trong mạng nhà và quảng bá qua Bonjour
/// (`_conbangtv._tcp`):
///   GET  /hello  — mã máy, tên, có đang cho ghép không (không cần khoá)
///   POST /pair   — {device, name, platform, code}: đúng mã ghép đang hiện trên máy này → trả khoá chung (token)
///   POST /sync   — thân = SyncPayload của máy gửi; ký X-Device / X-Time / X-Sig = HMAC-SHA256(token, "time\nthân").
///                  Máy nhận gộp vào dữ liệu của mình rồi trả lại SyncPayload đã gộp (cũng ký) — máy gửi gộp nốt.
/// Cổng chỉ mở khi đã ghép ít nhất một máy hoặc đang mở màn Đồng bộ (khỏi hỏi quyền mạng cục bộ khi không dùng).
@MainActor
final class SyncService: ObservableObject {
    static let shared = SyncService()
    /// Bản thử dùng cổng khác (cài song song bản chính trên cùng máy).
    #if DEVTOOLS
    static let port: UInt16 = 8792
    #else
    static let port: UInt16 = 8791
    #endif
    static let serviceType = "_conbangtv._tcp"

    @Published private(set) var peers: [SyncPeer] = []
    @Published private(set) var found: [SyncFound] = []
    /// Mã ghép đang hiện trên màn Đồng bộ (nil = không cho ghép).
    @Published private(set) var pairCode: String?
    @Published private(set) var busy = false
    @Published private(set) var serverState = "tắt"

    private var listener: NWListener?
    private var browser: NWBrowser?
    private var pairTries = 0
    private var autoTask: Task<Void, Never>?
    private let queue = DispatchQueue(label: "vn.chimto.sync")

    private init() { peers = Self.loadPeers() }

    var enabled: Bool {
        get { Prefs.store.object(forKey: "sync_auto") as? Bool ?? true }
        set {
            Prefs.store.set(newValue, forKey: "sync_auto")
            objectWillChange.send()
        }
    }

    static var deviceName: String { UIDevice.current.name }
    static var platform: String { "ios" }

    // MARK: - Máy đã ghép

    private static func loadPeers() -> [SyncPeer] { Prefs.decode([SyncPeer].self, "sync_peers") ?? [] }

    private func savePeers() { Prefs.encode(peers, "sync_peers") }

    private func updatePeer(_ id: String, _ change: (inout SyncPeer) -> Void) {
        guard let i = peers.firstIndex(where: { $0.id == id }) else { return }
        change(&peers[i])
        savePeers()
    }

    func unpair(_ p: SyncPeer) {
        peers.removeAll { $0.id == p.id }
        savePeers()
        if peers.isEmpty && pairCode == nil { stop() }
    }

    // MARK: - Mở cổng / tìm máy

    /// Mở app / quay lại app: đã ghép thì mở cổng, tìm máy, đồng bộ.
    func appActive() {
        guard !peers.isEmpty else { return }
        start()
        syncSoon(delay: 1.5)
    }

    func start() {
        startListener()
        startBrowser()
    }

    func stop() {
        listener?.cancel()
        listener = nil
        browser?.cancel()
        browser = nil
        serverState = "tắt"
    }

    private func startListener() {
        guard listener == nil, let p = NWEndpoint.Port(rawValue: Self.port) else { return }
        do {
            let params = NWParameters.tcp
            params.allowLocalEndpointReuse = true
            params.includePeerToPeer = false
            let l = try NWListener(using: params, on: p)
            let txt = NWTXTRecord(["id": Prefs.deviceId, "p": Self.platform])
            l.service = NWListener.Service(name: Self.deviceName, type: Self.serviceType, domain: nil, txtRecord: txt)
            l.stateUpdateHandler = { [weak self] s in
                Task { @MainActor in
                    self?.serverState = "\(s)"
                    if case .failed = s {
                        self?.listener?.cancel()
                        self?.listener = nil
                    }
                }
            }
            l.newConnectionHandler = { [weak self] c in self?.accept(c) }
            l.start(queue: queue)
            listener = l
        } catch {
            serverState = "lỗi: \(error.localizedDescription)"
            Log.d("Sync", "không mở được cổng \(Self.port): \(error)")
        }
    }

    private func startBrowser() {
        guard browser == nil else { return }
        let b = NWBrowser(for: .bonjourWithTXTRecord(type: Self.serviceType, domain: nil), using: .tcp)
        b.browseResultsChangedHandler = { [weak self] results, _ in
            let list: [SyncFound] = results.compactMap { r in
                guard case let .service(name, _, _, _) = r.endpoint,
                      case let .bonjour(txt) = r.metadata, let id = txt["id"], id != Prefs.deviceId else { return nil }
                return SyncFound(id: id, name: name, platform: txt["p"] ?? "", endpoint: r.endpoint)
            }
            Task { @MainActor in self?.found = list.sorted { $0.name < $1.name } }
        }
        b.start(queue: queue)
        browser = b
    }

    /// Màn Đồng bộ mở: hiện mã ghép 4 số (đổi mỗi lần mở), mở cổng + tìm máy.
    func openPairing() {
        pairCode = String(format: "%04d", Int.random(in: 0...9999))
        pairTries = 0
        start()
    }

    func closePairing() {
        pairCode = nil
        if peers.isEmpty { stop() }
    }

    // MARK: - Máy chủ (máy kia gọi tới)

    nonisolated private func accept(_ c: NWConnection) {
        c.start(queue: queue)
        read(c, Data())
    }

    nonisolated private func read(_ c: NWConnection, _ buf: Data) {
        c.receive(minimumIncompleteLength: 1, maximumLength: 1 << 20) { [weak self] data, _, done, error in
            guard let self = self else { return }
            var b = buf
            if let d = data { b.append(d) }
            if let req = SyncHTTP.parseRequest(b) {
                Task { @MainActor in
                    let resp = await self.handle(req)
                    self.queue.async { c.send(content: resp, completion: .contentProcessed { _ in c.cancel() }) }
                }
            } else if done || error != nil || b.count > (8 << 20) {
                c.cancel()
            } else {
                self.read(c, b)
            }
        }
    }

    private func handle(_ r: SyncHTTP.Message) async -> Data {
        switch (r.method, r.path) {
        case ("GET", "/hello"):
            return SyncHTTP.response(200, json: ["app": "conbangtv", "device": Prefs.deviceId, "name": Self.deviceName,
                                                 "platform": Self.platform, "version": UpdateCheck.currentVersion,
                                                 "pairing": pairCode != nil])
        case ("POST", "/pair"):
            return pairRequest(r.body)
        case ("POST", "/sync"):
            guard let id = r.headers["x-device"], let peer = peers.first(where: { $0.id == id }) else {
                return SyncHTTP.response(403, json: ["error": "unpaired"])
            }
            guard SyncHTTP.verify(r, token: peer.token) else { return SyncHTTP.response(401, json: ["error": "sig"]) }
            guard let remote = try? JSONDecoder().decode(SyncPayload.self, from: r.body) else {
                return SyncHTTP.response(400, json: ["error": "json"])
            }
            let changed = absorb(remote)
            updatePeer(id) {
                $0.name = remote.name
                $0.last = nowMs()
                $0.status = changed ? "Đã nhận dữ liệu mới" : "Đã khớp"
            }
            if changed { AppModel.shared.syncApplied() }
            let body = (try? JSONEncoder().encode(payload())) ?? Data()
            return SyncHTTP.response(200, body: body, signedWith: peer.token)
        default:
            return SyncHTTP.response(404, json: ["error": "not found"])
        }
    }

    private func pairRequest(_ body: Data) -> Data {
        guard let code = pairCode else { return SyncHTTP.response(403, json: ["error": "closed"]) }
        guard let o = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any],
              let device = o["device"] as? String, !device.isEmpty, let given = o["code"] as? String else {
            return SyncHTTP.response(400, json: ["error": "json"])
        }
        guard given == code else {
            pairTries += 1
            // Đoán sai nhiều lần: đổi mã
            if pairTries >= 5 { openPairing() }
            return SyncHTTP.response(403, json: ["error": "code"])
        }
        let token = SyncHTTP.randomToken()
        let peer = SyncPeer(id: device, name: (o["name"] as? String) ?? "Máy khác", platform: (o["platform"] as? String) ?? "",
                            token: token, host: nil, port: nil, last: nil, status: "Vừa ghép")
        peers.removeAll { $0.id == device }
        peers.append(peer)
        savePeers()
        pairCode = String(format: "%04d", Int.random(in: 0...9999)) // mã dùng một lần
        pairTries = 0
        AppModel.shared.showToast("Đã ghép với \(peer.name).")
        return SyncHTTP.response(200, json: ["device": Prefs.deviceId, "name": Self.deviceName, "platform": Self.platform,
                                             "token": token])
    }

    // MARK: - Dữ liệu

    func payload() -> SyncPayload {
        SyncPayload(device: Prefs.deviceId, name: Self.deviceName, platform: Self.platform,
                    profiles: Prefs.profiles.map { Prefs.syncProfile($0) }, sites: Prefs.syncSites)
    }

    /// Gộp dữ liệu máy kia vào máy này (người xem ghép theo tên). true = máy này có thay đổi.
    @discardableResult
    func absorb(_ remote: SyncPayload) -> Bool {
        let local = Prefs.profiles.map { Prefs.syncProfile($0) }
        var changed = false
        for (l, r) in SyncMerge.match(local, remote.profiles) {
            if Prefs.applySync(SyncMerge.profile(local[l], remote.profiles[r])) { changed = true }
        }
        if Prefs.applySites(SyncMerge.sites(Prefs.syncSites, remote.sites)) { changed = true }
        return changed
    }

    // MARK: - Máy khách (máy này gọi tới máy kia)

    private func endpoint(_ p: SyncPeer) -> NWEndpoint? {
        if let f = found.first(where: { $0.id == p.id }) { return f.endpoint }
        if let h = p.host { return .hostPort(host: NWEndpoint.Host(h), port: NWEndpoint.Port(rawValue: UInt16(p.port ?? Int(Self.port)))!) }
        return nil
    }

    /// Ghép với máy kia bằng mã 4 số đang hiện trên máy đó. Trả về lời báo lỗi (nil = được).
    func pair(_ target: NWEndpoint, code: String) async -> String? {
        let body = (try? JSONSerialization.data(withJSONObject: ["device": Prefs.deviceId, "name": Self.deviceName,
                                                                  "platform": Self.platform, "code": code])) ?? Data()
        do {
            let r = try await SyncHTTP.send(target, method: "POST", path: "/pair", body: body, queue: queue)
            switch r.status {
            case 200: break
            case 403:
                let err = ((try? JSONSerialization.jsonObject(with: r.body)) as? [String: Any])?["error"] as? String
                return err == "closed" ? "Máy kia chưa mở màn Đồng bộ (mở để hiện mã ghép)." : "Sai mã ghép."
            default: return "Máy kia trả lỗi \(r.status)."
            }
            guard let o = (try? JSONSerialization.jsonObject(with: r.body)) as? [String: Any],
                  let id = o["device"] as? String, let token = o["token"] as? String else { return "Máy kia trả dữ liệu lạ." }
            let host = r.remoteHost
            peers.removeAll { $0.id == id }
            peers.append(SyncPeer(id: id, name: (o["name"] as? String) ?? "Máy khác", platform: (o["platform"] as? String) ?? "",
                                  token: token, host: host, port: Int(Self.port), last: nil, status: "Vừa ghép"))
            savePeers()
            start()
            await syncAll(manual: true)
            return nil
        } catch {
            return "Không kết nối được: \(error.localizedDescription)"
        }
    }

    /// Đồng bộ sau một lúc (gộp các lần gọi sát nhau: đóng trình phát, theo dõi phim…).
    func syncSoon(delay: Double = 3) {
        guard enabled, !peers.isEmpty else { return }
        autoTask?.cancel()
        autoTask = Task {
            try? await Task.sleep(nanoseconds: UInt64(delay * 1e9))
            guard !Task.isCancelled else { return }
            await syncAll(manual: false)
        }
    }

    func syncAll(manual: Bool) async {
        guard !busy, !peers.isEmpty else { return }
        busy = true
        defer { busy = false }
        start()
        // Bonjour cần một lúc để thấy máy kia
        if found.isEmpty { try? await Task.sleep(nanoseconds: 1_500_000_000) }
        var changed = false
        for p in peers {
            if await sync(p) { changed = true }
        }
        if changed { AppModel.shared.syncApplied() }
        if manual { AppModel.shared.showToast(changed ? "Đã đồng bộ — có dữ liệu mới." : "Đã đồng bộ.") }
    }

    /// Đồng bộ với một máy. true = máy này có thay đổi.
    private func sync(_ p: SyncPeer) async -> Bool {
        guard let ep = endpoint(p) else {
            updatePeer(p.id) { $0.status = "Không thấy máy trong mạng" }
            return false
        }
        let body = (try? JSONEncoder().encode(payload())) ?? Data()
        do {
            let r = try await SyncHTTP.send(ep, method: "POST", path: "/sync", body: body, token: p.token, queue: queue)
            if r.status == 403 {
                updatePeer(p.id) { $0.status = "Máy kia đã bỏ ghép — ghép lại" }
                return false
            }
            guard r.status == 200, SyncHTTP.verify(r, token: p.token),
                  let remote = try? JSONDecoder().decode(SyncPayload.self, from: r.body) else {
                updatePeer(p.id) { $0.status = "Lỗi \(r.status)" }
                return false
            }
            let changed = absorb(remote)
            updatePeer(p.id) {
                $0.name = remote.name
                $0.last = nowMs()
                $0.status = changed ? "Đã nhận dữ liệu mới" : "Đã khớp"
                if let h = r.remoteHost { $0.host = h }
            }
            Log.d("Sync", "đồng bộ với \(remote.name): \(changed ? "có thay đổi" : "đã khớp")")
            return changed
        } catch {
            updatePeer(p.id) { $0.status = "Không kết nối được" }
            Log.d("Sync", "không đồng bộ được với \(p.name): \(error.localizedDescription)")
            return false
        }
    }
}

// MARK: - HTTP tối giản trên TCP (Network.framework)

enum SyncHTTP {
    struct Message {
        var method = ""
        var path = ""
        var status = 0
        var headers: [String: String] = [:]
        var body = Data()
        var remoteHost: String?
    }

    struct Failure: LocalizedError {
        let errorDescription: String?
    }

    /// Đủ một yêu cầu / trả lời (đầu + thân theo Content-Length) thì trả về, chưa đủ thì nil.
    static func parse(_ data: Data, request: Bool) -> Message? {
        guard let end = data.range(of: Data("\r\n\r\n".utf8)) else { return nil }
        let head = String(decoding: data.subdata(in: 0..<end.lowerBound), as: UTF8.self)
        var lines = head.components(separatedBy: "\r\n")
        guard !lines.isEmpty else { return nil }
        let first = lines.removeFirst().split(separator: " ")
        guard first.count >= 2 else { return nil }
        var m = Message()
        for l in lines {
            let p = l.split(separator: ":", maxSplits: 1)
            if p.count == 2 { m.headers[p[0].lowercased()] = p[1].trimmingCharacters(in: .whitespaces) }
        }
        let length = Int(m.headers["content-length"] ?? "0") ?? 0
        let start = end.upperBound
        guard data.count - start >= length else { return nil }
        m.body = data.subdata(in: start..<(start + length))
        if request {
            m.method = String(first[0])
            m.path = String(first[1]).components(separatedBy: "?")[0]
        } else {
            m.status = Int(first[1]) ?? 0
        }
        return m
    }

    static func parseRequest(_ data: Data) -> Message? { parse(data, request: true) }

    static func sign(_ token: String, time: String, body: Data) -> String {
        var d = Data("\(time)\n".utf8)
        d.append(body)
        let mac = HMAC<SHA256>.authenticationCode(for: d, using: SymmetricKey(data: Data(token.utf8)))
        return Data(mac).map { String(format: "%02x", $0) }.joined()
    }

    /// Chữ ký đúng và thời điểm lệch không quá 10 phút.
    static func verify(_ m: Message, token: String) -> Bool {
        guard let t = m.headers["x-time"], let sig = m.headers["x-sig"], let ms = Int64(t),
              abs(nowMs() - ms) < 10 * 60_000 else { return false }
        return sign(token, time: t, body: m.body) == sig.lowercased()
    }

    static func randomToken() -> String {
        (0..<32).map { _ in String(format: "%02x", UInt8.random(in: 0...255)) }.joined()
    }

    static func response(_ status: Int, json: [String: Any]) -> Data {
        response(status, body: (try? JSONSerialization.data(withJSONObject: json)) ?? Data())
    }

    static func response(_ status: Int, body: Data, signedWith token: String? = nil) -> Data {
        var head = "HTTP/1.1 \(status) \(status == 200 ? "OK" : "Error")\r\nContent-Type: application/json; charset=utf-8\r\n" +
            "Content-Length: \(body.count)\r\nConnection: close\r\n"
        if let t = token {
            let time = String(nowMs())
            head += "X-Time: \(time)\r\nX-Sig: \(sign(t, time: time, body: body))\r\n"
        }
        var d = Data((head + "\r\n").utf8)
        d.append(body)
        return d
    }

    /// Gửi một yêu cầu tới máy kia (thẳng trong mạng nhà, không qua proxy), chờ tối đa 8 giây.
    static func send(_ ep: NWEndpoint, method: String, path: String, body: Data, token: String? = nil,
                     queue: DispatchQueue) async throws -> Message {
        var head = "\(method) \(path) HTTP/1.1\r\nHost: conbangtv\r\nContent-Type: application/json\r\n" +
            "Content-Length: \(body.count)\r\nConnection: close\r\n"
        if let t = token {
            let time = String(nowMs())
            head += "X-Device: \(Prefs.deviceId)\r\nX-Time: \(time)\r\nX-Sig: \(sign(t, time: time, body: body))\r\n"
        }
        var req = Data((head + "\r\n").utf8)
        req.append(body)
        let c = NWConnection(to: ep, using: .tcp)
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Message, Error>) in
                let once = Once()
                let finish: (Result<Message, Error>) -> Void = { r in
                    guard once.claim() else { return }
                    c.cancel()
                    cont.resume(with: r)
                }
                queue.asyncAfter(deadline: .now() + 8) { finish(.failure(Failure(errorDescription: "quá giờ"))) }
                c.stateUpdateHandler = { s in
                    switch s {
                    case .ready:
                        c.send(content: req, completion: .contentProcessed { e in
                            if let e = e { finish(.failure(e)) }
                        })
                        receive(c, Data(), finish)
                    case .failed(let e), .waiting(let e):
                        finish(.failure(e))
                    default:
                        break
                    }
                }
                c.start(queue: queue)
            }
        } onCancel: {
            c.cancel()
        }
    }

    private static func receive(_ c: NWConnection, _ buf: Data, _ finish: @escaping (Result<Message, Error>) -> Void) {
        c.receive(minimumIncompleteLength: 1, maximumLength: 1 << 20) { data, _, done, error in
            var b = buf
            if let d = data { b.append(d) }
            if var m = parse(b, request: false) {
                if case let .hostPort(host, _)? = c.currentPath?.remoteEndpoint {
                    var h = "\(host)"
                    if let pct = h.firstIndex(of: "%") { h = String(h[..<pct]) } // bỏ "%en0" của IPv6
                    m.remoteHost = h
                }
                finish(.success(m))
            } else if let e = error {
                finish(.failure(e))
            } else if done {
                finish(.failure(Failure(errorDescription: "máy kia đóng kết nối")))
            } else {
                receive(c, b, finish)
            }
        }
    }

    /// Chỉ cho chạy một lần (trả lời / quá giờ / lỗi — cái nào tới trước).
    private final class Once {
        private var done = false
        private let lock = NSLock()
        func claim() -> Bool {
            lock.lock()
            defer { lock.unlock() }
            if done { return false }
            done = true
            return true
        }
    }
}
