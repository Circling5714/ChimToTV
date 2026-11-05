import AVKit
import UIKit
import WebKit

/// Phát một tập phim (bản Android: PlayerActivity.kt — giữ y logic).
///
/// Trình phát của trang chỉ chạy khi nằm trong iframe của chính trang phim, nên app dựng trang bọc (player.html) với
/// origin = địa chỉ tập (loadHTMLString + baseURL), đặt trình phát vào iframe; player-agent.js được WKWebView chèn vào đầu
/// mọi khung (WKUserScript, cả iframe khác tên miền). Agent nhận lệnh qua postMessage, báo trạng thái lên trang bọc;
/// app đọc trạng thái mỗi giây. Link HLS (.m3u8) thì phát bằng hls.html + hls.js (bỏ quảng cáo chèn giữa phim).
@MainActor
final class PlayerModel: NSObject, ObservableObject, WKNavigationDelegate, WKUIDelegate {
    struct PlayState: Equatable {
        var t: Double
        var d: Double
        var paused: Bool
        var ended: Bool
        var waiting: Bool
        var ready: Bool
        var droppedFrames: Int
        var totalFrames: Int
    }

    /// Trình phát đang mở (cho cổng gỡ lỗi).
    static weak var current: PlayerModel?

    static let rates: [Double] = [1.0, 1.25, 1.5, 2.0, 0.75]
    /// Hẹn giờ tắt (phút); -1 = hết tập đang xem.
    static let sleepOptions = [0, 30, 60, 90, -1]

    let web: WKWebView
    let filmUrl: String
    private let startEpUrl: String
    private let continueMode: Bool

    /// Đóng trình phát (PlayerController gắn vào); mở trang web của tập (trên trình phát).
    var onClose: () -> Void = {}
    var onOpenWeb: (String) -> Void = { _ in }

    @Published private(set) var title = ""
    @Published private(set) var loading: String? = "Đang tải…"
    @Published private(set) var loadingIsError = false
    @Published private(set) var state: PlayState?
    @Published private(set) var started = false
    @Published private(set) var barShown = false
    @Published private(set) var panelShown = false
    @Published private(set) var notice: String?
    @Published private(set) var seekTarget: Double?
    @Published private(set) var servers: [Server] = []
    @Published private(set) var server: Server?
    @Published private(set) var groups: [EpisodeGroup] = []
    @Published private(set) var group: EpisodeGroup?
    @Published private(set) var suggestion: Server?
    @Published private(set) var suggestionText = ""
    @Published private(set) var rate = 1.0
    @Published private(set) var sleepChoice = 0
    @Published private(set) var sleepAt: Int64 = 0
    @Published private(set) var pageMode = false
    /// Đang kéo thanh thời gian (khỏi tự ẩn thanh, khỏi nhảy số).
    @Published var dragging = false {
        didSet { if !dragging { showBar() } }
    }

    private var detail: FilmDetail?
    private var episodes: [Episode] { group?.episodes ?? [] }
    private var index = 0
    private var source: EpisodeSource?
    private var token = 0
    /// Số thứ tự lần nạp trang phát (window.__chimtoLoad): trang cũ còn trả lời vài giây sau khi chuyển tập.
    private var pageLoad = 0
    private var embedUrl: String?
    private(set) var episodeUrl = ""
    private var hlsUrl: String?
    private var errShown = false
    private var lastAdAt: Int64 = 0
    private var resumeAt = 0
    /// Đã ra lệnh tua tới chỗ xem dở mà trình phát chưa tua xong — thời gian báo lúc đó là giây đầu tập, không lưu đè.
    private var resumePending = false
    private var resumeIssuedAt: Int64 = 0
    private var endedHandled = false
    private var hintShown = false
    private var loadStartedAt: Int64 = 0
    private var lastSave: Int64 = 0
    private var sleepGraceUntil: Int64 = 0
    /// Chỗ bắt đầu báo trước cho trình phát của lần nạp tới (chỗ xem dở / hết nhạc mở đầu).
    private var startHint = 0
    private var introDone = false
    private var session: PlaySession?
    private var suggested = false
    private var shownByTap = false
    /// Số lần tự bấm phát hộ người xem (trình phát JW trên iPad không tự tải trước video như trên Android).
    private var autoPlays = 0
    private var lastAutoPlay: Int64 = 0
    private var expectedURL = ""
    private var expectedHost: String?
    private var timers: [String: Task<Void, Never>] = [:]
    private var polling: Task<Void, Never>?
    private var logRelay: ScriptRelay?
    /// Khung con đã gặp trong lần nạp này: địa chỉ → khung (cho cổng gỡ lỗi, hình trong hình).
    private(set) var frames: [(url: String, frame: WKFrameInfo)] = []

    // MARK: Hình trong hình

    /// Đang ở cửa sổ hình trong hình của iPad (agent báo theo video).
    @Published private(set) var inPip = false
    /// Màn trình phát đã đóng lúc phim đang hình trong hình (người xem dùng màn khác của app / app khác): trình phát vẫn
    /// chạy tới khi phóng to (mở lại màn trình phát) hoặc đóng cửa sổ nhỏ.
    private(set) var detached = false
    static var detachedModel: PlayerModel?
    /// PlayerController đóng màn trình phát mà không huỷ trình phát.
    var onDetach: () -> Void = {}
    /// Phim bắt đầu / thôi đang phát (PlayerController: chỉ để lại lần vuốt đầu từ mép dưới khi đang phát).
    var onPlayingChanged: () -> Void = {}

