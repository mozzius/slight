#if DEBUG
import Foundation
import SlightGateway

/// An in-memory `GatewayTransport` used by the fake host.
///
/// This is the UI-facing analogue of `FakeGatewayTransport` in the gateway test
/// target. It is deliberately tiny: framing and JSON stay in `GatewayCodec`, so
/// the fake only has to move bytes.
public final class FakeHostTransport: GatewayTransport, @unchecked Sendable {
    private let lock = NSLock()
    private let codec = GatewayCodec()
    private var incoming: [Data] = []
    private var waiters: [CheckedContinuation<Data?, Error>] = []
    private var isClosed = false
    private var sentFrames: [GatewayFrame] = []

    /// Called whenever the connection writes a frame to the host.
    public var onSend: (@Sendable (GatewayFrame) -> Void)?

    public init() {}

    public func start() async throws {}

    public func send(_ data: Data) async throws {
        let frame = try codec.decode(data)
        lock.withLock { sentFrames.append(frame) }
        onSend?(frame)
    }

    public func receive() async throws -> Data? {
        if let data = dequeue() { return data }
        if lock.withLock({ isClosed }) { return nil }
        return try await withCheckedThrowingContinuation { continuation in
            var immediate: Data??
            lock.withLock {
                if !incoming.isEmpty {
                    immediate = .some(incoming.removeFirst())
                } else if isClosed {
                    immediate = .some(nil)
                } else {
                    waiters.append(continuation)
                }
            }
            if let immediate {
                continuation.resume(returning: immediate)
            }
        }
    }

    public func close() async {
        let pending = lock.withLock { () -> [CheckedContinuation<Data?, Error>] in
            isClosed = true
            let pending = waiters
            waiters.removeAll()
            return pending
        }
        for waiter in pending {
            waiter.resume(returning: nil)
        }
    }

    public func push(_ frame: GatewayFrame) {
        guard let data = try? codec.encode(frame) else { return }
        lock.lock()
        if !waiters.isEmpty {
            let waiter = waiters.removeFirst()
            lock.unlock()
            waiter.resume(returning: data)
            return
        }
        incoming.append(data)
        lock.unlock()
    }

    public func sent() -> [GatewayFrame] {
        lock.withLock { sentFrames }
    }

    private func dequeue() -> Data? {
        lock.withLock { incoming.isEmpty ? nil : incoming.removeFirst() }
    }
}

/// A scripted, in-memory host that answers the gateway commands the client UI
/// uses and lets tests (or SwiftUI previews) push normalized events.
///
/// It exists so the session browser, transcript, permission prompt, reconnect
/// banner, and macOS admin surface can be exercised without a running Rust host
/// or an ACP agent. It is a development fixture only: nothing in the shipping
/// app constructs a `FakeHost`.
public final class FakeHost: @unchecked Sendable {
    private let lock = NSLock()
    private let codec = GatewayCodec()

    private var currentTransport: FakeHostTransport?
    private var commandLog: [GatewayCommand] = []
    private var sessions: [SessionSummary]
    private var replayEventsBySession: [String: [GatewayEvent]] = [:]
    private var sequences: [String: Int] = [:]
    private var pendingPermissions: [String: PermissionRequestPayload] = [:]
    private var pendingPermissionSessions: [String: String] = [:]
    private var resyncSessions: Set<String> = []
    private var failures: [String: GatewayErrorBody] = [:]
    private var hellos = 0
    private var agentSessions: [AgentSessionSummary]

    private var hostStatus: HostStatus
    private var hostDiagnostics: HostDiagnostics
    private var devices: [PairedDevice]
    private var nextPairing: PairingArtifact

    /// Invoked with `(sessionId, text)` after a `session.input` is acknowledged.
    /// Preview scenarios use this to script a streaming reply.
    public var autoReplyToInput: (@Sendable (String, String) -> Void)?

    /// Invoked for every inbound command frame, before it is answered.
    public var onCommand: (@Sendable (GatewayCommand) -> Void)?

