import Combine
import Foundation

/// Ordered transcript content for a session. Messages, tool calls, and notices
/// are stored in arrival order so the UI can render tool progress inline with
/// the conversation instead of in a separate list.
public enum TranscriptItem: Identifiable, Sendable, Equatable {
    case message(SessionMessage)
    case toolCall(ToolCall)
    case notice(TranscriptNotice)

    public var id: String {
        switch self {
        case .message(let message): return "message-\(message.id)"
        case .toolCall(let toolCall): return "tool-\(toolCall.id)"
        case .notice(let notice): return "notice-\(notice.id)"
        }
    }

    public var message: SessionMessage? {
        if case .message(let message) = self { return message }
        return nil
    }

    public var toolCall: ToolCall? {
        if case .toolCall(let toolCall) = self { return toolCall }
        return nil
    }
}

/// A non-conversational transcript annotation (resync, replay, diagnostics).
public struct TranscriptNotice: Identifiable, Sendable, Equatable {
    public enum Kind: String, Sendable {
        case info
        case warning
        case error
    }

    public var id: String
    public var kind: Kind
    public var text: String

    public init(id: String = UUID().uuidString, kind: Kind = .info, text: String) {
        self.id = id
        self.kind = kind
        self.text = text
    }
}

/// Visible replay lifecycle for a single session.
public enum ReplayState: Equatable, Sendable {
    case idle
    case replaying(fromSequence: Int)
    case resync(reason: String?)
    case failed(String)

    public var isActive: Bool {
        switch self {
        case .idle: return false
        case .replaying, .resync, .failed: return true
        }
    }

    public var isFailed: Bool {
        if case .failed = self { return true }
        return false
    }

    public var description: String? {
        switch self {
        case .idle: return nil
        case .replaying(let from) where from > 0: return "Requesting missed events from #\(from)…"
        case .replaying: return nil
        case .resync(let reason): return reason.map { "History was trimmed by the host: \($0)" }
            ?? "History was trimmed by the host; showing retained events."
        case .failed(let message): return message
        }
    }
}

/// Observable state for a single attached session.
@MainActor
public final class SessionViewModel: ObservableObject {
    @Published public private(set) var session: SessionSummary?
    @Published public private(set) var acpMetadata: SessionAcpMetadata?
    @Published public private(set) var transcript: [TranscriptItem] = []
    @Published public private(set) var permissionRequests: [PermissionRequest] = []
    @Published public private(set) var respondingPermissionIds: Set<String> = []
    @Published public private(set) var connectionState: GatewayConnectionState = .idle
    @Published public private(set) var replayState: ReplayState = .idle
    @Published public private(set) var isSending = false
    @Published public private(set) var isLoadingHistory = false
    @Published public private(set) var hasOlderHistory = false
    @Published public private(set) var hasLoadedHistory = false
    @Published public private(set) var exitReason: String?
    @Published public var draft: String = ""
    @Published public var lastError: String?

    public let sessionId: String
    public let connection: GatewayConnection
    private let codec: GatewayCodec
    private var observationTask: Task<Void, Never>?
    private var hasAttached = false
    private var isAttaching = false
    private var replayBuffer: [GatewayEvent] = []
    private var replaySequencesToIgnore: Set<Int> = []
    private var ignoreNextTranscriptReset = false
    private var loadedHistoryEvents: [GatewayEvent] = []
    private var loadedHistorySequences: Set<Int> = []
    private var oldestLoadedSequence: Int?

    public init(
        sessionId: String,
        connection: GatewayConnection,
        codec: GatewayCodec = GatewayCodec(),
        initialSummary: SessionSummary? = nil
    ) {
        self.sessionId = sessionId
        self.connection = connection
        self.codec = codec
        self.session = initialSummary
    }

    deinit {
        observationTask?.cancel()
    }

    // MARK: Derived transcript views

    public var messages: [SessionMessage] {
        transcript.compactMap(\.message)
    }