    /// Đang phát thật (vuốt lên từ mép dưới thì thu nhỏ phim).
    var playing: Bool { started && state?.paused == false && !inPip }
    private var pipRequestedAt: Int64 = 0
    /// Phóng to từ cửa sổ nhỏ lúc app chưa hiện lên hẳn (đang ở màn hình chính iPad): đợi app hiện rồi mở màn trình phát.
    private var waitingToReattach = false
    private var begun = false
    /// Đổi người xem: đã lưu chỗ xem cho người cũ, không ghi gì nữa.
    private var saveDisabled = false

    init(filmUrl: String, epUrl: String, continueMode: Bool) {
        self.filmUrl = filmUrl
        startEpUrl = epUrl
        self.continueMode = continueMode
        let config = WebSetup.configuration()
        web = WebSetup.make(config)
        super.init()
        web.navigationDelegate = self
        web.uiDelegate = self
        web.scrollView.isScrollEnabled = false
        #if DEVTOOLS
        let relay = ScriptRelay { m in Log.d("JS", "\(m.body)") }
        config.userContentController.add(relay, name: "chimtoLog")
        logRelay = relay
        #endif
        PlayerModel.current = self
    }

    // MARK: - Mở phim / tập / server

    func begin() {
        guard !begun else { return } // mở lại màn trình phát của phim đang hình trong hình: trình phát vẫn chạy
        begun = true
        startPolling()
        if let cached = Site.cachedDetail(filmUrl) {
            start(cached, epUrl: startEpUrl)
            return
        }
        showLoading("Đang tải thông tin phim…")
        Task {
            do {
                start(try await Site.detail(filmUrl), epUrl: startEpUrl)
            } catch {
                showError("Không tải được phim.\n\(friendly(error))")
            }
        }
    }

    private func start(_ d: FilmDetail, epUrl: String) {
        detail = d
        // Phim đang theo dõi: đã vào xem thì không còn là "tập mới"
        if let latest = d.latestEpisode { Prefs.updateFollow(d.url, count: d.episodeCount, latest: latest.title, seen: true) }
        groups = d.groups
        guard let g = d.groupOf(epUrl) ?? d.groups.first else {
            showError("Phim này chưa có tập nào.")
            return
        }
        group = g
        let i = max(0, g.episodes.firstIndex { $0.url == epUrl } ?? 0)
        let next = continueMode && Prefs.finished(epUrl) && i + 1 < g.episodes.count
        openEpisode(next ? i + 1 : i)
    }

    func openEpisode(_ i: Int) {
        guard let d = detail, let ep = episodes[safe: i] else { return }
        index = i
        token += 1
        let t = token
        stopVideo()
        episodeUrl = ep.url
        source = nil
        servers = []
        let groupName = d.groups.count > 1 ? (group.map { " (\($0.name))" } ?? "") : ""
        title = "\(d.title) — \(ep.title)\(groupName)"
        resumeAt = Prefs.position(ep.url)
        introDone = false
        if !Prefs.privateMode {
            Prefs.addHistory(HistoryItem(filmUrl: d.url, title: d.title, poster: d.poster, epTitle: ep.title, epUrl: ep.url, time: nowMs()))
        }
        showLoading("Đang mở \(ep.title)…")
        Task {
            do {
                let src = try await Site.source(ep.url)
                guard t == token else { return }
                source = src
                servers = src.pageMode ? [] : src.servers
                if src.pageMode {
                    playPage(src)
                } else {
                    let host = Site.hostOf(src.epUrl)
                    let usual = src.servers.first { $0.type == Prefs.serverType(host) } ?? src.servers.first
                    let pick = PlayStats.choose(AppProxy.statsHost(host), src.servers, usual)
                    if let u = usual, let p = pick, p != u {
                        let why = PlayStats.score(AppProxy.statsHost(host), u.type).label ?? "hay giật"
                        showNotice("\(u.label) \(why) gần đây — mở \(p.label) (Menu để đổi server).", 6)
                    }
                    playServer(pick)
                }
            } catch {
                guard t == token else { return }
                showError("Không mở được \(ep.title).\n\(friendly(error))")
            }
        }
    }

    /// Mở thẳng trang tập: trang tự nạp trình phát, page-focus.js phóng nó ra toàn màn hình.
    private func playPage(_ src: EpisodeSource) {
        stopVideo()
        pageMode = true
        loadStartedAt = nowMs()
        session = PlaySession(host: AppProxy.statsHost(Site.hostOf(src.epUrl)), server: "page", openedAt: loadStartedAt)
        showLoading("Đang mở trang tập phim…")
        startHint = startFor()
        installScripts(hls: false, page: true)
        guard let u = URL(string: src.epUrl) else { return }
        expect(src.epUrl)
        web.load(URLRequest(url: u))
    }

