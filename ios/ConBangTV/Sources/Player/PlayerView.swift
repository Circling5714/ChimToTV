import SwiftUI
import UIKit
import WebKit

/// Màn xem phim: WebView trình phát + lớp chạm (trang web không nhận chạm — khỏi bấm nhầm quảng cáo) + thanh nút,
/// menu, thông báo. Thiết kế theo thanh cảm ứng của bản Android 1.9.3.
struct PlayerView: View {
    @ObservedObject var m: PlayerModel

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            WebHost(web: m.web).ignoresSafeArea()
            TouchLayer(onTapUp: { m.tapUp() }, onTapConfirmed: { m.tapConfirmed() }, onDoubleTap: { m.doubleTap(left: $0) })
                .ignoresSafeArea()
            if let text = m.loading {
                LoadingOverlay(text: text, isError: m.loadingIsError)
                    .allowsHitTesting(false)
            }
            if m.barShown && !m.panelShown {
                ControlBar(m: m)
                    .transition(.opacity)
            }
            if m.panelShown {
                MenuPanel(m: m)
                    .transition(.move(edge: .trailing))
            }
            VStack {
                if let n = m.notice {
                    Text(n)
                        .font(.system(size: 17, weight: .medium))
                        .multilineTextAlignment(.center)
                        .foregroundColor(Theme.text)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 12)
                        .background(RoundedRectangle(cornerRadius: 12).fill(Color.black.opacity(0.78)))
                        .padding(.top, 24)
                        .padding(.horizontal, 40)
                        .allowsHitTesting(false)
                }
                Spacer()
            }
            if m.suggestion != nil {
                VStack {
                    Spacer()
                    SuggestionCard(m: m)
                        .padding(.bottom, m.barShown ? 190 : 40)
                }
            }
        }
        .animation(.easeOut(duration: 0.18), value: m.barShown)
        .animation(.easeOut(duration: 0.2), value: m.panelShown)
        .preferredColorScheme(.dark)
    }
}

struct WebHost: UIViewRepresentable {
    let web: WKWebView
    func makeUIView(context: Context) -> WKWebView { web }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
}

// MARK: - Lớp chạm

struct TouchLayer: UIViewRepresentable {
    var onTapUp: @MainActor () -> Void
    var onTapConfirmed: @MainActor () -> Void
    var onDoubleTap: @MainActor (Bool) -> Void

    func makeUIView(context: Context) -> TouchCatcher {
        let v = TouchCatcher()
        v.backgroundColor = .clear
        v.isMultipleTouchEnabled = false
        return v
    }

    func updateUIView(_ v: TouchCatcher, context: Context) {
        v.onTapUp = onTapUp
        v.onTapConfirmed = onTapConfirmed
        v.onDoubleTap = onDoubleTap
    }
}

/// Bắt chạm thay cho trang web: chạm một lần (báo ngay lúc nhấc tay, rồi báo "chắc chắn" khi hết thời gian chờ chạm
/// đúp), chạm đúp nửa trái / phải.
final class TouchCatcher: UIView {
    var onTapUp: @MainActor () -> Void = {}
    var onTapConfirmed: @MainActor () -> Void = {}
    var onDoubleTap: @MainActor (Bool) -> Void = { _ in }

    private var downAt: CGPoint = .zero
    private var moved = false
    private var lastTap: TimeInterval = 0
    private var lastPoint: CGPoint = .zero
    private var pending: Task<Void, Never>?

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = touches.first else { return }
        downAt = t.location(in: self)
        moved = false
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = touches.first else { return }
        let p = t.location(in: self)
        if hypot(p.x - downAt.x, p.y - downAt.y) > 24 { moved = true }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = touches.first, !moved else { return }
        let p = t.location(in: self)
        if t.timestamp - lastTap < 0.3 && hypot(p.x - lastPoint.x, p.y - lastPoint.y) < 80 {
            pending?.cancel()
            pending = nil
            lastTap = 0
            onDoubleTap(p.x < bounds.width / 2)
            return
        }
        lastTap = t.timestamp
        lastPoint = p
        onTapUp()
        pending?.cancel()
        pending = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 300_000_000)
            guard let self = self, !Task.isCancelled else { return }
            self.pending = nil
            self.onTapConfirmed()
        }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { moved = true }
}