    public init(
        sessions: [SessionSummary] = [],
        hostStatus: HostStatus = FakeHost.defaultHostStatus,
        hostDiagnostics: HostDiagnostics = FakeHost.defaultDiagnostics,
        devices: [PairedDevice] = [],
        nextPairing: PairingArtifact = FakeHost.defaultPairing,
        agentSessions: [AgentSessionSummary] = []
    ) {
        self.sessions = sessions
        self.hostStatus = hostStatus
        self.hostDiagnostics = hostDiagnostics
        self.devices = devices
        self.nextPairing = nextPairing
        self.agentSessions = agentSessions
    }

    // MARK: Factory

    /// Builds a transport factory for `GatewayConnection`. A new transport is
    /// created per connection attempt so reconnect can be exercised.
    public func makeTransportFactory() -> GatewayTransportFactory {
        { [weak self] in
            guard let self else { throw GatewayConnectionError.transportClosed }
            return self.makeTransport()
        }
    }

    private func makeTransport() -> FakeHostTransport {
        let transport = FakeHostTransport()
        transport.onSend = { [weak self, weak transport] frame in
            guard let transport else { return }
            self?.receive(frame, from: transport)
        }
        lock.lock()
        currentTransport = transport
        lock.unlock()
        return transport
    }

    // MARK: Inspection

    public var helloCount: Int {
        lock.lock()
        defer { lock.unlock() }
        return hellos
    }

    public func commands() -> [GatewayCommand] {
        lock.lock()
        defer { lock.unlock() }
        return commandLog
    }

    public func sessionSummaries() -> [SessionSummary] {
        lock.lock()
        defer { lock.unlock() }
        return sessions
    }

    // MARK: Scripting

    /// Preloads the events returned from a `session.attach` (i.e. replay).
    public func setReplayEvents(_ events: [GatewayEvent], for sessionId: String) {
        lock.lock()
        replayEventsBySession[sessionId] = events
        let maxSequence = events.map(\.sequence).max() ?? 0
        sequences[sessionId] = max(sequences[sessionId] ?? 0, maxSequence)
        if let index = sessions.firstIndex(where: { $0.id == sessionId }) {
            sessions[index].lastSequence = max(sessions[index].lastSequence, maxSequence)
        }
        lock.unlock()
    }

    /// Marks a session so the next `session.attach` reports `resyncRequired`.
    public func markResyncRequired(sessionId: String) {
        lock.lock()
        resyncSessions.insert(sessionId)
        lock.unlock()
    }

    /// Makes one command fail with a gateway error body.
    public func failCommand(_ name: GatewayCommandName, code: String, message: String) {
        lock.lock()
        failures[name.rawValue] = GatewayErrorBody(code: code, message: message)
        lock.unlock()
    }

    public func pendingPermission(for sessionId: String) -> PermissionRequestPayload? {
        lock.lock()
        defer { lock.unlock() }
        return pendingPermissions.first { pendingPermissionSessions[$0.key] == sessionId }?.value
    }

    /// Closes the active transport. `GatewayConnection` observes the close and
    /// reconnects on its backoff schedule.
    public func disconnect() {
        lock.lock()
        let transport = currentTransport
        currentTransport = nil
        lock.unlock()
        Task { await transport?.close() }
    }

    // MARK: Emitting events

    @discardableResult
    public func emit(_ event: GatewayEvent) -> GatewayEvent {
        lock.lock()
        sequences[event.sessionId] = max(sequences[event.sessionId] ?? 0, event.sequence)
        if let index = sessions.firstIndex(where: { $0.id == event.sessionId }) {
            sessions[index].lastSequence = max(sessions[index].lastSequence, event.sequence)
            sessions[index].lastActivityAt = event.at ?? Date()
        }
        let transport = currentTransport
        lock.unlock()
        transport?.push(.event(event))
        return event
    }

    @discardableResult
    public func emitEvent(
        named name: GatewayEventName,
        sessionId: String,
        payload: JSONValue? = nil,
        at: Date? = nil,
        sequence: Int? = nil
    ) -> GatewayEvent {
        let resolvedSequence = sequence ?? advanceSequence(for: sessionId)
        return emit(GatewayEvent(
            sessionId: sessionId,
            sequence: resolvedSequence,
            event: name.rawValue,
            at: at ?? Date(),
            payload: payload
        ))
    }

