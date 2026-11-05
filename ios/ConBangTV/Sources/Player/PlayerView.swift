import AVFoundation
import Combine
import MediaPlayer
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
            TouchLayer(m: m)
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
                        .appFont(17, .medium)
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
            if let h = m.gestureHint {
                GestureHintView(hint: h)
                    .allowsHitTesting(false)
            }
            if m.suggestion != nil {
                VStack {
                    Spacer()
                    SuggestionCard(m: m)
                        .padding(.bottom, m.barShown ? 190 : 40)
                }
            }
        }
        .overlay {
            if m.limitBlock != nil { LimitOverlay(m: m) }
        }
        .animation(.easeOut(duration: 0.18), value: m.barShown)
        .animation(.easeOut(duration: 0.2), value: m.panelShown)
        .preferredColorScheme(.dark)
        .appDynamicType()
    }
}

struct WebHost: UIViewRepresentable {
    let web: WKWebView
    func makeUIView(context: Context) -> WKWebView { web }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
}

// MARK: - Lớp chạm

struct TouchLayer: UIViewRepresentable {
    let m: PlayerModel

    func makeUIView(context: Context) -> TouchCatcher {
        let v = TouchCatcher()
        v.backgroundColor = .clear
        v.isMultipleTouchEnabled = true
        v.model = m
        return v
    }

    func updateUIView(_ v: TouchCatcher, context: Context) { v.model = m }
}

/// Bắt chạm thay cho trang web: chạm một lần (báo ngay lúc nhấc tay, rồi báo "chắc chắn" khi hết thời gian chờ chạm
/// đúp), chạm đúp nửa trái / phải (lùi / tới 10 giây). Cử chỉ: giữ tay — xem nhanh 2× tới khi nhấc tay; vuốt ngang — tua
/// (hiện giờ tới); vuốt dọc nửa trái — độ sáng, nửa phải — âm lượng; chụm / mở hai ngón — vừa / lấp đầy màn hình.
/// Vuốt dọc bắt đầu sát mép trên / dưới thì để cho iPad (Trung tâm điều khiển, vuốt lên thu nhỏ phim / về màn hình chính).
final class TouchCatcher: UIView, UIPointerInteractionDelegate {
    weak var model: PlayerModel? {
        didSet {
            guard model !== oldValue, let m = model else { return }
            // Thanh nút ẩn / hiện: con trỏ chuột ẩn / hiện theo
            barSub = m.$barShown.combineLatest(m.$panelShown).sink { [weak self] _ in
                DispatchQueue.main.async { self?.pointer?.invalidate() }
            }
        }
    }
    private var barSub: AnyCancellable?
    private var pointer: UIPointerInteraction?
    private var lastHover: CGPoint?
    private enum ScrollMode { case none, seek, volume }
    private var scrollMode = ScrollMode.none

    private enum Mode { case none, tap, hold, scrub, brightness, volume, ignored }
    private var mode = Mode.none
    private var downAt: CGPoint = .zero
    private var nearEdge = false
    private var startLevel: CGFloat = 0
    private var lastTap: TimeInterval = 0
    private var lastPoint: CGPoint = .zero
    private var pending: Task<Void, Never>?
    private var holdTask: Task<Void, Never>?

