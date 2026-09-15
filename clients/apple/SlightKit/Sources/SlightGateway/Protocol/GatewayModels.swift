import Foundation

/// The gateway protocol version implemented by this client.
///
/// NOTE: This is a provisional local boundary. The canonical contract lives in
/// `protocol/gateway-v1.md` (bean slight-3ude) and these types must be unified
/// with the Rust `gateway-protocol` crate and shared conformance fixtures.
public enum GatewayProtocol {
    public static let version = 1
    public static let minimumSupportedVersion = 1
    public static let clientName = "Slight"
}

/// Lifecycle state of a host-owned session.
public enum SessionStatus: String, Codable, Equatable, Sendable, CaseIterable {
    case idle
    case working
    case waitingPermission = "waiting_permission"
    case exited
    case failed

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = SessionStatus(rawValue: raw) ?? .failed
    }
}

/// Server-advertised capabilities. Unknown capabilities are ignored.
public struct GatewayCapabilities: Codable, Equatable, Sendable {
    public var supportsReplay: Bool
    public var maxReplayEvents: Int?
    public var maxEventJournal: Int?
    public var supportsRawAcp: Bool
    public var permissionOptions: Bool
    public var hostAdmin: Bool

    public init(
        supportsReplay: Bool = true,
        maxReplayEvents: Int? = nil,
        maxEventJournal: Int? = nil,
        supportsRawAcp: Bool = false,
        permissionOptions: Bool = true,
        hostAdmin: Bool = true
    ) {
        self.supportsReplay = supportsReplay
        self.maxReplayEvents = maxReplayEvents
        self.maxEventJournal = maxEventJournal
        self.supportsRawAcp = supportsRawAcp
        self.permissionOptions = permissionOptions
        self.hostAdmin = hostAdmin
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        supportsReplay = try container.decodeIfPresent(Bool.self, forKey: .supportsReplay) ?? true
        maxReplayEvents = try container.decodeIfPresent(Int.self, forKey: .maxReplayEvents)
        maxEventJournal = try container.decodeIfPresent(Int.self, forKey: .maxEventJournal)
        supportsRawAcp = try container.decodeIfPresent(Bool.self, forKey: .supportsRawAcp) ?? false
        permissionOptions = try container.decodeIfPresent(Bool.self, forKey: .permissionOptions) ?? true
        hostAdmin = try container.decodeIfPresent(Bool.self, forKey: .hostAdmin) ?? true
    }
}

/// A summary of a host-owned session, safe to send to clients.
public struct SessionSummary: Codable, Equatable, Sendable, Identifiable {
    public var id: String
    public var title: String
    public var agent: String
    public var model: String?
    public var effort: String?
    public var workingDirectoryLabel: String
    public var gitBranch: String?
    public var status: SessionStatus
    public var createdAt: Date?
    public var lastActivityAt: Date?
    public var lastSequence: Int
    /// How the session relates to its native agent session after a host restart.
    /// Omitted by hosts that predate recovery; missing values decode to `.live`.
    public var recovery: SessionRecoveryState
    public var archived: Bool

    public init(
        id: String,
        title: String,
        agent: String,
        model: String? = nil,
        effort: String? = nil,
        workingDirectoryLabel: String,
        gitBranch: String? = nil,
        status: SessionStatus,
        createdAt: Date? = nil,
        lastActivityAt: Date? = nil,
        lastSequence: Int = 0,
        recovery: SessionRecoveryState = .live,
        archived: Bool = false
    ) {
        self.id = id
        self.title = title
        self.agent = agent
        self.model = model
        self.effort = effort
        self.workingDirectoryLabel = workingDirectoryLabel
        self.gitBranch = gitBranch
        self.status = status
        self.createdAt = createdAt
        self.lastActivityAt = lastActivityAt
        self.lastSequence = lastSequence
        self.recovery = recovery
        self.archived = archived
    }

