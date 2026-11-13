#if targetEnvironment(macCatalyst)
import Foundation

/// Bản Mac tự cập nhật (iPad / iPhone cài bằng TrollStore; bản Windows: Updater.cs). Tải ConBangTV-Mac-<bản>.zip của release
/// mac-v…, giải nén bằng ditto, kiểm đúng app (cùng mã gói, đúng số bản, chữ ký còn nguyên), thay thư mục .app đang chạy rồi mở
/// bản mới và thoát. Tệp tải bằng URLSession không bị macOS gắn cờ «tải từ mạng» nên bản mới mở không hỏi Gatekeeper.
/// Không thay được (app chạy từ chỗ tạm của macOS vì chưa kéo vào Applications, hoặc thư mục không có quyền ghi) → báo lỗi,
/// AppModel mở link tải trong trình duyệt.
enum MacUpdate {
    struct Failure: LocalizedError {
        let errorDescription: String?
        init(_ s: String) { errorDescription = s }
    }

    /// Thư mục .app đang chạy.
    static var appURL: URL { Bundle.main.bundleURL }

    /// macOS chạy app vừa tải về (chưa chuyển chỗ) từ một bản sao tạm chỉ đọc — «App Translocation».
    static var translocated: Bool { appURL.path.contains("/AppTranslocation/") }

    /// nil = tự thay được; ngược lại là lý do không thay được.
    static var blocker: String? {
        if translocated {
            return "App đang chạy từ chỗ tạm của macOS — kéo Côn Bằng TV vào thư mục Applications (Ứng dụng) rồi mở lại để tự cập nhật."
        }
        let parent = appURL.deletingLastPathComponent().path
        if !FileManager.default.isWritableFile(atPath: parent) { return "Không có quyền ghi vào thư mục \(parent)." }
        return nil
    }

    static func install(_ rel: UpdateCheck.Release, progress: @escaping @MainActor (String) -> Void) async throws {
        if let why = blocker { throw Failure(why) }
        guard let url = URL(string: rel.fileUrl) else { throw Failure("Link tải sai.") }
        await progress("Đang tải bản \(rel.version)…")
        let (tmp, resp) = try await Http.session.download(from: url)
        if let h = resp as? HTTPURLResponse, h.statusCode >= 400 { throw Failure("Tải bản mới bị lỗi HTTP \(h.statusCode).") }
        let fm = FileManager.default
        let work = fm.temporaryDirectory.appendingPathComponent("conbangtv-capnhat-\(UUID().uuidString)")
        try fm.createDirectory(at: work, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: work) }
        let zip = work.appendingPathComponent("moi.zip")
        try fm.moveItem(at: tmp, to: zip)
        try run("/usr/bin/ditto", ["-x", "-k", zip.path, work.path])
        guard let newApp = try fm.contentsOfDirectory(at: work, includingPropertiesForKeys: nil)
            .first(where: { $0.pathExtension == "app" }) else { throw Failure("Tệp tải về không có ConBangTV.app.") }
        let info = Bundle(url: newApp)?.infoDictionary ?? [:]
        let newId = info["CFBundleIdentifier"] as? String
        let newVersion = (info["CFBundleShortVersionString"] as? String) ?? "?"
        guard newId == Bundle.main.bundleIdentifier else { throw Failure("Tệp tải về không phải Côn Bằng TV cho Mac (\(newId ?? "?")).") }
        guard newVersion == rel.version, UpdateCheck.isNewer(newVersion) else {
            throw Failure("Tệp tải về là bản \(newVersion), không phải bản \(rel.version).")
        }
        // Ký ad-hoc: không chứng minh ai làm ra, nhưng phát hiện tệp hỏng / bị sửa sau khi đóng gói
        try run("/usr/bin/codesign", ["--verify", "--deep", "--strict", newApp.path])
        await progress("Đang cài bản \(newVersion)…")
        let placed = try fm.replaceItemAt(appURL, withItemAt: newApp) ?? appURL
        Log.d("Update", "đã thay \(placed.path) bằng bản \(newVersion), mở lại")
        UserDefaults.standard.synchronize()
        // -n: mở thêm một bản dù bản cũ còn chạy; bản cũ thoát ngay sau đó
        try run("/usr/bin/open", ["-n", placed.path], wait: false)
        try? await Task.sleep(nanoseconds: 300_000_000)
        exit(0)
    }

    /// Chạy một công cụ của macOS. Mac Catalyst không cho dùng Process.run() → posix_spawn.
    private static func run(_ tool: String, _ args: [String], wait: Bool = true) throws {
        let name = (tool as NSString).lastPathComponent
        let argv: [UnsafeMutablePointer<CChar>?] = ([tool] + args).map { strdup($0) } + [nil]
        let envp: [UnsafeMutablePointer<CChar>?] = ProcessInfo.processInfo.environment.map { strdup("\($0.key)=\($0.value)") } + [nil]
        defer { (argv + envp).forEach { free($0) } }
        var pid: pid_t = 0
        let rc = posix_spawn(&pid, tool, nil, nil, argv, envp)
        guard rc == 0 else { throw Failure("Không chạy được \(name) (lỗi \(rc)).") }
        guard wait else { return }
        var status: Int32 = 0
        while waitpid(pid, &status, 0) == -1 && errno == EINTR {}
        // WIFEXITED / WEXITSTATUS: 7 bit thấp = tín hiệu làm dừng (0 = tự thoát), byte kế = mã thoát
        let code = (status & 0x7f) == 0 ? (status >> 8) & 0xff : -1
        if code != 0 { throw Failure("\(name) báo lỗi \(code).") }
    }
}
#endif
