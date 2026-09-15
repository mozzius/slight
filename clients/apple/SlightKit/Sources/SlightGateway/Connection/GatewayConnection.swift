import Foundation
import OSLog

/// Events pushed to consumers by `GatewayConnection`.
public enum GatewayInbound: Sendable {
    case stateChanged(GatewayConnectionState)
    case welcome(ServerWelcome)
    case event(GatewayEvent)
    case duplicateEvent(GatewayEvent)
    case replayRequested(sessionId: String, fromSequence: Int)
    /// A replay/attach response has been fully ingested for `sessionId`. Views
    /// use this to leave their replaying state even when the host does not send
    /// an explicit `replay.complete` event.
    case replayCompleted(sessionId: String, resyncRequired: Bool)
    /// The client must discard its local transcript for `sessionId` and rebuild
    /// it from the events that follow. Emitted on a first attach or a resync.
    case transcriptReset(sessionId: String, reason: String?)
    /// The host cannot replay the requested range; the client should re-attach
    /// from the retained journal.
    case resyncRequired(sessionId: String?, reason: String?)
    case ack(GatewayAck)
    case serverError(GatewayProtocolError)
    case close(reason: String?)
}

public struct GatewayConnectionConfig: Sendable {
    public var clientId: String
    public var clientVersion: String
    public var reconnect: ReconnectPolicy
    public var handshakeTimeout: Duration
    public var commandTimeout: Duration
    public var heartbeatTimeout: Duration

    public init(
        clientId: String = UUID().uuidString,
        clientVersion: String = "0.1.0",
        reconnect: ReconnectPolicy = ReconnectPolicy(),
        handshakeTimeout: Duration = .seconds(8),
        commandTimeout: Duration = .seconds(20),
        heartbeatTimeout: Duration = .seconds(45)
    ) {
        self.clientId = clientId
        self.clientVersion = clientVersion
        self.reconnect = reconnect
        self.handshakeTimeout = handshakeTimeout
        self.commandTimeout = commandTimeout
        self.heartbeatTimeout = heartbeatTimeout
    }
}

