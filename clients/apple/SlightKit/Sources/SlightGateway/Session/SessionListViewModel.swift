import Combine
import Foundation
import OSLog

/// Observable list of sessions owned by the connected host.
@MainActor
public final class SessionListViewModel: ObservableObject {
    private static let logger = Logger(subsystem: "sh.slight.client", category: "gateway.sessions")
    @Published public private(set) var sessions: [SessionSummary] = []
    /// Workspace paths supplied by the host, including native sessions that
    /// have not been imported into Slight yet.
    @Published public private(set) var recentWorkingDirectories: [String] = []
    @Published public private(set) var agentCatalog: [AgentCatalogEntry] = []
    @Published public private(set) var hostAvailabilityError: String?
    @Published public private(set) var connectionState: GatewayConnectionState = .idle
    @Published public private(set) var isLoading = false
    /// Native agent sessions surfaced by `agent.sessions.list`, scoped to the
    /// agent and working directory the caller asked for.
    @Published public private(set) var discoveryState: SessionDiscoveryState = .idle
    @Published public private(set) var discoveryNextCursor: String?
    /// The discovered session currently being imported, so one row can show
    /// progress without hiding the rest of the list.
    @Published public private(set) var importingAgentSessionId: String?
    @Published public var lastError: String?

    public let connection: GatewayConnection
    private let codec: GatewayCodec
    private var observationTask: Task<Void, Never>?

    public init(connection: GatewayConnection, codec: GatewayCodec = GatewayCodec()) {
        self.connection = connection
        self.codec = codec
    }

    deinit {
        observationTask?.cancel()
    }

    public func start() {
        guard observationTask == nil else { return }
        observationTask = Task { [weak self] in
            guard let self else { return }
            let stream = await self.connection.subscribe()
            self.connectionState = await self.connection.state
            if self.connectionState.isConnected {
                Task { await self.refresh() }
                Task { await self.refreshRecentWorkingDirectories() }
                Task { await self.refreshHostConfiguration() }
            }
            for await inbound in stream {
                self.handle(inbound)
            }
        }
    }

    public func connect() {
        lastError = nil
        Task {
            let state = await connection.state
            switch state {
            case .failed, .closed:
                await connection.reconnect()
            default:
                await connection.start()
            }
        }
    }

    public func disconnect() {
        Task { await connection.stop() }
    }