    private func playServer(_ s: Server?) {
        guard let src = source else { return }
        server = s
        token += 1
        let t = token
        stopVideo()
        showLoading("Đang tải \(s?.label ?? "trình phát")…")
        Task {
            do {
                let v = try await Site.video(src, s)
                guard t == token else { return }
                loadStartedAt = nowMs()
                session = PlaySession(host: AppProxy.statsHost(Site.hostOf(src.epUrl)), server: s?.type ?? "embed", openedAt: loadStartedAt)
                startHint = startFor()
                Log.d("Player", "phát \(v.url) (server \(s?.type ?? "-"), báo trước \(startHint) s)")
                if URL(string: v.url)?.path.hasSuffix(".m3u8") == true {
                    hlsUrl = v.url
                    installScripts(hls: true, page: false)
                    loadHTML(Assets.hlsPage(src: v.url, start: startHint, sub: v.subtitle), base: src.epUrl)
                } else {
                    embedUrl = v.url
                    installScripts(hls: false, page: false)
                    loadHTML(Assets.wrapperPage(src: v.url), base: src.epUrl)
                }
            } catch {
                guard t == token else { return }
                showError("\(friendly(error))\nMở Menu để chọn server khác.")
                showPanel()
            }
        }
    }

    /// Script chèn vào trang phát: agent (mọi khung, kèm chỗ bắt đầu + số lần nạp), hls.js, phóng trình phát (trang tập).
    private func installScripts(hls: Bool, page: Bool) {
        let ucc = web.configuration.userContentController
        ucc.removeAllUserScripts()
        let prefix = "window.__chimtoStart=\(startHint);window.__chimtoLoad=\(pageLoad);\n"
        ucc.addUserScript(WKUserScript(source: prefix + Assets.agent, injectionTime: .atDocumentStart, forMainFrameOnly: false))
        ucc.addUserScript(WKUserScript(source: WebSetup.fillCss, injectionTime: .atDocumentStart, forMainFrameOnly: false))
        if hls { ucc.addUserScript(WKUserScript(source: Assets.hlsJs, injectionTime: .atDocumentStart, forMainFrameOnly: true)) }
        if page { ucc.addUserScript(WKUserScript(source: Assets.pageFocus, injectionTime: .atDocumentStart, forMainFrameOnly: true)) }
        #if DEVTOOLS
        if !page { ucc.addUserScript(WKUserScript(source: WebSetup.consoleHook, injectionTime: .atDocumentStart, forMainFrameOnly: true)) }
        #endif
    }

    private func loadHTML(_ html: String, base: String) {
        expect(base)
        web.loadHTMLString(html, baseURL: URL(string: base))
    }

    private func expect(_ url: String) {
        expectedURL = url
        expectedHost = Site.hostOf(url)
    }

    private func stopVideo() {
        savePosition()
        session?.save(nowMs())
        session = nil
        suggested = false
        dismissSuggestion()
        pageMode = false
        embedUrl = nil
        hlsUrl = nil
        errShown = false
        lastAdAt = 0
        state = nil
        started = false
        resumePending = false
        endedHandled = false
        hintShown = false
        autoPlays = 0
        lastAutoPlay = 0
        seekTarget = nil
        cancel("seek")
        pageLoad += 1
        frames.removeAll()
        expect("about:blank")
        if let blank = URL(string: "about:blank") { web.load(URLRequest(url: blank)) }
    }

    // MARK: - Server / bản / tập

    func serverText(_ s: Server) -> String {
        let host = source.flatMap { Site.hostOf($0.epUrl) }
        return PlayStats.score(AppProxy.statsHost(host), s.type).summary.map { "\(s.label)  ·  \($0)" } ?? s.label
    }

    var sessionText: String? { session?.describe() }

    /// Người xem chọn server khác: nhớ lựa chọn, xem tiếp từ chỗ đang xem.
    func switchServer(_ s: Server) {
        guard let src = source else { return }
        Prefs.setServer(Site.hostOf(src.epUrl), type: s.type, label: s.label)
        if let t = currentTime() { resumeAt = t }
        hidePanel()
        playServer(s)
    }

    /// Sang bản khác (Vietsub / Thuyết Minh…): mở tập cùng tên và xem tiếp đúng chỗ; bản đó chưa có tập này thì mở tập
    /// cùng vị trí / tập cuối của bản đó, không mang giờ sang.
    func switchGroup(_ g: EpisodeGroup) {
        hidePanel()
        guard let d = detail, g != group, !g.episodes.isEmpty else { return }
        let cur = episodes[safe: index]
        let same = g.episodes.firstIndex { $0.title == cur?.title }
        let target = same ?? min(index, g.episodes.count - 1)
        if same != nil {
            if let t = currentTime() { Prefs.savePosition(g.episodes[target].url, seconds: t, filmUrl: d.url) }
        } else {
            showNotice("\(g.name) chưa có \(cur?.title ?? "tập này") — mở \(g.episodes[target].title).", 6)
        }
        group = g
        openEpisode(target)
    }

    func next() {
        if index + 1 < episodes.count { openEpisode(index + 1) } else { showNotice("Đây là tập mới nhất.") }
    }

    func prev() {
        if index > 0 { openEpisode(index - 1) } else { showNotice("Đây là tập đầu tiên.") }
    }

    func reload() {
        hidePanel()
        if let t = currentTime() { resumeAt = t }
        guard let src = source else {
            openEpisode(index)
            return
        }
        if src.pageMode { playPage(src) } else { playServer(server) }
    }

    func openWebPage() {
        hidePanel()
        cmd("pause")
        onOpenWeb(episodeUrl)
    }

    // MARK: - Server đang giật → gợi ý server khác (người xem chọn mới chuyển)

