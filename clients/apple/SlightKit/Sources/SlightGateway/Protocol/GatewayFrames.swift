import Foundation

// MARK: - Frame union

/// The versioned frame union carried over the gateway transport.
///
/// The provisional wire shape flattens the frame body into the top-level JSON
/// object with a `type` discriminator, for example:
///
/// ```json
/// { "v": 1, "type": "welcome", "connection_id": "...", "capabilities": { } }
/// ```
///
/// Unknown `type` values decode to `.unknown(type:raw:)` so the client can keep
/// running against a newer host.
public enum GatewayFrame: Equatable, Sendable {
    case hello(ClientHello)
    case welcome(ServerWelcome)
    case command(GatewayCommand)
    case ack(GatewayAck)
    case event(GatewayEvent)
    case ping(PingFrame)
    case pong(PongFrame)
    case resyncRequired(ResyncRequired)
    case error(GatewayProtocolError)
    case unknown(type: String, raw: JSONValue)

    public var frameType: String {
        switch self {
        case .hello: return "hello"
        case .welcome: return "welcome"
        case .command: return "command"
        case .ack: return "ack"
        case .event: return "event"
        case .ping: return "ping"
        case .pong: return "pong"
        case .resyncRequired: return "resync_required"
        case .error: return "error"
        case .unknown(let type, _): return type
        }
    }
}

extension GatewayFrame: Codable {
    private enum Discriminator: String, CodingKey {
        case type
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Discriminator.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "hello": self = .hello(try ClientHello(from: decoder))
        case "welcome": self = .welcome(try ServerWelcome(from: decoder))
        case "command": self = .command(try GatewayCommand(from: decoder))
        case "ack": self = .ack(try GatewayAck(from: decoder))
        case "event": self = .event(try GatewayEvent(from: decoder))
        case "ping": self = .ping(try PingFrame(from: decoder))
        case "pong": self = .pong(try PongFrame(from: decoder))
        case "resync_required": self = .resyncRequired(try ResyncRequired(from: decoder))
        case "error": self = .error(try GatewayProtocolError(from: decoder))
        default: self = .unknown(type: type, raw: try JSONValue(from: decoder))
        }
    }

    public func encode(to encoder: Encoder) throws {
        // `JSONEncoder` cannot merge a keyed container with a single-value
        // container that holds an object, so the unknown case encodes the
        // preserved object exactly once.
        if case .unknown(let type, let raw) = self {
            if case .object(var object) = raw {
                object["type"] = .string(type)
                try JSONValue.object(object).encode(to: encoder)
            } else {
                var container = encoder.container(keyedBy: Discriminator.self)
                try container.encode(type, forKey: .type)
            }
            return
        }

        var container = encoder.container(keyedBy: Discriminator.self)
        switch self {
        case .hello(let value):
            try container.encode("hello", forKey: .type)
            try value.encode(to: encoder)
        case .welcome(let value):
            try container.encode("welcome", forKey: .type)
            try value.encode(to: encoder)
        case .command(let value):
            try container.encode("command", forKey: .type)
            try value.encode(to: encoder)
        case .ack(let value):
            try container.encode("ack", forKey: .type)
            try value.encode(to: encoder)
        case .event(let value):
            try container.encode("event", forKey: .type)
            try value.encode(to: encoder)
        case .ping(let value):
            try container.encode("ping", forKey: .type)
            try value.encode(to: encoder)
        case .pong(let value):
            try container.encode("pong", forKey: .type)
            try value.encode(to: encoder)
        case .resyncRequired(let value):
            try container.encode("resync_required", forKey: .type)
            try value.encode(to: encoder)
        case .error(let value):
            try container.encode("error", forKey: .type)
            try value.encode(to: encoder)
        case .unknown:
            break
        }
    }
}

// MARK: - Handshake

public struct ResumeRequest: Codable, Equatable, Sendable {
    public var sessionId: String?
    public var lastEventSequence: Int?