// MARK: - Màn chờ

private struct LoadingOverlay: View {
    let text: String
    let isError: Bool

    var body: some View {
        ZStack {
            Color.black.opacity(0.55).ignoresSafeArea()
            VStack(spacing: 18) {
                if !isError { ProgressView().scaleEffect(1.5).tint(Theme.text) }
                Text(text)
                    .font(.system(size: 20, weight: .medium))
                    .foregroundColor(Theme.text)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 40)
                Text("Chạm vào màn hình để hiện nút điều khiển (dừng / phát, tua, đổi tập, menu, thoát)  •  Chạm đúp trái / phải: lùi / tới 10 giây")
                    .font(.system(size: 14))
                    .foregroundColor(Theme.dim)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 60)
            }
        }
    }
}

// MARK: - Thanh nút

private struct ControlBar: View {
    @ObservedObject var m: PlayerModel
    @State private var value: Double = 0
    @State private var editing = false

    private var duration: Double { m.state?.d ?? 0 }
    private var shownTime: Double { editing ? value * duration : (m.seekTarget ?? m.state?.t ?? 0) }
    private var playing: Bool { m.started && m.state?.paused == false }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 14) {
                RoundButton(icon: "xmark", label: "Thoát") { m.close() }
                Text(m.title)
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundColor(Theme.text)
                    .lineLimit(1)
                Spacer()
            }
            .padding(.horizontal, 20)
            .padding(.top, 14)
            .padding(.bottom, 30)
            .background(LinearGradient(colors: [Color.black.opacity(0.75), .clear], startPoint: .top, endPoint: .bottom))

            Spacer()

            VStack(spacing: 14) {
                HStack(spacing: 14) {
                    Text(formatTime(shownTime))
                        .font(.system(size: 15, weight: .medium).monospacedDigit())
                        .foregroundColor(Theme.text)
                    Slider(value: $value, in: 0...1, onEditingChanged: { isEditing in
                        editing = isEditing
                        m.dragging = isEditing
                        if !isEditing { m.seek(toFraction: value) }
                    })
                    .disabled(!(m.started && duration > 0))
                    .tint(Theme.brand)
                    Text(duration > 0 ? formatTime(duration) : "--:--")
                        .font(.system(size: 15, weight: .medium).monospacedDigit())
                        .foregroundColor(Theme.dim)
                }
                HStack(spacing: 12) {
                    RoundButton(icon: "backward.end.fill", label: "Tập trước") { m.prev(); m.showBar() }
                    RoundButton(icon: "gobackward.10", label: "Lùi 10 giây") { m.seekBy(-10) }
                    RoundButton(icon: playing ? "pause.fill" : "play.fill", label: playing ? "Dừng" : "Phát", big: true) { m.togglePlay() }
                    RoundButton(icon: "goforward.10", label: "Tới 10 giây") { m.seekBy(10) }
                    RoundButton(icon: "forward.end.fill", label: "Tập sau") { m.next(); m.showBar() }
                    Spacer()
                    if PlayerModel.pipSupported {
                        RoundButton(icon: "pip.enter", label: "Thu nhỏ") { m.startPip() }
                            .disabled(!m.started)
                    }
                    RoundButton(icon: "list.bullet", label: "Menu") { m.showPanel() }
                }
            }
            .padding(.horizontal, 24)
            .padding(.top, 40)
            .padding(.bottom, 22)
            .background(LinearGradient(colors: [.clear, Color.black.opacity(0.85)], startPoint: .top, endPoint: .bottom))
        }
        .onAppear { syncValue() }
        .onChange(of: m.state) { _ in syncValue() }
        .onChange(of: m.seekTarget) { _ in syncValue() }
    }

    private func syncValue() {
        guard !editing else { return }
        let t = m.seekTarget ?? m.state?.t ?? 0
        value = duration > 0 ? min(1, max(0, t / duration)) : 0
    }
}

