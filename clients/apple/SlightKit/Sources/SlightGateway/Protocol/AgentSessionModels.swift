import Foundation

/// A native, agent-owned session that Slight has not imported yet.
///
/// Mirrors `gateway_protocol::dto::AgentSessionSummaryDto`. The native
/// `agentSessionId` is exposed deliberately: the discovery/import contract is
/// the one place a client may see native identity so it can ask the host to
/// import a specific session. It is untrusted agent input and must never be used
/// for filesystem access or assumed stable across agent versions.
public struct AgentSessionSummary: Codable, Equatable, Sendable, Identifiable {
    /// Agent kind that owns the session, e.g. `opencode`.
    public var agent: String
    /// The agent's own session identifier.
    public var agentSessionId: String
    /// The absolute working directory the agent associates with the session.
    /// Clients must treat this as authoritative and pass it back on import.
    public var cwd: String
    /// Extra directories the agent attached to the session, when any.
    public var additionalDirectories: [String]
    public var title: String?
    public var updatedAt: Date?

    /// Stable client-side identity for list rendering. Uses the native pair so
    /// the same agent session is not duplicated across repeated discoveries.
    public var id: String { "\(agent):\(agentSessionId)" }

    public init(
        agent: String,
        agentSessionId: String,
        cwd: String,
        additionalDirectories: [String] = [],
        title: String? = nil,
        updatedAt: Date? = nil
    ) {
        self.agent = agent
        self.agentSessionId = agentSessionId
        self.cwd = cwd
        self.additionalDirectories = additionalDirectories
        self.title = title
        self.updatedAt = updatedAt
    }

    private enum CodingKeys: String, CodingKey {
        case agent
        case agentSessionId
        case cwd
        case additionalDirectories
        case title
        case updatedAt
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        agent = try container.decode(String.self, forKey: .agent)
        agentSessionId = try container.decode(String.self, forKey: .agentSessionId)
        cwd = try container.decode(String.self, forKey: .cwd)
        additionalDirectories = try container.decodeIfPresent([String].self, forKey: .additionalDirectories) ?? []
        title = try container.decodeIfPresent(String.self, forKey: .title)
        updatedAt = try container.decodeIfPresent(Date.self, forKey: .updatedAt)
    }
}

/// How a session relates to its native agent session after a host restart.
///
/// Mirrors `session_core::types::SessionRecoveryState`. A host that predates
/// recovery omits the field entirely; clients must treat that as `.live`.
public enum SessionRecoveryState: Equatable, Sendable {
    /// Created on this host and not restored from disk.
    case live
    /// Rehydrated by resuming the persisted native agent session. The local
    /// journal is intact and the session is live again.
    case recovered
    /// The native agent session no longer exists. The local journal is retained
    /// and replayable, but the session will not accept input.
    case stale(reason: String)
    /// The agent is missing/unregistered or cannot resume its sessions. Same
    /// retention rule as `stale`.
    case unavailable(reason: String)

    public var isLive: Bool {
        if case .live = self { return true }
        return false
    }

    public var isRecovered: Bool {
        if case .recovered = self { return true }
        return false
    }

    public var isStale: Bool {
        if case .stale = self { return true }
        return false
    }

    public var isUnavailable: Bool {
        if case .unavailable = self { return true }
        return false
    }

    /// Whether the session is connected to a live agent and can accept input.
    public var acceptsInput: Bool {
        isLive || isRecovered
    }

    /// The agent's explanation for a `stale`/`unavailable` session, when given.
    public var reason: String? {
        switch self {
        case .stale(let reason), .unavailable(let reason):
            return reason.isEmpty ? nil : reason
        case .live, .recovered:
            return nil
        }
    }
}

extension SessionRecoveryState: Codable {
    private enum CodingKeys: String, CodingKey {
        case state
        case reason
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let state = try container.decodeIfPresent(String.self, forKey: .state) ?? "live"
        switch state {
        case "recovered":
            self = .recovered
        case "stale":
            self = .stale(reason: try container.decodeIfPresent(String.self, forKey: .reason) ?? "")
        case "unavailable":
            self = .unavailable(reason: try container.decodeIfPresent(String.self, forKey: .reason) ?? "")
        default:
            self = .live
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .live:
            try container.encode("live", forKey: .state)
        case .recovered:
            try container.encode("recovered", forKey: .state)
        case .stale(let reason):
            try container.encode("stale", forKey: .state)
            try container.encode(reason, forKey: .reason)
        case .unavailable(let reason):
            try container.encode("unavailable", forKey: .state)
            try container.encode(reason, forKey: .reason)
        }
    }
}

/// How an existing agent session is brought into Slight on import.
///
/// Mirrors `session_core::types::SessionRecovery`. `load` replays the agent's
/// retained history into Slight's journal; `resume` reconnects without replaying
/// agent-side history. Neither is the same as `session.attach`, which only
/// replays Slight's local journal.
public enum SessionRecovery: String, Codable, Equatable, Sendable, CaseIterable, Identifiable {
    case load
    case resume

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .load: return "Import history"
        case .resume: return "Reconnect only"
        }
    }
}

/// Params for `agent.sessions.list`.
public struct AgentSessionsListParams: Codable, Equatable, Sendable {
    public var agent: String
    public var workingDirectoryLabel: String?
    public var cursor: String?

    public init(agent: String, workingDirectoryLabel: String? = nil, cursor: String? = nil) {
        self.agent = agent
        self.workingDirectoryLabel = workingDirectoryLabel
        self.cursor = cursor
    }
}

/// Result of `agent.sessions.list`.
public struct AgentSessionsListResult: Codable, Equatable, Sendable {
    public var sessions: [AgentSessionSummary]
    public var nextCursor: String?

    public init(sessions: [AgentSessionSummary], nextCursor: String? = nil) {
        self.sessions = sessions
        self.nextCursor = nextCursor
    }
}

public struct SessionWorkingDirectoriesResult: Codable, Equatable, Sendable {
    public var paths: [String]

    public init(paths: [String]) {
        self.paths = paths
    }
}

/// Params for `agent.sessions.import`.
public struct ImportAgentSessionParams: Codable, Equatable, Sendable {
    public var agent: String
    public var agentSessionId: String
    public var workingDirectoryLabel: String
    public var recovery: SessionRecovery
    public var title: String?

    public init(
        agent: String,
        agentSessionId: String,
        workingDirectoryLabel: String,
        recovery: SessionRecovery,
        title: String? = nil
    ) {
        self.agent = agent
        self.agentSessionId = agentSessionId
        self.workingDirectoryLabel = workingDirectoryLabel
        self.recovery = recovery
        self.title = title
    }
}

/// Result of `agent.sessions.import`.
public struct ImportAgentSessionResult: Codable, Equatable, Sendable {
    public var session: SessionSummary

    public init(session: SessionSummary) {
        self.session = session
    }
}
