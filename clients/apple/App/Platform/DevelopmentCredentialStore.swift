#if DEBUG
import SlightGateway

/// Uses a paired Keychain credential when one exists, then falls back to the
/// development token understood by `acp-host serve` in dev mode. The fallback
/// is compiled out of release builds.
actor DevelopmentCredentialStore: DeviceCredentialStore {
    private let persistentStore: DeviceCredentialStore

    init(persistentStore: DeviceCredentialStore) {
        self.persistentStore = persistentStore
    }

    func load(hostId: String) async throws -> DeviceCredential? {
        if let credential = try await persistentStore.load(hostId: hostId) {
            return credential
        }
        return DeviceCredential(
            deviceId: "dev-device",
            hostId: hostId,
            token: "dev",
            displayName: "Local Development"
        )
    }

    func save(_ credential: DeviceCredential) async throws {
        try await persistentStore.save(credential)
    }

    func delete(hostId: String) async throws {
        try await persistentStore.delete(hostId: hostId)
    }

    func loadAll() async throws -> [DeviceCredential] {
        try await persistentStore.loadAll()
    }
}
#endif