    @discardableResult
    public func emitMessage(
        sessionId: String,
        role: SessionMessage.Role,
        text: String,
        id: String? = nil,
        isStreaming: Bool = false,
        append: Bool = false
    ) -> GatewayEvent {
        let payload = try? codec.encodeToJSONValue(SessionMessagePayload(
            role: role,
            text: text,
            id: id,
            isStreaming: isStreaming,
            append: append
        ))
        return emitEvent(named: .sessionMessage, sessionId: sessionId, payload: payload)
    }

    @discardableResult
    public func emitToolCall(
        sessionId: String,
        id: String,
        title: String,
        kind: String? = nil,
        status: ToolCall.Status,
        detail: String? = nil
    ) -> GatewayEvent {
        let payload = try? codec.encodeToJSONValue(ToolCallPayload(
            id: id,
            title: title,
            kind: kind,
            status: status,
            detail: detail
        ))
        return emitEvent(named: .sessionToolCall, sessionId: sessionId, payload: payload)
    }

    @discardableResult
    public func emitPermissionRequest(sessionId: String, request: PermissionRequestPayload) -> GatewayEvent {
        lock.lock()
        pendingPermissions[request.id] = request
        pendingPermissionSessions[request.id] = sessionId
        lock.unlock()
        let payload = try? codec.encodeToJSONValue(request)
        return emitEvent(named: .sessionPermissionRequest, sessionId: sessionId, payload: payload)
    }

    @discardableResult
    public func emitStatus(sessionId: String, status: SessionStatus) -> GatewayEvent {
        lock.lock()
        if let index = sessions.firstIndex(where: { $0.id == sessionId }) {
            sessions[index].status = status
        }
        lock.unlock()
        let payload = try? codec.encodeToJSONValue(SessionStatusPayload(status: status))
        return emitEvent(named: .sessionStatus, sessionId: sessionId, payload: payload)
    }

    @discardableResult
    public func emitExit(sessionId: String, code: Int? = 0, reason: String) -> GatewayEvent {
        lock.lock()
        if let index = sessions.firstIndex(where: { $0.id == sessionId }) {
            sessions[index].status = .exited
        }
        lock.unlock()
        let payload = try? codec.encodeToJSONValue(SessionExitPayload(code: code, reason: reason))
        return emitEvent(named: .sessionExit, sessionId: sessionId, payload: payload)
    }

    /// Pushes a `resync_required` control frame. The connection resets its
    /// journal cursor and surfaces the resync state to attached views.
    public func emitResyncRequired(sessionId: String, reason: String) {
        lock.lock()
        let transport = currentTransport
        lock.unlock()
        transport?.push(.resyncRequired(ResyncRequired(reason: reason, sessionId: sessionId, oldestAvailableSequence: 1)))
    }

    // MARK: Inbound handling

    private func receive(_ frame: GatewayFrame, from transport: FakeHostTransport) {
        switch frame {
        case .hello:
            lock.lock()
            hellos += 1
            let connection = hellos
            let status = hostStatus
            lock.unlock()
            transport.push(.welcome(ServerWelcome(
                connectionId: "fake-conn-\(connection)",
                serverName: "Fake Slight Host",
                serverVersion: "0.1.0",
                hostId: "fake-host",
                capabilities: GatewayCapabilities(
                    supportsReplay: true,
                    maxReplayEvents: 200,
                    maxEventJournal: 500,
                    supportsRawAcp: false,
                    permissionOptions: true,
                    hostAdmin: true
                ),
                heartbeatIntervalMs: 60_000
            )))
            _ = status
        case .command(let command):
            lock.lock()
            commandLog.append(command)
            lock.unlock()
            onCommand?(command)
            respond(to: command, from: transport)
        case .ping, .pong, .welcome, .ack, .event, .resyncRequired, .error, .unknown:
            break
        }
    }