    public init(sessionId: String? = nil, lastEventSequence: Int? = nil) {
        self.sessionId = sessionId
        self.lastEventSequence = lastEventSequence
    }
}

public struct ClientHello: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var clientId: String
    public var clientName: String
    public var clientVersion: String
    public var deviceId: String?
    public var credential: String?
    public var resume: ResumeRequest?

    public init(
        protocolVersion: Int = GatewayProtocol.version,
        clientId: String,
        clientName: String = GatewayProtocol.clientName,
        clientVersion: String,
        deviceId: String? = nil,
        credential: String? = nil,
        resume: ResumeRequest? = nil
    ) {
        self.protocolVersion = protocolVersion
        self.clientId = clientId
        self.clientName = clientName
        self.clientVersion = clientVersion
        self.deviceId = deviceId
        self.credential = credential
        self.resume = resume
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case clientId
        case clientName
        case clientVersion
        case deviceId
        case credential
        case resume
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        clientId = try container.decode(String.self, forKey: .clientId)
        clientName = try container.decodeIfPresent(String.self, forKey: .clientName) ?? GatewayProtocol.clientName
        clientVersion = try container.decodeIfPresent(String.self, forKey: .clientVersion) ?? "0"
        deviceId = try container.decodeIfPresent(String.self, forKey: .deviceId)
        credential = try container.decodeIfPresent(String.self, forKey: .credential)
        resume = try container.decodeIfPresent(ResumeRequest.self, forKey: .resume)
    }
}

public struct ServerWelcome: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var connectionId: String
    public var serverName: String
    public var serverVersion: String
    public var hostId: String?
    public var capabilities: GatewayCapabilities
    public var heartbeatIntervalMs: Int?
    public var resyncRequired: Bool
    public var resyncReason: String?

    public init(
        protocolVersion: Int = GatewayProtocol.version,
        connectionId: String,
        serverName: String,
        serverVersion: String,
        hostId: String? = nil,
        capabilities: GatewayCapabilities = GatewayCapabilities(),
        heartbeatIntervalMs: Int? = nil,
        resyncRequired: Bool = false,
        resyncReason: String? = nil
    ) {
        self.protocolVersion = protocolVersion
        self.connectionId = connectionId
        self.serverName = serverName
        self.serverVersion = serverVersion
        self.hostId = hostId
        self.capabilities = capabilities
        self.heartbeatIntervalMs = heartbeatIntervalMs
        self.resyncRequired = resyncRequired
        self.resyncReason = resyncReason
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case connectionId
        case serverName
        case serverVersion
        case hostId
        case capabilities
        case heartbeatIntervalMs
        case resyncRequired
        case resyncReason
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        connectionId = try container.decode(String.self, forKey: .connectionId)
        serverName = try container.decodeIfPresent(String.self, forKey: .serverName) ?? "slight-host"
        serverVersion = try container.decodeIfPresent(String.self, forKey: .serverVersion) ?? "0"
        hostId = try container.decodeIfPresent(String.self, forKey: .hostId)
        capabilities = try container.decodeIfPresent(GatewayCapabilities.self, forKey: .capabilities)
            ?? GatewayCapabilities()
        heartbeatIntervalMs = try container.decodeIfPresent(Int.self, forKey: .heartbeatIntervalMs)
        resyncRequired = try container.decodeIfPresent(Bool.self, forKey: .resyncRequired) ?? false
        resyncReason = try container.decodeIfPresent(String.self, forKey: .resyncReason)
    }
}

// MARK: - Commands and acknowledgements