/// Nút tròn của thanh điều khiển (có nhãn cho trợ năng / cổng gỡ lỗi).
struct RoundButton: View {
    let icon: String
    let label: String
    var big = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: big ? 30 : 22, weight: .semibold))
                .foregroundColor(Theme.text)
                .frame(width: big ? 72 : 54, height: big ? 72 : 54)
                .background(Circle().fill(Color.white.opacity(big ? 0.22 : 0.14)))
        }
        .buttonStyle(PressScale())
        .accessibilityLabel(label)
    }
}

struct PressScale: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.92 : 1)
            .opacity(configuration.isPressed ? 0.7 : 1)
    }
}

// MARK: - Menu

private struct MenuPanel: View {
    @ObservedObject var m: PlayerModel
    private let cols = [GridItem(.adaptive(minimum: 150), spacing: 10, alignment: .leading)]

    var body: some View {
        GeometryReader { geo in
            HStack(spacing: 0) {
                Color.black.opacity(0.3)
                    .contentShape(Rectangle())
                    .onTapGesture { m.hidePanel() }
                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        HStack(alignment: .top) {
                            Text(m.title)
                                .font(.system(size: 19, weight: .semibold))
                                .foregroundColor(Theme.text)
                            Spacer()
                            Button { m.hidePanel() } label: {
                                Image(systemName: "xmark.circle.fill").font(.system(size: 28)).foregroundColor(Theme.dim)
                            }
                            .accessibilityLabel("Đóng menu")
                        }
                        LazyVGrid(columns: cols, alignment: .leading, spacing: 10) {
                            ChipButton(text: "Tập trước", icon: "backward.end") { m.hidePanel(); m.prev() }
                            ChipButton(text: "Tập sau", icon: "forward.end") { m.hidePanel(); m.next() }
                            ChipButton(text: "Tải lại", icon: "arrow.clockwise") { m.reload() }
                            ChipButton(text: "Danh sách tập", icon: "square.grid.2x2") { m.close() }
                            ChipButton(text: "Mở trang web", icon: "safari") { m.openWebPage() }
                            if PlayerModel.pipSupported {
                                ChipButton(text: "Thu nhỏ (hình trong hình)", icon: "pip.enter") { m.startPip() }
                                    .disabled(!m.started)
                            }
                        }
                        if !m.servers.isEmpty {
                            PanelLabel(m.sessionText.map { "Server — lần xem này: \($0)" } ?? "Server (điểm theo những lần xem trước trên máy này)")
                            LazyVGrid(columns: cols, alignment: .leading, spacing: 10) {
                                ForEach(m.servers) { s in
                                    ChipButton(text: m.serverText(s), active: s == m.server) { m.switchServer(s) }
                                }
                            }
                        }
                        if m.groups.count > 1 {
                            PanelLabel("Bản phim")
                            LazyVGrid(columns: cols, alignment: .leading, spacing: 10) {
                                ForEach(m.groups) { g in
                                    ChipButton(text: g.name, active: g == m.group) { m.switchGroup(g) }
                                }
                            }
                        }
                        PanelLabel("Nhạc mở đầu (cả phim): \(m.introText)")
                        LazyVGrid(columns: cols, alignment: .leading, spacing: 10) {
                            ChipButton(text: "Bắt đầu ở \(formatTime(Double(m.nowSeconds)))", icon: "music.note") { m.markIntroStart() }
                                .disabled(!m.started)
                            ChipButton(text: "Hết nhạc ở \(formatTime(Double(m.nowSeconds)))", icon: "forward") { m.markIntroEnd() }
                                .disabled(!m.started)
                            if m.hasIntro {
                                ChipButton(text: "Bỏ mốc", icon: "trash") { m.clearIntro() }
                            }
                        }
                        PanelLabel("Xem")
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 220), spacing: 10, alignment: .leading)],
                                  alignment: .leading, spacing: 10) {
                            ChipButton(text: "Tốc độ: \(m.rateText)", icon: "speedometer") { m.cycleRate() }
                            ChipButton(text: "Khung hình: \(m.fitText)", icon: "aspectratio") { m.toggleFit() }
                            ChipButton(text: "Hẹn giờ tắt: \(m.sleepText)", icon: "moon.zzz") { m.cycleSleep() }
                        }
                    }
                    .padding(22)
                }
                .frame(width: min(520, geo.size.width * 0.92))
                .background(Theme.surface.opacity(0.97).ignoresSafeArea())
            }
        }
    }
}