    private func suggestServer(_ now: Int64) {
        suggested = true // mỗi lần phát chỉ gợi ý một lần
        guard let src = source, !src.pageMode, let cur = server,
              let pair = PlayStats.alternative(AppProxy.statsHost(Site.hostOf(src.epUrl)), src.servers, cur) else { return }
        let (alt, score) = pair
        suggestion = alt
        suggestionText = "\(cur.label) đang giật nhiều. Chuyển sang \(alt.label)\(score.summary.map { " (\($0))" } ?? "")?"
        after("suggest", 15) { [weak self] in self?.dismissSuggestion() }
    }

    func acceptSuggestion() {
        guard let alt = suggestion else { return }
        dismissSuggestion()
        switchServer(alt)
    }

    func dismissSuggestion() {
        suggestion = nil
        cancel("suggest")
    }

    // MARK: - Trạng thái phát

    private func startPolling() {
        polling?.cancel()
        polling = Task { [weak self] in
            while !Task.isCancelled {
                self?.poll()
                try? await Task.sleep(nanoseconds: 1_000_000_000)
            }
        }
    }

    private func poll() {
        web.evaluateJavaScript("[window.__chimtoLoad||0,window.__chimtoState||null]") { [weak self] result, _ in
            Task { @MainActor in self?.onState(result) }
        }
    }

    private static func num(_ v: Any?) -> Double { (v as? NSNumber)?.doubleValue ?? 0 }
    private static func flag(_ v: Any?) -> Bool { (v as? NSNumber)?.boolValue ?? false }

    private func onState(_ raw: Any?) {
        // [lần nạp trang, trạng thái]: trạng thái của trang tập cũ (còn trả lời ngay sau khi chuyển tập) thì bỏ
        var o: [String: Any]?
        if let arr = raw as? [Any], arr.count == 2, (arr[0] as? NSNumber)?.intValue == pageLoad { o = arr[1] as? [String: Any] }
        let now = nowMs()
        let inAd = Self.flag(o?["ad"])
        if inAd { lastAdAt = now }
        // Mở 25 giây mà video chưa chạy. Không tính lúc quảng cáo, không đè lên thông báo lỗi server.
        let waited = now - max(loadStartedAt, lastAdAt)
        if !started && !errShown && !hintShown && !inAd && (embedUrl != nil || hlsUrl != nil || pageMode) && waited > 25_000 {
            hintShown = true
            hideLoading()
            let other = pageMode ? "Menu → Tải lại / Mở trang web" : "Menu để đổi server"
            showNotice("Video chưa chạy? Chạm ▶ để thử phát, hoặc \(other).", 10)
        }
        guard let o = o else { return }
        if inAd {
            // Trình phát đang chạy quảng cáo (agent tự bỏ qua khi được phép) — chưa phải phim
            session?.ad()
            if !started { showLoading("Trình phát đang chạy quảng cáo — tự bỏ qua khi được phép…") }
            return
        }
        let s = PlayState(t: Self.num(o["t"]), d: Self.num(o["d"]), paused: Self.flag(o["paused"]), ended: Self.flag(o["ended"]),
                          waiting: Self.flag(o["waiting"]), ready: Self.flag(o["ready"]),
                          droppedFrames: Int(Self.num(o["df"])), totalFrames: Int(Self.num(o["tf"])))
        let wasPaused = state?.paused
        let wasPlaying = playing
        if state != s { state = s }
        let pip = Self.flag(o["pip"])
        if pip != inPip { pipChanged(pip) }
        let err = (o["err"] as? String) ?? ""
        if !err.isEmpty && !errShown {
            errShown = true
            session?.fail()
            if started {
                showNotice("Mất kết nối video (\(err)).\nMenu → Tải lại, hoặc chọn server khác." + AppProxy.videoNote(), 10)
            } else {
                showError("\(server?.label ?? "Server") không phát được video (\(err)).\nMở Menu để chọn server khác." + AppProxy.videoNote())
                showPanel()
            }
        }
        // Trình phát JW trên iPad (Safari) không tự tải trước video khi chưa ai bấm phát — khác Android (agent chỉ tự
        // phát khi video đã có dữ liệu). Còn dừng, chưa có dữ liệu sau vài giây → bấm ▶ hộ người xem, tối đa 4 lần.
        if !started && s.paused && !s.ready && !errShown && now - loadStartedAt > 2500 && autoPlays < 4 && now - lastAutoPlay > 4000 {
            autoPlays += 1
            lastAutoPlay = now
            Log.d("Player", "trình phát đứng chờ — tự bấm phát (lần \(autoPlays))")
            cmd("play")
        }
        if let sess = session {
            sess.update(now, paused: s.paused, waiting: s.waiting, ready: s.ready, droppedFrames: s.droppedFrames, totalFrames: s.totalFrames)
            if !suggested && sess.struggling(now) { suggestServer(now) }
        }
        // Bắt đầu khi video thật sự chạy (không phải lúc mới nạp xong mà còn dừng)
        if !started && s.ready && !s.paused {
            started = true
            Log.d("Player", "video chạy t=\(Int(s.t)), báo trước \(startHint), \(now - loadStartedAt) ms")
            hideLoading()
            if resumeAt > 30 && (s.d <= 0 || Double(resumeAt) < s.d - 30) {
                introDone = true // xem tiếp đúng chỗ, không tua thêm qua nhạc mở đầu
                if abs(s.t - Double(resumeAt)) > 10 {
                    resumePending = true
                    resumeIssuedAt = now
                    session?.seeked()
                    cmd("seekTo", Double(resumeAt))
                }
                showNotice("Xem tiếp từ \(formatTime(Double(resumeAt)))")
            } else if startHint > 0 && s.t >= Double(startHint - 10) {
                introDone = true
                showNotice("Đã bỏ qua nhạc mở đầu (tới \(formatTime(Double(startHint)))) — lùi lại để xem.")
            }
            if rate != 1.0 { cmd("rate", rate) }
            if Prefs.videoFill { cmd("fit", 1) }
            showBar()
        }
        if sleepAt > 0 && sleepAt <= now { sleepNow() }
        if sleepGraceUntil > 0 && sleepGraceUntil <= now {
            sleepGraceUntil = 0
            exitForSleep()
            return
        }
        if playing != wasPlaying { onPlayingChanged() }
        if resumePending && (s.t >= Double(resumeAt - 5) || now - resumeIssuedAt > 30_000) { resumePending = false }
        if started && !introDone && !s.paused { skipIntro(s) }
        if let w = wasPaused, w != s.paused { showBar() }
        if started && now - lastSave > 5000 {
            lastSave = now
            savePosition()
        }
        if s.ended && !endedHandled {
            endedHandled = true
            if Self.sleepOptions[sleepChoice] < 0 {
                sleepNow()
            } else if Prefs.autoNext && index + 1 < episodes.count {
                showNotice("Hết tập — chuyển sang \(episodes[index + 1].title)")
                openEpisode(index + 1)
            }
        }
    }

