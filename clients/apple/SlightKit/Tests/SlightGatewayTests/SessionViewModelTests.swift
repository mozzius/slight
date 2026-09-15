import XCTest
@testable import SlightGateway

@MainActor
final class SessionViewModelTests: XCTestCase {
    func testReceivesEventsAndSendsInput() async throws {
        let codec = GatewayCodec()
        let transport = FakeGatewayTransport()
        let summary = SessionSummary(
            id: "s1",
            title: "Test session",
            agent: "fake",
            model: "auto",
            effort: "medium",
            workingDirectoryLabel: "~/project",
            status: .idle
        )
        let attach = SessionAttachResult(
            session: summary,
            replayed: [],
            latestSequence: 0,
            oldestAvailableSequence: nil,
            resyncRequired: false
        )

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
                if command.command == GatewayCommandName.sessionAttach.rawValue,
                   let result = try? codec.encodeToJSONValue(attach) {
                    transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
                } else {
                    transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true)))
                }
            default:
                break
            }
        }

        let connection = GatewayConnection(transportFactory: { transport })
        let viewModel = SessionViewModel(sessionId: "s1", connection: connection, codec: codec, initialSummary: summary)
        viewModel.start()
        await connection.start()

        try await waitUntil { viewModel.connectionState.isConnected }
        try await waitUntil { viewModel.session?.id == "s1" }
        XCTAssertTrue(viewModel.canSend)

        let payload = SessionMessagePayload(
            role: .assistant,
            text: "hello there",
            blocks: []
        )
        transport.push(.event(GatewayEvent(
            sessionId: "s1",
            sequence: 1,
            event: GatewayEventName.sessionMessage.rawValue,
            payload: try codec.encodeToJSONValue(payload)
        )))
        try await waitUntil { viewModel.messages.count == 1 }
        XCTAssertEqual(viewModel.messages.first?.text, "hello there")

        viewModel.draft = "run the tests"
        await viewModel.send()
        try await waitUntil { viewModel.messages.count == 2 }
        XCTAssertEqual(viewModel.messages.last?.role, .user)
        XCTAssertEqual(viewModel.messages.last?.text, "run the tests")
        XCTAssertTrue(viewModel.draft.isEmpty)

        var staleSummary = summary
        staleSummary.recovery = .stale(reason: "native session no longer exists")
        viewModel.updateSummary(staleSummary)
        XCTAssertFalse(viewModel.canSend)
    }

    private func waitUntil(timeout: TimeInterval = 3, _ condition: () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
        throw GatewayConnectionError.timedOut
    }
}
