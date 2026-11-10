#if DEVTOOLS
import Foundation
import Network
import UIKit
import WebKit

/// Cổng gỡ lỗi của bản thử (Beta), chỉ trong mạng nội bộ — máy tính điều khiển app như adb với TV:
///   GET  /ping                     thông tin máy / app
///   GET  /shot?scale=0.5           ảnh màn hình (JPEG)
///   GET  /log?since=N              nhật ký app + console trang phát
///   GET  /state                    trạng thái màn chính, trình phát, trình duyệt
///   GET  /ui                       cây trợ năng (nhãn, khung) của màn hình — như uiautomator dump
///   POST /press?label=…&i=0        bấm phần tử có nhãn (trợ năng)
///   POST /type?text=…              gõ chữ vào ô đang nhập
///   POST /go?to=menu&title=… | film&url=… | play&film=…&ep=… | resume&film=… | back | search&q=… | site&url=…
///            | settings | sites | browse&url=… | home | follows | update
///   POST /player?do=toggle|play|pause|seek&s=|seekTo&s=|next|prev|menu|hide|close|reload|server&type=|group&name=
///            |rate|fit|sleep|introStart|introEnd|introClear|tap|confirm|double&left=1|escape|accept|bar
///   POST /eval?target=player|browser   (thân = mã JS) chạy JS ở khung trên cùng
///   GET  /dnsblock?hosts=a.com,b.com   giả DNS của máy chặn các tên miền này (thử Vượt chặn); trống = bỏ
///   GET  /dohfail?labels=DoH NextDNS,…  giả máy chủ DNS mã hoá bị chặn (thử chuỗi DoH → DoT → DNS máy); trống = bỏ
///   GET  /probe?fn=home|list|detail|source|video|search|searchAll|filters&url=…&q=…&server=…   chạy bộ đọc trang
///   POST /orient?to=portrait|landscape|landscapeLeft   ép xoay màn hình  ·  POST /textscale?v=1.3   cỡ chữ
///   GET  /widget                   ghi lại + đọc dữ liệu widget «Xem tiếp»  ·  GET /sync[?do=open|close|all]  đồng bộ trong nhà
///   GET  /prefs  ·  POST /prefs?key=…&value=…&type=int|bool|string|double|del
///   GET  /proxy  ·  POST /proxy?direct=1|0 | pass=… | apply=1 | check=1   cổng proxy nội bộ: trạng thái, thử, đi thẳng
///   POST /upload  (thân = .ipa)    đẩy bản mới lên iPad  ·  GET /ipa  tải lại tệp đó
///   POST /update[?url=…]           cài .ipa qua TrollStore (apple-magnifier://install); không url = tệp vừa đẩy lên
final class DebugServer {
    static let shared = DebugServer()
    let port: UInt16 = 8790
    private var listener: NWListener?
    private let queue = DispatchQueue(label: "vn.chimto.debug")
    private(set) var state = "chưa chạy"

    var addressText: String { "http://\(Self.wifiAddress() ?? "?"):\(port)  (\(state))" }

    func start() {
        guard listener == nil, let p = NWEndpoint.Port(rawValue: port) else { return }
        do {
            let params = NWParameters.tcp
            params.allowLocalEndpointReuse = true
            let l = try NWListener(using: params, on: p)
            l.stateUpdateHandler = { [weak self] s in
                self?.state = "\(s)"
                Log.d("Debug", "cổng gỡ lỗi \(s) — \(self?.addressText ?? "")")
                if case .failed = s {
                    self?.listener = nil
                    self?.queue.asyncAfter(deadline: .now() + 3) { self?.start() }
                }
            }
            l.newConnectionHandler = { [weak self] c in self?.accept(c) }
            l.start(queue: queue)
            listener = l
        } catch {
            state = "lỗi: \(error)"
            Log.d("Debug", "không mở được cổng gỡ lỗi: \(error)")
        }
    }

    private func accept(_ c: NWConnection) {
        c.start(queue: queue)
        read(c, Data())
    }

    private func read(_ c: NWConnection, _ buf: Data) {
        c.receive(minimumIncompleteLength: 1, maximumLength: 1 << 20) { [weak self] data, _, done, error in
            guard let self = self else { return }
            var b = buf
            if let d = data { b.append(d) }
            if let req = Request(b) {
                Task { @MainActor in
                    let resp = await DebugRoutes.handle(req)
                    self.queue.async {
                        c.send(content: resp.bytes, completion: .contentProcessed { _ in c.cancel() })
                    }
                }
            } else if done || error != nil || b.count > (16 << 20) {
                c.cancel()
            } else {
                self.read(c, b)
            }
        }
    }