    // MARK: - Nhạc mở đầu

    private var introKey: String { detail?.url ?? filmUrl }

    /// Phim đã đánh dấu nhạc mở đầu: video vừa chạy tới mốc đầu (trễ tối đa 8 giây) thì tua tới mốc hết, một lần mỗi tập.
    private func skipIntro(_ s: PlayState) {
        guard let mark = Prefs.intro(introKey), mark.end > mark.start, seekTarget == nil else { return }
        if s.t >= Double(mark.start + 8) || s.t >= Double(mark.end - 3) {
            introDone = true
            return
        }
        if s.t < Double(mark.start - 1) || (s.d > 0 && Double(mark.end) > s.d - 30) { return }
        introDone = true
        session?.seeked()
        cmd("seekTo", Double(mark.end))
        showNotice("Đã bỏ qua nhạc mở đầu (tới \(formatTime(Double(mark.end)))) — lùi lại để xem.")
    }

    /// Chỗ bắt đầu báo trước cho lần nạp trang này: chỗ xem dở; hoặc hết nhạc mở đầu nếu nhạc nằm ngay đầu tập.
    private func startFor() -> Int {
        if resumeAt > 30 { return resumeAt }
        guard let mark = Prefs.intro(introKey) else { return 0 }
        return (!introDone && mark.start <= 1 && mark.end > mark.start) ? mark.end : 0
    }

    var introText: String {
        guard let mark = Prefs.intro(introKey) else { return "chưa đánh dấu — đang ở đầu / cuối nhạc thì chạm nút tương ứng" }
        if mark.end < 0 { return "bắt đầu \(formatTime(Double(mark.start))), chưa đánh dấu chỗ hết" }
        return "\(formatTime(Double(mark.start))) → \(formatTime(Double(mark.end))), tự bỏ qua ở các tập"
    }

    var hasIntro: Bool { Prefs.intro(introKey) != nil }

    var nowSeconds: Int { Int(state?.t ?? 0) }

    func markIntroStart() {
        hidePanel()
        let t = nowSeconds
        let end = Prefs.intro(introKey).flatMap { $0.end > t ? $0.end : nil } ?? -1
        Prefs.setIntro(introKey, start: t, end: end)
        showNotice(end > t ? "Nhạc mở đầu: \(formatTime(Double(t))) → \(formatTime(Double(end))), các tập sẽ tự bỏ qua."
                   : "Đã đánh dấu đầu nhạc mở đầu. Hết nhạc thì mở Menu → «Hết nhạc ở …».", 6)
        objectWillChange.send()
    }

    func markIntroEnd() {
        hidePanel()
        let t = nowSeconds
        let start = Prefs.intro(introKey).flatMap { $0.start < t ? $0.start : nil } ?? 0
        Prefs.setIntro(introKey, start: start, end: t)
        introDone = true
        showNotice("Nhạc mở đầu: \(formatTime(Double(start))) → \(formatTime(Double(t))), các tập sẽ tự bỏ qua.", 6)
        objectWillChange.send()
    }

    func clearIntro() {
        hidePanel()
        Prefs.clearIntro(introKey)
        showNotice("Đã bỏ mốc nhạc mở đầu của phim này.")
        objectWillChange.send()
    }

    // MARK: - Tốc độ / khung hình / hẹn giờ tắt

    func cycleRate() {
        let i = Self.rates.firstIndex(of: rate) ?? 0
        rate = Self.rates[(i + 1) % Self.rates.count]
        cmd("rate", rate)
    }

    var rateText: String {
        (rate.truncatingRemainder(dividingBy: 1) == 0 ? String(Int(rate)) : String(rate).replacingOccurrences(of: ".", with: ",")) + "×"
    }

    func toggleFit() {
        Prefs.videoFill.toggle()
        cmd("fit", Prefs.videoFill ? 1 : 0)
        objectWillChange.send()
    }

    var fitText: String { Prefs.videoFill ? "Lấp đầy" : "Vừa màn hình" }

