import Foundation

/// A scoped, revocable device credential issued by the host during pairing.
///
/// This is the only secret the client persists. It is intentionally small so
/// the Keychain adapter stays trivial.
public struct DeviceCredential: Codable, Equatable, Sendable, Identifiable {
    public var deviceId: String
    public var hostId: String
    public var token: String
    public var displayName: String
    public var pairedAt: Date?

    public var id: String { deviceId }

    public init(
        deviceId: String,
        hostId: String,
        token: String,
        displayName: String,
        pairedAt: Date? = nil
    ) {
        self.deviceId = deviceId
        self.hostId = hostId
        self.token = token
        self.displayName = displayName
        self.pairedAt = pairedAt
    }
}

/// Platform-neutral credential storage boundary. iOS and macOS supply Keychain
/// adapters; tests and previews use `InMemoryCredentialStore`.
public protocol DeviceCredentialStore: Sendable {
    func load(hostId: String) async throws -> DeviceCredential?
    func save(_ credential: DeviceCredential) async throws
    func delete(hostId: String) async throws
    func loadAll() async throws -> [DeviceCredential]
}

public actor InMemoryCredentialStore: DeviceCredentialStore {
    private var credentials: [String: DeviceCredential]

    public init(credentials: [DeviceCredential] = []) {
        self.credentials = Dictionary(uniqueKeysWithValues: credentials.map { ($0.hostId, $0) })
    }

    public func load(hostId: String) async throws -> DeviceCredential? {
        credentials[hostId]
    }

    public func save(_ credential: DeviceCredential) async throws {
        credentials[credential.hostId] = credential
    }

    public func delete(hostId: String) async throws {
        credentials.removeValue(forKey: hostId)
    }

    public func loadAll() async throws -> [DeviceCredential] {
        Array(credentials.values)
    }
}