    private func respond(to command: GatewayCommand, from transport: FakeHostTransport) {
        lock.lock()
        let failure = failures[command.command]
        lock.unlock()
        if let failure {
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: false, error: failure)))
            return
        }

        func succeed(_ result: JSONValue? = nil) {
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
        }

        switch command.command {
        case GatewayCommandName.sessionList.rawValue:
            succeed(encode(SessionListResult(sessions: sessionsSnapshot())))

        case GatewayCommandName.sessionWorkingDirectories.rawValue:
            let paths = Array(Set(sessionsSnapshot().map(\.workingDirectoryLabel))).sorted()
            succeed(encode(SessionWorkingDirectoriesResult(paths: paths)))

        case GatewayCommandName.sessionCreate.rawValue:
            let summary = makeSession(from: command)
            lock.lock()
            sessions.insert(summary, at: 0)
            lock.unlock()
            succeed(encode(SessionCreateResult(session: summary)))

        case GatewayCommandName.agentSessionsList.rawValue:
            let agent = command.params?["agent"]?.stringValue
            lock.lock()
            let matching = agentSessions.filter { agent == nil || $0.agent == agent }
            lock.unlock()
            succeed(encode(AgentSessionsListResult(sessions: matching)))

        case GatewayCommandName.agentSessionImport.rawValue:
            let imported = makeImportedSession(from: command)
            lock.lock()
            sessions.insert(imported, at: 0)
            lock.unlock()
            succeed(encode(ImportAgentSessionResult(session: imported)))

        case GatewayCommandName.sessionAttach.rawValue:
            succeed(encode(attachResult(for: command)))

        case GatewayCommandName.sessionDetach.rawValue:
            succeed()

        case GatewayCommandName.sessionRename.rawValue:
            if let id = command.sessionId,
               let title = command.params?["title"]?.stringValue {
                lock.lock()
                if let index = sessions.firstIndex(where: { $0.id == id }) {
                    sessions[index].title = title
                    let renamed = sessions[index]
                    lock.unlock()
                    succeed(encode(SessionRenameResult(session: renamed)))
                } else {
                    lock.unlock()
                    succeed()
                }
            } else {
                succeed()
            }

        case GatewayCommandName.sessionArchive.rawValue:
            if let id = command.sessionId,
               case .bool(let archived) = command.params?["archived"] ?? .null {
                lock.lock()
                if let index = sessions.firstIndex(where: { $0.id == id }) {
                    sessions[index].archived = archived
                    let updated = sessions[index]
                    lock.unlock()
                    succeed(encode(SessionArchiveResult(session: updated)))
                } else {
                    lock.unlock()
                    succeed()
                }
            } else {
                succeed()
            }

        case GatewayCommandName.sessionSetMode.rawValue:
            succeed()

        case GatewayCommandName.sessionInput.rawValue:
            succeed()
            if let autoReplyToInput, let text = command.params?["text"]?.stringValue, let id = command.sessionId {
                autoReplyToInput(id, text)
            }

        case GatewayCommandName.sessionCancel.rawValue:
            if let id = command.sessionId {
                _ = emitStatus(sessionId: id, status: .idle)
            }
            succeed()

        case GatewayCommandName.permissionRespond.rawValue:
            let permissionId = command.params?["permission_id"]?.stringValue
            let optionId = command.params?["option_id"]?.stringValue ?? "allow"
            if let permissionId {
                lock.lock()
                pendingPermissions.removeValue(forKey: permissionId)
                pendingPermissionSessions.removeValue(forKey: permissionId)
                lock.unlock()
                if let id = command.sessionId {
                    let payload = try? codec.encodeToJSONValue(
                        PermissionResolvedPayload(id: permissionId, optionId: optionId)
                    )
                    _ = emitEvent(named: .sessionPermissionResolved, sessionId: id, payload: payload)
                    _ = emitStatus(sessionId: id, status: .working)
                }
            }
            succeed()

        case GatewayCommandName.eventsReplay.rawValue:
            succeed(encode(replayResult(for: command)))

        case GatewayCommandName.sessionInspect.rawValue:
            succeed(encode(inspectResult(for: command)))

        case GatewayCommandName.sessionHistory.rawValue:
            succeed(encode(historyResult(for: command)))

        case GatewayCommandName.hostStatus.rawValue:
            succeed(encode(hostStatus))

        case GatewayCommandName.hostConfiguration.rawValue:
            // `host.configuration` currently returns the host status payload.
            succeed(encode(hostStatus))

        case GatewayCommandName.hostDiagnostics.rawValue:
            succeed(encode(hostDiagnostics))

        case GatewayCommandName.hostStart.rawValue:
            setHostState(.running)
            succeed(encode(HostLifecycleResult(state: .running, message: "Host started")))

        case GatewayCommandName.hostStop.rawValue:
            setHostState(.stopped)
            succeed(encode(HostLifecycleResult(state: .stopped, message: "Host stopped")))

        case GatewayCommandName.hostRestart.rawValue:
            setHostState(.running)
            succeed(encode(HostLifecycleResult(state: .running, message: "Host restarted")))

        case GatewayCommandName.deviceList.rawValue:
            lock.lock()
            let current = devices
            lock.unlock()
            succeed(encode(["devices": current]))

        case GatewayCommandName.deviceRevoke.rawValue:
            let deviceId = command.params?["deviceId"]?.stringValue
            lock.lock()
            if let deviceId, let index = devices.firstIndex(where: { $0.deviceId == deviceId }) {
                devices[index].revoked = true
            }
            lock.unlock()
            succeed()

        case GatewayCommandName.pairingCreate.rawValue:
            lock.lock()
            let pairing = nextPairing
            lock.unlock()
            succeed(encode(pairing))

        default:
            succeed()
        }
    }

    // MARK: Result builders

    private func sessionsSnapshot() -> [SessionSummary] {
        lock.lock()
        defer { lock.unlock() }
        return sessions
    }

    private func makeSession(from command: GatewayCommand) -> SessionSummary {
        let title = command.params?["title"]?.stringValue ?? "New session"
        let agent = command.params?["agent"]?.stringValue ?? "fake"
        let directory = command.params?["working_directory_label"]?.stringValue ?? "~"
        return SessionSummary(
            id: "sess-\(UUID().uuidString.prefix(8))",
            title: title,
            agent: agent,
            model: command.params?["model"]?.stringValue ?? "auto",
            effort: command.params?["effort"]?.stringValue ?? "medium",
            workingDirectoryLabel: directory,
            status: .idle,
            createdAt: Date(),
            lastActivityAt: Date(),
            lastSequence: 0
        )
    }

    private func makeImportedSession(from command: GatewayCommand) -> SessionSummary {
        let agent = command.params?["agent"]?.stringValue ?? "fake"
        let directory = command.params?["working_directory_label"]?.stringValue ?? "~"
        let title = command.params?["title"]?.stringValue ?? "Imported session"
        return SessionSummary(
            id: "sess-\(UUID().uuidString.prefix(8))",
            title: title,
            agent: agent,
            model: "auto",
            workingDirectoryLabel: directory,
            status: .idle,
            createdAt: Date(),
            lastActivityAt: Date(),
            lastSequence: 0,
            recovery: .live
        )
    }

    private func attachResult(for command: GatewayCommand) -> SessionAttachResult {
        let sessionId = command.sessionId ?? ""
        let afterSequence = command.params?["afterSequence"]?.intValue ?? 0
        lock.lock()
        let summary = sessions.first(where: { $0.id == sessionId }) ?? SessionSummary(
            id: sessionId,
            title: sessionId,
            agent: "fake",
            model: "auto",
            effort: "medium",
            workingDirectoryLabel: "~",
            status: .idle
        )
        let events = (replayEventsBySession[sessionId] ?? []).filter { $0.sequence > afterSequence }
        let latest = max(events.map(\.sequence).max() ?? 0, sequences[sessionId] ?? 0)
        let resync = resyncSessions.contains(sessionId)
        lock.unlock()
        return SessionAttachResult(
            session: summary,
            replayed: events,
            latestSequence: latest,
            oldestAvailableSequence: resync ? 1 : nil,
            resyncRequired: resync
        )
    }

    private func replayResult(for command: GatewayCommand) -> SessionAttachResult {
        let sessionId = command.sessionId ?? ""
        let fromSequence = command.params?["fromSequence"]?.intValue ?? 1
        lock.lock()
        let summary = sessions.first(where: { $0.id == sessionId }) ?? SessionSummary(
            id: sessionId,
            title: sessionId,
            agent: "fake",
            model: "auto",
            effort: "medium",
            workingDirectoryLabel: "~",
            status: .idle
        )
        let events = (replayEventsBySession[sessionId] ?? []).filter { $0.sequence >= fromSequence }
        let latest = max(events.map(\.sequence).max() ?? 0, sequences[sessionId] ?? 0)
        lock.unlock()
        return SessionAttachResult(
            session: summary,
            replayed: events,
            latestSequence: latest,
            oldestAvailableSequence: nil,
            resyncRequired: false
        )
    }

    private func historyResult(for command: GatewayCommand) -> SessionHistoryResult {
        let sessionId = command.sessionId ?? ""
        let beforeSequence = command.params?["before_sequence"]?.intValue
        let limit = command.params?["limit"]?.intValue ?? 100
        lock.lock()
        let summary = sessions.first(where: { $0.id == sessionId }) ?? SessionSummary(
            id: sessionId,
            title: sessionId,
            agent: "fake",
            model: "auto",
            effort: "medium",
            workingDirectoryLabel: "~",
            status: .idle
        )
        let allEvents = replayEventsBySession[sessionId] ?? []
        let boundary = beforeSequence ?? Int.max
        let eligible = allEvents.filter { $0.sequence < boundary }
        let events = Array(eligible.suffix(min(limit, 100)))
        let latest = max(allEvents.map(\ .sequence).max() ?? 0, sequences[sessionId] ?? 0)
        let hasMore = eligible.count > events.count
        lock.unlock()
        return SessionHistoryResult(
            session: summary,
            events: events,
            latestSequence: latest,
            oldestAvailableSequence: allEvents.map(\ .sequence).min(),
            hasMore: hasMore
        )
    }

    private func inspectResult(for command: GatewayCommand) -> SessionInspectResult {
        let sessionId = command.sessionId ?? ""
        lock.lock()
        let summary = sessions.first(where: { $0.id == sessionId }) ?? SessionSummary(
            id: sessionId,
            title: sessionId,
            agent: "fake",
            model: "auto",
            effort: "medium",
            workingDirectoryLabel: "~",
            status: .idle
        )
        let events = replayEventsBySession[sessionId] ?? []
        let pending = pendingPermissions.first { pendingPermissionSessions[$0.key] == sessionId }?.value
        lock.unlock()
        return SessionInspectResult(
            session: summary,
            agent: AgentDescriptor(kind: summary.agent, displayName: summary.agent),
            acp: SessionAcpMetadata(
                modes: AcpModeState(
                    currentModeId: "auto",
                    availableModes: [
                        AcpMode(id: "none", name: "None"),
                        AcpMode(id: "auto", name: "Auto"),
                        AcpMode(id: "all", name: "All"),
                    ]
                )
            ),
            running: summary.status == .working || summary.status == .idle,
            pendingPermission: pending,
            recentEvents: events
        )
    }

    private func setHostState(_ state: HostRunState) {
        lock.lock()
        hostStatus.state = state
        lock.unlock()
    }

    private func advanceSequence(for sessionId: String) -> Int {
        lock.lock()
        defer { lock.unlock() }
        let next = (sequences[sessionId] ?? 0) + 1
        sequences[sessionId] = next
        return next
    }

    private func encode<T: Encodable>(_ value: T) -> JSONValue? {
        try? codec.encodeToJSONValue(value)
    }
}