    func cycleSleep() {
        sleepChoice = (sleepChoice + 1) % Self.sleepOptions.count
        let m = Self.sleepOptions[sleepChoice]
        sleepAt = m > 0 ? nowMs() + Int64(m) * 60_000 : 0
    }

    var sleepText: String {
        if sleepAt > 0 { return "còn \(max(1, (sleepAt - nowMs()) / 60_000 + 1)) phút" }
        return Self.sleepOptions[sleepChoice] < 0 ? "hết tập này" : "Tắt"
    }

    /// Hết giờ hẹn: dừng phim, cho 60 giây chạm / bấm phím bất kỳ để xem tiếp.
    private func sleepNow() {
        sleepAt = 0
        sleepChoice = 0
        cmd("pause")
        sleepGraceUntil = nowMs() + 60_000
        showNotice("Hết giờ hẹn tắt. Chạm hoặc bấm phím bất kỳ trong 60 giây để xem tiếp, không thì app tự đóng trình phát.", 60)
    }

    /// Không ai bấm: lưu chỗ xem, đóng trình phát (iPad tự tắt màn hình như thường).
    private func exitForSleep() { close() }

    /// Có người chạm / bấm phím: huỷ đếm ngược sau giờ hẹn.
    func userActivity() {
        if sleepGraceUntil > 0 {
            sleepGraceUntil = 0
            showNotice("Đã huỷ hẹn giờ tắt — xem tiếp.")
        }
    }

    // MARK: - Điều khiển

    func togglePlay() {
        cmd(started ? "toggle" : "play")
        showBar()
    }

    func play() { cmd("play") }
    func pause() { cmd("pause") }

    /// Gộp các lần tua liên tiếp thành một lệnh tua (đỡ giật khi giữ phím / chạm nhiều lần).
    func seekBy(_ delta: Double) {
        guard let s = state, started else { return }
        let maxT = s.d > 0 ? s.d - 1 : Double.greatestFiniteMagnitude
        seekTarget = min(maxT, max(0, (seekTarget ?? s.t) + delta))
        showBar()
        after("seek", 0.5) { [weak self] in self?.flushSeek() }
    }

    /// Bước tua của phím ◀ ▶ (giữ phím: tăng dần, tối đa 4 lần).
    func step(repeatCount: Int) -> Double { Double(Prefs.seekStep) * min(4, 1 + Double(repeatCount) / 8) }

    private func flushSeek() {
        guard let target = seekTarget else { return }
        seekTarget = nil
        resumePending = false // người xem tự tua: từ giờ lấy theo trình phát
        session?.seeked()
        cmd("seekTo", target)
        state?.t = target
    }

    /// Kéo thanh thời gian xong (0…1).
    func seek(toFraction f: Double) {
        guard started, let d = state?.d, d > 0 else { return }
        let target = min(d - 1, max(0, d * f))
        resumePending = false
        session?.seeked()
        cmd("seekTo", target)
        state?.t = target
        showBar()
    }

    /// Phím số 0–9: nhảy tới 0–90%.
    func seekPercent(_ digit: Int) {
        guard let s = state, s.d > 0 else { return }
        seekTarget = s.d * Double(digit) / 10
        showBar()
        flushSeek()
    }

    func cmd(_ name: String, _ arg: Double? = nil) {
        let a = arg.map { String($0) } ?? "null"
        web.evaluateJavaScript("window.chimtoCmd&&chimtoCmd('\(name)',\(a))", completionHandler: nil)
    }

    /// Chỗ đang xem để mang sang (đổi server / tải lại / đổi bản): lệnh tua xem tiếp chưa xong thì vẫn là chỗ cũ.
    private func currentTime() -> Int? {
        if resumePending { return resumeAt }
        guard started, let s = state, s.t > 5 else { return nil }
        return Int(s.t)
    }

    func savePosition() {
        guard let s = state, started, !episodeUrl.isEmpty, !resumePending, !saveDisabled else { return }
        // Xem gần hết thì coi như xong (lần sau mở lại từ đầu)
        let done = s.ended || (s.d > 0 && s.t > s.d - 90)
        Prefs.savePosition(episodeUrl, seconds: done ? 0 : Int(s.t), filmUrl: detail?.url ?? filmUrl)
    }

    // MARK: - Chạm

    /// Nhấc tay: thanh nút hiện ngay (không chờ xem có chạm đúp không).
    func tapUp() {
        userActivity()
        if !panelShown && !barShown {
            showBar()
            shownByTap = true
        }
    }

    /// Chắc chắn là chạm một lần: thanh / menu đang hiện từ trước thì ẩn đi.
    func tapConfirmed() {
        if shownByTap {
            shownByTap = false
            return
        }
        if panelShown { hidePanel() } else { hideBar() }
    }

    func doubleTap(left: Bool) {
        shownByTap = false
        seekBy(left ? -10 : 10)
        showNotice(left ? "◀◀ 10 giây" : "10 giây ▶▶", 1.2)
        if !panelShown { showBar() }
    }

    // MARK: - Lớp phủ

    func showBar() {
        barShown = true
        cancel("bar")
        // Đang dừng thì giữ thanh trên màn hình
        if !(started && state?.paused == true) && !dragging {
            after("bar", 5) { [weak self] in self?.barShown = false }
        }
    }

    func hideBar() {
        cancel("bar")
        barShown = false
    }

    func showPanel() {
        hideBar()
        panelShown = true
    }

    func hidePanel() { panelShown = false }