private struct PanelLabel: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).font(.system(size: 14, weight: .medium)).foregroundColor(Theme.dim)
    }
}

/// Nút dạng viên (menu, trang phim).
struct ChipButton: View {
    let text: String
    var icon: String?
    var active = false
    let action: () -> Void

    @Environment(\.isEnabled) private var enabled

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let icon = icon { Image(systemName: icon).font(.system(size: 15, weight: .semibold)) }
                Text(text).font(.system(size: 15, weight: .medium)).lineLimit(2).multilineTextAlignment(.leading)
            }
            .foregroundColor(active ? Theme.onBrand : Theme.text)
            .padding(.horizontal, 14)
            .padding(.vertical, 11)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 10).fill(active ? Theme.brand : Theme.surface2))
            .opacity(enabled ? 1 : 0.45)
        }
        .buttonStyle(PressScale())
    }
}

private struct SuggestionCard: View {
    @ObservedObject var m: PlayerModel

    var body: some View {
        VStack(spacing: 14) {
            Text(m.suggestionText)
                .font(.system(size: 17, weight: .medium))
                .foregroundColor(Theme.text)
                .multilineTextAlignment(.center)
            HStack(spacing: 12) {
                ChipButton(text: "Chuyển server", icon: "arrow.triangle.2.circlepath", active: true) { m.acceptSuggestion() }
                    .frame(width: 200)
                ChipButton(text: "Bỏ qua", icon: "xmark") { m.dismissSuggestion() }
                    .frame(width: 140)
            }
        }
        .padding(20)
        .background(RoundedRectangle(cornerRadius: 16).fill(Theme.surface.opacity(0.96)))
        .padding(.horizontal, 40)
    }
}

// MARK: - Bộ điều khiển (UIKit): toàn màn hình, phím tắt cho bàn phím rời

final class PlayerController: UIHostingController<PlayerView> {
    let model: PlayerModel
    var onDismissed: () -> Void = {}
    private var lastArrow: (input: String, at: TimeInterval, repeats: Int) = ("", 0, 0)

    init(model: PlayerModel) {
        self.model = model
        super.init(rootView: PlayerView(m: model))
        modalPresentationStyle = .fullScreen
        view.backgroundColor = .black
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("không dùng") }

