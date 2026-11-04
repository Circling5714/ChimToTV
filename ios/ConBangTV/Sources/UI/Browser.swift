import UIKit
import WebKit

/// Duyệt web trong app (bản Android: WebActivity — trên iPad chạm thẳng, không cần con trỏ ảo).
/// Chặn quảng cáo như trình phát; trang tự chuyển sang tên miền lạ (không do người xem bấm) thì chặn.
final class BrowserController: UIViewController, WKNavigationDelegate, WKUIDelegate, UITextFieldDelegate {
    private let startURL: String
    let web = WebSetup.make()
    private let bar = UIView()
    private let field = UITextField()
    private let progress = UIProgressView(progressViewStyle: .bar)
    private var backButton: UIButton!
    private var forwardButton: UIButton!
    private var observations: [NSKeyValueObservation] = []

    /// Trình duyệt đang mở (cho cổng gỡ lỗi).
    static weak var current: BrowserController?

    init(url: String) {
        startURL = url
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .fullScreen
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("không dùng") }

    override var preferredStatusBarStyle: UIStatusBarStyle { .lightContent }
    override var canBecomeFirstResponder: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        BrowserController.current = self
        view.backgroundColor = Theme.uiBg
        bar.backgroundColor = UIColor(red: 0x17 / 255, green: 0x1A / 255, blue: 0x21 / 255, alpha: 1)

        let close = button("xmark", "Đóng") { [weak self] in self?.dismiss(animated: true) }
        backButton = button("chevron.left", "Lùi trang") { [weak self] in self?.web.goBack() }
        forwardButton = button("chevron.right", "Tới trang") { [weak self] in self?.web.goForward() }
        let reload = button("arrow.clockwise", "Tải lại") { [weak self] in self?.web.reload() }

        field.borderStyle = .none
        field.backgroundColor = UIColor(white: 1, alpha: 0.08)
        field.layer.cornerRadius = 9
        field.textColor = .white
        field.font = .systemFont(ofSize: 16)
        field.keyboardType = .URL
        field.autocapitalizationType = .none
        field.autocorrectionType = .no
        field.returnKeyType = .go
        field.clearButtonMode = .whileEditing
        field.delegate = self
        field.leftView = UIView(frame: CGRect(x: 0, y: 0, width: 12, height: 10))
        field.leftViewMode = .always

        let stack = UIStackView(arrangedSubviews: [close, backButton, forwardButton, reload, field])
        stack.axis = .horizontal
        stack.spacing = 8
        stack.alignment = .center
        stack.translatesAutoresizingMaskIntoConstraints = false
        bar.translatesAutoresizingMaskIntoConstraints = false
        web.translatesAutoresizingMaskIntoConstraints = false
        progress.translatesAutoresizingMaskIntoConstraints = false
        progress.progressTintColor = UIColor(red: 0x2B / 255, green: 0xAF / 255, blue: 0x85 / 255, alpha: 1)
        bar.addSubview(stack)
        view.addSubview(web)
        view.addSubview(bar)
        view.addSubview(progress)
        NSLayoutConstraint.activate([
            bar.topAnchor.constraint(equalTo: view.topAnchor),
            bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            stack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 6),
            stack.bottomAnchor.constraint(equalTo: bar.bottomAnchor, constant: -6),
            stack.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 10),
            stack.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -10),
            field.heightAnchor.constraint(equalToConstant: 38),
            progress.topAnchor.constraint(equalTo: bar.bottomAnchor),
            progress.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            progress.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            web.topAnchor.constraint(equalTo: bar.bottomAnchor),
            web.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            web.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            web.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])

        web.navigationDelegate = self
        web.uiDelegate = self
        web.allowsBackForwardNavigationGestures = true
        observations = [
            web.observe(\.estimatedProgress, options: [.new]) { [weak self] w, _ in
                Task { @MainActor in
                    self?.progress.progress = Float(w.estimatedProgress)
                    self?.progress.isHidden = w.estimatedProgress >= 1
                }
            },
            web.observe(\.url, options: [.new]) { [weak self] w, _ in
                Task { @MainActor in self?.syncBar() }
            },
            web.observe(\.canGoBack, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in self?.syncBar() }
            },
            web.observe(\.canGoForward, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in self?.syncBar() }
            },
        ]
        go(startURL)
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        if isBeingDismissed {
            web.stopLoading()
            observations.removeAll()
        }
    }

    private func button(_ icon: String, _ label: String, _ action: @escaping () -> Void) -> UIButton {
        let b = UIButton(type: .system, primaryAction: UIAction { _ in action() })
        b.setImage(UIImage(systemName: icon, withConfiguration: UIImage.SymbolConfiguration(pointSize: 19, weight: .semibold)), for: .normal)
        b.tintColor = .white
        b.accessibilityLabel = label
        b.widthAnchor.constraint(equalToConstant: 44).isActive = true
        b.heightAnchor.constraint(equalToConstant: 44).isActive = true
        return b
    }

    private func syncBar() {
        if !field.isEditing { field.text = web.url?.absoluteString ?? startURL }
        backButton.isEnabled = web.canGoBack
        forwardButton.isEnabled = web.canGoForward
        backButton.alpha = web.canGoBack ? 1 : 0.35
        forwardButton.alpha = web.canGoForward ? 1 : 0.35
    }

    func go(_ text: String) {
        var s = text.trimmed
        if s.isEmpty { return }
        if !s.lowercased().hasPrefix("http://") && !s.lowercased().hasPrefix("https://") { s = "https://" + s }
        guard let u = URL(string: s) else { return }
        web.load(URLRequest(url: u))
        syncBar()
    }

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        textField.resignFirstResponder()
        go(textField.text ?? "")
        return true
    }

    override var keyCommands: [UIKeyCommand]? {
        let esc = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(escape))
        esc.wantsPriorityOverSystemBehavior = true
        return [esc, UIKeyCommand(input: "[", modifierFlags: .command, action: #selector(escape))]
    }

    /// ESC = Back: lùi trang, hết trang thì đóng.
    @objc private func escape() {
        if field.isFirstResponder {
            field.resignFirstResponder()
        } else if web.canGoBack {
            web.goBack()
        } else {
            dismiss(animated: true)
        }
    }

    // MARK: - Chặn quảng cáo chuyển trang / cửa sổ mới

    private func knownHost(_ host: String?) -> Bool {
        guard let h = host.flatMap({ Site.hostOf("https://\($0)") }) else { return false }
        if h == Site.hostOf(web.url?.absoluteString ?? "") || h == Site.hostOf(startURL) { return true }
        return Prefs.sites.contains { Site.hostOf($0) == h }
    }

    @objc(webView:decidePolicyForNavigationAction:decisionHandler:)
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
                 decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard action.targetFrame?.isMainFrame ?? true, let url = action.request.url else {
            decisionHandler(.allow)
            return
        }
        // Người xem tự bấm link / gửi biểu mẫu / lùi tới: cho qua. Trang tự chuyển (JS) sang tên miền lạ: quảng cáo → chặn.
        if action.navigationType != .other || url.scheme == "about" || knownHost(url.host) || webView.url == nil {
            decisionHandler(.allow)
            return
        }
        Log.d("Browser", "chặn trang tự chuyển sang \(url.absoluteString)")
        decisionHandler(.cancel)
    }

    @objc(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:)
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                 for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        // Link target=_blank người xem bấm: mở ngay trong trang này; cửa sổ tự bật (quảng cáo): bỏ
        if action.navigationType == .linkActivated || knownHost(action.request.url?.host) { webView.load(action.request) }
        return nil
    }
}