    public func refresh() async {
        guard !isLoading else { return }
        guard case .connected = await connection.state else {
            Self.logger.debug("skipping session.list while gateway is not fully connected")
            return
        }
        isLoading = true
        defer { isLoading = false }
        Self.logger.debug("requesting session.list")
        do {
            let result = try await connection.perform(.sessionList)
            guard let payload = try codec.decodePayload(SessionListResult.self, from: result) else {
                Self.logger.error("session.list returned no result payload")
                return
            }
            sessions = payload.sessions.sorted { lhs, rhs in
                (lhs.lastActivityAt ?? lhs.createdAt ?? .distantPast) > (rhs.lastActivityAt ?? rhs.createdAt ?? .distantPast)
            }
        } catch {
            Self.logger.error("session.list failed: \(String(describing: error), privacy: .public)")
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    public func refreshRecentWorkingDirectories() async {
        guard case .connected = await connection.state else { return }
        do {
            let result = try await connection.perform(.sessionWorkingDirectories)
            if let payload = try codec.decodePayload(SessionWorkingDirectoriesResult.self, from: result) {
                recentWorkingDirectories = payload.paths
            }
        } catch {
            Self.logger.error("session.working_directories failed: \(String(describing: error), privacy: .public)")
        }
    }

    public func createSession(
        title: String?,
        agent: String,
        model: String?,
        effort: String?,
        workingDirectory: String,
        initialPrompt: String? = nil
    ) async -> SessionSummary? {
        let params = JSONValue.object([
            ("title", title.map { .string($0) } ?? .null),
            ("agent", .string(agent)),
            ("model", model.map { .string($0) } ?? .null),
            ("effort", effort.map { .string($0) } ?? .null),
            ("working_directory_label", .string(workingDirectory)),
            ("initial_prompt", initialPrompt.map { .string($0) } ?? .null),
        ])
        do {
            let result = try await connection.perform(.sessionCreate, params: params)
            guard let wrapper = try codec.decodePayload(SessionCreateResult.self, from: result) else { return nil }
            upsert(wrapper.session)
            return wrapper.session
        } catch {
            Self.logger.error("session.create failed: \(String(describing: error), privacy: .public)")
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
            return nil
        }
    }

    public func setArchived(_ archived: Bool, for summary: SessionSummary) async {
        let params = JSONValue.object([("archived", .bool(archived))])
        do {
            let result = try await connection.perform(.sessionArchive, sessionId: summary.id, params: params)
            guard let wrapper = try codec.decodePayload(SessionArchiveResult.self, from: result) else { return }
            upsert(wrapper.session)
        } catch {
            Self.logger.error("session.archive failed: \(String(describing: error), privacy: .public)")
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    // MARK: Discovery and import

    /// Asks the connected agent for its retained sessions, optionally scoped to
    /// a working directory. Discovery is read-only and does not touch Slight's
    /// local journal, so it never disturbs existing attach/replay state.
    public func discoverAgentSessions(agent: String, workingDirectory: String? = nil) async {
        discoveryNextCursor = nil
        await requestAgentSessions(agent: agent, workingDirectory: workingDirectory, cursor: nil)
    }

    public func loadMoreAgentSessions(agent: String, workingDirectory: String? = nil) async {
        guard let cursor = discoveryNextCursor else { return }
        await requestAgentSessions(agent: agent, workingDirectory: workingDirectory, cursor: cursor)
    }

    private func requestAgentSessions(agent: String, workingDirectory: String?, cursor: String?) async {
        guard case .connected = await connection.state else {
            discoveryState = .failed("Connect to a host before discovering sessions.")
            return
        }
        guard !discoveryState.isLoading else { return }
        let existingSessions = discoveryState.sessions
        discoveryState = .loading
        let directory = Self.normalized(workingDirectory)
        let params = AgentSessionsListParams(agent: agent, workingDirectoryLabel: directory, cursor: cursor)
        do {
            let encoded = try codec.encodeToJSONValue(params)
            let result = try await connection.perform(.agentSessionsList, params: encoded)
            guard let payload = try codec.decodePayload(AgentSessionsListResult.self, from: result) else {
                discoveryState = .failed("The host returned no discovery result.")
                return
            }
            let page = payload.sessions.sorted { lhs, rhs in
                switch (lhs.updatedAt, rhs.updatedAt) {
                case let (l?, r?) where l != r: return l > r
                case (.some, .none): return true
                case (.none, .some): return false
                default: return (lhs.title ?? lhs.agentSessionId)
                    .localizedCaseInsensitiveCompare(rhs.title ?? rhs.agentSessionId) == .orderedAscending
                }
            }
            let combined = cursor == nil ? page : existingSessions + page
            discoveryState = .loaded(combined)
            discoveryNextCursor = payload.nextCursor
            lastError = nil
        } catch {
            Self.logger.error("agent.sessions.list failed: \(String(describing: error), privacy: .public)")
            discoveryState = .failed((error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription)
        }
    }

    /// Imports one discovered native session into Slight and upserts the result
    /// into the session list. The local journal is not replayed here; `load`
    /// replays agent-side history into the host's journal, while `resume` only
    /// reconnects.
    @discardableResult
    public func importAgentSession(
        _ session: AgentSessionSummary,
        recovery: SessionRecovery,
        workingDirectory: String? = nil,
        title: String? = nil
    ) async -> SessionSummary? {
        guard importingAgentSessionId == nil else { return nil }
        importingAgentSessionId = session.id
        defer { importingAgentSessionId = nil }

        let params = ImportAgentSessionParams(
            agent: session.agent,
            agentSessionId: session.agentSessionId,
            workingDirectoryLabel: Self.normalized(workingDirectory) ?? session.cwd,
            recovery: recovery,
            title: Self.normalized(title) ?? session.title
        )
        do {
            let encoded = try codec.encodeToJSONValue(params)
            let result = try await connection.perform(.agentSessionImport, params: encoded)
            guard let wrapper = try codec.decodePayload(ImportAgentSessionResult.self, from: result) else {
                return nil
            }
            upsert(wrapper.session)
            lastError = nil
            return wrapper.session
        } catch {
            Self.logger.error("agent.sessions.import failed: \(String(describing: error), privacy: .public)")
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
            return nil
        }
    }

    /// Clears the discovery list so a reopened surface starts fresh.
    public func clearAgentSessionDiscovery() {
        discoveryState = .idle
        discoveryNextCursor = nil
    }

    private static func normalized(_ value: String?) -> String? {
        guard let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines), !trimmed.isEmpty else {
            return nil
        }
        return trimmed
    }

    // MARK: Inbound

    private func handle(_ inbound: GatewayInbound) {
        switch inbound {
        case .stateChanged(let state):
            connectionState = state
            if state.isConnected {
                lastError = nil
                Task { await refresh() }
                Task { await refreshRecentWorkingDirectories() }
                Task { await refreshHostConfiguration() }
            }
        case .event(let event):
            handleEvent(event)
        case .serverError(let error):
            lastError = error.message
        case .replayCompleted, .resyncRequired:
            break
        case .welcome, .duplicateEvent, .replayRequested, .transcriptReset, .ack, .close:
            break
        }
    }

    public func refreshHostConfiguration() async {
        do {
            let result = try await connection.perform(.hostConfiguration)
            if let status = try codec.decodePayload(HostStatus.self, from: result) {
                let available = status.agentCatalog.filter(\.available)
                agentCatalog = available
                if available.isEmpty, !status.agentCatalog.isEmpty {
                    let names = status.agentCatalog.map(\.displayName).joined(separator: ", ")
                    hostAvailabilityError = "Host connected, but no agent executable is available. Check the host installation or PATH for: \(names)."
                } else {
                    hostAvailabilityError = nil
                }
            }
        } catch {
            Self.logger.error("host.configuration failed: \(String(describing: error), privacy: .public)")
        }
    }

    private func handleEvent(_ event: GatewayEvent) {
        switch event.event {
        case GatewayEventName.sessionStatus.rawValue:
            if let status = try? codec.decodePayload(SessionStatusPayload.self, from: event.payload),
               var existing = sessions.first(where: { $0.id == event.sessionId }) {
                existing.status = status.status
                existing.lastActivityAt = event.at ?? Date()
                existing.lastSequence = event.sequence
                upsert(existing)
            }
        case GatewayEventName.sessionSnapshot.rawValue:
            if let snapshot = try? codec.decodePayload(SessionSnapshotPayload.self, from: event.payload) {
                upsert(snapshot.summary)
            }
        default:
            if var existing = sessions.first(where: { $0.id == event.sessionId }) {
                existing.lastActivityAt = event.at ?? Date()
                existing.lastSequence = event.sequence
                upsert(existing)
            }
        }
    }

    private func upsert(_ summary: SessionSummary) {
        if let index = sessions.firstIndex(where: { $0.id == summary.id }) {
            sessions[index] = summary
        } else {
            sessions.append(summary)
        }
        sessions.sort {
            let lhsDate = $0.lastActivityAt ?? $0.createdAt ?? .distantPast
            let rhsDate = $1.lastActivityAt ?? $1.createdAt ?? .distantPast
            if lhsDate != rhsDate { return lhsDate > rhsDate }
            return $0.id < $1.id
        }
    }
}
