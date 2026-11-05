import Foundation
import Security

/// Chuỗi bí mật (mật khẩu proxy) trong Keychain của iPad — bản Android: Secret.kt (Android Keystore).
enum Secret {
    private static let service = "vn.chimto.tv.secret"

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

    /// Keychain có dùng được không (cổng gỡ lỗi).
    static var works: Bool {
        guard set("probe", "1") else { return false }
        defer { set("probe", "") }
        return get("probe") == "1"
    }
}