// MARK: - Default fixtures

public extension FakeHost {
    static let defaultHostStatus = HostStatus(
        state: .running,
        hostName: "fake-mac.local",
        serverId: "fake-host",
        serverVersion: "0.1.0",
        protocolVersion: GatewayProtocol.version,
        listener: "127.0.0.1:8787",
        sessionCount: 3,
        activeSessionCount: 1,
        pairedDeviceCount: 1,
        uptimeMs: 3_725_000,
        supportedAgents: ["claude_code", "codex", "fake"]
    )

    static let defaultDiagnostics = HostDiagnostics(
        serverVersion: "0.1.0",
        protocolVersion: GatewayProtocol.version,
        logLevel: "info",
        sessions: [
            SessionDiagnostics(
                sessionId: "sess-1",
                status: "waiting_permission",
                agent: "claude_code",
                lastSequence: 6,
                eventCount: 6,
                running: true
            ),
        ],
        recentLogs: [
            LogEntry(timestamp: Date(), level: "info", message: "host started on 127.0.0.1:8787"),
            LogEntry(timestamp: Date(), level: "debug", message: "accepted pair request"),
        ],
        capabilities: GatewayCapabilities()
    )

    static let defaultPairing = PairingArtifact(
        pairingId: "pair-1",
        code: "4A9K-2QX7",
        label: "Slight client",
        expiresAt: Date().addingTimeInterval(300),
        qrPayload: "slight://pair?code=4A9K-2QX7&host=fake-host"
    )
}