    override init(frame: CGRect) {
        super.init(frame: frame)
        addGestureRecognizer(UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:))))
        // Chuột / bàn di chuột: rê chuột thì hiện thanh nút, đứng yên (thanh ẩn) thì ẩn con trỏ; cuộn ngang = tua,
        // cuộn dọc = âm lượng
        addGestureRecognizer(UIHoverGestureRecognizer(target: self, action: #selector(hovered(_:))))
        let scroll = UIPanGestureRecognizer(target: self, action: #selector(scrolled(_:)))
        scroll.allowedScrollTypesMask = .all
        scroll.allowedTouchTypes = [] // chỉ sự kiện cuộn, không nhận chạm
        addGestureRecognizer(scroll)
        let p = UIPointerInteraction(delegate: self)
        addInteraction(p)
        pointer = p
    }

    func pointerInteraction(_ interaction: UIPointerInteraction, styleFor region: UIPointerRegion) -> UIPointerStyle? {
        guard let m = model, !m.barShown, !m.panelShown, m.started else { return nil }
        return .hidden()
    }

    @objc private func hovered(_ g: UIHoverGestureRecognizer) {
        guard let m = model else { return }
        let p = g.location(in: self)
        switch g.state {
        case .began:
            lastHover = p
        case .changed:
            // Rung tay nhẹ không tính
            if let l = lastHover, hypot(p.x - l.x, p.y - l.y) < 6 { return }
            lastHover = p
            if !m.panelShown && m.limitBlock == nil { m.showBar() }
        default:
            lastHover = nil
        }
    }

    @objc private func scrolled(_ g: UIPanGestureRecognizer) {
        guard let m = model else { return }
        let t = g.translation(in: self)
        switch g.state {
        case .changed:
            if scrollMode == .none {
                guard hypot(t.x, t.y) > 8, m.gesturesAllowed else { return }
                if abs(t.x) > abs(t.y) {
                    scrollMode = m.scrubBegan() ? .seek : .none
                } else {
                    scrollMode = .volume
                    startLevel = CGFloat(SystemLevels.shared.volume)
                }
            }
            switch scrollMode {
            case .seek: m.scrubChanged(Double(-t.x / max(1, bounds.width)))
            case .volume:
                let v = min(1, max(0, startLevel + t.y / max(1, bounds.height * 0.7)))
                SystemLevels.shared.volume = Float(v)
                m.showLevel(brightness: false, Double(v))
            case .none: break
            }
        case .ended, .cancelled, .failed:
            if scrollMode == .seek { m.scrubEnded(apply: g.state == .ended) }
            if scrollMode == .volume { m.levelEnded() }
            scrollMode = .none
        default:
            break
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("không dùng") }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil { SystemLevels.shared.attach(to: self) }
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        // Hai ngón trở lên: để cho cử chỉ chụm (không tính là chạm / vuốt)
        if (event?.allTouches?.count ?? 1) > 1 {
            finish(apply: false)
            mode = .ignored
            return
        }
        guard let t = touches.first else { return }
        downAt = t.location(in: self)
        nearEdge = downAt.y < 40 || downAt.y > bounds.height - 40
        mode = .tap
        holdTask?.cancel()
        holdTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 500_000_000)
            guard let self = self, !Task.isCancelled, self.mode == .tap, self.model?.holdFast(true) == true else { return }
            self.mode = .hold
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = touches.first, let m = model else { return }
        let p = t.location(in: self)
        let dx = p.x - downAt.x, dy = p.y - downAt.y
        switch mode {
        case .tap:
            guard hypot(dx, dy) > 24 else { return }
            holdTask?.cancel()
            if !m.gesturesAllowed {
                mode = .ignored
            } else if abs(dx) > abs(dy) {
                mode = m.scrubBegan() ? .scrub : .ignored
            } else if nearEdge {
                mode = .ignored
            } else if downAt.x < bounds.width / 2 {
                mode = .brightness
                startLevel = SystemLevels.shared.brightness
            } else {
                mode = .volume
                startLevel = CGFloat(SystemLevels.shared.volume)
            }
            if mode != .ignored { touchesMoved(touches, with: event) }
        case .scrub:
            m.scrubChanged(Double(dx / max(1, bounds.width)))
        case .brightness, .volume:
            // Vuốt hết ~70% chiều cao màn hình = từ 0 lên 100%
            let v = min(1, max(0, startLevel - dy / max(1, bounds.height * 0.7)))
            if mode == .brightness { SystemLevels.shared.brightness = v } else { SystemLevels.shared.volume = Float(v) }
            m.showLevel(brightness: mode == .brightness, Double(v))
        case .hold, .none, .ignored:
            break
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        let was = mode
        finish(apply: true)
        if was == .tap, let t = touches.first { tapped(t) }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { finish(apply: false) }

    /// Kết thúc cử chỉ đang làm: nhấc tay thì áp dụng (tua tới chỗ đã chọn), bị huỷ (iPad lấy chạm) thì bỏ.
    private func finish(apply: Bool) {
        holdTask?.cancel()
        let was = mode
        mode = .none
        switch was {
        case .hold: model?.holdFast(false)
        case .scrub: model?.scrubEnded(apply: apply)
        case .brightness, .volume: model?.levelEnded()
        case .tap, .none, .ignored: break
        }
    }

    private func tapped(_ t: UITouch) {
        guard let m = model else { return }
        let p = t.location(in: self)
        if t.timestamp - lastTap < 0.3 && hypot(p.x - lastPoint.x, p.y - lastPoint.y) < 80 {
            pending?.cancel()
            pending = nil
            lastTap = 0
            m.doubleTap(left: p.x < bounds.width / 2)
            return
        }
        lastTap = t.timestamp
        lastPoint = p
        m.tapUp()
        pending?.cancel()
        pending = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 300_000_000)
            guard let self = self, !Task.isCancelled else { return }
            self.pending = nil
            self.model?.tapConfirmed()
        }
    }

    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        guard g.state == .ended, let m = model, m.gesturesAllowed else { return }
        if g.scale > 1.15 { m.setFill(true) } else if g.scale < 0.87 { m.setFill(false) }
    }
}