    private enum CodingKeys: String, CodingKey {
        case id
        case title
        case agent
        case model
        case effort
        case workingDirectoryLabel
        case gitBranch
        case status
        case createdAt
        case lastActivityAt
        case lastSequence
        case recovery
        case archived
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decode(String.self, forKey: .title)
        agent = try container.decode(String.self, forKey: .agent)
        model = try container.decodeIfPresent(String.self, forKey: .model)
        effort = try container.decodeIfPresent(String.self, forKey: .effort)
        workingDirectoryLabel = try container.decode(String.self, forKey: .workingDirectoryLabel)
        gitBranch = try container.decodeIfPresent(String.self, forKey: .gitBranch)
        status = try container.decodeIfPresent(SessionStatus.self, forKey: .status) ?? .failed
        createdAt = try container.decodeIfPresent(Date.self, forKey: .createdAt)
        lastActivityAt = try container.decodeIfPresent(Date.self, forKey: .lastActivityAt)
        lastSequence = try container.decodeIfPresent(Int.self, forKey: .lastSequence) ?? 0
        recovery = try container.decodeIfPresent(SessionRecoveryState.self, forKey: .recovery) ?? .live
        archived = try container.decodeIfPresent(Bool.self, forKey: .archived) ?? false
    }
}

/// A selectable option attached to a permission request.
public struct PermissionOption: Codable, Equatable, Sendable, Identifiable {
    public enum Kind: String, Codable, Sendable {
        case allow
        case allowAlways = "allow_always"
        case deny
        case denyAlways = "deny_always"
        case custom

        public init(from decoder: Decoder) throws {
            let raw = try decoder.singleValueContainer().decode(String.self)
            self = Kind(rawValue: raw) ?? .custom
        }
    }

    public var optionId: String
    public var label: String
    public var kind: Kind

    public var id: String { optionId }

    public init(optionId: String, label: String, kind: Kind) {
        self.optionId = optionId
        self.label = label
        self.kind = kind
    }
}

/// A block of normalized assistant/user content.
///
/// Text, code, reasoning, and tool results carry their payload in `text`.
/// Media and resource blocks carry base64 data and always label it with a MIME
/// type. Resource links carry a URI but the client never resolves or fetches
/// it. Unknown kinds decode to `.text` so a newer host never breaks the client.
public struct ContentBlock: Codable, Equatable, Sendable, Identifiable {
    public enum Kind: String, Codable, Sendable {
        case text
        case code
        case reasoning
        case toolResult = "tool_result"
        case image
        case audio
        case resourceLink = "resource_link"
        case resourceText = "resource_text"
        case resourceBlob = "resource_blob"

        public init(from decoder: Decoder) throws {
            let raw = try decoder.singleValueContainer().decode(String.self)
            self = Kind(rawValue: raw) ?? .text
        }
    }

    public var id: String
    public var kind: Kind
    public var text: String
    public var language: String?
    public var mimeType: String?
    public var dataBase64: String?
    public var uri: String?
    public var name: String?
    public var title: String?
    public var description: String?
    public var size: Int?

    public init(
        id: String = UUID().uuidString,
        kind: Kind = .text,
        text: String,
        language: String? = nil,
        mimeType: String? = nil,
        dataBase64: String? = nil,
        uri: String? = nil,
        name: String? = nil,
        title: String? = nil,
        description: String? = nil,
        size: Int? = nil
    ) {
        self.id = id
        self.kind = kind
        self.text = text
        self.language = language
        self.mimeType = mimeType
        self.dataBase64 = dataBase64
        self.uri = uri
        self.name = name
        self.title = title
        self.description = description
        self.size = size
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decodeIfPresent(String.self, forKey: .id) ?? UUID().uuidString
        let raw = try container.decodeIfPresent(String.self, forKey: .kind) ?? Kind.text.rawValue
        kind = Kind(rawValue: raw) ?? .text
        text = try container.decodeIfPresent(String.self, forKey: .text) ?? ""
        language = try container.decodeIfPresent(String.self, forKey: .language)
        mimeType = try container.decodeIfPresent(String.self, forKey: .mimeType)
        dataBase64 = try container.decodeIfPresent(String.self, forKey: .dataBase64)
        uri = try container.decodeIfPresent(String.self, forKey: .uri)
        name = try container.decodeIfPresent(String.self, forKey: .name)
        title = try container.decodeIfPresent(String.self, forKey: .title)
        description = try container.decodeIfPresent(String.self, forKey: .description)
        size = try container.decodeIfPresent(Int.self, forKey: .size)
    }
}