/// Known command names. `GatewayCommand.command` stays a `String` so newer host
/// verbs do not require a client update.
public enum GatewayCommandName: String, Sendable, CaseIterable {
    case resume = "session.resume"
    case sessionList = "session.list"
    case sessionWorkingDirectories = "session.working_directories"
    case sessionCreate = "session.create"
    case sessionAttach = "session.attach"
    case sessionDetach = "session.detach"
    case sessionInput = "session.input"
    case sessionCancel = "session.cancel"
    case permissionRespond = "session.permission.respond"
    case sessionRename = "session.rename"
    case sessionArchive = "session.archive"
    case sessionSetMode = "session.set_mode"
    case sessionSetConfigOption = "session.set_config_option"
    case sessionInspect = "session.inspect"
    case sessionHistory = "session.history"
    case eventsReplay = "events.replay"

    case agentSessionsList = "agent.sessions.list"
    case agentSessionImport = "agent.sessions.import"

    case hostStatus = "host.status"
    case hostStart = "host.start"
    case hostStop = "host.stop"
    case hostRestart = "host.restart"
    case hostConfiguration = "host.configuration"
    case hostDiagnostics = "host.diagnostics"
    case hostShutdown = "host.shutdown"
    case deviceList = "device.list"
    case deviceRevoke = "device.revoke"
    case pairingCreate = "pairing.create"
    case pairingList = "pairing.list"
}

public struct GatewayCommand: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var requestId: String
    public var command: String
    public var sessionId: String?
    public var params: JSONValue?

    public init(
        protocolVersion: Int = GatewayProtocol.version,
        requestId: String = UUID().uuidString,
        command: String,
        sessionId: String? = nil,
        params: JSONValue? = nil
    ) {
        self.protocolVersion = protocolVersion
        self.requestId = requestId
        self.command = command
        self.sessionId = sessionId
        self.params = params
    }

    public init(
        requestId: String = UUID().uuidString,
        name: GatewayCommandName,
        sessionId: String? = nil,
        params: JSONValue? = nil
    ) {
        self.init(requestId: requestId, command: name.rawValue, sessionId: sessionId, params: params)
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case requestId
        case command
        case sessionId
        case params
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        requestId = try container.decode(String.self, forKey: .requestId)
        command = try container.decode(String.self, forKey: .command)
        sessionId = try container.decodeIfPresent(String.self, forKey: .sessionId)
        params = try container.decodeIfPresent(JSONValue.self, forKey: .params)
    }
}

public struct GatewayErrorBody: Codable, Equatable, Sendable {
    public var code: String
    public var message: String
    public var retryable: Bool

    public init(code: String, message: String, retryable: Bool = false) {
        self.code = code
        self.message = message
        self.retryable = retryable
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        code = try container.decodeIfPresent(String.self, forKey: .code) ?? "unknown"
        message = try container.decodeIfPresent(String.self, forKey: .message) ?? ""
        retryable = try container.decodeIfPresent(Bool.self, forKey: .retryable) ?? false
    }
}

public struct GatewayAck: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var requestId: String
    public var ok: Bool
    public var result: JSONValue?
    public var error: GatewayErrorBody?

    public init(
        protocolVersion: Int = GatewayProtocol.version,
        requestId: String,
        ok: Bool,
        result: JSONValue? = nil,
        error: GatewayErrorBody? = nil
    ) {
        self.protocolVersion = protocolVersion
        self.requestId = requestId
        self.ok = ok
        self.result = result
        self.error = error
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case requestId
        case ok
        case result
        case error
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        requestId = try container.decode(String.self, forKey: .requestId)
        ok = try container.decodeIfPresent(Bool.self, forKey: .ok) ?? false
        result = try container.decodeIfPresent(JSONValue.self, forKey: .result)
        error = try container.decodeIfPresent(GatewayErrorBody.self, forKey: .error)
    }
}

// MARK: - Events