/// Âm lượng hệ thống (qua thanh trượt của một MPVolumeView gần như trong suốt — có nó trên màn hình thì iPad không hiện
/// khung âm lượng che phim) và độ sáng màn hình cho cử chỉ vuốt dọc. Độ sáng trả lại như cũ khi rời trình phát.
@MainActor
final class SystemLevels {
    static let shared = SystemLevels()

    private let volumeView = MPVolumeView(frame: CGRect(x: 0, y: 0, width: 2, height: 2))
    private var savedBrightness: CGFloat?

    private init() {
        volumeView.alpha = 0.01
        volumeView.isUserInteractionEnabled = false
    }

    func attach(to v: UIView) {
        if volumeView.superview !== v { v.addSubview(volumeView) }
    }

    private var slider: UISlider? { volumeView.subviews.lazy.compactMap { $0 as? UISlider }.first }

    var volume: Float {
        get { slider?.value ?? AVAudioSession.sharedInstance().outputVolume }
        set { slider?.setValue(newValue, animated: false) }
    }

    var brightness: CGFloat {
        get { UIScreen.main.brightness }
        set {
            if savedBrightness == nil { savedBrightness = UIScreen.main.brightness }
            UIScreen.main.brightness = newValue
        }
    }

    /// Rời trình phát (đóng / ra màn hình chính): trả độ sáng như trước khi vuốt.
    func restoreBrightness() {
        guard let b = savedBrightness else { return }
        savedBrightness = nil
        UIScreen.main.brightness = b
    }
}

/// Màn «Hết giờ xem» (WatchLimit): người lớn nhập mã PIN phụ huynh rồi chọn cho xem thêm bao lâu (nút ngay trong
/// khung — hộp chọn xổ lên của iPad không hiện được trong lớp phủ trình phát), hoặc thoát trình phát.
private struct LimitOverlay: View {
    @ObservedObject var m: PlayerModel
    @State private var askPin = false
    @State private var choose = false