/// Static sample data for previews and tests.
public enum FakeHostFixture {
    public static let sessionId = "sess-1"

    public static func sessions() -> [SessionSummary] {
        [
            SessionSummary(
                id: sessionId,
                title: "Fix the login flow",
                agent: "claude_code",
                model: "auto",
                effort: "high",
                workingDirectoryLabel: "~/dev/acme-app",
                gitBranch: "feature/login-flow",
                status: .waitingPermission,
                createdAt: Date().addingTimeInterval(-3_600),
                lastActivityAt: Date().addingTimeInterval(-20),
                lastSequence: 6
            ),
            SessionSummary(
                id: "sess-2",
                title: "Add snapshot tests",
                agent: "codex",
                model: "auto",
                effort: "medium",
                workingDirectoryLabel: "~/dev/acme-app",
                gitBranch: "tests/snapshots",
                status: .working,
                createdAt: Date().addingTimeInterval(-7_200),
                lastActivityAt: Date().addingTimeInterval(-90),
                lastSequence: 3
            ),
            SessionSummary(
                id: "sess-3",
                title: "Bump dependencies",
                agent: "fake",
                model: "auto",
                effort: "low",
                workingDirectoryLabel: "~/dev/labs",
                gitBranch: "chore/dependencies",
                status: .exited,
                createdAt: Date().addingTimeInterval(-86_400),
                lastActivityAt: Date().addingTimeInterval(-80_000),
                lastSequence: 12,
                recovery: .stale(reason: "the agent no longer retains this session")
            ),
            SessionSummary(
                id: "sess-4",
                title: "Investigate flaky deploy",
                agent: "codex",
                model: "auto",
                effort: "medium",
                workingDirectoryLabel: "~/dev/infra",
                status: .exited,
                createdAt: Date().addingTimeInterval(-172_800),
                lastActivityAt: Date().addingTimeInterval(-160_000),
                lastSequence: 4,
                recovery: .unavailable(reason: "agent codex is not installed on this host")
            ),
        ]
    }

