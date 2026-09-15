import Foundation

public enum HostAdminError: Error, Equatable, Sendable {
    case missingPayload(command: String)

    public var userMessage: String {
        switch self {
        case .missingPayload(let command): return "The host did not return \(command) data."
        }
    }
}

/// Host administration boundary used by the macOS destination.
///
/// Every method maps to a canonical gateway command; the client never supervises
/// a process itself. A future Rust-side supervisor API can replace only this
/// implementation.
public protocol HostAdminService: Sendable {
    func status() async throws -> HostStatus
    func startHost() async throws -> HostStatus
    func stopHost() async throws -> HostStatus
    func restartHost() async throws -> HostStatus
    func configuration() async throws -> HostConfiguration
    func diagnostics() async throws -> HostDiagnostics
    func devices() async throws -> [PairedDevice]
    func createPairing(deviceName: String?) async throws -> PairingArtifact
    func revoke(deviceId: String) async throws
    func sessions() async throws -> [SessionSummary]
    func inspect(sessionId: String) async throws -> SessionInspectResult
}

public actor GatewayHostAdminService: HostAdminService {
    private let connection: GatewayConnection
    private let codec: GatewayCodec

    public init(connection: GatewayConnection, codec: GatewayCodec = GatewayCodec()) {
        self.connection = connection
        self.codec = codec
    }

    public func status() async throws -> HostStatus {
        let result = try await connection.perform(.hostStatus)
        guard let value = try decode(HostStatus.self, from: result, command: "host.status") else {
            throw HostAdminError.missingPayload(command: "host.status")
        }
        return value
    }

    public func startHost() async throws -> HostStatus {
        try await lifecycle(.hostStart)
    }

    public func stopHost() async throws -> HostStatus {
        try await lifecycle(.hostStop)
    }

    public func restartHost() async throws -> HostStatus {
        try await lifecycle(.hostRestart)
    }

    public func configuration() async throws -> HostConfiguration {
        let result = try await connection.perform(.hostConfiguration)
        // `host.configuration` returns `HostStatusResult` today (see
        // protocol/gateway-v1.md); project it until a dedicated DTO lands.
        guard let status = try decode(HostStatus.self, from: result, command: "host.configuration") else {
            throw HostAdminError.missingPayload(command: "host.configuration")
        }
        return HostConfiguration(hostStatus: status)
    }

    public func diagnostics() async throws -> HostDiagnostics {
        let result = try await connection.perform(.hostDiagnostics)
        guard let value = try decode(HostDiagnostics.self, from: result, command: "host.diagnostics") else {
            throw HostAdminError.missingPayload(command: "host.diagnostics")
        }
        return value
    }

    public func devices() async throws -> [PairedDevice] {
        let result = try await connection.perform(.deviceList)
        guard let value = try decode(DeviceListResult.self, from: result, command: "device.list") else {
            return []
        }
        return value.devices
    }

    public func createPairing(deviceName: String?) async throws -> PairingArtifact {
        let params = JSONValue.object([
            ("label", .string(deviceName ?? "Slight client")),
        ])
        let result = try await connection.perform(.pairingCreate, params: params)
        guard let value = try decode(PairingArtifact.self, from: result, command: "pairing.create") else {
            throw HostAdminError.missingPayload(command: "pairing.create")
        }
        return value
    }

    public func revoke(deviceId: String) async throws {
        let params = JSONValue.object([("deviceId", .string(deviceId))])
        try await connection.perform(.deviceRevoke, params: params)
    }

    public func sessions() async throws -> [SessionSummary] {
        let result = try await connection.perform(.sessionList)
        return try decode(SessionListResult.self, from: result, command: "session.list")?.sessions ?? []
    }

    public func inspect(sessionId: String) async throws -> SessionInspectResult {
        let result = try await connection.perform(.sessionInspect, sessionId: sessionId)
        guard let value = try decode(SessionInspectResult.self, from: result, command: "session.inspect") else {
            throw HostAdminError.missingPayload(command: "session.inspect")
        }
        return value
    }

    private func lifecycle(_ name: GatewayCommandName) async throws -> HostStatus {
        let result = try await connection.perform(name)
        // Lifecycle commands return a HostLifecycleResult; refresh the full
        // status so callers get a consistent snapshot.
        _ = try decode(HostLifecycleResult.self, from: result, command: name.rawValue)
        return try await status()
    }

    private func decode<T: Decodable>(_ type: T.Type, from value: JSONValue?, command: String) throws -> T? {
        do {
            return try codec.decodePayload(type, from: value)
        } catch {
            throw HostAdminError.missingPayload(command: command)
        }
    }
}

private struct DeviceListResult: Codable, Equatable, Sendable {
    var devices: [PairedDevice]
}
