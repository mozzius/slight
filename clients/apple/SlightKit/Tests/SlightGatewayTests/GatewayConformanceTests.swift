import XCTest
@testable import SlightGateway

/// Consumes the canonical `protocol/conformance` vectors shared with the Rust
/// `gateway-protocol` crate. These are the source of truth for wire compatibility.
final class GatewayConformanceTests: XCTestCase {
    private let codec = GatewayCodec()

    func testHelloFixture() throws {
        guard case .hello(let hello) = try decodeConformance("hello") else {
            return XCTFail("Expected hello")
        }
        XCTAssertEqual(hello.protocolVersion, 1)
        XCTAssertEqual(hello.clientId, "conformance-client")
        XCTAssertEqual(hello.clientName, "Conformance")
        XCTAssertEqual(hello.clientVersion, "1.0.0")
        XCTAssertEqual(hello.deviceId, "device-1")
        XCTAssertEqual(hello.credential, "dev")
        XCTAssertEqual(hello.resume?.sessionId, "s-1")
        XCTAssertEqual(hello.resume?.lastEventSequence, 3)
    }

    func testAckOkFixture() throws {
        guard case .ack(let ack) = try decodeConformance("ack-ok") else {
            return XCTFail("Expected ack")
        }
        XCTAssertEqual(ack.requestId, "req-1")
        XCTAssertTrue(ack.ok)
        XCTAssertNil(ack.error)

        let result = try XCTUnwrap(codec.decodePayload(SessionCreateResult.self, from: ack.result))
        XCTAssertEqual(result.session.id, "s-1")
        XCTAssertEqual(result.session.title, "Conformance session")
        XCTAssertEqual(result.session.agent, "fake")
        XCTAssertEqual(result.session.workingDirectoryLabel, "~/work/slight")
        XCTAssertEqual(result.session.status, .idle)
        XCTAssertEqual(result.session.lastSequence, 0)
    }

    func testSessionCreateCommandFixture() throws {        guard case .command(let command) = try decodeConformance("session-create-command") else {
            return XCTFail("Expected command")
        }
        XCTAssertEqual(command.command, "session.create")
        XCTAssertEqual(command.requestId, "req-1")
        let params = try XCTUnwrap(command.params)
        XCTAssertEqual(params["agent"]?.stringValue, "fake")
        XCTAssertEqual(params["working_directory_label"]?.stringValue, "~/work/slight")
        XCTAssertEqual(params["title"]?.stringValue, "Conformance session")
        XCTAssertEqual(params["initial_prompt"]?.stringValue, "hello")
    }

    func testRichToolCallFixtureDecodes() throws {
        guard case .ack(let ack) = try decodeConformance("session-tool-call-rich") else {
            return XCTFail("Expected ack")
        }
        XCTAssertTrue(ack.ok)
        let result = try XCTUnwrap(codec.decodePayload(SessionInspectResult.self, from: ack.result))

        let message = try XCTUnwrap(result.recentEvents.first { $0.event == "session.message" })
        let messagePayload = try XCTUnwrap(codec.decodePayload(SessionMessagePayload.self, from: message.payload))
        XCTAssertEqual(messagePayload.blocks.first?.kind, .image)
        XCTAssertEqual(messagePayload.blocks.first?.mimeType, "image/png")

        let toolEvent = try XCTUnwrap(result.recentEvents.first { $0.event == "session.tool_call" })
        let toolPayload = try XCTUnwrap(codec.decodePayload(ToolCallPayload.self, from: toolEvent.payload))
        XCTAssertEqual(toolPayload.id, "tool-1")
        XCTAssertEqual(toolPayload.content.count, 3)
        guard case .diff(let diff) = toolPayload.content[1] else {
            return XCTFail("Expected diff content")
        }
        XCTAssertEqual(diff.path, "/tmp/work/src/main.rs")
        XCTAssertEqual(toolPayload.locations.first?.line, 12)
        XCTAssertEqual(toolPayload.rawOutput?["exit"]?.intValue, 0)
    }

    func testMalformedUnknownTypeIsPreserved() throws {        guard case .unknown(let type, let raw) = try decodeConformance("malformed-unknown-type") else {
            return XCTFail("Expected unknown frame")
        }
        XCTAssertEqual(type, "totally_unknown_frame")
        XCTAssertEqual(raw["future_field"], .bool(true))

        let roundTripped = try codec.decode(try codec.encode(GatewayFrame.unknown(type: type, raw: raw)))
        XCTAssertEqual(roundTripped, .unknown(type: type, raw: raw))
    }

    // MARK: Agent session discovery and recovery

    func testAgentSessionsListCommandFixture() throws {
        guard case .command(let command) = try decodeConformance("agent-sessions-list-command") else {
            return XCTFail("Expected command")
        }
        XCTAssertEqual(command.command, GatewayCommandName.agentSessionsList.rawValue)
        let params = try XCTUnwrap(codec.decodePayload(AgentSessionsListParams.self, from: command.params))
        XCTAssertEqual(params.agent, "fake")
        XCTAssertEqual(params.workingDirectoryLabel, "~/work/slight")
        XCTAssertEqual(params.cursor, "cursor-1")
    }

    func testAgentSessionImportCommandFixture() throws {
        guard case .command(let command) = try decodeConformance("agent-session-import-command") else {
            return XCTFail("Expected command")
        }
        XCTAssertEqual(command.command, GatewayCommandName.agentSessionImport.rawValue)
        let params = try XCTUnwrap(codec.decodePayload(ImportAgentSessionParams.self, from: command.params))
        XCTAssertEqual(params.agent, "fake")
        XCTAssertEqual(params.agentSessionId, "fake-listed-1")
        XCTAssertEqual(params.workingDirectoryLabel, "~/work/slight")
        XCTAssertEqual(params.recovery, .load)
        XCTAssertEqual(params.title, "Imported fake session")
    }

