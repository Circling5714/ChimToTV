import SwiftUI
import UIKit
import AVFoundation

@main
struct ConBangTVApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(AppModel.shared)
                .preferredColorScheme(.dark)
                .modifier(DisclaimerGate())
        }
        .onChange(of: phase) { p in
            Log.d("App", "trạng thái: \(p)")
            switch p {
            case .active:
                MacWindow.configure()
                AppProxy.revive() // vượt chặn / proxy: cổng nội bộ bị thu hồi lúc nằm nền thì mở lại
                PlayerModel.detachedModel?.appBecameActive() // phóng to từ cửa sổ nhỏ lúc đang ở màn hình chính iPad
                AppModel.shared.appReturned() // quay lại sau ≥ 30 phút: hỏi lại "Ai đang xem?"
                AppModel.shared.checkUpdate(manual: false)
                AppModel.shared.checkFollows()
                SyncService.shared.appActive() // đồng bộ trong nhà (đã ghép máy)
            case .background:
                AppModel.shared.appLeft()
                AutoClear.leftApp()
            default:
                break
            }
        }
    }
}

final class AppDelegate: UIResponder, UIApplicationDelegate {
    /// Thanh menu của bản Mac: bỏ các menu không dùng (định dạng chữ, mở cửa sổ mới — app chỉ có một cửa sổ).
    override func buildMenu(with builder: UIMenuBuilder) {
        super.buildMenu(with: builder)
        guard builder.system == .main else { return }
        builder.remove(menu: .format)
        builder.remove(menu: .newScene)
        builder.remove(menu: .toolbar)
    }

    func application(_ application: UIApplication,
                     didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        // Phim vẫn có tiếng khi gạt công tắc im lặng
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback)
        URLCache.shared = URLCache(memoryCapacity: 20 << 20, diskCapacity: 100 << 20, directory: nil)

        let surface = UIColor(red: 0x17 / 255, green: 0x1A / 255, blue: 0x21 / 255, alpha: 1)
        let nav = UINavigationBarAppearance()
        nav.configureWithOpaqueBackground()
        nav.backgroundColor = surface
        nav.titleTextAttributes = [.foregroundColor: UIColor.white]
        nav.largeTitleTextAttributes = [.foregroundColor: UIColor.white]
        UINavigationBar.appearance().standardAppearance = nav
        UINavigationBar.appearance().scrollEdgeAppearance = nav
        UINavigationBar.appearance().compactAppearance = nav
        UINavigationBar.appearance().tintColor = UIColor(red: 0x2B / 255, green: 0xAF / 255, blue: 0x85 / 255, alpha: 1)

        Prefs.migrateMovedHosts()
        Prefs.addNewPresets()
        Prefs.applyNetDefaults()
        AppProxy.apply()
        WebSetup.prepare()
        #if DEVTOOLS
        DebugServer.shared.start()
        #endif
        AutoClear.launched()
        #if targetEnvironment(macCatalyst)
        let os = "macOS \(ProcessInfo.processInfo.operatingSystemVersionString)"
        #else
        let os = "iOS \(UIDevice.current.systemVersion)"
        #endif
        Log.d("App", "mở app \(UpdateCheck.currentVersion), \(os), \(UIDevice.current.model)")
        return true
    }
}

/// Cửa sổ của bản Mac (Mac Catalyst): cỡ nhỏ nhất để giao diện iPad không vỡ, ẩn chữ tiêu đề (app nền tối, thanh tiêu đề trống
/// cho gọn). Trên iPad / iPhone không làm gì.
@MainActor
enum MacWindow {
    static let minimumSize = CGSize(width: 900, height: 600)

    static func configure() {
        #if targetEnvironment(macCatalyst)
        for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
            scene.title = "Côn Bằng TV"
            scene.sizeRestrictions?.minimumSize = minimumSize
            if let bar = scene.titlebar {
                bar.titleVisibility = .hidden
                bar.toolbar = nil
            }
        }
        #endif
    }
}

/// Tự xoá dữ liệu duyệt web khi rời app (bản Android: AutoClear.kt). Trình phát / duyệt web còn mở giữa chừng thì để
/// lại (quay vào xem tiếp được); app bị tắt trước khi kịp xoá thì lần mở sau tự xoá.
@MainActor
enum AutoClear {
    static func launched() {
        guard Prefs.autoClearWeb, Prefs.webDirty else { return }
        Task {
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            BrowsingData.clear(images: false) { Log.d("AutoClear", "đã xoá dữ liệu web (lần mở trước chưa kịp xoá)") }
        }
    }

    static func leftApp() {
        guard Prefs.autoClearWeb, Prefs.webDirty else { return }
        let top = Router.topController
        if top is PlayerController || top is BrowserController || PlayerModel.detachedModel != nil { return }
        BrowsingData.clear(images: false) { Log.d("AutoClear", "đã xoá dữ liệu web khi rời app") }
    }
}