    func showNotice(_ text: String, _ seconds: Double = 3.5) {
        notice = text
        after("notice", seconds) { [weak self] in self?.notice = nil }
    }

    private func showLoading(_ text: String) {
        loading = text
        loadingIsError = false
    }

    private func showError(_ text: String) {
        loading = text
        loadingIsError = true
    }

    private func hideLoading() { loading = nil }

    /// ESC / nút Quay lại: đóng menu / bỏ gợi ý / thoát.
    func escape() {
        if panelShown { hidePanel() } else if suggestion != nil { dismissSuggestion() } else { close() }
    }

    func close() {
        savePosition()
        session?.save(nowMs())
        session = nil
        onClose()
    }

    // MARK: - Hình trong hình

    static var pipSupported: Bool { AVPictureInPictureController.isPictureInPictureSupported() }

    /// Thu nhỏ phim (cửa sổ hình trong hình của iPad). Gọi thẳng trong khung có <video>: WebKit chỉ cho vào hình trong
    /// hình khi "người dùng vừa bấm" — evaluateJavaScript của app được tính là vậy, qua postMessage của trang bọc thì mất.
    func startPip() {
        guard started, !inPip else { return }
        pipRequestedAt = nowMs()
        hidePanel()
        hideBar()
        pipInFrames(true)
    }

    func stopPip() { pipInFrames(false) }

    private func pipInFrames(_ on: Bool) {
        let js = "(window.__chimtoHere&&window.__chimtoHere('pip',\(on ? 1 : 0)))||false"
        web.evaluateJavaScript(js, in: nil, in: .page) { r in Log.d("Pip", "khung chính: \(r)") }
        for f in frames {
            web.evaluateJavaScript(js, in: f.frame, in: .page) { r in Log.d("Pip", "\(f.url.prefix(60)): \(r)") }
        }
    }

    private func pipChanged(_ on: Bool) {
        inPip = on
        Log.d("Pip", on ? "vào hình trong hình" : "ra khỏi hình trong hình (\(state?.paused == true ? "đang dừng" : "đang phát"))")
        if on {
            // Đang dùng app (bấm Thu nhỏ): đóng màn trình phát, phim chạy tiếp ở cửa sổ nổi — dùng các màn khác được
            if UIApplication.shared.applicationState == .active && !detached {
                detached = true
                PlayerModel.detachedModel = self
                onDetach()
            }
        } else if detached {
            if state?.paused == false {
                reattach() // nút phóng to của cửa sổ nhỏ: mở lại màn trình phát (trình phát vẫn chạy)
            } else {
                closeDetached() // nút ✕ của cửa sổ nhỏ (phim dừng)
            }
        }
    }

    /// Mở lại màn trình phát cho phim vừa ra khỏi cửa sổ nhỏ. App chưa hiện lên hẳn (bấm phóng to lúc đang ở màn hình
    /// chính iPad — iOS mở app rồi mới hiện) thì đợi tới appBecameActive, không thì màn trình phát không mở được mà phim
    /// vẫn chạy ngầm.
    private func reattach() {
        guard UIApplication.shared.applicationState == .active else {
            waitingToReattach = true
            return
        }
        waitingToReattach = false
        detached = false
        PlayerModel.detachedModel = nil
        Router.present(self)
    }

    /// App vừa hiện lên (ConBangTVApp): đang đợi mở lại màn trình phát thì mở.
    func appBecameActive() {
        if waitingToReattach { reattach() }
    }

    /// Vuốt lên từ mép dưới màn hình lúc đang xem (PlayerController để lại lần vuốt đầu cho app): thu nhỏ phim — app vẫn
    /// đang mở hẳn nên iPad cho vào hình trong hình; vuốt tiếp mới về màn hình chính, phim vẫn nổi.
    func swipeUpFromEdge() {
        guard Prefs.pipOnHome, Self.pipSupported, started, state?.paused == false, !inPip, !detached else { return }
        Log.d("Pip", "vuốt lên từ mép dưới — thu nhỏ phim")
        startPip()
    }

    /// Đóng phim đang ở cửa sổ hình trong hình (màn trình phát đã đóng): lưu chỗ xem, huỷ trình phát.
    func closeDetached() {
        guard detached else { return }
        detached = false
        if PlayerModel.detachedModel === self { PlayerModel.detachedModel = nil }
        savePosition()
        session?.save(nowMs())
        session = nil
        teardown()
        AppModel.shared.playerClosed()
    }

    /// Đổi người xem: lưu chỗ xem cho người cũ rồi đóng trình phát (đang hình trong hình / đang mở).
    static func closeForProfileSwitch() {
        if let m = detachedModel {
            m.closeDetached()
            m.saveDisabled = true
        }
        if let c = Router.topController as? PlayerController {
            c.model.close()
            c.model.saveDisabled = true
        }
    }

    // MARK: - Vòng đời

    // Không tự thu nhỏ lúc app sắp ra nền (willResignActive): đo trên iPad 15.4 — lệnh tới đúng khung video nhưng iPad
    // không mở cửa sổ nổi khi app đã thôi "đang mở hẳn" (5/11/2026). Vì vậy dùng lần vuốt đầu từ mép dưới (swipeUpFromEdge).