    func testImportAgentSessionParamsEncodeSnakeCase() throws {
        let params = ImportAgentSessionParams(
            agent: "fake",
            agentSessionId: "ses_1",
            workingDirectoryLabel: "~/work/slight",
            recovery: .resume,
            title: "Imported"
        )
        let value = try codec.encodeToJSONValue(params)
        XCTAssertEqual(value["agent"]?.stringValue, "fake")
        XCTAssertEqual(value["agent_session_id"]?.stringValue, "ses_1")
        XCTAssertEqual(value["working_directory_label"]?.stringValue, "~/work/slight")
        XCTAssertEqual(value["recovery"]?.stringValue, "resume")
        XCTAssertEqual(value["title"]?.stringValue, "Imported")
    }

    func testAgentSessionsListAckFixture() throws {
        guard case .ack(let ack) = try decodeConformance("agent-sessions-list-ack") else {
            return XCTFail("Expected ack")
        }
        XCTAssertTrue(ack.ok)
        let result = try XCTUnwrap(codec.decodePayload(AgentSessionsListResult.self, from: ack.result))
        XCTAssertEqual(result.sessions.count, 2)
        XCTAssertEqual(result.nextCursor, "cursor-2")

        let first = try XCTUnwrap(result.sessions.first)
        XCTAssertEqual(first.agent, "fake")
        XCTAssertEqual(first.agentSessionId, "fake-listed-1")
        XCTAssertEqual(first.cwd, "/tmp/slight/fake-project")
        XCTAssertEqual(first.additionalDirectories, ["/tmp/slight/shared"])
        XCTAssertEqual(first.title, "Prior fake session")
        XCTAssertNotNil(first.updatedAt)
        XCTAssertEqual(first.id, "fake:fake-listed-1")

        let second = result.sessions[1]
        XCTAssertTrue(second.additionalDirectories.isEmpty)
        XCTAssertNil(second.title)
        XCTAssertNil(second.updatedAt)
    }

    func testAgentSessionImportAckFixture() throws {
        guard case .ack(let ack) = try decodeConformance("agent-session-import-ack") else {
            return XCTFail("Expected ack")
        }
        XCTAssertTrue(ack.ok)
        let result = try XCTUnwrap(codec.decodePayload(ImportAgentSessionResult.self, from: ack.result))
        XCTAssertEqual(result.session.id, "s-imported")
        XCTAssertEqual(result.session.title, "Imported fake session")
        XCTAssertEqual(result.session.recovery, .live)
    }

    func testAgentSessionImportUnsupportedFixture() throws {
        guard case .ack(let ack) = try decodeConformance("agent-session-import-unsupported") else {
            return XCTFail("Expected ack")
        }
        XCTAssertFalse(ack.ok)
        XCTAssertEqual(ack.error?.code, "unsupported_capability")
        XCTAssertFalse(ack.error?.retryable ?? true)
    }

    func testSessionListRecoveryFixtureDecodesAllStates() throws {
        guard case .ack(let ack) = try decodeConformance("session-list-recovery") else {
            return XCTFail("Expected ack")
        }
        let result = try XCTUnwrap(codec.decodePayload(SessionListResult.self, from: ack.result))
        XCTAssertEqual(result.sessions.count, 4)
        XCTAssertEqual(result.sessions[0].recovery, .live)
        XCTAssertEqual(result.sessions[1].recovery, .recovered)
        XCTAssertEqual(
            result.sessions[2].recovery,
            .stale(reason: "loaded session fake-listed-1 is no longer available: unknown session")
        )
        XCTAssertEqual(
            result.sessions[3].recovery,
            .unavailable(reason: "agent codex is not installed on this host")
        )
        XCTAssertFalse(result.sessions[2].recovery.acceptsInput)
        XCTAssertFalse(result.sessions[3].recovery.acceptsInput)
        XCTAssertTrue(result.sessions[0].recovery.acceptsInput)
        XCTAssertTrue(result.sessions[1].recovery.acceptsInput)
    }

    func testSessionSummaryWithoutRecoveryDefaultsToLive() throws {
        let data = Data("""
        {
          "id": "s-legacy",
          "title": "Legacy",
          "agent": "fake",
          "working_directory_label": "~/work",
          "status": "idle",
          "last_sequence": 0
        }
        """.utf8)
        let summary = try codec.decodeJSON(SessionSummary.self, from: data)
        XCTAssertEqual(summary.recovery, .live)
    }

    // MARK: Helpers

    private func decodeConformance(_ name: String) throws -> GatewayFrame {
        let directory = try conformanceDirectory()
        let url = directory.appendingPathComponent("\(name).json")
        let data = try Data(contentsOf: url)
        return try codec.decode(data)
    }

    private func conformanceDirectory() throws -> URL {
        // Walk up from this source file to the repository root, which contains
        // `protocol/conformance`.
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        let fileManager = FileManager.default
        for _ in 0..<12 {
            let candidate = directory.appendingPathComponent("protocol/conformance", isDirectory: true)
            if fileManager.fileExists(atPath: candidate.path) {
                return candidate
            }
            let parent = directory.deletingLastPathComponent()
            if parent.path == directory.path { break }
            directory = parent
        }
        throw XCTSkip("protocol/conformance not found; run from the repository checkout")
    }
}