/// One piece of rich tool-call output.
///
/// The Rust host flattens the tagged content block into the `content` entry, so
/// `.content` decodes a `ContentBlock` from the same container that carries the
/// `type` discriminator.
public enum ToolCallContent: Codable, Equatable, Sendable {
    case content(ContentBlock)
    case diff(Diff)
    case terminal(TerminalRef)

    private enum CodingKeys: String, CodingKey {
        case type
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "content":
            self = .content(try ContentBlock(from: decoder))
        case "diff":
            self = .diff(try Diff(from: decoder))
        case "terminal":
            self = .terminal(try TerminalRef(from: decoder))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "Unknown tool-call content type: \(type)"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .content(let block):
            try container.encode("content", forKey: .type)
            try block.encode(to: encoder)
        case .diff(let diff):
            try container.encode("diff", forKey: .type)
            try diff.encode(to: encoder)
        case .terminal(let terminal):
            try container.encode("terminal", forKey: .type)
            try terminal.encode(to: encoder)
        }
    }
}

/// A normalized file diff from a tool call.
public struct Diff: Codable, Equatable, Sendable {
    public var path: String
    public var oldText: String?
    public var newText: String

    public init(path: String, oldText: String? = nil, newText: String) {
        self.path = path
        self.oldText = oldText
        self.newText = newText
    }
}

/// A reference to a terminal that produced tool output.
public struct TerminalRef: Codable, Equatable, Sendable, Identifiable {
    public var terminalId: String

    public var id: String { terminalId }

    public init(terminalId: String) {
        self.terminalId = terminalId
    }
}

/// A file location touched by a tool call.
public struct ToolLocation: Codable, Equatable, Sendable {
    public var path: String
    public var line: Int?

    public init(path: String, line: Int? = nil) {
        self.path = path
        self.line = line
    }
}

/// A normalized message in a session transcript.
public struct SessionMessage: Codable, Equatable, Sendable, Identifiable {
    public enum Role: String, Codable, Sendable {
        case user
        case assistant
        case system
    }

    public var id: String
    public var role: Role
    public var blocks: [ContentBlock]
    public var at: Date?
    public var isStreaming: Bool
    public var reasoningDuration: TimeInterval?

    public init(
        id: String = UUID().uuidString,
        role: Role,
        blocks: [ContentBlock],
        at: Date? = nil,
        isStreaming: Bool = false,
        reasoningDuration: TimeInterval? = nil
    ) {
        self.id = id
        self.role = role
        self.blocks = blocks
        self.at = at
        self.isStreaming = isStreaming
        self.reasoningDuration = reasoningDuration
    }

    public var text: String {
        blocks.map(\.text).joined()
    }
}

/// Progress for a normalized tool call.
public struct ToolCall: Codable, Equatable, Sendable, Identifiable {
    public enum Status: String, Codable, Sendable {
        case pending
        case inProgress = "in_progress"
        case completed
        case failed
        case cancelled

        public init(from decoder: Decoder) throws {
            let raw = try decoder.singleValueContainer().decode(String.self)
            self = Status(rawValue: raw) ?? .failed
        }
    }

    public var id: String
    public var title: String
    public var kind: String?
    public var status: Status
    public var detail: String?
    /// Rich tool output: content blocks, diffs, and terminal references.
    public var content: [ToolCallContent]
    /// File locations the tool touched, for follow-along UIs.
    public var locations: [ToolLocation]
    /// Raw, untyped tool input, only surfaced behind an explicit debug view.
    public var rawInput: JSONValue?
    /// Raw, untyped tool result, only surfaced behind an explicit debug view.
    public var rawOutput: JSONValue?

