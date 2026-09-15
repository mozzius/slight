import Foundation

/// Typed decoders for the canonical normalized event payloads emitted by
/// `session-core` and transported in `GatewayEvent.payload`.
///
/// These mirror `session_core::types` exactly. They are decoded through
/// `GatewayCodec.decodePayload(_:from:)`.

public struct SessionStatusPayload: Codable, Equatable, Sendable {
    public var status: SessionStatus

    public init(status: SessionStatus) {
        self.status = status
    }
}

/// A `session.message` event payload.
///
/// The canonical host currently emits one complete message per event, so `id`,
/// `isStreaming`, and `append` are absent. They are additive, optional fields
/// the client understands for forward compatibility: a host that streams can
/// reuse an `id` across chunks, set `append` to concatenate onto the previous
/// chunk, and clear `is_streaming` on the final chunk.
public struct SessionMessagePayload: Codable, Equatable, Sendable {
    public var role: SessionMessage.Role
    public var text: String
    public var blocks: [ContentBlock]
    public var id: String?
    public var isStreaming: Bool
    public var append: Bool

    public init(
        role: SessionMessage.Role,
        text: String = "",
        blocks: [ContentBlock] = [],
        id: String? = nil,
        isStreaming: Bool = false,
        append: Bool = false
    ) {
        self.role = role
        self.text = text
        self.blocks = blocks
        self.id = id
        self.isStreaming = isStreaming
        self.append = append
    }

    public func message() -> SessionMessage {
        let resolved: [ContentBlock]
        if !blocks.isEmpty {
            resolved = blocks
        } else if !text.isEmpty {
            resolved = [ContentBlock(kind: .text, text: text)]
        } else {
            resolved = []
        }
        return SessionMessage(
            id: id ?? UUID().uuidString,
            role: role,
            blocks: resolved,
            isStreaming: isStreaming
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        role = try container.decode(SessionMessage.Role.self, forKey: .role)
        text = try container.decodeIfPresent(String.self, forKey: .text) ?? ""
        blocks = try container.decodeIfPresent([ContentBlock].self, forKey: .blocks) ?? []
        id = try container.decodeIfPresent(String.self, forKey: .id)
        isStreaming = try container.decodeIfPresent(Bool.self, forKey: .isStreaming) ?? false
        append = try container.decodeIfPresent(Bool.self, forKey: .append) ?? false
    }
}

public struct ToolCallPayload: Codable, Equatable, Sendable {
    public var id: String
    public var title: String
    public var kind: String?
    public var status: ToolCall.Status
    public var detail: String?
    public var content: [ToolCallContent]
    public var locations: [ToolLocation]
    public var rawInput: JSONValue?
    public var rawOutput: JSONValue?

    public init(
        id: String,
        title: String,
        kind: String? = nil,
        status: ToolCall.Status = .pending,
        detail: String? = nil,
        content: [ToolCallContent] = [],
        locations: [ToolLocation] = [],
        rawInput: JSONValue? = nil,
        rawOutput: JSONValue? = nil
    ) {
        self.id = id
        self.title = title
        self.kind = kind
        self.status = status
        self.detail = detail
        self.content = content
        self.locations = locations
        self.rawInput = rawInput
        self.rawOutput = rawOutput
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decode(String.self, forKey: .title)
        kind = try container.decodeIfPresent(String.self, forKey: .kind)
        status = try container.decodeIfPresent(ToolCall.Status.self, forKey: .status) ?? .pending
        detail = try container.decodeIfPresent(String.self, forKey: .detail)
        content = try container.decodeIfPresent([ToolCallContent].self, forKey: .content) ?? []
        locations = try container.decodeIfPresent([ToolLocation].self, forKey: .locations) ?? []
        rawInput = try container.decodeIfPresent(JSONValue.self, forKey: .rawInput)
        rawOutput = try container.decodeIfPresent(JSONValue.self, forKey: .rawOutput)
    }

    public func toolCall() -> ToolCall {
        ToolCall(
            id: id,
            title: title,
            kind: kind,
            status: status,
            detail: detail,
            content: content,
            locations: locations,
            rawInput: rawInput,
            rawOutput: rawOutput
        )
    }
}

public struct PermissionRequestPayload: Codable, Equatable, Sendable {
    public var id: String
    public var title: String
    public var detail: String?
    public var toolCallId: String?
    public var options: [PermissionOption]

    public init(
        id: String,
        title: String,
        detail: String? = nil,
        toolCallId: String? = nil,
        options: [PermissionOption]
    ) {
        self.id = id
        self.title = title
        self.detail = detail
        self.toolCallId = toolCallId
        self.options = options
    }

    public func request() -> PermissionRequest {
        PermissionRequest(
            id: id,
            title: title,
            detail: detail,
            toolCallId: toolCallId,
            options: options
        )
    }
}

public struct PermissionResolvedPayload: Codable, Equatable, Sendable {
    public var id: String
    public var optionId: String

    public init(id: String, optionId: String) {
        self.id = id
        self.optionId = optionId
    }
}

public struct SessionExitPayload: Codable, Equatable, Sendable {
    public var code: Int?
    public var reason: String