    public var modelOptions: [String] {
        configOptions(category: .model)
    }

    public var modelOptionId: String? {
        acpMetadata?.configOptions.first { $0.category == .model }?.id
    }

    public func setConfigOption(configId: String, valueId: String) async {
        do {
            _ = try await connection.perform(
                .sessionSetConfigOption,
                sessionId: sessionId,
                params: JSONValue.object([
                    ("config_id", .string(configId)),
                    ("value_id", .string(valueId)),
                ])
            )
        } catch {
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    public var effortOptions: [String] {
        configOptions(category: .thoughtLevel)
    }

    public var toolCalls: [ToolCall] {
        transcript.compactMap(\.toolCall)
    }

    public var streamingMessageId: String? {
        messages.last(where: \.isStreaming)?.id
    }

    public var isAgentWorking: Bool {
        // A persisted working status is not enough to show live activity while
        // the attach replay is rebuilding the transcript.
        guard !isAttaching, !replayState.isActive else { return false }
        guard let status = session?.status else { return false }
        return status == .working || status == .waitingPermission
    }

    private func configOptions(category: AgentConfigCategory) -> [String] {
        acpMetadata?.configOptions
            .filter { $0.category == category }
            .flatMap { $0.kind.choices.map(\.valueId) } ?? []
    }

    public var canCancel: Bool {
        isAgentWorking
    }

    public var canSend: Bool {
        guard connectionState.isConnected, let session else { return false }
        return session.status != .exited && session.recovery.acceptsInput
    }

    // MARK: Lifecycle

    public func start() {
        guard observationTask == nil else { return }
        observationTask = Task { [weak self] in
            guard let self else { return }
            let stream = await self.connection.subscribe()
            self.connectionState = await self.connection.state
            for await inbound in stream {
                self.handle(inbound)
            }
        }
        // The view model can start just before the connection handshake. Wait
        // briefly for that first connection instead of losing the initial attach
        // to a transient not-connected error.
        Task { await loadSnapshotWhenConnected() }
    }

    public func updateSummary(_ summary: SessionSummary) {
        session = summary
    }

    private func loadSnapshotWhenConnected() async {
        for _ in 0..<80 {
            if await connection.state.isConnected {
                await loadSnapshot()
                return
            }
            try? await Task.sleep(for: .milliseconds(25))
        }
    }

    /// Attaches to the session. The connection replays retained events and
    /// emits them as `.event` frames, followed by `.replayCompleted`; this
    /// method only refreshes the session summary and surfaces immediate errors.
    public func loadSnapshot() async {
        guard !isAttaching else { return }
        if !hasAttached {
            hasLoadedHistory = false
        }
        isAttaching = true
        replayBuffer = []
        replaySequencesToIgnore = []
        replayState = .replaying(fromSequence: 0)
        defer { isAttaching = false }
        do {
            // This view model owns its transcript. Ask for the retained history
            // on every foreground attach instead of relying on the connection's
            // shared replay cursor, which may belong to a previous view model.
            let result = try await connection.perform(.sessionAttach, sessionId: sessionId)
            guard let attach = try codec.decodePayload(SessionAttachResult.self, from: result) else { return }

            // The connection resolves the attach acknowledgement before its
            // replay frames reach subscribers. Ingest the inline history here
            // so SwiftUI receives one completed transcript instead of watching
            // persisted events arrive as a live stream.
            clearTranscript()
            ignoreNextTranscriptReset = true
            if attach.resyncRequired {
                appendNotice(.warning, "History was trimmed by the host; showing the retained events.")
            }
            replaySequencesToIgnore = Set(attach.replayed.map(\.sequence))
            for event in attach.replayed {
                handleEvent(event)
            }
            let pendingEvents = replayBuffer
            replayBuffer.removeAll(keepingCapacity: true)
            for event in pendingEvents where !replaySequencesToIgnore.contains(event.sequence) {
                handleEvent(event)
            }
            finishReplay()
            session = attach.session
            hasAttached = true
            let before = attach.latestSequence == Int.max ? nil : attach.latestSequence + 1
            await loadHistoryPage(beforeSequence: before, replacing: true)
            hasLoadedHistory = true
            await loadMetadata()
            lastError = nil
        } catch {
            let message = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
            replayState = .failed(message)
            lastError = message
        }
    }

    public func loadOlderHistory() async {
        guard hasOlderHistory, let oldestLoadedSequence else { return }
        await loadHistoryPage(beforeSequence: oldestLoadedSequence, replacing: false)
    }

    private func loadHistoryPage(beforeSequence: Int?, replacing: Bool) async {
        guard !isLoadingHistory else { return }
        isLoadingHistory = true
        defer { isLoadingHistory = false }

        let params = JSONValue.object([
            ("before_sequence", beforeSequence.map { .number(Double($0)) } ?? .null),
            ("limit", .number(200)),
        ])
        do {
            let result = try await connection.perform(
                .sessionHistory,
                sessionId: sessionId,
                params: params
            )
            guard let result,
                  let page = try codec.decodePayload(SessionHistoryResult.self, from: result)
            else { return }

            let newEvents = page.events.filter { loadedHistorySequences.insert($0.sequence).inserted }
            if replacing {
                loadedHistoryEvents = newEvents
            } else {
                loadedHistoryEvents = newEvents + loadedHistoryEvents
            }
            oldestLoadedSequence = loadedHistoryEvents.map(\ .sequence).min()
            hasOlderHistory = page.hasMore
            session = page.session
            if replacing {
                rebuildTranscript()
            } else if !newEvents.isEmpty {
                prependOlderItems(transcriptItems(for: newEvents))
            }
        } catch {
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    public func rename(to title: String) async {
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        do {
            let params = JSONValue.object([("title", .string(trimmed))])
            let result = try await connection.perform(.sessionRename, sessionId: sessionId, params: params)
            guard let renamed = try codec.decodePayload(SessionRenameResult.self, from: result) else { return }
            session = renamed.session
            lastError = nil
        } catch {
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    public func setMode(_ mode: AcpMode) async {
        do {
            let params = JSONValue.object([("mode_id", .string(mode.id))])
            try await connection.perform(.sessionSetMode, sessionId: sessionId, params: params)
            if var modes = acpMetadata?.modes {
                modes.currentModeId = mode.id
                acpMetadata?.modes = modes
            }
            lastError = nil
        } catch {
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    private func loadMetadata() async {
        do {
            let result = try await connection.perform(.sessionInspect, sessionId: sessionId)
            let inspection = try codec.decodePayload(SessionInspectResult.self, from: result)
            acpMetadata = inspection?.acp
            if session?.model == nil,
               let model = inspection?.acp?.configOptions.first(where: { $0.category == .model })?.kind.currentValueId {
                session?.model = model
            }
            if let pending = inspection?.pendingPermission {
                permissionRequests = [pending.request()]
            } else {
                permissionRequests.removeAll()
            }
        } catch {
            // Session details remain usable when metadata is unavailable.
            acpMetadata = nil
        }
    }

    public func send() async {
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, !isSending else { return }
        isSending = true
        defer { isSending = false }

        let localId = "local-\(UUID().uuidString)"
        let local = SessionMessage(
            id: localId,
            role: .user,
            blocks: [ContentBlock(text: text)],
            at: Date()
        )
        transcript.append(.message(local))
        draft = ""

        do {
            let params = JSONValue.object([("text", .string(text))])
            try await connection.perform(.sessionInput, sessionId: sessionId, params: params)
            lastError = nil
        } catch {
            // Surface a failed delivery instead of leaving a phantom local
            // echo, and restore the text so the user can retry.
            transcript.removeAll { $0.message?.id == localId }
            if draft.isEmpty { draft = text }
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    public func cancel() async {
        do {
            try await connection.perform(.sessionCancel, sessionId: sessionId)
        } catch {
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    public func respond(to request: PermissionRequest, option: PermissionOption) async {
        guard !respondingPermissionIds.contains(request.id) else { return }
        respondingPermissionIds.insert(request.id)
        defer { respondingPermissionIds.remove(request.id) }
        let params = JSONValue.object([
            ("permission_id", .string(request.id)),
            ("option_id", .string(option.optionId)),
        ])
        do {
            try await connection.perform(.permissionRespond, sessionId: sessionId, params: params)
            permissionRequests.removeAll { $0.id == request.id }
            lastError = nil
        } catch {
            if let gatewayError = error as? GatewayConnectionError,
               case .commandFailed(_, let code, _) = gatewayError,
               code == "invalid_frame" {
                // A replayed permission request can outlive the host-side
                // pending interaction after a restart or agent exit.
                permissionRequests.removeAll { $0.id == request.id }
                Task { await loadSnapshot() }
            }
            lastError = (error as? GatewayConnectionError)?.userMessage ?? error.localizedDescription
        }
    }

    // MARK: Inbound

    private func handle(_ inbound: GatewayInbound) {
        switch inbound {
        case .stateChanged(let state):
            connectionState = state
            if state.isConnected, !hasAttached {
                Task { await loadSnapshot() }
            }

        case .event(let event):
            guard event.sessionId == sessionId else { return }
            if replaySequencesToIgnore.remove(event.sequence) != nil {
                return
            }
            if event.event == GatewayEventName.replayComplete.rawValue {
                flushReplayBuffer()
                finishReplay()
                replayState = .idle
                return
            }
            if isAttaching {
                replayBuffer.append(event)
                return
            }
            handleEvent(event)

        case .replayRequested(let eventSessionId, let fromSequence) where eventSessionId == sessionId:
            replayState = .replaying(fromSequence: fromSequence)

        case .replayCompleted(let eventSessionId, _) where eventSessionId == sessionId:
            flushReplayBuffer()
            finishReplay()
            replayState = .idle

        case .transcriptReset(let eventSessionId, let reason) where eventSessionId == sessionId:
            if ignoreNextTranscriptReset {
                ignoreNextTranscriptReset = false
                if reason != nil {
                    appendNotice(.warning, "History was trimmed by the host; showing the retained events.")
                }
                return
            }
            clearTranscript()
            hasAttached = true
            if reason != nil {
                appendNotice(.warning, "History was trimmed by the host; showing the retained events.")
                replayState = .resync(reason: reason)
            } else {
                replayState = .idle
            }

        case .resyncRequired(let eventSessionId, let reason) where eventSessionId == nil || eventSessionId == sessionId:
            replayState = .resync(reason: reason)
            // The connection dropped the session journal and its attach state,
            // so re-attach to restate the transcript from the retained events.
            Task { await loadSnapshot() }

        case .serverError(let error):
            if error.code == "resync_required" {
                replayState = .resync(reason: error.message)
            } else {
                lastError = error.message
            }

        case .welcome, .duplicateEvent, .replayRequested, .replayCompleted,
             .transcriptReset, .resyncRequired, .ack, .close:
            break
        }
    }

    private func handleEvent(_ event: GatewayEvent, record: Bool = true) {
        if record, event.event != GatewayEventName.replayComplete.rawValue,
           loadedHistorySequences.insert(event.sequence).inserted {
            loadedHistoryEvents.append(event)
        }
        switch event.event {
        case GatewayEventName.sessionMessage.rawValue:
            guard let payload = try? codec.decodePayload(SessionMessagePayload.self, from: event.payload) else { return }
            merge(payload, eventSequence: event.sequence)
        case GatewayEventName.sessionToolCall.rawValue:
            guard let payload = try? codec.decodePayload(ToolCallPayload.self, from: event.payload) else { return }
            upsert(toolCall: payload.toolCall())
        case GatewayEventName.sessionPermissionRequest.rawValue:
            guard let payload = try? codec.decodePayload(PermissionRequestPayload.self, from: event.payload) else { return }
            let request = payload.request()
            if let index = permissionRequests.firstIndex(where: { $0.id == request.id }) {
                permissionRequests[index] = request
            } else {
                permissionRequests.append(request)
            }
        case GatewayEventName.sessionPermissionResolved.rawValue:
            guard let payload = try? codec.decodePayload(PermissionResolvedPayload.self, from: event.payload) else { return }
            permissionRequests.removeAll { $0.id == payload.id }
        case GatewayEventName.sessionStatus.rawValue:
            guard let payload = try? codec.decodePayload(SessionStatusPayload.self, from: event.payload) else { return }
            session?.status = payload.status
                session?.lastActivityAt = event.at ?? Date()
            if payload.status == .exited || payload.status == .failed {
                replayState = .idle
            }
        case GatewayEventName.sessionMode.rawValue:
            guard let payload = try? codec.decodePayload(SessionModePayload.self, from: event.payload),
                  var modes = acpMetadata?.modes else { return }
            modes.currentModeId = payload.currentModeId
            acpMetadata?.modes = modes
        case GatewayEventName.sessionConfigOptions.rawValue:
            guard let payload = try? codec.decodePayload(SessionConfigOptionsPayload.self, from: event.payload) else { return }
            acpMetadata?.configOptions = payload.options
            if let model = payload.options.first(where: { $0.category == .model })?.kind.currentValueId {
                session?.model = model
            }
        case GatewayEventName.sessionExit.rawValue:
            let payload = try? codec.decodePayload(SessionExitPayload.self, from: event.payload)
            session?.status = .exited
            exitReason = payload?.reason
            appendNotice(.warning, payload.map { "Session exited: \($0.reason)" } ?? "Session exited")
        case GatewayEventName.sessionSnapshot.rawValue:
            guard let payload = try? codec.decodePayload(SessionSnapshotPayload.self, from: event.payload) else { return }
            session = payload.summary
            if let pending = payload.pendingPermission {
                let request = pending.request()
                if !permissionRequests.contains(where: { $0.id == request.id }) {
                    permissionRequests.append(request)
                }
            }
        case GatewayEventName.sessionDiagnostics.rawValue:
            guard let payload = try? codec.decodePayload(SessionDiagnosticsPayload.self, from: event.payload) else { return }
            if payload.level == "error" || payload.level == "warning" {
                lastError = payload.message
            }
        case GatewayEventName.sessionTurnEnded.rawValue:
            guard let payload = try? codec.decodePayload(SessionTurnEndedPayload.self, from: event.payload),
                  let started = payload.reasoningStartedAtMs,
                  let ended = payload.reasoningEndedAtMs,
                  ended >= started else { return }
            let duration = TimeInterval(ended - started) / 1_000
            if let index = transcript.lastIndex(where: { $0.message?.role == .assistant }),
               case .message(var message) = transcript[index] {
                message.reasoningDuration = duration
                transcript[index] = .message(message)
            }
        default:
            break
        }
    }

    private func merge(_ payload: SessionMessagePayload, eventSequence: Int? = nil) {
        var incoming = payload.message()
        if payload.id == nil, let eventSequence {
            incoming.id = "event-\(eventSequence)"
        }

        // A streaming host reuses an id across chunks; update in place.
        if let id = payload.id, let index = transcript.firstIndex(where: { $0.message?.id == id }) {
            guard case .message(var existing) = transcript[index] else { return }
            if payload.append, let lastIndex = existing.blocks.indices.last {
                if existing.blocks[lastIndex].kind == .text,
                   let first = incoming.blocks.first,
                   first.kind == .text {
                    existing.blocks[lastIndex].text += first.text
                } else if !incoming.blocks.isEmpty {
                    existing.blocks.append(contentsOf: incoming.blocks)
                }
            } else {
                let reasoning = existing.blocks.filter { $0.kind == .reasoning }
                existing.blocks = reasoning.isEmpty || incoming.blocks.contains(where: { $0.kind == .reasoning })
                    ? incoming.blocks
                    : reasoning + incoming.blocks
            }
            existing.isStreaming = incoming.isStreaming
            existing.at = incoming.at ?? existing.at
            transcript[index] = .message(existing)
            return
        }

        // ACP message chunks currently arrive without a stable message id.
        // Consecutive assistant messages are one streamed turn unless a tool,
        // user message, or notice has appeared between them.
        if incoming.role == .assistant,
           let index = transcript.indices.last,
           case .message(var existing) = transcript[index],
           existing.role == .assistant {
            for block in incoming.blocks {
                if let lastIndex = existing.blocks.indices.last,
                   existing.blocks[lastIndex].kind == block.kind,
                   block.kind == .text || block.kind == .reasoning || block.kind == .toolResult {
                    existing.blocks[lastIndex].text += block.text
                } else {
                    existing.blocks.append(block)
                }
            }
            existing.isStreaming = incoming.isStreaming
            existing.at = incoming.at ?? existing.at
            transcript[index] = .message(existing)
            return
        }

        // Replace an optimistic local echo matched by role and text.
        if incoming.role == .user,
           let index = transcript.firstIndex(where: {
               ($0.message?.id.hasPrefix("local-") ?? false) && $0.message?.text == incoming.text
           }) {
            transcript[index] = .message(incoming)
            return
        }

        transcript.append(.message(incoming))
    }

    private func upsert(toolCall: ToolCall) {
        if let index = transcript.firstIndex(where: { $0.toolCall?.id == toolCall.id }),
           let existing = transcript[index].toolCall {
            transcript[index] = .toolCall(existing.merging(toolCall))
        } else {
            transcript.append(.toolCall(toolCall))
        }
    }

    private func appendNotice(_ kind: TranscriptNotice.Kind, _ text: String) {
        transcript.append(.notice(TranscriptNotice(kind: kind, text: text)))
    }

    private func clearTranscript() {
        transcript = []
        replayBuffer = []
        permissionRequests = []
        exitReason = nil
        loadedHistoryEvents = []
        loadedHistorySequences = []
        oldestLoadedSequence = nil
        hasOlderHistory = false
    }

    private func rebuildTranscript() {
        let accumulator = SessionViewModel(
            sessionId: sessionId,
            connection: connection,
            codec: codec
        )
        for event in loadedHistoryEvents.sorted(by: { $0.sequence < $1.sequence }) {
            accumulator.handleEvent(event, record: false)
        }
        transcript = accumulator.transcript
        permissionRequests = accumulator.permissionRequests
        exitReason = accumulator.exitReason
    }

    private func transcriptItems(for events: [GatewayEvent]) -> [TranscriptItem] {
        let accumulator = SessionViewModel(
            sessionId: sessionId,
            connection: connection,
            codec: codec
        )
        for event in events.sorted(by: { $0.sequence < $1.sequence }) {
            accumulator.handleEvent(event, record: false)
        }
        return accumulator.transcript
    }

    private func prependOlderItems(_ items: [TranscriptItem]) {
        // Keep the existing first item's identity and contents unchanged. A
        // page can split one assistant turn, but folding the older blocks into
        // that item moves its visual top and defeats scroll-position retention.
        transcript = items + transcript
    }

    private func flushReplayBuffer() {
        guard !replayBuffer.isEmpty else { return }
        let events = replayBuffer
        replayBuffer.removeAll(keepingCapacity: true)
        for event in events {
            handleEvent(event)
        }
    }

    private func finishReplay() {
        transcript = transcript.map { item in
            guard case .message(var message) = item else { return item }
            message.isStreaming = false
            return .message(message)
        }
    }
}