    public init(
        id: String,
        title: String,
        kind: String? = nil,
        status: Status = .pending,
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

    /// Merges a partial update, retaining rich fields the update omits.
    ///
    /// ACP tool-call updates are patches: an absent collection or raw value
    /// means "unchanged", not "cleared". Non-optional fields are always
    /// overwritten.
    public func merging(_ update: ToolCall) -> ToolCall {
        var merged = update
        if update.title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            merged.title = title
        }
        if update.content.isEmpty { merged.content = content }
        if update.locations.isEmpty { merged.locations = locations }
        if update.kind == nil { merged.kind = kind }
        if update.detail == nil { merged.detail = detail }
        if update.rawInput == nil { merged.rawInput = rawInput }
        if update.rawOutput == nil { merged.rawOutput = rawOutput }
        return merged
    }
}

/// A pending permission request surfaced by the host.
public struct PermissionRequest: Codable, Equatable, Sendable, Identifiable {
    public var id: String
    public var title: String
    public var detail: String?
    public var toolCallId: String?
    public var options: [PermissionOption]
    public var requestedAt: Date?

    public init(
        id: String,
        title: String,
        detail: String? = nil,
        toolCallId: String? = nil,
        options: [PermissionOption],
        requestedAt: Date? = nil
    ) {
        self.id = id
        self.title = title
        self.detail = detail
        self.toolCallId = toolCallId
        self.options = options
        self.requestedAt = requestedAt
    }
}

// MARK: - Normalized ACP capabilities and metadata

/// The kind of authentication an agent advertises during initialization.
public enum AuthMethodKind: String, Codable, Sendable {
    case agent
    case terminal

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = AuthMethodKind(rawValue: raw) ?? .agent
    }
}

/// A normalized authentication method advertised by an agent.
public struct AuthMethodSummary: Codable, Equatable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var description: String?
    public var kind: AuthMethodKind

    public init(id: String, name: String, description: String? = nil, kind: AuthMethodKind = .agent) {
        self.id = id
        self.name = name
        self.description = description
        self.kind = kind
    }
}

/// Flattened agent capabilities negotiated during `initialize`.
///
/// Every field decodes to `false` when absent so older hosts and future
/// capability additions stay decodable.
public struct AgentCapabilitiesSummary: Codable, Equatable, Sendable {
    public var loadSession: Bool
    public var promptImage: Bool
    public var promptAudio: Bool
    public var promptEmbeddedContext: Bool
    public var mcpHttp: Bool
    public var mcpSse: Bool
    public var sessionList: Bool
    public var sessionResume: Bool
    public var sessionClose: Bool
    public var sessionDelete: Bool
    public var sessionAdditionalDirectories: Bool

    public init(
        loadSession: Bool = false,
        promptImage: Bool = false,
        promptAudio: Bool = false,
        promptEmbeddedContext: Bool = false,
        mcpHttp: Bool = false,
        mcpSse: Bool = false,
        sessionList: Bool = false,
        sessionResume: Bool = false,
        sessionClose: Bool = false,
        sessionDelete: Bool = false,
        sessionAdditionalDirectories: Bool = false
    ) {
        self.loadSession = loadSession
        self.promptImage = promptImage
        self.promptAudio = promptAudio
        self.promptEmbeddedContext = promptEmbeddedContext
        self.mcpHttp = mcpHttp
        self.mcpSse = mcpSse
        self.sessionList = sessionList
        self.sessionResume = sessionResume
        self.sessionClose = sessionClose
        self.sessionDelete = sessionDelete
        self.sessionAdditionalDirectories = sessionAdditionalDirectories
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        loadSession = try container.decodeIfPresent(Bool.self, forKey: .loadSession) ?? false
        promptImage = try container.decodeIfPresent(Bool.self, forKey: .promptImage) ?? false
        promptAudio = try container.decodeIfPresent(Bool.self, forKey: .promptAudio) ?? false
        promptEmbeddedContext = try container.decodeIfPresent(Bool.self, forKey: .promptEmbeddedContext) ?? false
        mcpHttp = try container.decodeIfPresent(Bool.self, forKey: .mcpHttp) ?? false
        mcpSse = try container.decodeIfPresent(Bool.self, forKey: .mcpSse) ?? false
        sessionList = try container.decodeIfPresent(Bool.self, forKey: .sessionList) ?? false
        sessionResume = try container.decodeIfPresent(Bool.self, forKey: .sessionResume) ?? false
        sessionClose = try container.decodeIfPresent(Bool.self, forKey: .sessionClose) ?? false
        sessionDelete = try container.decodeIfPresent(Bool.self, forKey: .sessionDelete) ?? false
        sessionAdditionalDirectories =
            try container.decodeIfPresent(Bool.self, forKey: .sessionAdditionalDirectories) ?? false
    }
}