/// Owns one logical gateway connection across reconnects.
///
/// Responsibilities:
/// - hello/welcome handshake with optional resume;
/// - ping/pong heartbeats and a liveness watchdog;
/// - request IDs, acknowledgements, and command timeouts;
/// - per-session sequence tracking with replay-on-gap;
/// - reconnect with backoff via `ConnectionStateMachine`.
///
/// Every dependency that touches the outside world (transport, clock,
/// credential storage) is injected so the connection can be driven by an
/// in-memory transport in tests.
public actor GatewayConnection {
    private static let logger = Logger(subsystem: "sh.slight.client", category: "gateway.connection")
    public private(set) var state: GatewayConnectionState = .idle
    private var subscribers: [UUID: AsyncStream<GatewayInbound>.Continuation] = [:]

    private let codec: GatewayCodec
    private let config: GatewayConnectionConfig
    private var transportFactory: GatewayTransportFactory
    private let clock: GatewayClock
    private var credentialProvider: @Sendable () async -> DeviceCredential?
    private let resumeProvider: @Sendable () -> ResumeRequest?

    private var stateMachine: ConnectionStateMachine
    private var journal = EventJournal()
    private var transport: GatewayTransport?
    private var runTask: Task<Void, Never>?
    private var heartbeatTask: Task<Void, Never>?
    private var pending: [String: CheckedContinuation<GatewayAck, Error>] = [:]
    private var pendingCommandNames: [String: String] = [:]
    private var backgroundReplayRequests: Set<String> = []
    private var pendingAttachSessions: Set<String> = []
    private var pendingAttachSessionByRequest: [String: String] = [:]
    private var deferredAttachEvents: [String: [GatewayEvent]] = [:]
    private var attachedSessions: Set<String> = []
    private var timeouts: [String: Task<Void, Never>] = [:]
    private var currentWelcome: ServerWelcome?
    private var lastInboundAt: Date?
    private var isStopping = false

    public init(
        transportFactory: @escaping GatewayTransportFactory,
        codec: GatewayCodec = GatewayCodec(),
        config: GatewayConnectionConfig = GatewayConnectionConfig(),
        clock: GatewayClock = SystemGatewayClock(),
        credentialProvider: @escaping @Sendable () async -> DeviceCredential? = { nil },
        resumeProvider: @escaping @Sendable () -> ResumeRequest? = { nil }
    ) {
        self.transportFactory = transportFactory
        self.codec = codec
        self.config = config
        self.clock = clock
        self.credentialProvider = credentialProvider
        self.resumeProvider = resumeProvider
        self.stateMachine = ConnectionStateMachine(policy: config.reconnect)
    }

    // MARK: Subscriptions

    /// Registers a new subscriber. Events are fanned out to every active
    /// subscriber with a bounded per-subscriber buffer, so one slow UI view
    /// cannot stall the connection or starve another view.
    public func subscribe() -> AsyncStream<GatewayInbound> {
        let id = UUID()
        let (stream, continuation) = AsyncStream.makeStream(
            of: GatewayInbound.self,
            bufferingPolicy: .bufferingNewest(8192)
        )
        continuation.onTermination = { [weak self] _ in
            Task { await self?.removeSubscriber(id) }
        }
        subscribers[id] = continuation
        return stream
    }

    private func removeSubscriber(_ id: UUID) {
        subscribers.removeValue(forKey: id)
    }

    // MARK: Lifecycle

    public func start() {
        guard runTask == nil else { return }
        Self.logger.info("starting gateway connection")
        isStopping = false
        runTask = Task { [weak self] in
            await self?.run()
        }
    }

    /// Explicitly starts a fresh connection attempt. Used by user-facing
    /// Retry actions, including after the previous run reached a terminal
    /// failure state.
    public func reconnect() async {
        await stop()
        start()
    }

    public func stop() async {
        Self.logger.info("stopping gateway connection")
        isStopping = true
        let task = runTask
        runTask = nil
        task?.cancel()
        heartbeatTask?.cancel()
        heartbeatTask = nil
        stateMachine.markClosed()
        publishState()
        await closeTransport()
        failAllPending(with: GatewayConnectionError.transportClosed)
        // Wait for the previous run loop to fully unwind so its cleanup cannot
        // clear a run task installed by a subsequent `start()`.
        await task?.value
        emit(.close(reason: "client_stopped"))
    }

    /// Replaces the host endpoint wiring and, if the connection was running,
    /// reconnects against the new transport. The connection object stays the
    /// same so view models and subscriptions holding it remain valid when the
    /// user points the client at a different host.
    public func reconfigure(
        transportFactory: @escaping GatewayTransportFactory,
        credentialProvider: (@Sendable () async -> DeviceCredential?)? = nil
    ) async {
        let wasRunning = runTask != nil
        await stop()
        self.transportFactory = transportFactory
        if let credentialProvider {
            self.credentialProvider = credentialProvider
        }
        // Per-session cursors and attachments belong to the previous host; a new
        // endpoint starts from a clean slate.
        journal = EventJournal()
        attachedSessions.removeAll()
        backgroundReplayRequests.removeAll()
        currentWelcome = nil
        if wasRunning {
            start()
        }
    }

    public func journalSnapshot() -> [String: Int] {
        journal.lastSequenceBySession
    }

    // MARK: Commands

    public func send(_ command: GatewayCommand) async throws -> GatewayAck {
        guard transport != nil else { throw GatewayConnectionError.notConnected }
        if Self.attachLikeCommands.contains(command.command), let sessionId = command.sessionId {
            pendingAttachSessions.insert(sessionId)
            pendingAttachSessionByRequest[command.requestId] = sessionId
        }
        return try await withCheckedThrowingContinuation { continuation in
            pending[command.requestId] = continuation
            pendingCommandNames[command.requestId] = command.command
            startTimeout(for: command.requestId)
            Task { [weak self] in
                guard let self else { return }
                do {
                    try await self.sendFrame(.command(command))
                } catch {
                    Self.logger.error("command send failed request=\(command.requestId, privacy: .public) command=\(command.command, privacy: .public) error=\(String(describing: error), privacy: .public)")
                    await self.failPending(command.requestId, error: error)
                    await self.closeTransport()
                }
            }
        }
    }

    @discardableResult
    public func perform(
        _ name: GatewayCommandName,
        sessionId: String? = nil,
        params: JSONValue? = nil
    ) async throws -> JSONValue? {
        let ack = try await send(GatewayCommand(name: name, sessionId: sessionId, params: params))
        guard ack.ok else {
            let error = ack.error ?? GatewayErrorBody(code: "command_failed", message: "\(name.rawValue) failed")
            throw GatewayConnectionError.commandFailed(command: name.rawValue, code: error.code, message: error.message)
        }
        return ack.result
    }

    // MARK: Connection loop

    private func run() async {
        while !Task.isCancelled {
            var failureReason: String?
            _ = stateMachine.handle(.connectRequested)
            publishState()
            Self.logger.info("connection attempt started")

            do {
                try await openTransport()
                _ = stateMachine.handle(.transportOpened)
                publishState()

                let welcome = try await handshake()
                currentWelcome = welcome
                _ = stateMachine.handle(.handshakeSucceeded(welcome))
                publishState()
                emit(.welcome(welcome))
                startHeartbeatWatchdog(intervalMs: welcome.heartbeatIntervalMs)
                restoreAttachments()

                try await receiveLoop()
                throw GatewayConnectionError.transportClosed
            } catch is CancellationError {
                break
            } catch {
                failureReason = connectionFailureMessage(for: error)
                Self.logger.error("connection attempt failed: \(String(describing: error), privacy: .public)")
                emit(.serverError(GatewayProtocolError(
                    code: "connection_attempt_failed",
                    message: failureReason ?? "Unable to reach the host."
                )))
            }

            heartbeatTask?.cancel()
            heartbeatTask = nil
            await closeTransport()
            failAllPending(with: GatewayConnectionError.transportClosed)

            guard !Task.isCancelled, !isStopping else { break }

            let effect = stateMachine.handle(.disconnected(reason: failureReason))
            publishState()
            if case .scheduleReconnect(let delay) = effect {
                emit(.close(reason: "reconnecting"))
                do {
                    try await clock.sleep(for: delay)
                } catch {
                    break
                }
                guard !Task.isCancelled, !isStopping else { break }
                _ = stateMachine.handle(.reconnectDelayElapsed)
                publishState()
            } else {
                if case .failed(let failure) = stateMachine.state {
                    emit(.serverError(GatewayProtocolError(code: failure.code, message: failure.message)))
                }
                break
            }
        }

        heartbeatTask?.cancel()
        heartbeatTask = nil
        await closeTransport()
        failAllPending(with: GatewayConnectionError.transportClosed)
        runTask = nil
    }

    private func connectionFailureMessage(for error: Error) -> String {
        if let connectionError = error as? GatewayConnectionError {
            return connectionError.userMessage
        }
        if let urlError = error as? URLError {
            switch urlError.code {
            case .cannotFindHost:
                return "The host name could not be resolved."
            case .cannotConnectToHost:
                return "The host refused the connection. Check its address and listener configuration."
            case .secureConnectionFailed, .serverCertificateHasBadDate,
                 .serverCertificateUntrusted, .serverCertificateHasUnknownRoot,
                 .serverCertificateNotYetValid:
                return "The secure WebSocket connection failed. Check the host's TLS configuration and certificate."
            case .timedOut:
                return "The connection attempt timed out."
            default:
                return urlError.localizedDescription
            }
        }
        return error.localizedDescription
    }

    private func openTransport() async throws {
        Self.logger.debug("creating gateway transport")
        let newTransport = try transportFactory()
        transport = newTransport
        try await newTransport.start()
        Self.logger.info("gateway transport started")
    }

    private func closeTransport() async {
        guard let transport else { return }
        self.transport = nil
        await transport.close()
    }

    private func handshake() async throws -> ServerWelcome {
        let credential = await credentialProvider()
        let hello = ClientHello(
            clientId: config.clientId,
            clientVersion: config.clientVersion,
            deviceId: credential?.deviceId,
            credential: credential?.token,
            resume: resumeProvider()
        )
        Self.logger.debug("sending hello client_id=\(self.config.clientId, privacy: .public) credential_present=\(credential != nil) resume_present=\(hello.resume != nil)")
        try await sendFrame(.hello(hello))

        while !Task.isCancelled {
            let frame = try await receiveFrame(timeout: config.handshakeTimeout)
            Self.logger.debug("received handshake frame \(frame.frameType, privacy: .public)")
            switch frame {
            case .welcome(let welcome):
                guard welcome.protocolVersion >= GatewayProtocol.minimumSupportedVersion else {
                    throw GatewayConnectionError.handshakeFailed("unsupported protocol version \(welcome.protocolVersion)")
                }
                Self.logger.info("gateway handshake succeeded server=\(welcome.serverName, privacy: .public) version=\(welcome.serverVersion, privacy: .public) protocol=\(welcome.protocolVersion)")
                return welcome
            case .ping(let ping):
                try await sendFrame(.pong(PongFrame(nonce: ping.nonce)))
            case .error(let error):
                throw GatewayConnectionError.handshakeFailed(error.message)
            default:
                continue
            }
        }
        throw CancellationError()
    }

    private func receiveLoop() async throws {
        while !Task.isCancelled {
            guard let transport else { throw GatewayConnectionError.transportClosed }
            let data = try await transport.receive()
            guard let data else { throw GatewayConnectionError.transportClosed }
            lastInboundAt = clock.now()
            let frame: GatewayFrame
            do {
                frame = try codec.decode(data)
            } catch {
                Self.logger.error("failed to decode inbound gateway frame bytes=\(data.count) error=\(String(describing: error), privacy: .public)")
                throw error
            }
            Self.logger.debug("received gateway frame \(frame.frameType, privacy: .public) bytes=\(data.count)")
            await handle(frame)
        }
    }

    private func sendFrame(_ frame: GatewayFrame) async throws {
        guard let transport else { throw GatewayConnectionError.transportClosed }
        let data = try codec.encode(frame)
        Self.logger.debug("sending gateway frame \(frame.frameType, privacy: .public) bytes=\(data.count)")
        try await transport.send(data)
    }

    private func receiveFrame(timeout: Duration) async throws -> GatewayFrame {
        guard let transport else { throw GatewayConnectionError.transportClosed }
        let data = try await withTimeout(timeout) {
            try await transport.receive()
        }
        guard let data else { throw GatewayConnectionError.transportClosed }
        do {
            return try codec.decode(data)
        } catch {
            Self.logger.error("failed to decode handshake frame bytes=\(data.count) error=\(String(describing: error), privacy: .public)")
            throw error
        }
    }

    // MARK: Inbound handling

    private func handle(_ frame: GatewayFrame) async {
        switch frame {
        case .event(let event):
            handleEvent(event)
        case .ack(let ack):
            handleAck(ack)
        case .ping(let ping):
            try? await sendFrame(.pong(PongFrame(nonce: ping.nonce)))
        case .pong:
            break
        case .resyncRequired(let resync):
            journal.remove(sessionId: resync.sessionId)
            if let sessionId = resync.sessionId {
                // Force the next attach to be treated as a first attach so the
                // client rebuilds its transcript instead of merging into stale
                // state.
                attachedSessions.remove(sessionId)
            }
            _ = stateMachine.handle(.resyncRequired(reason: resync.reason))
            publishState()
            emit(.resyncRequired(sessionId: resync.sessionId, reason: resync.reason))
            emit(.serverError(GatewayProtocolError(code: "resync_required", message: resync.reason)))
        case .error(let error):
            if let requestId = error.requestId {
                failPending(requestId, error: GatewayConnectionError.commandFailed(
                    command: requestId, code: error.code, message: error.message
                ))
            }
            emit(.serverError(error))
        case .welcome(let welcome):
            currentWelcome = welcome
            emit(.welcome(welcome))
        case .hello, .command, .unknown:
            break
        }
    }

    private static let attachLikeCommands: Set<String> = [
        GatewayCommandName.sessionAttach.rawValue,
        GatewayCommandName.resume.rawValue,
        GatewayCommandName.eventsReplay.rawValue,
    ]

    private func handleAck(_ ack: GatewayAck) {
        let commandName = pendingCommandNames[ack.requestId]
        let isBackgroundReplay = backgroundReplayRequests.remove(ack.requestId) != nil
        let pendingAttachSession = pendingAttachSessionByRequest.removeValue(forKey: ack.requestId)
        resolveAck(ack)
        if ack.ok, let commandName, Self.attachLikeCommands.contains(commandName) {
            ingestAttachResult(ack, isBackgroundReplay: isBackgroundReplay)
        }
        if let sessionId = pendingAttachSession {
            pendingAttachSessions.remove(sessionId)
            let deferred = deferredAttachEvents.removeValue(forKey: sessionId) ?? []
            for event in deferred.sorted(by: { $0.sequence < $1.sequence }) {
                handleEvent(event)
            }
        }
        emit(.ack(ack))
    }

    /// Replay/attach responses carry their events inline in the `ack` result
    /// instead of as `event` frames. Ingest them so the journal cursor and any
    /// attached view stay consistent, and emit a reset when the client must
    /// rebuild its transcript from scratch.
    private func ingestAttachResult(_ ack: GatewayAck, isBackgroundReplay: Bool) {
        guard let result = ack.result,
              let attach = try? codec.decodePayload(SessionAttachResult.self, from: result) else {
            return
        }
        let sessionId = attach.session.id

        if isBackgroundReplay {
            replayEvents(attach.replayed, sessionId: sessionId, resync: false)
            let lastSequence = journal.lastSequence(for: sessionId)
            if lastSequence < attach.latestSequence {
                let nextSequence = lastSequence + 1
                emit(.replayRequested(sessionId: sessionId, fromSequence: nextSequence))
                requestReplay(
                    sessionId: sessionId,
                    fromSequence: nextSequence,
                    received: lastSequence
                )
                return
            }
            if let welcome = currentWelcome {
                _ = stateMachine.handle(.handshakeSucceeded(welcome))
                publishState()
            }
            emit(.replayCompleted(sessionId: sessionId, resyncRequired: false))
            return
        }

        let isFirstAttach = !attachedSessions.contains(sessionId)
        attachedSessions.insert(sessionId)
        if isFirstAttach || attach.resyncRequired {
            emit(.transcriptReset(
                sessionId: sessionId,
                reason: attach.resyncRequired ? "resync_required" : nil
            ))
        }
        // The requesting SessionViewModel consumes attach.replayed directly
        // from the acknowledgement. Rebroadcasting the entire transcript to
        // every subscriber makes replay size-dependent on each UI buffer.
        if attach.resyncRequired {
            journal.remove(sessionId: sessionId)
            if let oldest = attach.oldestAvailableSequence, oldest > 0 {
                journal.markReplayed(sessionId: sessionId, to: oldest - 1)
            }
        }
        journal.markReplayed(sessionId: sessionId, to: attach.latestSequence)
        emit(.replayCompleted(sessionId: sessionId, resyncRequired: attach.resyncRequired))
    }

    private func replayEvents(
        _ events: [GatewayEvent],
        sessionId: String,
        resync: Bool,
        oldest: Int? = nil,
        forceEmit: Bool = false
    ) {
        if resync {
            journal.remove(sessionId: sessionId)
            if let oldest, oldest > 0 {
                journal.markReplayed(sessionId: sessionId, to: oldest - 1)
            }
        }
        for event in events {
            if forceEmit {
                // A new detail view may need the retained transcript even when
                // the shared connection has already seen these sequence numbers.
                switch journal.observe(event) {
                case .accepted, .duplicate:
                    emit(.event(event))
                case .gap:
                    handleEvent(event)
                }
            } else {
                handleEvent(event)
            }
        }
    }

    private func handleEvent(_ event: GatewayEvent) {
        if event.event == GatewayEventName.replayComplete.rawValue {
            journal.markReplayed(sessionId: event.sessionId, to: event.sequence)
            if let currentWelcome {
                _ = stateMachine.handle(.handshakeSucceeded(currentWelcome))
                publishState()
            }
            emit(.event(event))
            emit(.replayCompleted(sessionId: event.sessionId, resyncRequired: false))
            return
        }

        switch journal.observe(event) {
        case .accepted:
            emit(.event(event))
        case .duplicate:
            emit(.duplicateEvent(event))
        case .gap(let expected, let received):
            if pendingAttachSessions.contains(event.sessionId) {
                deferredAttachEvents[event.sessionId, default: []].append(event)
                return
            }
            _ = stateMachine.handle(.resyncRequired(reason: "sequence_gap"))
            publishState()
            emit(.replayRequested(sessionId: event.sessionId, fromSequence: expected))
            requestReplay(sessionId: event.sessionId, fromSequence: expected, received: received)
        }
    }

    /// Re-requests replay for every session this client had attached before a
    /// reconnect. The host keeps the agent alive across client disconnects, so
    /// the client only needs the events that were appended while it was away.
    private func restoreAttachments() {
        guard !attachedSessions.isEmpty else { return }
        for sessionId in attachedSessions {
            let last = journal.lastSequence(for: sessionId)
            let from = last + 1
            emit(.replayRequested(sessionId: sessionId, fromSequence: from))
            requestReplay(sessionId: sessionId, fromSequence: from, received: last)
        }
    }

    private func requestReplay(sessionId: String, fromSequence: Int, received: Int) {
        let command = GatewayCommand(
            name: .eventsReplay,
            sessionId: sessionId,
            params: .object([
                ("fromSequence", .number(Double(fromSequence))),
                ("receivedSequence", .number(Double(received))),
            ])
        )
        // Background replay has no caller waiting on an ack, so correlate the
        // response by request id and clean up if it never arrives.
        backgroundReplayRequests.insert(command.requestId)
        pendingCommandNames[command.requestId] = command.command
        Task { [weak self] in
            guard let self else { return }
            do {
                try await self.sendFrame(.command(command))
            } catch {
                await self.clearReplayRequest(command.requestId)
                await self.emit(.serverError(GatewayProtocolError(
                    code: "replay_failed",
                    message: "Unable to request replay from \(fromSequence)."
                )))
            }
        }
        Task { [weak self] in
            guard let self else { return }
            try? await self.clock.sleep(for: self.config.commandTimeout)
            await self.clearReplayRequest(command.requestId)
        }
    }

    private func clearReplayRequest(_ requestId: String) {
        backgroundReplayRequests.remove(requestId)
        pendingCommandNames.removeValue(forKey: requestId)
    }

    // MARK: Heartbeat

    private func startHeartbeatWatchdog(intervalMs: Int?) {
        heartbeatTask?.cancel()
        let intervalSeconds = Double(intervalMs ?? 15_000) / 1000.0
        let timeout = max(config.heartbeatTimeout, .seconds(intervalSeconds * 2.5))
        heartbeatTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                try? await self.clock.sleep(for: timeout)
                if Task.isCancelled { return }
                if await self.heartbeatIsStale(timeout: timeout) {
                    await self.handleHeartbeatTimeout()
                    return
                }
            }
        }
    }

    private func heartbeatIsStale(timeout: Duration) -> Bool {
        guard let lastInboundAt else { return false }
        return clock.now().timeIntervalSince(lastInboundAt) > timeout.secondsValue
    }

    private func handleHeartbeatTimeout() async {
        await closeTransport()
    }

    // MARK: Pending requests

    private func startTimeout(for requestId: String) {
        timeouts[requestId]?.cancel()
        timeouts[requestId] = Task { [weak self] in
            guard let self else { return }
            try? await self.clock.sleep(for: self.config.commandTimeout)
            guard !Task.isCancelled else { return }
            await self.failPending(requestId, error: GatewayConnectionError.commandTimedOut(requestId))
        }
    }

    private func resolveAck(_ ack: GatewayAck) {
        timeouts.removeValue(forKey: ack.requestId)?.cancel()
        pendingCommandNames.removeValue(forKey: ack.requestId)
        pending.removeValue(forKey: ack.requestId)?.resume(returning: ack)
    }

    private func failPending(_ requestId: String, error: Error) {
        timeouts.removeValue(forKey: requestId)?.cancel()
        pendingCommandNames.removeValue(forKey: requestId)
        if let sessionId = pendingAttachSessionByRequest.removeValue(forKey: requestId) {
            pendingAttachSessions.remove(sessionId)
            deferredAttachEvents.removeValue(forKey: sessionId)
        }
        pending.removeValue(forKey: requestId)?.resume(throwing: error)
    }

    private func failAllPending(with error: Error) {
        for (requestId, continuation) in pending {
            timeouts.removeValue(forKey: requestId)?.cancel()
            continuation.resume(throwing: error)
        }
        pending.removeAll()
        pendingCommandNames.removeAll()
        pendingAttachSessions.removeAll()
        pendingAttachSessionByRequest.removeAll()
        deferredAttachEvents.removeAll()
        timeouts.removeAll()
    }

    // MARK: State publishing

    private func publishState() {
        state = stateMachine.state
        Self.logger.info("gateway state changed: \(self.state.shortDescription, privacy: .public)")
        emit(.stateChanged(state))
    }

    private func emit(_ value: GatewayInbound) {
        for continuation in subscribers.values {
            continuation.yield(value)
        }
    }
}

// MARK: - Timeout helper

private func withTimeout<T: Sendable>(
    _ duration: Duration,
    operation: @escaping @Sendable () async throws -> T
) async throws -> T {
    try await withThrowingTaskGroup(of: T.self) { group in
        group.addTask { try await operation() }
        group.addTask {
            try await Task.sleep(for: duration)
            throw GatewayConnectionError.timedOut
        }
        defer { group.cancelAll() }
        guard let result = try await group.next() else {
            throw GatewayConnectionError.timedOut
        }
        return result
    }
}
