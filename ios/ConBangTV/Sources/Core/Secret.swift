import Foundation
import Security

/// Chuỗi bí mật (mật khẩu proxy) trong Keychain của iPad — bản Android: Secret.kt (Android Keystore).
///
/// Bản Mac (Mac Catalyst): app ký ad-hoc, không có Team ID → Keychain kiểu iOS cần quyền keychain-access-groups (phải có chứng
/// chỉ Apple), còn Keychain kiểu cũ gắn quyền đọc với chữ ký của app — mỗi lần app tự cập nhật macOS lại hỏi mật khẩu máy. Nên
/// bản Mac cất vào tệp riêng quyền 0600 trong Application Support: chỉ tài khoản macOS này đọc được (như DPAPI của bản Windows).
enum Secret {
    private static let service = "vn.chimto.tv.secret"

    #if targetEnvironment(macCatalyst)
    private static let lock = NSLock()

    private static var file: URL? {
        guard let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else { return nil }
        return base.appendingPathComponent(Bundle.main.bundleIdentifier ?? "ConBangTV").appendingPathComponent("secret.json")
    }

    private static func load() -> [String: String] {
        guard let f = file, let d = try? Data(contentsOf: f) else { return [:] }
        return (try? JSONDecoder().decode([String: String].self, from: d)) ?? [:]
    }

    static func get(_ key: String) -> String? {
        lock.lock()
        defer { lock.unlock() }
        return load()[key]
    }

    @discardableResult
    static func set(_ key: String, _ value: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard let f = file else { return false }
        var all = load()
        all[key] = value.isEmpty ? nil : value
        let fm = FileManager.default
        do {
            try fm.createDirectory(at: f.deletingLastPathComponent(), withIntermediateDirectories: true,
                                   attributes: [.posixPermissions: 0o700])
            if all.isEmpty {
                try? fm.removeItem(at: f)
                return true
            }
            // Tạo tệp với quyền 0600 trước rồi mới ghi nội dung (không có lúc nào tệp đọc được bởi người khác)
            if !fm.fileExists(atPath: f.path) {
                fm.createFile(atPath: f.path, contents: nil, attributes: [.posixPermissions: 0o600])
            }
            let h = try FileHandle(forWritingTo: f)
            defer { try? h.close() }
            try h.truncate(atOffset: 0)
            try h.write(contentsOf: try JSONEncoder().encode(all))
            try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: f.path)
            return true
        } catch {
            return false
        }
    }
    #else
    private static func query(_ key: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: key]
    }

    /// nil = chưa lưu, hoặc Keychain không đọc được.
    static func get(_ key: String) -> String? {
        var q = query(key)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let d = out as? Data else { return nil }
        return String(data: d, encoding: .utf8)
    }

    /// Lưu (chuỗi rỗng = xoá). false = Keychain không dùng được (app thiếu quyền Keychain).
    @discardableResult
    static func set(_ key: String, _ value: String) -> Bool {
        let del = SecItemDelete(query(key) as CFDictionary)
        if value.isEmpty { return del == errSecSuccess || del == errSecItemNotFound }
        var add = query(key)
        add[kSecValueData as String] = Data(value.utf8)
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        return SecItemAdd(add as CFDictionary, nil) == errSecSuccess
    }
    #endif

    /// Keychain / tệp bí mật có dùng được không (cổng gỡ lỗi).
    static var works: Bool {
        guard set("probe", "1") else { return false }
        defer { set("probe", "") }
        return get("probe") == "1"
    }
}