/// A single mode the agent can operate in.
public struct AcpMode: Codable, Equatable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var description: String?

    public init(id: String, name: String, description: String? = nil) {
        self.id = id
        self.name = name
        self.description = description
    }
}

/// The agent's mode state for a session.
public struct AcpModeState: Codable, Equatable, Sendable {
    public var currentModeId: String
    public var availableModes: [AcpMode]

    public init(currentModeId: String, availableModes: [AcpMode] = []) {
        self.currentModeId = currentModeId
        self.availableModes = availableModes
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        currentModeId = try container.decode(String.self, forKey: .currentModeId)
        availableModes = try container.decodeIfPresent([AcpMode].self, forKey: .availableModes) ?? []
    }
}

/// Negotiated ACP capabilities and metadata for one session, as exposed by
/// `session.inspect`.
public struct SessionAcpMetadata: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var agentTitle: String?
    public var authMethods: [AuthMethodSummary]
    public var capabilities: AgentCapabilitiesSummary
    public var modes: AcpModeState?
    public var configOptions: [AgentConfigOption]

    public init(
        protocolVersion: Int = 0,
        agentTitle: String? = nil,
        authMethods: [AuthMethodSummary] = [],
        capabilities: AgentCapabilitiesSummary = AgentCapabilitiesSummary(),
        modes: AcpModeState? = nil,
        configOptions: [AgentConfigOption] = []
    ) {
        self.protocolVersion = protocolVersion
        self.agentTitle = agentTitle
        self.authMethods = authMethods
        self.capabilities = capabilities
        self.modes = modes
        self.configOptions = configOptions
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? 0
        agentTitle = try container.decodeIfPresent(String.self, forKey: .agentTitle)
        authMethods = try container.decodeIfPresent([AuthMethodSummary].self, forKey: .authMethods) ?? []
        capabilities = try container.decodeIfPresent(AgentCapabilitiesSummary.self, forKey: .capabilities)
            ?? AgentCapabilitiesSummary()
        modes = try container.decodeIfPresent(AcpModeState.self, forKey: .modes)
        configOptions = try container.decodeIfPresent([AgentConfigOption].self, forKey: .configOptions) ?? []
    }
}

/// A single entry in an agent execution plan.
public struct PlanEntrySummary: Codable, Equatable, Sendable {
    public var content: String
    public var priority: String
    public var status: String

    public init(content: String, priority: String, status: String) {
        self.content = content
        self.priority = priority
        self.status = status
    }
}

/// A normalized agent execution plan. Each update replaces the whole plan.
public struct PlanSummary: Codable, Equatable, Sendable {
    public var entries: [PlanEntrySummary]

    public init(entries: [PlanEntrySummary] = []) {
        self.entries = entries
    }
}

/// A normalized slash command advertised by the agent.
public struct AvailableCommandSummary: Codable, Equatable, Sendable, Identifiable {
    public var name: String
    public var description: String
    public var inputHint: String?

    public var id: String { name }

    public init(name: String, description: String = "", inputHint: String? = nil) {
        self.name = name
        self.description = description
        self.inputHint = inputHint
    }
}