    override var prefersStatusBarHidden: Bool { true }
    override var prefersHomeIndicatorAutoHidden: Bool { true }
    override var canBecomeFirstResponder: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        model.onClose = { [weak self] in self?.dismiss(animated: true) }
        // Phim vào hình trong hình lúc đang dùng app: đóng màn trình phát, trình phát vẫn chạy (PlayerModel.detachedModel)
        model.onDetach = { [weak self] in self?.dismiss(animated: true) }
        model.onOpenWeb = { [weak self] url in
            self?.present(BrowserController(url: url), animated: true)
        }
        NotificationCenter.default.addObserver(self, selector: #selector(didEnterBackground),
                                               name: UIApplication.didEnterBackgroundNotification, object: nil)
        // Vuốt lên từ mép dưới: lần đầu iPad để lại cho app (preferredScreenEdgesDeferringSystemGestures) → thu nhỏ phim
        let edge = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(bottomEdge(_:)))
        edge.edges = .bottom
        view.addGestureRecognizer(edge)
        model.begin()
    }

    /// Đang bật "vuốt lên thì thu nhỏ phim": lần vuốt đầu từ mép dưới thuộc về app (thu nhỏ phim lúc app còn mở hẳn — iPad
    /// chỉ cho vào hình trong hình lúc đó), vuốt tiếp mới về màn hình chính.
    override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge {
        Prefs.pipOnHome && PlayerModel.pipSupported ? .bottom : []
    }

    private var edgeFired = false

    @objc private func bottomEdge(_ g: UIScreenEdgePanGestureRecognizer) {
        switch g.state {
        case .began:
            edgeFired = false
        case .changed:
            // Vuốt lên hẳn (khỏi bắt nhầm lúc ngón tay chạm sát mép dưới)
            if !edgeFired && g.translation(in: view).y < -50 {
                edgeFired = true
                model.swipeUpFromEdge()
            }
        default:
            break
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        becomeFirstResponder()
        UIApplication.shared.isIdleTimerDisabled = true
        setNeedsUpdateOfScreenEdgesDeferringSystemGestures() // theo Cài đặt mới nhất (vuốt lên thì thu nhỏ phim)
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        if isBeingDismissed {
            UIApplication.shared.isIdleTimerDisabled = false
            // Đóng vì phim vào hình trong hình: trình phát chạy tiếp tới khi phóng to / đóng cửa sổ nhỏ
            if !model.detached { model.teardown() }
            NotificationCenter.default.removeObserver(self)
            onDismissed()
        }
    }

    @objc private func didEnterBackground() { model.didEnterBackground() }

    override var keyCommands: [UIKeyCommand]? {
        let inputs = [" ", "k", "j", "l", "m", "n", "p", "i", "\r", UIKeyCommand.inputLeftArrow, UIKeyCommand.inputRightArrow,
                      UIKeyCommand.inputUpArrow, UIKeyCommand.inputDownArrow, UIKeyCommand.inputEscape,
                      UIKeyCommand.inputPageUp, UIKeyCommand.inputPageDown] + (0...9).map { String($0) }
        return inputs.map { input in
            let c = UIKeyCommand(input: input, modifierFlags: [], action: #selector(key(_:)))
            c.wantsPriorityOverSystemBehavior = true
            return c
        }
    }

    @objc private func key(_ c: UIKeyCommand) {
        guard let input = c.input else { return }
        model.userActivity()
        if model.panelShown {
            if input == UIKeyCommand.inputEscape || input == "m" || input == UIKeyCommand.inputDownArrow { model.hidePanel() }
            return
        }
        if model.suggestion != nil {
            if input == "\r" { model.acceptSuggestion(); return }
            if input == UIKeyCommand.inputEscape { model.dismissSuggestion(); return }
        }
        switch input {
        case " ", "k", "\r": model.togglePlay()
        case UIKeyCommand.inputLeftArrow, "j": model.seekBy(-model.step(repeatCount: repeats(input)))
        case UIKeyCommand.inputRightArrow, "l": model.seekBy(model.step(repeatCount: repeats(input)))
        case UIKeyCommand.inputUpArrow, "i": model.showBar()
        case UIKeyCommand.inputDownArrow, "m": model.showPanel()
        case "n", UIKeyCommand.inputPageDown: model.next()
        case "p", UIKeyCommand.inputPageUp: model.prev()
        case UIKeyCommand.inputEscape: model.escape()
        default:
            if let d = Int(input), (0...9).contains(d) { model.seekPercent(d) }
        }
    }

    /// Giữ phím mũi tên: đếm số lần lặp (bước tua tăng dần).
    private func repeats(_ input: String) -> Int {
        let now = ProcessInfo.processInfo.systemUptime
        if lastArrow.input == input && now - lastArrow.at < 0.25 {
            lastArrow = (input, now, lastArrow.repeats + 1)
        } else {
            lastArrow = (input, now, 0)
        }
        return lastArrow.repeats
    }
}