    var body: some View {
        ZStack {
            Color.black.opacity(0.82).ignoresSafeArea()
            VStack(spacing: 16) {
                Image(systemName: m.limitBlock == .bedtime ? "moon.zzz.fill" : "hourglass")
                    .appFont(46, .semibold)
                    .foregroundColor(Theme.focus)
                Text(choose ? "Cho \(Prefs.profile.name) xem thêm" : "Hết giờ xem").appFont(30, .bold)
                if choose {
                    VStack(spacing: 10) {
                        ForEach(WatchLimit.grants, id: \.self) { g in
                            grantButton("Cho xem thêm \(WatchLimit.dailyLabel(g))") { m.grantLimit(g) }
                        }
                        grantButton("Không giới hạn tới 6 giờ sáng") { m.grantLimit(nil) }
                        Button("Huỷ") { choose = false }
                            .buttonStyle(.bordered)
                            .padding(.top, 4)
                    }
                    .appFont(18, .semibold)
                } else {
                    Text(m.limitText + "\nNgười lớn nhập mã PIN phụ huynh để cho xem thêm.")
                        .appFont(17)
                        .foregroundColor(Theme.dim)
                        .multilineTextAlignment(.center)
                    HStack(spacing: 14) {
                        Button("Thoát") { m.close() }
                            .buttonStyle(.bordered)
                        Button("Cho xem thêm…") {
                            // Không có mã phụ huynh (không thể có giới hạn mà thiếu mã — phòng hờ): cho chọn luôn
                            if Prefs.hasParentPin { askPin = true } else { choose = true }
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(Theme.brand)
                    }
                    .appFont(18, .semibold)
                    .padding(.top, 6)
                }
            }
            .foregroundColor(Theme.text)
            .padding(34)
            .frame(maxWidth: 540)
            .background(RoundedRectangle(cornerRadius: 22).fill(Theme.surface))
            if askPin {
                ParentPinView(title: "Mã PIN phụ huynh để cho xem thêm", onCancel: { askPin = false }) {
                    askPin = false
                    choose = true
                }
            }
        }
    }

    private func grantButton(_ title: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).frame(maxWidth: 360).padding(.vertical, 4)
        }
        .buttonStyle(.borderedProminent)
        .tint(Theme.surface2)
    }
}

/// Chữ giữa màn hình lúc vuốt / giữ tay.
private struct GestureHintView: View {
    let hint: PlayerModel.GestureHint

    var body: some View {
        VStack(spacing: 6) {
            HStack(spacing: 12) {
                Image(systemName: hint.icon).appFont(26, .semibold)
                Text(hint.text).appFont(26, .semibold, mono: true)
            }
            if let d = hint.detail {
                Text(d).appFont(16, .medium, mono: true).foregroundColor(Theme.dim)
            }
        }
        .foregroundColor(Theme.text)
        .padding(.horizontal, 26)
        .padding(.vertical, 16)
        .background(RoundedRectangle(cornerRadius: 16).fill(Color.black.opacity(0.72)))
    }
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
                    .appFont(20, .medium)
                    .foregroundColor(Theme.text)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 40)
                Text("Chạm vào màn hình để hiện nút điều khiển (dừng / phát, tua, đổi tập, menu, thoát)  •  Chạm đúp trái / phải: lùi / tới 10 giây  •  Vuốt ngang: tua  •  Vuốt dọc bên trái: độ sáng, bên phải: âm lượng  •  Giữ tay: xem nhanh 2×  •  Chụm / mở hai ngón: vừa / lấp đầy màn hình")
                    .appFont(14)
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
                    .appFont(18, .semibold)
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
                        .appFont(15, .medium, mono: true)
                        .foregroundColor(Theme.text)
                    Slider(value: $value, in: 0...1, onEditingChanged: { isEditing in
                        editing = isEditing
                        m.dragging = isEditing
                        if !isEditing { m.seek(toFraction: value) }
                    })
                    .disabled(!(m.started && duration > 0))
                    .tint(Theme.brand)
                    Text(duration > 0 ? formatTime(duration) : "--:--")
                        .appFont(15, .medium, mono: true)
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
                .appFont(big ? 30 : 22, .semibold)
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
                                .appFont(19, .semibold)
                                .foregroundColor(Theme.text)
                            Spacer()
                            Button { m.hidePanel() } label: {
                                Image(systemName: "xmark.circle.fill").appFont(28).foregroundColor(Theme.dim)
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
        Text(text).appFont(14, .medium).foregroundColor(Theme.dim)
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
                if let icon = icon { Image(systemName: icon).appFont(15, .semibold) }
                Text(text).appFont(15, .medium).lineLimit(2).multilineTextAlignment(.leading)
            }
            .foregroundColor(active ? Theme.onBrand : Theme.text)
            .padding(.horizontal, 14)
            .padding(.vertical, 11)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 10).fill(active ? Theme.brand : Theme.surface2))
            .opacity(enabled ? 1 : 0.45)
        }
        .buttonStyle(PressScale())
        .contentShape(.hoverEffect, RoundedRectangle(cornerRadius: 10))
        .hoverEffect(.highlight)
    }
}