    /// Địa chỉ IPv4 Wi-Fi của iPad.
    static func wifiAddress() -> String? {
        var ifaddr: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&ifaddr) == 0, let first = ifaddr else { return nil }
        defer { freeifaddrs(ifaddr) }
        var other: String?
        for p in sequence(first: first, next: { $0.pointee.ifa_next }) {
            let i = p.pointee
            guard let sa = i.ifa_addr, sa.pointee.sa_family == UInt8(AF_INET) else { continue }
            let name = String(cString: i.ifa_name)
            guard name.hasPrefix("en") else { continue }
            var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            guard getnameinfo(sa, socklen_t(sa.pointee.sa_len), &host, socklen_t(host.count), nil, 0, NI_NUMERICHOST) == 0 else { continue }
            let ip = String(cString: host)
            if name == "en0" { return ip }
            if other == nil { other = ip }
        }
        return other
    }
}

struct Request {
    let method: String
    let path: String
    let query: [String: String]
    let body: Data

    init?(_ data: Data) {
        guard let end = data.range(of: Data("\r\n\r\n".utf8)) else { return nil }
        let head = String(decoding: data.subdata(in: 0..<end.lowerBound), as: UTF8.self)
        var lines = head.components(separatedBy: "\r\n")
        guard !lines.isEmpty else { return nil }
        let first = lines.removeFirst().split(separator: " ")
        guard first.count >= 2 else { return nil }
        var length = 0
        for l in lines {
            let p = l.split(separator: ":", maxSplits: 1)
            if p.count == 2, p[0].lowercased() == "content-length" { length = Int(p[1].trimmingCharacters(in: .whitespaces)) ?? 0 }
        }
        let start = end.upperBound
        guard data.count - start >= length else { return nil }
        body = data.subdata(in: start..<(start + length))
        method = String(first[0])
        let comps = URLComponents(string: "http://x" + String(first[1]))
        path = comps?.path ?? String(first[1])
        var q: [String: String] = [:]
        for item in comps?.queryItems ?? [] { q[item.name] = item.value ?? "" }
        query = q
    }

    var text: String { String(decoding: body, as: UTF8.self) }
}

struct Response {
    var status = 200
    var type = "application/json; charset=utf-8"
    var body = Data()

    static func json(_ o: Any, status: Int = 200) -> Response {
        let d = (try? JSONSerialization.data(withJSONObject: sanitize(o), options: [.prettyPrinted, .sortedKeys])) ?? Data("{}".utf8)
        return Response(status: status, type: "application/json; charset=utf-8", body: d)
    }

    static func text(_ s: String, status: Int = 200) -> Response {
        Response(status: status, type: "text/plain; charset=utf-8", body: Data(s.utf8))
    }

    var bytes: Data {
        let reason = status == 200 ? "OK" : (status == 404 ? "Not Found" : "Error")
        let head = "HTTP/1.1 \(status) \(reason)\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\n" +
            "Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
        return Data(head.utf8) + body
    }

    /// Đổi về kiểu JSON hợp lệ (số không hữu hạn → 0, kiểu lạ → chuỗi).
    static func sanitize(_ v: Any) -> Any {
        switch v {
        case let d as [String: Any]: return d.mapValues { sanitize($0) }
        case let a as [Any]: return a.map { sanitize($0) }
        case let x as Double: return x.isFinite ? x : 0
        case is String, is Int, is Int64, is Bool, is NSNull, is NSNumber: return v
        default: return String(describing: v)
        }
    }
}

@MainActor
enum DebugRoutes {
    static var uploadFile: URL { FileManager.default.temporaryDirectory.appendingPathComponent("update.ipa") }
    private static var bgTask: UIBackgroundTaskIdentifier = .invalid