    /// Native sessions an agent would report through `agent.sessions.list`.
    public static func agentSessions() -> [AgentSessionSummary] {
        [
            AgentSessionSummary(
                agent: "claude_code",
                agentSessionId: "claude-9f2c",
                cwd: "/Users/me/dev/acme-app",
                title: "Fix the login flow",
                updatedAt: Date().addingTimeInterval(-40)
            ),
            AgentSessionSummary(
                agent: "claude_code",
                agentSessionId: "claude-71ab",
                cwd: "/Users/me/dev/acme-app",
                additionalDirectories: ["/Users/me/dev/shared"],
                title: "Add snapshot tests",
                updatedAt: Date().addingTimeInterval(-5_400)
            ),
            AgentSessionSummary(
                agent: "codex",
                agentSessionId: "codex-0031",
                cwd: "/Users/me/dev/infra",
                title: nil,
                updatedAt: nil
            ),
        ]
    }

    public static func devices() -> [PairedDevice] {
        [
            PairedDevice(
                deviceId: "device-iphone",
                label: "Samuel's iPhone",
                createdAt: Date().addingTimeInterval(-86_400),
                lastSeenAt: Date().addingTimeInterval(-120),
                revoked: false
            ),
            PairedDevice(
                deviceId: "device-ipad",
                label: "Studio iPad",
                createdAt: Date().addingTimeInterval(-172_800),
                lastSeenAt: Date().addingTimeInterval(-8_000),
                revoked: true
            ),
        ]
    }