    func didEnterBackground() {
        savePosition()
        session?.save(nowMs())
        // Hình trong hình (hoặc vừa bấm thu nhỏ, cửa sổ đang mở ra): phim chạy tiếp; không vào được thì dừng (khỏi phát
        // tiếng ở nền)
        if inPip { return }
        if nowMs() - pipRequestedAt < 3000 {
            after("pipcheck", 2.5) { [weak self] in
                guard let self = self, !self.inPip, UIApplication.shared.applicationState == .background else { return }
                Log.d("Pip", "không vào được hình trong hình — dừng phim")
                self.cmd("pause")
            }
            return
        }
        cmd("pause")
    }

    func teardown() {
        if PlayerModel.detachedModel === self { PlayerModel.detachedModel = nil }
        token += 1
        polling?.cancel()
        polling = nil
        for t in timers.values { t.cancel() }
        timers.removeAll()
        web.stopLoading()
        web.configuration.userContentController.removeAllUserScripts()
        web.configuration.userContentController.removeScriptMessageHandler(forName: "chimtoLog")
        if let blank = URL(string: "about:blank") { web.load(URLRequest(url: blank)) }
        if PlayerModel.current === self { PlayerModel.current = nil }
    }

    private func after(_ key: String, _ seconds: Double, _ body: @escaping @MainActor () -> Void) {
        timers[key]?.cancel()
        timers[key] = Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
            if Task.isCancelled { return }
            body()
        }
    }

    private func cancel(_ key: String) {
        timers[key]?.cancel()
        timers[key] = nil
    }

    // MARK: - WKNavigationDelegate / WKUIDelegate

    // Ghi rõ bộ chọn Objective-C: SDK mới gắn @MainActor @Sendable vào khối hoàn tất, chữ ký Swift lệch một chút
    // thì phương thức vẫn được WebKit gọi.

    /// Chặn mọi điều hướng cả trang do trang tự gây ra (quảng cáo); app tự nạp trang bằng code.
    /// Chuyển hướng trong cùng trang (vd http → https của trang tập) thì cho qua.
    @objc(webView:decidePolicyForNavigationAction:decisionHandler:)
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
                 decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        // Nhớ các khung con (iframe trình phát…) để cổng gỡ lỗi chạy JS ngay trong khung đó
        if let f = action.targetFrame, !f.isMainFrame, let u = action.request.url?.absoluteString {
            frames.removeAll { $0.url == u }
            frames.append((u, f))
            if frames.count > 40 { frames.removeFirst() }
        }
        let isMain = action.targetFrame?.isMainFrame ?? true
        guard isMain, let url = action.request.url else {
            decisionHandler(.allow)
            return
        }
        let s = url.absoluteString
        if url.scheme == "about" || s == expectedURL || (expectedHost != nil && Site.hostOf(s) == expectedHost) {
            decisionHandler(.allow)
            return
        }
        Log.d("Player", "chặn điều hướng cả trang: \(s)")
        decisionHandler(.cancel)
    }

    /// Không mở cửa sổ mới (popup quảng cáo).
    @objc(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:)
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                 for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        Log.d("Player", "chặn cửa sổ mới: \(navigationAction.request.url?.absoluteString ?? "")")
        return nil
    }

    @objc(webViewWebContentProcessDidTerminate:)
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        Log.d("Player", "tiến trình web bị tắt — nạp lại")
        reload()
    }

    @objc(webView:didFailNavigation:withError:)
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        Log.d("Player", "lỗi nạp trang: \(error.localizedDescription)")
    }

    @objc(webView:didFailProvisionalNavigation:withError:)
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        let e = error as NSError
        if e.code == NSURLErrorCancelled { return }
        Log.d("Player", "lỗi mở trang: \(e.localizedDescription)")
        if pageMode && !started { showError("Không mở được trang tập phim.\n\(friendly(error))") }
    }

    // MARK: - Gỡ lỗi

    /// Chạy JS trong khung con có địa chỉ chứa `match` (vd "streamfree"); trả (kết quả, lỗi).
    func evalInFrame(_ match: String, _ js: String) async -> (Any?, String?) {
        guard let f = frames.last(where: { $0.url.contains(match) })?.frame else {
            return (nil, "không thấy khung \(match) — có: \(frames.map { $0.url })")
        }
        return await withCheckedContinuation { cont in
            web.evaluateJavaScript(js, in: f, in: .page) { r in
                switch r {
                case .success(let v): cont.resume(returning: (v, nil))
                case .failure(let e): cont.resume(returning: (nil, e.localizedDescription))
                }
            }
        }
    }

    func debugState() -> [String: Any] {
        var o: [String: Any] = [
            "title": title, "episode": episodeUrl, "started": started, "pageMode": pageMode,
            "loading": loading ?? NSNull(), "notice": notice ?? NSNull(), "bar": barShown, "panel": panelShown,
            "server": server?.type ?? NSNull(), "servers": servers.map { ["label": $0.label, "type": $0.type] },
            "groups": groups.map { $0.name }, "group": group?.name ?? NSNull(),
            "embed": embedUrl ?? NSNull(), "hls": hlsUrl ?? NSNull(), "startHint": startHint, "resumeAt": resumeAt,
            "pageLoad": pageLoad, "rate": rate, "session": sessionText ?? NSNull(),
            "pip": inPip, "detached": detached,
        ]
        if let s = state {
            o["state"] = ["t": s.t, "d": s.d, "paused": s.paused, "ended": s.ended, "waiting": s.waiting, "ready": s.ready,
                          "df": s.droppedFrames, "tf": s.totalFrames]
        }
        return o
    }
}