private struct SuggestionCard: View {
    @ObservedObject var m: PlayerModel

    var body: some View {
        VStack(spacing: 14) {
            Text(m.suggestionText)
                .appFont(17, .medium)
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
        model.onPlayingChanged = { [weak self] in self?.setNeedsUpdateOfScreenEdgesDeferringSystemGestures() }
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

    /// Đang bật "vuốt lên thì thu nhỏ phim" và phim đang phát: lần vuốt đầu từ mép dưới thuộc về app (thu nhỏ phim lúc app
    /// còn mở hẳn — iPad chỉ cho vào hình trong hình lúc đó), vuốt tiếp mới về màn hình chính. Phim dừng / chưa chạy thì
    /// vuốt lên về thẳng màn hình chính như thường.
    override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge {
        Prefs.pipOnHome && PlayerModel.pipSupported && model.playing ? .bottom : []
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
            SystemLevels.shared.restoreBrightness()
            NotificationCenter.default.removeObserver(self)
            onDismissed()
        }
    }

    @objc private func didEnterBackground() {
        SystemLevels.shared.restoreBrightness()
        model.didEnterBackground()
    }

    /// Phím tắt; có tên thì hiện trong bảng phím tắt của iPad (giữ phím ⌘).
    private static let keys: [(input: String, title: String?)] = [
        (" ", "Dừng / phát"), ("k", nil), ("\r", nil),
        (UIKeyCommand.inputLeftArrow, "Lùi (giữ để lùi nhanh)"), ("j", nil),
        (UIKeyCommand.inputRightArrow, "Tới (giữ để tới nhanh)"), ("l", nil),
        (UIKeyCommand.inputUpArrow, "Hiện thanh thời gian"), ("i", nil),
        (UIKeyCommand.inputDownArrow, "Menu: server, tập, tốc độ…"), ("m", nil),
        ("n", "Tập sau"), (UIKeyCommand.inputPageDown, nil),
        ("p", "Tập trước"), (UIKeyCommand.inputPageUp, nil),
        ("f", "Vừa / lấp đầy màn hình"), ("s", "Đổi tốc độ phát"), ("w", "Thu nhỏ phim (hình trong hình)"),
        (UIKeyCommand.inputEscape, "Thoát"),
    ] + (0...9).map { (String($0), $0 == 0 ? "Số 0–9: tới 0–90% phim" : nil) }

    override var keyCommands: [UIKeyCommand]? {
        var list = Self.keys.map { k -> UIKeyCommand in
            let c = UIKeyCommand(input: k.input, modifierFlags: [], action: #selector(key(_:)))
            if let t = k.title {
                c.title = t
                c.discoverabilityTitle = t
            }
            c.wantsPriorityOverSystemBehavior = true
            return c
        }
        let close = UIKeyCommand(input: "w", modifierFlags: .command, action: #selector(closeKey))
        close.title = "Thoát trình phát"
        close.discoverabilityTitle = "Thoát trình phát"
        list.append(close)
        return list
    }

    @objc private func closeKey() { model.close() }

    @objc private func key(_ c: UIKeyCommand) {
        guard let input = c.input else { return }
        // Đang hiện màn hết giờ xem: phím không điều khiển phim (Esc = thoát)
        if model.limitBlock != nil {
            if input == UIKeyCommand.inputEscape { model.close() }
            return
        }
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
        case "f": model.setFill(!Prefs.videoFill)
        case "s":
            model.cycleRate()
            model.showNotice("Tốc độ \(model.rateText)", 1.2)
        case "w": if PlayerModel.pipSupported { model.startPip() }
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