    public static let permission = PermissionRequestPayload(
        id: "perm-1",
        title: "Run shell command?",
        detail: "git status --short",
        toolCallId: "tool-edit",
        options: [
            PermissionOption(optionId: "allow", label: "Allow once", kind: .allow),
            PermissionOption(optionId: "allow_always", label: "Always allow git", kind: .allowAlways),
            PermissionOption(optionId: "deny", label: "Deny", kind: .deny),
        ]
    )

    /// Builds a preloaded transcript for `sessionId`: a user prompt, assistant
    /// reasoning, completed and in-progress tool calls, a pending permission
    /// request, and a streaming assistant message.
    public static func replayEvents(for sessionId: String = sessionId) -> [GatewayEvent] {
        let codec = GatewayCodec()
        func event(_ sequence: Int, _ name: GatewayEventName, _ payload: JSONValue?) -> GatewayEvent {
            GatewayEvent(
                sessionId: sessionId,
                sequence: sequence,
                event: name.rawValue,
                at: Date().addingTimeInterval(Double(sequence) * -3),
                payload: payload
            )
        }
        func message(_ sequence: Int, role: SessionMessage.Role, text: String, streaming: Bool = false) -> GatewayEvent {
            event(sequence, .sessionMessage, try? codec.encodeToJSONValue(
                SessionMessagePayload(role: role, text: text, isStreaming: streaming)
            ))
        }
        func tool(_ sequence: Int, id: String, title: String, kind: String, status: ToolCall.Status, detail: String? = nil) -> GatewayEvent {
            event(sequence, .sessionToolCall, try? codec.encodeToJSONValue(
                ToolCallPayload(id: id, title: title, kind: kind, status: status, detail: detail)
            ))
        }
        return [
            message(1, role: .user, text: "The login button does nothing on iOS."),
            message(2, role: .assistant, text: "Let me look at the authentication screen and the current git state."),
            tool(3, id: "tool-read", title: "Read LoginView.swift", kind: "read", status: .completed, detail: "142 lines"),
            tool(4, id: "tool-edit", title: "Edit LoginView.swift", kind: "edit", status: .inProgress),
            message(5, role: .assistant, text: "I found the issue: the button's action is only wired when the feature flag is on. ", streaming: true),
            event(6, .sessionPermissionRequest, try? codec.encodeToJSONValue(permission)),
        ]
    }

    /// A host preloaded with sessions, a transcript, a permission request, and
    /// a scripted streaming reply for previews.
    public static func demoHost() -> FakeHost {
        let host = FakeHost(
            sessions: sessions(),
            hostStatus: FakeHost.defaultHostStatus,
            hostDiagnostics: FakeHost.defaultDiagnostics,
            devices: devices(),
            nextPairing: FakeHost.defaultPairing,
            agentSessions: agentSessions()
        )
        host.setReplayEvents(replayEvents(), for: sessionId)
        host.autoReplyToInput = { [weak host] session, text in
            host?.streamAssistantReply(sessionId: session, prompt: text)
        }
        return host
    }
}

public extension FakeHost {
    /// Streams a short assistant reply in chunks, ending with a completed
    /// message and an idle status. Used by the demo fixture.
    func streamAssistantReply(
        sessionId: String,
        prompt: String,
        chunkDelay: Duration = .milliseconds(120)
    ) {
        let messageId = "reply-\(UUID().uuidString.prefix(6))"
        let chunks = [
            "Looking into \"\(prompt)\". ",
            "I'll trace the call site and ",
            "then run the focused tests.",
        ]
        Task { [weak self] in
            guard let self else { return }
            _ = self.emitStatus(sessionId: sessionId, status: .working)
            try? await Task.sleep(for: .milliseconds(150))
            _ = self.emitMessage(sessionId: sessionId, role: .assistant, text: "", id: messageId, isStreaming: true)
            for chunk in chunks {
                try? await Task.sleep(for: chunkDelay)
                _ = self.emitMessage(sessionId: sessionId, role: .assistant, text: chunk, id: messageId, isStreaming: true, append: true)
            }
            try? await Task.sleep(for: chunkDelay)
            _ = self.emitMessage(sessionId: sessionId, role: .assistant, text: "", id: messageId, isStreaming: false, append: true)
            _ = self.emitStatus(sessionId: sessionId, status: .idle)
        }
    }
}
#endif
