import Foundation

/// Host-run lifecycle state, matching `gateway_protocol::admin::HostRunState`.
public enum HostRunState: String, Codable, Equatable, Sendable, CaseIterable {
    case stopped
    case starting
    case running
    case stopping
    case failed

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = HostRunState(rawValue: raw) ?? .failed
    }

    public var isRunning: Bool { self == .running }
}

/// A host endpoint the client can pair with and connect to. Local to the client
/// until host discovery/settings land.
public struct HostProfile: Codable, Equatable, Sendable, Identifiable {
    public var id: String
    public var displayName: String
    public var endpoint: URL
    public var isLoopback: Bool

    public init(id: String = UUID().uuidString, displayName: String, endpoint: URL, isLoopback: Bool = false) {
        self.id = id
        self.displayName = displayName
        self.endpoint = endpoint
        self.isLoopback = isLoopback
    }

    public static func loopback(port: Int = 8787) -> HostProfile {
        HostProfile(
            id: "loopback",
            displayName: "This Mac (local)",
            endpoint: URL(string: "ws://127.0.0.1:\(port)/gateway")!,
            isLoopback: true
        )
    }
}

/// Canonical `HostStatusResult`.
public struct HostStatus: Codable, Equatable, Sendable {
    public var state: HostRunState
    public var hostName: String
    public var serverId: String
    public var serverVersion: String
    public var protocolVersion: Int
    public var listener: String?
    public var sessionCount: Int
    public var activeSessionCount: Int
    public var pairedDeviceCount: Int
    public var uptimeMs: Int
    public var supportedAgents: [String]
    public var agentCatalog: [AgentCatalogEntry]

    public init(
        state: HostRunState,
        hostName: String,
        serverId: String,
        serverVersion: String,
        protocolVersion: Int,
        listener: String? = nil,
        sessionCount: Int = 0,
        activeSessionCount: Int = 0,
        pairedDeviceCount: Int = 0,
        uptimeMs: Int = 0,
        supportedAgents: [String] = [],
        agentCatalog: [AgentCatalogEntry] = []
    ) {
        self.state = state
        self.hostName = hostName
        self.serverId = serverId
        self.serverVersion = serverVersion
        self.protocolVersion = protocolVersion
        self.listener = listener
        self.sessionCount = sessionCount
        self.activeSessionCount = activeSessionCount
        self.pairedDeviceCount = pairedDeviceCount
        self.uptimeMs = uptimeMs
        self.supportedAgents = supportedAgents
        self.agentCatalog = agentCatalog
    }
}

public enum AgentConfigCategory: Equatable, Sendable, Codable {
    case mode
    case model
    case modelConfig
    case thoughtLevel
    case other(String)

    public init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer().decode(String.self)
        switch value {
        case "mode": self = .mode
        case "model": self = .model
        case "model_config": self = .modelConfig
        case "thought_level": self = .thoughtLevel
        default: self = .other(value)
        }
    }

    public func encode(to encoder: Encoder) throws {
        let value: String
        switch self {
        case .mode: value = "mode"
        case .model: value = "model"
        case .modelConfig: value = "model_config"
        case .thoughtLevel: value = "thought_level"
        case .other(let customValue): value = customValue
        }
        var container = encoder.singleValueContainer()
        try container.encode(value)
    }
}

public struct AgentConfigChoice: Codable, Equatable, Sendable, Identifiable {
    public var valueId: String
    public var name: String
    public var description: String?
    public var id: String { valueId }
}

public struct AgentConfigGroup: Codable, Equatable, Sendable {
    public var id: String?
    public var name: String?
    public var options: [AgentConfigChoice]
}

public enum AgentConfigKind: Codable, Equatable, Sendable {
    case select(currentValueId: String, groups: [AgentConfigGroup])
    case boolean(currentValue: Bool)

    private enum CodingKeys: String, CodingKey { case kind, currentValueId, groups, currentValue }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "select":
            self = .select(
                currentValueId: try container.decode(String.self, forKey: .currentValueId),
                groups: try container.decode([AgentConfigGroup].self, forKey: .groups)
            )
        case "boolean":
            self = .boolean(currentValue: try container.decode(Bool.self, forKey: .currentValue))
        default:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: container, debugDescription: "Unknown config kind")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .select(let currentValueId, let groups):
            try container.encode("select", forKey: .kind)
            try container.encode(currentValueId, forKey: .currentValueId)
            try container.encode(groups, forKey: .groups)
        case .boolean(let currentValue):
            try container.encode("boolean", forKey: .kind)
            try container.encode(currentValue, forKey: .currentValue)
        }
    }

    public var choices: [AgentConfigChoice] {
        if case .select(_, let groups) = self { return groups.flatMap(\.options) }
        return []
    }

    public var currentValueId: String? {
        if case .select(let value, _) = self { return value }
        return nil
    }
}

public struct AgentConfigOption: Codable, Equatable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var description: String?
    public var category: AgentConfigCategory?
    public var kind: AgentConfigKind
}

