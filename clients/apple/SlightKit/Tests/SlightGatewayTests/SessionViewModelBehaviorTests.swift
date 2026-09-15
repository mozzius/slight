import XCTest
@testable import SlightGateway

@MainActor
final class SessionViewModelBehaviorTests: XCTestCase {
    // MARK: - Harness

    /// Mutable script shared with the transport's send callback.
    private final class Script: @unchecked Sendable {
        var attachResult: SessionAttachResult
        var failures: [String: GatewayErrorBody] = [:]

        init(attachResult: SessionAttachResult) {
            self.attachResult = attachResult
        }
    }

    @MainActor
    private struct Harness {
        let codec = GatewayCodec()
        let transport: FakeGatewayTransport
        let script: Script
        let connection: GatewayConnection
        let viewModel: SessionViewModel
        let summary: SessionSummary

        init(summary: SessionSummary, attachResult: SessionAttachResult, failures: [String: GatewayErrorBody] = [:]) {
            self.summary = summary
            let script = Script(attachResult: attachResult)
            script.failures = failures
            self.script = script

            let codec = GatewayCodec()
            let transport = FakeGatewayTransport()
            self.transport = transport
            transport.onSend = { frame in
                switch frame {
                case .hello:
                    transport.push(.welcome(ServerWelcome(
                        connectionId: "c1",
                        serverName: "host",
                        serverVersion: "1",
                        heartbeatIntervalMs: 60_000
                    )))
                case .command(let command):
                    if let failure = script.failures[command.command] {
                        transport.push(.ack(GatewayAck(requestId: command.requestId, ok: false, error: failure)))
                        return
                    }
                    if command.command == GatewayCommandName.sessionAttach.rawValue,
                       let result = try? codec.encodeToJSONValue(script.attachResult) {
                        transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
                    } else {
                        transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true)))
                    }
                default:
                    break
                }
            }
            self.connection = GatewayConnection(transportFactory: { transport })
            self.viewModel = SessionViewModel(
                sessionId: summary.id,
                connection: connection,
                codec: codec,
                initialSummary: summary
            )
        }

        func start() async throws {
            viewModel.start()
            await connection.start()
            try await waitUntil { viewModel.connectionState.isConnected }
            try await waitUntil {
                transport.frames().contains { frame in
                    if case .command(let command) = frame {
                        return command.command == GatewayCommandName.sessionAttach.rawValue
                    }
                    return false
                }
            }
        }

        func waitUntil(timeout: TimeInterval = 3, _ condition: () -> Bool) async throws {
            let deadline = Date().addingTimeInterval(timeout)
            while Date() < deadline {
                if condition() { return }
                try await Task.sleep(nanoseconds: 5_000_000)
            }
            throw GatewayConnectionError.timedOut
        }

        func sentCommands() -> [String] {
            transport.frames().compactMap { frame in
                if case .command(let command) = frame { return command.command }
                return nil
            }
        }

    func pushMessage(
        sequence: Int,
        text: String,
        role: SessionMessage.Role = .assistant,
        id: String? = nil,
        isStreaming: Bool = false,
        append: Bool = false
    ) {
        let payload = SessionMessagePayload(
            role: role,
                text: text,
                id: id,
                isStreaming: isStreaming,
                append: append
            )
            push(.sessionMessage, sequence: sequence, payload: try? codec.encodeToJSONValue(payload))
        }

        func pushToolCall(sequence: Int, id: String, status: ToolCall.Status) {
            let payload = ToolCallPayload(id: id, title: "Edit file", kind: "edit", status: status)
            push(.sessionToolCall, sequence: sequence, payload: try? codec.encodeToJSONValue(payload))
        }

        func pushPermissionRequest(sequence: Int, id: String) {
            let payload = PermissionRequestPayload(
                id: id,
                title: "Run shell command?",
                detail: "git status",
                toolCallId: "tool-1",
                options: [PermissionOption(optionId: "allow", label: "Allow", kind: .allow)]
            )
            push(.sessionPermissionRequest, sequence: sequence, payload: try? codec.encodeToJSONValue(payload))
        }

        func pushStatus(sequence: Int, status: SessionStatus) {
            push(.sessionStatus, sequence: sequence, payload: try? codec.encodeToJSONValue(SessionStatusPayload(status: status)))
        }

        private func push(_ name: GatewayEventName, sequence: Int, payload: JSONValue?) {
            transport.push(.event(GatewayEvent(
                sessionId: summary.id,
                sequence: sequence,
                event: name.rawValue,
                at: Date(),
                payload: payload
            )))
        }
    }

    private func makeSummary(id: String = "s1") -> SessionSummary {
        SessionSummary(
            id: id,
            title: "Test session",
            agent: "fake",
            model: "auto",
            effort: "medium",
            workingDirectoryLabel: "~/project",
            status: .idle
        )
    }

    private func makeAttach(
        summary: SessionSummary,
        replayed: [GatewayEvent] = [],
        resyncRequired: Bool = false,
        oldest: Int? = nil
    ) -> SessionAttachResult {
        SessionAttachResult(
            session: summary,
            replayed: replayed,
            latestSequence: replayed.map(\.sequence).max() ?? 0,
            oldestAvailableSequence: oldest,
            resyncRequired: resyncRequired
        )
    }

    // MARK: - Streaming transcript

    func testStreamingChunksMergeIntoOneMessage() async throws {
        let summary = makeSummary()
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary))
        try await harness.start()

        harness.pushMessage(sequence: 1, text: "Hello ", id: "m1", isStreaming: true)
        harness.pushMessage(sequence: 2, text: "world", id: "m1", isStreaming: true, append: true)
        harness.pushMessage(sequence: 3, text: "", id: "m1", isStreaming: false, append: true)

        try await harness.waitUntil { harness.viewModel.messages.count == 1 && harness.viewModel.messages[0].text == "Hello world" }
        let message = try XCTUnwrap(harness.viewModel.messages.first)
        XCTAssertFalse(message.isStreaming)
        XCTAssertNil(harness.viewModel.streamingMessageId)
    }

    // MARK: - Tool progress

    func testToolProgressUpdatesInPlace() async throws {
        let summary = makeSummary()
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary))
        try await harness.start()

        harness.pushToolCall(sequence: 1, id: "tool-1", status: .pending)
        harness.pushToolCall(sequence: 2, id: "tool-1", status: .inProgress)
        harness.pushToolCall(sequence: 3, id: "tool-1", status: .completed)

        try await harness.waitUntil { harness.viewModel.toolCalls.first?.status == .completed }
        XCTAssertEqual(harness.viewModel.toolCalls.count, 1)
    }

    // MARK: - Cancel

    func testCancelIsAvailableWhileWorkingAndSendsCommand() async throws {
        let summary = makeSummary()
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary))
        try await harness.start()

        harness.pushStatus(sequence: 1, status: .working)
        try await harness.waitUntil { harness.viewModel.canCancel }

        await harness.viewModel.cancel()
        try await harness.waitUntil { harness.sentCommands().contains(GatewayCommandName.sessionCancel.rawValue) }
    }

    // MARK: - Permissions

    func testPermissionRequestCanBeRespondedTo() async throws {
        let summary = makeSummary()
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary))
        try await harness.start()

        harness.pushPermissionRequest(sequence: 1, id: "perm-1")
        try await harness.waitUntil { harness.viewModel.permissionRequests.count == 1 }

        let request = try XCTUnwrap(harness.viewModel.permissionRequests.first)
        let option = try XCTUnwrap(request.options.first)
        await harness.viewModel.respond(to: request, option: option)

        try await harness.waitUntil { harness.viewModel.permissionRequests.isEmpty }
        XCTAssertTrue(harness.sentCommands().contains(GatewayCommandName.permissionRespond.rawValue))
        let permissionCommand = try XCTUnwrap(harness.transport.frames().compactMap { frame -> GatewayCommand? in
            guard case .command(let command) = frame,
                  command.command == GatewayCommandName.permissionRespond.rawValue else { return nil }
            return command
        }.first)
        XCTAssertEqual(permissionCommand.params?["permission_id"]?.stringValue, "perm-1")
        XCTAssertEqual(permissionCommand.params?["option_id"]?.stringValue, option.optionId)
        XCTAssertNil(permissionCommand.params?["permissionId"])
        XCTAssertFalse(harness.viewModel.respondingPermissionIds.contains("perm-1"))
    }

    // MARK: - Resync and replay

    func testResyncAttachResetsTranscriptAndClearsReplayState() async throws {
        let summary = makeSummary()
        let codec = GatewayCodec()
        let replayedMessage = GatewayEvent(
            sessionId: summary.id,
            sequence: 1,
            event: GatewayEventName.sessionMessage.rawValue,
            payload: try codec.encodeToJSONValue(SessionMessagePayload(role: .assistant, text: "retained"))
        )
        let attach = makeAttach(summary: summary, replayed: [replayedMessage], resyncRequired: true, oldest: 1)
        let harness = Harness(summary: summary, attachResult: attach)
        try await harness.start()

        try await harness.waitUntil { harness.viewModel.replayState == .idle }
        XCTAssertEqual(harness.viewModel.messages.count, 1)
        XCTAssertEqual(harness.viewModel.messages.first?.text, "retained")
        XCTAssertTrue(harness.viewModel.transcript.contains { item in
            if case .notice(let notice) = item { return notice.kind == .warning }
            return false
        })
    }

    func testReplayedUserMessagesPreserveAssistantTurnBoundaries() async throws {
        let summary = makeSummary()
        let codec = GatewayCodec()
        let replayed = [
            GatewayEvent(
                sessionId: summary.id,
                sequence: 1,
                event: GatewayEventName.sessionMessage.rawValue,
                payload: try codec.encodeToJSONValue(SessionMessagePayload(role: .assistant, text: "first answer"))
            ),
            GatewayEvent(
                sessionId: summary.id,
                sequence: 2,
                event: GatewayEventName.sessionMessage.rawValue,
                payload: try codec.encodeToJSONValue(SessionMessagePayload(role: .user, text: "second prompt"))
            ),
            GatewayEvent(
                sessionId: summary.id,
                sequence: 3,
                event: GatewayEventName.sessionMessage.rawValue,
                payload: try codec.encodeToJSONValue(SessionMessagePayload(role: .assistant, text: "second answer"))
            ),
        ]
        let harness = Harness(
            summary: summary,
            attachResult: makeAttach(summary: summary, replayed: replayed)
        )
        try await harness.start()

        try await harness.waitUntil { harness.viewModel.messages.count == 3 }
        XCTAssertEqual(harness.viewModel.messages.map(\.role), [.assistant, .user, .assistant])
        XCTAssertEqual(harness.viewModel.messages.map(\.text), ["first answer", "second prompt", "second answer"])
    }

    func testAttachRestoresWorkingStateAfterReplayCompletes() async throws {
        let summary = makeSummary()
        var workingSummary = summary
        workingSummary.status = .working
        let harness = Harness(
            summary: summary,
            attachResult: makeAttach(summary: workingSummary)
        )

        try await harness.start()
        XCTAssertTrue(harness.viewModel.isAgentWorking)
    }

    func testReplayedStreamingMessageIsRenderedAsHistory() async throws {
        let summary = makeSummary()
        let codec = GatewayCodec()
        let event = GatewayEvent(
            sessionId: summary.id,
            sequence: 1,
            event: GatewayEventName.sessionMessage.rawValue,
            payload: try codec.encodeToJSONValue(
                SessionMessagePayload(role: .assistant, text: "finished answer", isStreaming: true)
            )
        )
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary, replayed: [event]))
        try await harness.start()

        let message = try XCTUnwrap(harness.viewModel.messages.first)
        XCTAssertFalse(message.isStreaming)
    }

    func testAttachBuffersReplayUntilCompletion() async throws {
        let summary = makeSummary()
        let codec = GatewayCodec()
        let first = GatewayEvent(
            sessionId: summary.id,
            sequence: 1,
            event: GatewayEventName.sessionMessage.rawValue,
            payload: try codec.encodeToJSONValue(SessionMessagePayload(role: .assistant, text: "history"))
        )
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary, replayed: [first]))
        try await harness.start()

        XCTAssertEqual(harness.viewModel.messages.map(\.text), ["history"])
        XCTAssertEqual(harness.viewModel.replayState, .idle)
    }

    func testReplayCompletedClearsReplayingBanner() async throws {
        let summary = makeSummary()
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary))
        try await harness.start()

        // A sequence gap makes the connection request replay and surface the
        // replaying state; the host's replay response then clears it.
        harness.transport.push(.event(GatewayEvent(sessionId: summary.id, sequence: 1, event: "session.message")))
        harness.transport.push(.event(GatewayEvent(sessionId: summary.id, sequence: 3, event: "session.message")))
        try await harness.waitUntil {
            if case .replaying = harness.viewModel.replayState { return true }
            return false
        }
        try await harness.waitUntil { harness.viewModel.replayState == .idle }
    }

    // MARK: - Send failures

    func testFailedSendRestoresDraftAndRemovesLocalEcho() async throws {
        let summary = makeSummary()
        let failure = GatewayErrorBody(code: "internal", message: "host exploded")
        let harness = Harness(
            summary: summary,
            attachResult: makeAttach(summary: summary),
            failures: [GatewayCommandName.sessionInput.rawValue: failure]
        )
        try await harness.start()

        harness.viewModel.draft = "does this send?"
        await harness.viewModel.send()

        XCTAssertEqual(harness.viewModel.draft, "does this send?")
        XCTAssertEqual(harness.viewModel.lastError, "host exploded")
        XCTAssertFalse(harness.viewModel.messages.contains { $0.text == "does this send?" })
    }

    // MARK: - Session actions

    func testRenameSendsSessionRenameCommand() async throws {
        let summary = makeSummary()
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary))
        try await harness.start()

        await harness.viewModel.rename(to: "Renamed session")

        XCTAssertTrue(harness.sentCommands().contains(GatewayCommandName.sessionRename.rawValue))
    }

    func testSetModeSendsSessionSetModeCommand() async throws {
        let summary = makeSummary()
        let harness = Harness(summary: summary, attachResult: makeAttach(summary: summary))
        try await harness.start()

        await harness.viewModel.setMode(AcpMode(id: "auto", name: "Auto"))

        XCTAssertTrue(harness.sentCommands().contains(GatewayCommandName.sessionSetMode.rawValue))
    }
}