    /// Giữ app chạy thêm ~25 giây khi chuyển sang TrollStore (để TrollStore tải xong .ipa từ cổng này).
    private static func keepServing() {
        if bgTask != .invalid { return }
        bgTask = UIApplication.shared.beginBackgroundTask(withName: "update") {
            UIApplication.shared.endBackgroundTask(DebugRoutes.bgTask)
            DebugRoutes.bgTask = .invalid
        }
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 25_000_000_000)
            if bgTask != .invalid {
                UIApplication.shared.endBackgroundTask(bgTask)
                bgTask = .invalid
            }
        }
    }

    static func handle(_ r: Request) async -> Response {
        let q = r.query
        switch r.path {
        case "/", "/help":
            return .text("Cổng gỡ lỗi Côn Bằng TV (bản thử). Xem chú thích trong ios/ConBangTV/Sources/Debug/DebugServer.swift")
        case "/ping":
            let s = UIScreen.main.bounds
            return .json([
                "app": UpdateCheck.currentVersion, "build": Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") ?? "",
                "bundle": Bundle.main.bundleIdentifier ?? "", "ios": UIDevice.current.systemVersion, "model": UIDevice.current.model,
                "machine": machine(), "screen": [Int(s.width), Int(s.height), Int(UIScreen.main.scale)],
                "site": Prefs.baseUrl, "ua": Http.userAgent, "ip": DebugServer.wifiAddress() ?? "",
                "appState": appState(),
            ])
        case "/frames":
            return .json(PlayerModel.current?.frames.map { $0.url } ?? [])
        case "/log":
            let since = Int(q["since"] ?? "0") ?? 0
            let l = Log.since(since)
            return .json(["last": l.last, "lines": l.lines])
        case "/shot":
            let scale = CGFloat(Double(q["scale"] ?? "1") ?? 1)
            guard let data = screenshot(scale: scale) else { return .text("không chụp được (app: \(appState()))", status: 409) }
            return Response(status: 200, type: "image/jpeg", body: data)
        case "/state":
            var o: [String: Any] = ["app": AppModel.shared.debugState()]
            if let p = PlayerModel.current { o["player"] = p.debugState() }
            if let b = BrowserController.current { o["browser"] = b.web.url?.absoluteString ?? NSNull() }
            o["top"] = Router.topController.map { String(describing: type(of: $0)) } ?? NSNull()
            o["appState"] = appState()
            return .json(o)
        case "/ui":
            return .json(uiDump())
        case "/press":
            let label = q["label"] ?? ""
            let (ok, hit) = press(label: label, index: Int(q["i"] ?? "0") ?? 0, partial: q["partial"] == "1")
            return .json(["ok": ok, "label": label, "hit": hit ?? ""])
        case "/scroll":
            // Cuộn khung cuộn lớn nhất đang hiện: dir=top|bottom|up|down. Dải vuốt ngang ở độ cao y (điểm trên màn hình):
            // dir=left|right|start|end&y=…
            let dir = q["dir"] ?? "down"
            if ["left", "right", "start", "end"].contains(dir) {
                return .json(["ok": scrollX(dir, y: CGFloat(Double(q["y"] ?? "0") ?? 0))])
            }
            return .json(["ok": scroll(dir)])
        case "/type":
            let text = q["text"] ?? r.text
            let ok = UIApplication.shared.sendAction(#selector(UIKeyInput.insertText(_:)), to: nil, from: text, for: nil)
            return .json(["ok": ok])
        case "/orient":
            // Ép xoay màn hình (thử iPhone dọc / ngang khi không cầm máy): to=portrait|landscape|landscapeLeft
            let to = q["to"] ?? "portrait"
            let o: UIInterfaceOrientation = to == "landscape" ? .landscapeRight : to == "landscapeLeft" ? .landscapeLeft : .portrait
            if #available(iOS 16.0, *) {
                let mask: UIInterfaceOrientationMask = o == .portrait ? .portrait : o == .landscapeLeft ? .landscapeLeft : .landscapeRight
                for s in UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }) {
                    s.requestGeometryUpdate(.iOS(interfaceOrientations: mask)) { e in Log.d("Debug", "xoay: \(e)") }
                }
                Router.topController?.setNeedsUpdateOfSupportedInterfaceOrientations()
            } else {
                UIDevice.current.setValue(o.rawValue, forKey: "orientation")
                UIViewController.attemptRotationToDeviceOrientation()
            }
            return .json(["ok": true, "to": to])
        case "/textscale":
            TextScale.shared.set(Double(q["v"] ?? "1") ?? 1)
            return .json(["ok": true, "value": TextScale.shared.value])
        case "/widget":
            // Ghi lại dữ liệu widget «Xem tiếp» rồi trả nội dung + thư mục App Group
            WidgetFeed.update()
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            let feed = WidgetFeedData.feedFile.flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? "(chưa có)"
            return .json(["group": WidgetFeedData.groupId, "folder": WidgetFeedData.folder?.path ?? "(không mở được App Group)",
                          "feed": feed])
        case "/sync":
            // Đồng bộ trong nhà: trạng thái; do=open|close|all
            let s = SyncService.shared
            switch q["do"] {
            case "open": s.openPairing()
            case "close": s.closePairing()
            case "all": await s.syncAll(manual: true)
            default: break
            }
            return .json(["code": s.pairCode ?? "", "server": s.serverState, "busy": s.busy,
                          "peers": s.peers.map { ["id": $0.id, "name": $0.name, "host": $0.host ?? "", "status": $0.status ?? "",
                                                  "last": $0.last ?? 0] },
                          "found": s.found.map { ["id": $0.id, "name": $0.name, "platform": $0.platform, "endpoint": "\($0.endpoint)"] }])
        case "/go":
            return go(q)
        case "/row":
            // Sửa nhanh hàng trang chủ (như giữ ô phim): /row?id=<id hàng>&do=info|hide|up|down|rows|sources
            let app = AppModel.shared
            guard let id = q["id"] ?? (q["do"] == "rows" || q["do"] == "sources" ? "" : nil) else { return .text("thiếu id", status: 400) }
            switch q["do"] ?? "info" {
            case "hide": app.hideRow(id)
            case "up": app.moveRow(id, up: true)
            case "down": app.moveRow(id, up: false)
            case "rows": app.sheet = .homeRows
            case "sources": app.sheet = .homeSources
            default: break
            }
            let info = id.isEmpty ? nil : app.rowInfo(id)
            return .json(["ok": true, "title": info?.title ?? "", "slot": info?.slot ?? "", "up": info?.up != nil, "down": info?.down != nil,
                          "rows": Prefs.homeRows, "srcOff": Prefs.homeSrcOff.sorted(),
                          "sections": app.sections.map { $0.title ?? $0.id }])
        case "/player":
            return player(q)
        case "/eval":
            let js = q["js"] ?? r.text
            if q["target"] == "frame" {
                guard let p = PlayerModel.current else { return .text("chưa mở trình phát", status: 400) }
                let (v, e) = await p.evalInFrame(q["match"] ?? "", js)
                return .json(e.map { ["error": $0] } ?? ["result": v ?? NSNull()])
            }
            return await eval(target: q["target"] ?? "player", js: js)
        case "/probe":
            return await probe(q)
        case "/proxy":
            // POST /proxy?direct=1|0 (cổng nội bộ đi thẳng, không proxy thật) · pass=… (lưu mật khẩu) · apply=1 · check=1
            // · ss=<link ss://> (lưu máy chủ Shadowsocks, chọn kiểu Shadowsocks) · kind=socks5|ss · on=1|0
            if r.method == "POST" {
                if let d = q["direct"] { AppProxy.setDebugDirect(d == "1") }
                if let p = q["pass"] { Prefs.proxyPass = p }
                if let l = q["ss"] {
                    do {
                        let c = try SsLink.parse(l)
                        Prefs.ssHost = c.host
                        Prefs.ssPort = c.port
                        Prefs.ssMethod = c.method
                        Prefs.ssKey = c.password
                        Prefs.proxyKind = "ss"
                    } catch { return .json(["ssError": error.localizedDescription]) }
                }
                if let k = q["kind"] { Prefs.proxyKind = k }
                if let o = q["on"] { Prefs.proxyOn = o == "1" }
                if q["apply"] == "1" { AppProxy.apply() }
                if q["break"] == "1" { AppProxy.debugBreak() } // giả iOS thu hồi ổ cắm nghe lúc app nằm nền
                if q["revive"] == "1" { AppProxy.revive() }
                if q["check"] == "1", Prefs.proxyKind == "ss" {
                    let c = SsConfig(host: Prefs.ssHost, port: Prefs.ssPort, method: Prefs.ssMethod, password: Prefs.ssKey,
                                     remoteDns: Prefs.proxyRemoteDns)
                    do { return .json(["check": try await AppProxy.check(c)]) } catch { return .json(["checkError": friendly(error, proxyNote: false)]) }
                }
                if q["check"] == "1" {
                    let c = SocksConfig(host: Prefs.proxyHost, port: Prefs.proxyPort, user: Prefs.proxyUser, pass: Prefs.proxyPass,
                                        remoteDns: Prefs.proxyRemoteDns)
                    do { return .json(["check": try await AppProxy.check(c)]) } catch { return .json(["checkError": friendly(error, proxyNote: false)]) }
                }
            }
            var o = AppProxy.debugState()
            o["spi"] = CBTWebKitSPIInfo()
            o["canProxy"] = WebSetup.canProxy
            o["webMode"] = WebSetup.proxyMode
            o["keychain"] = Secret.works
            o["unblock"] = Prefs.unblock ? Prefs.unblockLevel : 0
            o["disorderCount"] = Socks5Bridge.disorderCount
            o["savedPassword"] = !Prefs.proxyPass.isEmpty // mật khẩu proxy đã lưu còn đọc được (đổi quyền Keychain)
            return .json(o)
        case "/dnsblock":
            // Giả DNS của máy chặn (trả 0.0.0.0) để thử Vượt chặn: /dnsblock?hosts=a.com,b.com (trống = bỏ)
            let hosts = Set((q["hosts"] ?? "").split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).lowercased() }.filter { !$0.isEmpty })
            Doh.setDebugBlock(hosts)
            return .json(["hosts": hosts.sorted()])
        case "/dohfail":
            // Giả máy chủ DNS mã hoá bị chặn: /dohfail?labels=DoH NextDNS,DoT NextDNS (trống = bỏ); trả thứ tự máy chủ
            let labels = Set((q["labels"] ?? "").split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty })
            Doh.setDebugFail(labels)
            return .json(["servers": Doh.servers.map { $0.label + (labels.contains($0.label) ? " (giả chặn)" : "") }])
        case "/prefs":
            if r.method == "POST", let key = q["key"] {
                let v = q["value"] ?? ""
                switch q["type"] ?? "string" {
                case "int": Prefs.store.set(Int(v) ?? 0, forKey: key)
                case "double": Prefs.store.set(Double(v) ?? 0, forKey: key)
                case "bool": Prefs.store.set(v == "true" || v == "1", forKey: key)
                case "json": Prefs.store.set(Data(v.utf8), forKey: key) // vd follows, history (dữ liệu JSON)
                case "del": Prefs.store.removeObject(forKey: key)
                default: Prefs.store.set(v, forKey: key)
                }
            }
            return .json(Prefs.dump().mapValues { v -> Any in
                if let d = v as? Data { return (try? JSONSerialization.jsonObject(with: d)) ?? "<\(d.count) byte>" }
                return v
            })
        case "/upload":
            // Máy tính đẩy .ipa bản mới lên (máy tính không mở cổng cho iPad tải được — tường lửa)
            guard r.method == "POST", r.body.count > 1000 else { return .text("thân rỗng", status: 400) }
            do {
                try r.body.write(to: uploadFile, options: .atomic)
            } catch {
                return .text("không ghi được: \(error)", status: 500)
            }
            Log.d("Debug", "nhận \(r.body.count) byte .ipa")
            return .json(["ok": true, "bytes": r.body.count])
        case "/ipa":
            guard let d = try? Data(contentsOf: uploadFile) else { return .text("chưa có .ipa", status: 404) }
            return Response(status: 200, type: "application/octet-stream", body: d)
        case "/update":
            // Không có url: cài .ipa vừa đẩy lên, TrollStore tải từ chính app này (127.0.0.1)
            let ipa = q["url"] ?? "http://127.0.0.1:\(DebugServer.shared.port)/ipa"
            guard let u = UpdateCheck.trollStoreURL(ipa) else { return .text("url sai", status: 400) }
            keepServing()
            UIApplication.shared.open(u, options: [:]) { ok in Log.d("Debug", "mở TrollStore: \(ok)") }
            return .json(["ok": true, "open": u.absoluteString])
        default:
            return .text("không có \(r.path)", status: 404)
        }
    }

    // MARK: - Điều hướng

    private static func go(_ q: [String: String]) -> Response {
        let app = AppModel.shared
        switch q["to"] ?? "" {
        case "home":
            app.select(app.menu[0])
        case "menu":
            let t = q["title"] ?? ""
            guard let e = app.menu.first(where: { $0.title == t }) ?? app.menu.first(where: { $0.title.lowercased().hasPrefix(t.lowercased()) }) else {
                return .json(["ok": false, "menu": app.menu.map { $0.title }])
            }
            app.select(e)
        case "film":
            guard let u = q["url"] else { return .text("thiếu url", status: 400) }
            app.detailFilm = Film(url: u, title: q["title"] ?? u, poster: nil, badge: nil)
        case "play":
            guard let f = q["film"], let e = q["ep"] else { return .text("thiếu film / ep", status: 400) }
            Router.play(filmUrl: f, epUrl: e)
        case "resume":
            guard let f = q["film"] ?? Prefs.history.first?.filmUrl, let h = Prefs.historyFor(f) else { return .text("không có trong lịch sử", status: 400) }
            Router.play(filmUrl: h.filmUrl, epUrl: h.epUrl, continueMode: true)
        case "back":
            Router.back()
        case "search":
            if let e = app.menu.first(where: { $0.kind == .search }) { app.select(e) }
            app.searchText = q["q"] ?? ""
            app.doSearch()
        case "site":
            guard let u = q["url"] else { return .text("thiếu url", status: 400) }
            app.selectSite(u)
        case "settings": app.sheet = .settings
        case "sites": app.sheet = .sites
        case "browse":
            Router.browse(q["url"] ?? Prefs.baseUrl)
        case "follows":
            app.checkFollows(force: true)
        case "update":
            app.checkUpdate(manual: true)
        default:
            return .text("to=? (home, menu, film, play, resume, back, search, site, settings, sites, browse, follows, update)", status: 400)
        }
        return .json(["ok": true])
    }

    static func appState() -> String {
        switch UIApplication.shared.applicationState {
        case .active: return "active"
        case .inactive: return "inactive"
        case .background: return "background"
        @unknown default: return "?"
        }
    }

    private static func player(_ q: [String: String]) -> Response {
        guard let m = PlayerModel.current else { return .text("chưa mở trình phát", status: 400) }
        // App đang ở nền (người dùng sang app khác / tắt màn hình): không tự phát tiếng ở nền
        if ["play", "toggle"].contains(q["do"] ?? ""), UIApplication.shared.applicationState != .active, q["force"] != "1" {
            return .json(["ok": false, "why": "app đang ở nền (\(appState())) — thêm force=1 nếu thật sự muốn"])
        }
        let s = Double(q["s"] ?? "0") ?? 0
        switch q["do"] ?? "" {
        // Cử chỉ chạm (thay cho ngón tay): scrub&f=0.3 (vuốt ngang 30% bề ngang, end=0 thì giữ nguyên chưa nhấc tay)
        //   · hold&on=1|0 · level&kind=volume|brightness&v=0.4 · pinch&fill=1|0
        case "scrub":
            guard m.scrubBegan() else { return .json(["ok": false, "why": "chưa tua được"]) }
            m.scrubChanged(Double(q["f"] ?? "0") ?? 0)
            if q["end"] != "0" { m.scrubEnded(apply: true) }
        case "hold": return .json(["ok": m.holdFast(q["on"] != "0")])
        case "level":
            let v = Double(q["v"] ?? "0.5") ?? 0.5
            if q["kind"] == "brightness" { SystemLevels.shared.brightness = CGFloat(v) } else { SystemLevels.shared.volume = Float(v) }
            m.showLevel(brightness: q["kind"] == "brightness", v)
            return .json(["ok": true, "volume": SystemLevels.shared.volume, "brightness": Double(SystemLevels.shared.brightness)])
        case "levelEnd":
            m.levelEnded()
            SystemLevels.shared.restoreBrightness()
        case "pinch": m.setFill(q["fill"] == "1")
        case "toggle": m.togglePlay()
        case "play": m.play()
        case "pause": m.pause()
        case "seek": m.seekBy(s)
        case "seekTo": m.cmd("seekTo", s)
        case "next": m.next()
        case "prev": m.prev()
        case "menu": m.showPanel()
        case "hide": m.hidePanel()
        case "bar": m.showBar()
        case "close": m.close()
        case "reload": m.reload()
        case "server":
            guard let sv = m.servers.first(where: { $0.type == q["type"] || $0.label == q["type"] }) else {
                return .json(["ok": false, "servers": m.servers.map { $0.type }])
            }
            m.switchServer(sv)
        case "group":
            guard let g = m.groups.first(where: { $0.name == q["name"] }) else { return .json(["ok": false, "groups": m.groups.map { $0.name }]) }
            m.switchGroup(g)
        case "rate": m.cycleRate()
        case "fit": m.toggleFit()
        case "sleep": m.cycleSleep()
        case "introStart": m.markIntroStart()
        case "introEnd": m.markIntroEnd()
        case "introClear": m.clearIntro()
        case "tap": m.tapUp()
        case "confirm": m.tapConfirmed()
        case "double": m.doubleTap(left: q["left"] == "1")
        case "escape": m.escape()
        case "accept": m.acceptSuggestion()
        case "pip": m.startPip()
        case "unpip": m.stopPip()
        default: return .text("do=?", status: 400)
        }
        return .json(["ok": true, "state": m.debugState()])
    }

    private static func eval(target: String, js: String) async -> Response {
        let web: WKWebView? = target == "browser" ? BrowserController.current?.web : PlayerModel.current?.web
        guard let w = web else { return .text("không có WebView \(target)", status: 400) }
        let result: (Any?, Error?) = await withCheckedContinuation { cont in
            w.evaluateJavaScript(js) { v, e in cont.resume(returning: (v, e)) }
        }
        if let e = result.1 { return .json(["error": e.localizedDescription]) }
        return .json(["result": result.0 ?? NSNull()])
    }

    // MARK: - Chạy thử bộ đọc trang ngay trên iPad

    private static func probe(_ q: [String: String]) async -> Response {
        let url = q["url"] ?? Prefs.baseUrl
        let t0 = Date()
        func done(_ o: [String: Any]) -> Response {
            var r = o
            r["ms"] = Int(Date().timeIntervalSince(t0) * 1000)
            return .json(r)
        }
        do {
            switch q["fn"] ?? "" {
            case "home":
                let h = try await Site.home(url)
                return done(["sections": h.sections.map { section($0) }, "categories": h.categories.map { [$0.title, $0.url] },
                             "movedTo": h.movedTo ?? NSNull(), "next": NextSite.isKnown(url)])
            case "list":
                return done(page(try await Site.list(url)))
            case "search":
                return done(page(try await Site.search(q["q"] ?? "", base: url)))
            case "searchAll":
                return done(page(try await Site.searchAll(q["q"] ?? "")))
            case "detail":
                let d = try await Site.detail(url)
                return done([
                    "title": d.title, "other": d.otherTitle ?? NSNull(), "poster": d.poster ?? NSNull(), "info": d.info ?? NSNull(),
                    "desc": String((d.description ?? "").prefix(200)),
                    "groups": d.groups.map { g in
                        ["name": g.name, "count": g.episodes.count, "first": g.episodes.prefix(3).map { [$0.title, $0.url] },
                         "last": g.episodes.suffix(2).map { [$0.title, $0.url] }]
                    },
                ])
            case "source":
                let s = try await Site.source(url)
                return done(source(s))
            case "video":
                let s = try await Site.source(url)
                let server = s.servers.first { $0.type == q["server"] } ?? s.servers.first
                let v = try await Site.video(s, server)
                return done(["source": source(s), "server": server?.type ?? NSNull(), "url": v.url, "subtitle": v.subtitle ?? NSNull()])
            case "filters":
                let f = try await NextSite.filters(url)
                return done(["genres": f.genres.map { $0.title }, "countries": f.countries.map { $0.title }])
            default:
                return .text("fn=? (home, list, search, searchAll, detail, source, video, filters)", status: 400)
            }
        } catch {
            return done(["error": friendly(error), "raw": "\(error)"])
        }
    }

    private static func film(_ f: Film) -> [String: Any] {
        ["title": f.title, "url": f.url, "poster": f.poster ?? NSNull(), "badge": f.badge ?? NSNull()]
    }

    private static func section(_ s: FilmSection) -> [String: Any] {
        ["title": s.title, "count": s.films.count, "films": s.films.prefix(4).map { film($0) }]
    }

    private static func page(_ p: FilmPage) -> [String: Any] {
        ["count": p.films.count, "films": p.films.prefix(6).map { film($0) }, "next": p.nextUrl ?? NSNull(),
         "sections": p.sections.map { section($0) }]
    }

    private static func source(_ s: EpisodeSource) -> [String: Any] {
        ["servers": s.servers.map { [$0.label, $0.type] }, "pageMode": s.pageMode, "playerUrl": s.playerUrl ?? NSNull(),
         "direct": s.directEmbed ?? NSNull(), "postId": s.postId, "ep": s.ep, "sv": s.sv, "next": s.next,
         "links": s.links.mapValues { [$0.url, $0.subtitle ?? ""] }]
    }

    // MARK: - Màn hình

    /// Hộp thoại đang hiện ở bất kỳ cửa sổ nào (hộp «Có bản mới» nằm ở cửa sổ riêng, không phải cửa sổ chính).
    private static var topAlert: UIAlertController? {
        let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap { $0.windows }
        for w in windows.filter({ !$0.isHidden }).sorted(by: { $0.windowLevel > $1.windowLevel }) {
            var vc = w.rootViewController
            while let p = vc?.presentedViewController, !p.isBeingDismissed { vc = p }
            if let a = vc as? UIAlertController { return a }
        }
        return nil
    }

    private static var keyWindow: UIWindow? {
        let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap { $0.windows }
        return windows.first { $0.isKeyWindow } ?? windows.first
    }

    private static func screenshot(scale: CGFloat) -> Data? {
        guard let w = keyWindow else { return nil }
        let fmt = UIGraphicsImageRendererFormat()
        fmt.scale = max(0.2, min(3, scale))
        guard UIApplication.shared.applicationState == .active else { return nil }
        fmt.opaque = true
        var ok = false
        var img = UIGraphicsImageRenderer(bounds: w.bounds, format: fmt).image { _ in
            ok = w.drawHierarchy(in: w.bounds, afterScreenUpdates: false)
        }
        if !ok {
            img = UIGraphicsImageRenderer(bounds: w.bounds, format: fmt).image { _ in
                ok = w.drawHierarchy(in: w.bounds, afterScreenUpdates: true)
            }
        }
        return ok ? img.jpegData(compressionQuality: 0.75) : nil
    }

    private static func machine() -> String {
        var info = utsname()
        uname(&info)
        return withUnsafeBytes(of: &info.machine) { raw in
            String(decoding: raw.prefix { $0 != 0 }, as: UTF8.self)
        }
    }

    /// Duyệt cây trợ năng (gồm nút SwiftUI) — nhãn, giá trị, khung trên màn hình.
    private static func walk(_ o: NSObject, _ depth: Int, _ visit: (NSObject) -> Void) {
        if depth > 80 { return }
        if let v = o as? UIView, v.isHidden || v.alpha < 0.02 { return }
        if o.isAccessibilityElement { visit(o) }
        if let els = o.accessibilityElements {
            for e in els { if let n = e as? NSObject { walk(n, depth + 1, visit) } }
        } else {
            let n = o.accessibilityElementCount()
            if n != NSNotFound && n > 0 {
                for i in 0..<n { if let e = o.accessibilityElement(at: i) as? NSObject { walk(e, depth + 1, visit) } }
            }
        }
        if let v = o as? UIView { for s in v.subviews { walk(s, depth + 1, visit) } }
    }

    private static func uiDump() -> [[String: Any]] {
        var out: [[String: Any]] = []
        var seen = Set<ObjectIdentifier>()
        guard let w = keyWindow else { return out }
        walk(w, 0) { e in
            guard seen.insert(ObjectIdentifier(e)).inserted else { return }
            let f = e.accessibilityFrame
            out.append([
                "label": e.accessibilityLabel ?? "", "value": e.accessibilityValue ?? "",
                // Khung chưa xác định (NaN / vô cực, vd phần tử đang ẩn) → 0 — Int(NaN) làm sập app
                "frame": [f.minX, f.minY, f.width, f.height].map { $0.isFinite ? Int($0) : 0 },
                "button": e.accessibilityTraits.contains(.button), "type": String(describing: type(of: e)),
            ])
        }
        return out
    }

    /// Bấm phần tử theo nhãn: khớp đúng nhãn; partial = khớp một phần (dễ trúng nhầm mục khác cùng chữ).
    private static func press(label: String, index: Int, partial: Bool) -> (Bool, String?) {
        guard let w = keyWindow, !label.isEmpty else { return (false, nil) }
        // Hộp chọn (confirmationDialog / alert): bấm dòng theo tên — gọi khối xử lý của UIAlertAction (KVC, chỉ bản thử)
        if let alert = topAlert {
            let acts = alert.actions.filter { $0.title == label || (partial && ($0.title ?? "").localizedCaseInsensitiveContains(label)) }
            if let act = acts[safe: index] {
                typealias Handler = @convention(block) (UIAlertAction) -> Void
                let block = act.value(forKey: "handler")
                alert.dismiss(animated: true) {
                    if let b = block { unsafeBitCast(b as AnyObject, to: Handler.self)(act) }
                }
                return (true, act.title)
            }
        }
        var exact: [NSObject] = [], loose: [NSObject] = []
        var seen = Set<ObjectIdentifier>()
        walk(w, 0) { e in
            guard seen.insert(ObjectIdentifier(e)).inserted, let l = e.accessibilityLabel else { return }
            if l == label { exact.append(e) } else if partial && l.localizedCaseInsensitiveContains(label) { loose.append(e) }
        }
        guard let target = (exact + loose)[safe: index] else { return (false, nil) }
        let hit = target.accessibilityLabel
        if target.accessibilityActivate() { return (true, hit) }
        if let c = target as? UIControl {
            c.sendActions(for: .touchUpInside)
            return (true, hit)
        }
        return (false, hit)
    }

    private static func scroll(_ dir: String) -> Bool {
        guard let w = keyWindow else { return false }
        var best: UIScrollView?
        func find(_ v: UIView) {
            if v.isHidden || v.alpha < 0.02 { return }
            if let s = v as? UIScrollView, !(s is UITextView), s.contentSize.height > s.bounds.height + 1,
               s.bounds.width * s.bounds.height > (best.map { $0.bounds.width * $0.bounds.height } ?? 0) {
                best = s
            }
            v.subviews.forEach(find)
        }
        find(w)
        guard let s = best else { return false }
        let maxY = max(-s.adjustedContentInset.top, s.contentSize.height - s.bounds.height + s.adjustedContentInset.bottom)
        let page = s.bounds.height * 0.8
        var y = s.contentOffset.y
        switch dir {
        case "top": y = -s.adjustedContentInset.top
        case "bottom": y = maxY
        case "up": y -= page
        default: y += page
        }
        s.setContentOffset(CGPoint(x: s.contentOffset.x, y: min(maxY, max(-s.adjustedContentInset.top, y))), animated: false)
        return true
    }

    /// Cuộn ngang khung cuộn ngang đang hiện có chứa độ cao y (toạ độ cửa sổ).
    private static func scrollX(_ dir: String, y: CGFloat) -> Bool {
        guard let w = keyWindow else { return false }
        var hit: UIScrollView?
        func find(_ v: UIView) {
            if v.isHidden || v.alpha < 0.02 || hit != nil { return }
            if let s = v as? UIScrollView, s.contentSize.width > s.bounds.width + 1 {
                let f = s.convert(s.bounds, to: w)
                if f.minY <= y && y <= f.maxY { hit = s; return }
            }
            v.subviews.forEach(find)
        }
        find(w)
        guard let s = hit else { return false }
        let maxX = max(0, s.contentSize.width - s.bounds.width)
        var x = s.contentOffset.x
        switch dir {
        case "start": x = 0
        case "end": x = maxX
        case "left": x -= s.bounds.width * 0.8
        default: x += s.bounds.width * 0.8
        }
        s.setContentOffset(CGPoint(x: min(maxX, max(0, x)), y: s.contentOffset.y), animated: false)
        return true
    }
}
#endif