public struct AgentCatalogEntry: Codable, Equatable, Sendable, Identifiable {
    public var id: String
    public var displayName: String
    public var version: String?
    public var available: Bool
    public var configOptions: [AgentConfigOption]
}

/// Canonical `HostLifecycleResult`.
public struct HostLifecycleResult: Codable, Equatable, Sendable {
    public var state: HostRunState
    public var message: String?

    public init(state: HostRunState, message: String? = nil) {
        self.state = state
        self.message = message
    }
}

/// Canonical `AgentDescriptorDto`.
public struct AgentDescriptor: Codable, Equatable, Sendable {
    public var kind: String
    public var displayName: String
    public var version: String?

    public init(kind: String, displayName: String, version: String? = nil) {
        self.kind = kind
        self.displayName = displayName
        self.version = version
    }
}

/// Canonical `SessionDiagnosticsDto`.
public struct SessionDiagnostics: Codable, Equatable, Sendable, Identifiable {
    public var sessionId: String
    public var status: String
    public var agent: String
    public var lastSequence: Int
    public var eventCount: Int
    public var running: Bool

    public var id: String { sessionId }

    public init(sessionId: String, status: String, agent: String, lastSequence: Int, eventCount: Int, running: Bool) {
        self.sessionId = sessionId
        self.status = status
        self.agent = agent
        self.lastSequence = lastSequence
        self.eventCount = eventCount
        self.running = running
    }
}

/// Canonical `LogEntryDto`.
public struct LogEntry: Codable, Equatable, Sendable, Identifiable {
    public var timestamp: Date
    public var level: String
    public var message: String

    public var id: String { "\(timestamp.timeIntervalSince1970)-\(message)" }

    public init(timestamp: Date, level: String, message: String) {
        self.timestamp = timestamp
        self.level = level
        self.message = message
    }

    private enum CodingKeys: String, CodingKey {
        case timestamp
        case level
        case message
    }
}

/// Canonical `HostDiagnosticsResult`.
public struct HostDiagnostics: Codable, Equatable, Sendable {
    public var serverVersion: String
    public var protocolVersion: Int
    public var logLevel: String
    public var sessions: [SessionDiagnostics]
    public var recentLogs: [LogEntry]
    public var capabilities: GatewayCapabilities

    public init(
        serverVersion: String,
        protocolVersion: Int,
        logLevel: String,
        sessions: [SessionDiagnostics] = [],
        recentLogs: [LogEntry] = [],
        capabilities: GatewayCapabilities = GatewayCapabilities()
    ) {
        self.serverVersion = serverVersion
        self.protocolVersion = protocolVersion
        self.logLevel = logLevel
        self.sessions = sessions
        self.recentLogs = recentLogs
        self.capabilities = capabilities
    }
}

/// Canonical `PairingCreateResult`.
public struct PairingArtifact: Codable, Equatable, Sendable {
    public var pairingId: String
    public var code: String
    public var label: String
    public var expiresAt: Date
    public var qrPayload: String

    public init(pairingId: String, code: String, label: String, expiresAt: Date, qrPayload: String) {
        self.pairingId = pairingId
        self.code = code
        self.label = label
        self.expiresAt = expiresAt
        self.qrPayload = qrPayload
    }
}

/// Canonical `DeviceSummaryDto`.
public struct PairedDevice: Codable, Equatable, Sendable, Identifiable {
    public var deviceId: String
    public var label: String
    public var createdAt: Date
    public var lastSeenAt: Date
    public var revoked: Bool

    public var id: String { deviceId }

    public init(deviceId: String, label: String, createdAt: Date, lastSeenAt: Date, revoked: Bool) {
        self.deviceId = deviceId
        self.label = label
        self.createdAt = createdAt
        self.lastSeenAt = lastSeenAt
        self.revoked = revoked
    }
}

/// Provisional host configuration view. `host.configuration` has no canonical
/// result type yet (bean slight-3ude); replace when it lands.
public struct HostConfiguration: Codable, Equatable, Sendable {
    public var listenerAddress: String
    public var transport: String
    public var dataDirectory: String?
    public var logLevel: String
    public var maxJournalEvents: Int?
    public var allowLoopback: Bool
    public var pairingRequired: Bool

    public init(
        listenerAddress: String,
        transport: String = "websocket",
        dataDirectory: String? = nil,
        logLevel: String = "info",
        maxJournalEvents: Int? = nil,
        allowLoopback: Bool = false,
        pairingRequired: Bool = true
    ) {
        self.listenerAddress = listenerAddress
        self.transport = transport
        self.dataDirectory = dataDirectory
        self.logLevel = logLevel
        self.maxJournalEvents = maxJournalEvents
        self.allowLoopback = allowLoopback
        self.pairingRequired = pairingRequired
    }

    /// The canonical `host.configuration` command currently returns the same
    /// payload as `host.status`, so project the fields the UI knows about.
    public init(hostStatus: HostStatus) {
        self.init(
            listenerAddress: hostStatus.listener ?? "not listening",
            transport: "tcp-json",
            dataDirectory: nil,
            logLevel: "info",
            maxJournalEvents: nil,
            allowLoopback: true,
            pairingRequired: true
        )
    }
}