/// Known normalized event names.
public enum GatewayEventName: String, Sendable, CaseIterable {
    case sessionStatus = "session.status"
    case sessionMessage = "session.message"
    case sessionToolCall = "session.tool_call"
    case sessionPlan = "session.plan"
    case sessionMode = "session.mode"
    case sessionConfigOptions = "session.config_options"
    case sessionCommands = "session.commands"
    case sessionTurnEnded = "session.turn_ended"
    case sessionPermissionRequest = "session.permission_request"
    case sessionPermissionResolved = "session.permission_resolved"
    case sessionExit = "session.exit"
    case sessionSnapshot = "session.snapshot"
    case sessionDiagnostics = "session.diagnostics"
    case replayComplete = "replay.complete"
    case hostStatusChanged = "host.status_changed"
}

public struct GatewayEvent: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var sessionId: String
    public var sequence: Int
    public var event: String
    public var at: Date?
    public var payload: JSONValue?

    public init(
        protocolVersion: Int = GatewayProtocol.version,
        sessionId: String,
        sequence: Int,
        event: String,
        at: Date? = nil,
        payload: JSONValue? = nil
    ) {
        self.protocolVersion = protocolVersion
        self.sessionId = sessionId
        self.sequence = sequence
        self.event = event
        self.at = at
        self.payload = payload
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case sessionId
        case sequence
        case event
        case at
        case payload
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        sessionId = try container.decode(String.self, forKey: .sessionId)
        sequence = try container.decode(Int.self, forKey: .sequence)
        event = try container.decode(String.self, forKey: .event)
        at = try container.decodeIfPresent(Date.self, forKey: .at)
        payload = try container.decodeIfPresent(JSONValue.self, forKey: .payload)
    }
}

// MARK: - Heartbeats, resync, protocol errors

public struct PingFrame: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var nonce: String

    public init(protocolVersion: Int = GatewayProtocol.version, nonce: String = UUID().uuidString) {
        self.protocolVersion = protocolVersion
        self.nonce = nonce
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case nonce
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        nonce = try container.decodeIfPresent(String.self, forKey: .nonce) ?? ""
    }
}

public struct PongFrame: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var nonce: String

    public init(protocolVersion: Int = GatewayProtocol.version, nonce: String) {
        self.protocolVersion = protocolVersion
        self.nonce = nonce
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case nonce
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        nonce = try container.decodeIfPresent(String.self, forKey: .nonce) ?? ""
    }
}

public struct ResyncRequired: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var reason: String
    public var sessionId: String?
    public var oldestAvailableSequence: Int?

    public init(
        protocolVersion: Int = GatewayProtocol.version,
        reason: String,
        sessionId: String? = nil,
        oldestAvailableSequence: Int? = nil
    ) {
        self.protocolVersion = protocolVersion
        self.reason = reason
        self.sessionId = sessionId
        self.oldestAvailableSequence = oldestAvailableSequence
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case reason
        case sessionId
        case oldestAvailableSequence
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        reason = try container.decodeIfPresent(String.self, forKey: .reason) ?? "history_unavailable"
        sessionId = try container.decodeIfPresent(String.self, forKey: .sessionId)
        oldestAvailableSequence = try container.decodeIfPresent(Int.self, forKey: .oldestAvailableSequence)
    }
}

public struct GatewayProtocolError: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var code: String
    public var message: String
    public var requestId: String?
    public var retryable: Bool

    public init(
        protocolVersion: Int = GatewayProtocol.version,
        code: String,
        message: String,
        requestId: String? = nil,
        retryable: Bool = false
    ) {
        self.protocolVersion = protocolVersion
        self.code = code
        self.message = message
        self.requestId = requestId
        self.retryable = retryable
    }

    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "v"
        case code
        case message
        case requestId
        case retryable
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? GatewayProtocol.version
        code = try container.decodeIfPresent(String.self, forKey: .code) ?? "unknown"
        message = try container.decodeIfPresent(String.self, forKey: .message) ?? ""
        requestId = try container.decodeIfPresent(String.self, forKey: .requestId)
        retryable = try container.decodeIfPresent(Bool.self, forKey: .retryable) ?? false
    }
}