    public init(code: Int? = nil, reason: String = "") {
        self.code = code
        self.reason = reason
    }
}

public struct SessionDiagnosticsPayload: Codable, Equatable, Sendable {
    public var level: String
    public var message: String

    public init(level: String, message: String) {
        self.level = level
        self.message = message
    }
}

/// A `session.plan` event payload. Each update replaces the whole plan.
public struct SessionPlanPayload: Codable, Equatable, Sendable {
    public var entries: [PlanEntrySummary]

    public init(entries: [PlanEntrySummary] = []) {
        self.entries = entries
    }
}

/// A `session.mode` event payload. The agent switched its current mode.
public struct SessionModePayload: Codable, Equatable, Sendable {
    public var currentModeId: String

    public init(currentModeId: String) {
        self.currentModeId = currentModeId
    }
}

/// A `session.config_options` event payload replacing the current options.
public struct SessionConfigOptionsPayload: Codable, Equatable, Sendable {
    public var options: [AgentConfigOption]

    public init(options: [AgentConfigOption] = []) {
        self.options = options
    }
}

/// A `session.commands` event payload listing the agent's slash commands.
public struct SessionCommandsPayload: Codable, Equatable, Sendable {
    public var commands: [AvailableCommandSummary]

    public init(commands: [AvailableCommandSummary] = []) {
        self.commands = commands
    }
}

/// A `session.turn_ended` event payload carrying the ACP stop reason.
public struct SessionTurnEndedPayload: Codable, Equatable, Sendable {
    public var stopReason: String
    public var reasoningStartedAtMs: Int64?
    public var reasoningEndedAtMs: Int64?

    public init(
        stopReason: String = "end_turn",
        reasoningStartedAtMs: Int64? = nil,
        reasoningEndedAtMs: Int64? = nil
    ) {
        self.stopReason = stopReason
        self.reasoningStartedAtMs = reasoningStartedAtMs
        self.reasoningEndedAtMs = reasoningEndedAtMs
    }
}

public struct SessionSnapshotPayload: Codable, Equatable, Sendable {
    public var summary: SessionSummary
    public var pendingPermission: PermissionRequestPayload?

    public init(summary: SessionSummary, pendingPermission: PermissionRequestPayload? = nil) {
        self.summary = summary
        self.pendingPermission = pendingPermission
    }
}

// MARK: - Command results

public struct SessionListResult: Codable, Equatable, Sendable {
    public var sessions: [SessionSummary]

    public init(sessions: [SessionSummary]) {
        self.sessions = sessions
    }
}

public struct SessionCreateResult: Codable, Equatable, Sendable {
    public var session: SessionSummary

    public init(session: SessionSummary) {
        self.session = session
    }
}

public struct SessionAttachResult: Codable, Equatable, Sendable {
    public var session: SessionSummary
    public var replayed: [GatewayEvent]
    public var latestSequence: Int
    public var oldestAvailableSequence: Int?
    public var resyncRequired: Bool

    public init(
        session: SessionSummary,
        replayed: [GatewayEvent] = [],
        latestSequence: Int = 0,
        oldestAvailableSequence: Int? = nil,
        resyncRequired: Bool = false
    ) {
        self.session = session
        self.replayed = replayed
        self.latestSequence = latestSequence
        self.oldestAvailableSequence = oldestAvailableSequence
        self.resyncRequired = resyncRequired
    }
}

public struct SessionHistoryResult: Codable, Equatable, Sendable {
    public var session: SessionSummary
    public var events: [GatewayEvent]
    public var latestSequence: Int
    public var oldestAvailableSequence: Int?
    public var hasMore: Bool

    public init(
        session: SessionSummary,
        events: [GatewayEvent] = [],
        latestSequence: Int = 0,
        oldestAvailableSequence: Int? = nil,
        hasMore: Bool = false
    ) {
        self.session = session
        self.events = events
        self.latestSequence = latestSequence
        self.oldestAvailableSequence = oldestAvailableSequence
        self.hasMore = hasMore
    }
}

public struct SessionRenameResult: Codable, Equatable, Sendable {
    public var session: SessionSummary

    public init(session: SessionSummary) {
        self.session = session
    }
}

public struct SessionArchiveResult: Codable, Equatable, Sendable {
    public var session: SessionSummary

    public init(session: SessionSummary) {
        self.session = session
    }
}

public struct SessionInspectResult: Codable, Equatable, Sendable {
    public var session: SessionSummary
    public var agent: AgentDescriptor
    /// Negotiated ACP capabilities and session metadata. Optional so older
    /// hosts (and hosts that predate this contract) still decode.
    public var acp: SessionAcpMetadata?
    public var running: Bool
    public var pendingPermission: PermissionRequestPayload?
    public var recentEvents: [GatewayEvent]

    public init(
        session: SessionSummary,
        agent: AgentDescriptor,
        acp: SessionAcpMetadata? = nil,
        running: Bool,
        pendingPermission: PermissionRequestPayload? = nil,
        recentEvents: [GatewayEvent] = []
    ) {
        self.session = session
        self.agent = agent
        self.acp = acp
        self.running = running
        self.pendingPermission = pendingPermission
        self.recentEvents = recentEvents
    }
}
