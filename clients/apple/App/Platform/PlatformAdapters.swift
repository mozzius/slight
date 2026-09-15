import Foundation
import Security
import SlightGateway
import SlightGatewayUI

/// Small platform adapters. Everything protocol-related lives in SlightKit; this
/// file only bridges OS facilities (Keychain, default endpoints).
@MainActor
enum PlatformSupport {
    /// Loads the persisted host endpoint. An install with no saved host remains
    /// unconfigured until the onboarding flow creates a profile.
    static func defaultHostProfile() -> HostProfile? {
        UserDefaultsHostProfileStore().load()
    }

    static func makeCredentialStore() -> DeviceCredentialStore {
        let keychainStore = KeychainDeviceCredentialStore(service: "sh.slight.gateway")
        #if DEBUG
        return DevelopmentCredentialStore(persistentStore: keychainStore)
        #else
        return keychainStore
        #endif
    }

    /// Builds the shared app model against the real WebSocket gateway.
    static func makeAppModel() -> SlightAppModel {
        let profile = defaultHostProfile()
        return SlightAppModel(
            hostProfile: profile,
            credentialStore: makeCredentialStore(),
            hostProfileStore: UserDefaultsHostProfileStore()
        )
    }
}

enum KeychainError: Error, Equatable {
    case unexpectedStatus(OSStatus)
    case malformedItem
}

/// Keychain-backed device credential storage shared by iOS and macOS. Each host
/// gets one revocable credential keyed by host id.
struct KeychainDeviceCredentialStore: DeviceCredentialStore {
    let service: String

    init(service: String) {
        self.service = service
    }

    func load(hostId: String) async throws -> DeviceCredential? {
        var query = baseQuery(hostId: hostId)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = item as? Data else {
            throw KeychainError.unexpectedStatus(status)
        }
        return try JSONDecoder().decode(DeviceCredential.self, from: data)
    }

    func save(_ credential: DeviceCredential) async throws {
        try await delete(hostId: credential.hostId)
        var query = baseQuery(hostId: credential.hostId)
        query[kSecValueData as String] = try JSONEncoder().encode(credential)
        query[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.unexpectedStatus(status)
        }
    }

    func delete(hostId: String) async throws {
        let status = SecItemDelete(baseQuery(hostId: hostId) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainError.unexpectedStatus(status)
        }
    }

    func loadAll() async throws -> [DeviceCredential] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecReturnData as String: true,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitAll,
        ]
        if let accessGroup = accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }

        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return [] }
        guard status == errSecSuccess, let items = result as? [[String: Any]] else {
            throw KeychainError.unexpectedStatus(status)
        }
        return try items.compactMap { item in
            guard let data = item[kSecValueData as String] as? Data else {
                throw KeychainError.malformedItem
            }
            return try JSONDecoder().decode(DeviceCredential.self, from: data)
        }
    }

    private var accessGroup: String? { nil }

    private func baseQuery(hostId: String) -> [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: hostId,
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }
        return query
    }
}
