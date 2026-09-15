import XCTest
@testable import SlightGateway

final class GatewayCodecTests: XCTestCase {
    private let codec = GatewayCodec()

    func testHelloRoundTripsWithSnakeCaseWireKeys() throws {
        let hello = ClientHello(
            clientId: "client-1",
            clientVersion: "1.2.3",
            deviceId: "device-1",
            credential: "secret",
            resume: ResumeRequest(sessionId: "session-1", lastEventSequence: 7)
        )

        let data = try codec.encode(.hello(hello))
        let json = try XCTUnwrap(String(data: data, encoding: .utf8))

        XCTAssertTrue(json.contains("\"type\":\"hello\""))
        XCTAssertTrue(json.contains("\"v\":1"))
        XCTAssertTrue(json.contains("\"client_id\":\"client-1\""))
        XCTAssertTrue(json.contains("\"last_event_sequence\":7"))

        let decoded = try codec.decode(data)
        XCTAssertEqual(decoded, .hello(hello))
    }

    func testWelcomeDecodesWithMissingOptionalFields() throws {
        let json = """
        { "v": 1, "type": "welcome", "connection_id": "c-1", "server_name": "host", "server_version": "0.1" }
        """
        let decoded = try codec.decode(Data(json.utf8))
        guard case .welcome(let welcome) = decoded else {
            return XCTFail("Expected welcome frame")
        }
        XCTAssertEqual(welcome.connectionId, "c-1")
        XCTAssertFalse(welcome.resyncRequired)
        // Missing capabilities fall back to the canonical host defaults.
        XCTAssertTrue(welcome.capabilities.supportsReplay)
        XCTAssertTrue(welcome.capabilities.hostAdmin)
    }

    func testUnknownFrameTypeIsPreserved() throws {
        let json = """
        { "v": 1, "type": "future_thing", "payload": { "keep": true } }
        """
        let decoded = try codec.decode(Data(json.utf8))
        guard case .unknown(let type, let raw) = decoded else {
            return XCTFail("Expected unknown frame")
        }
        XCTAssertEqual(type, "future_thing")
        XCTAssertEqual(raw["payload"]?["keep"], .bool(true))

        let reencoded = try codec.decode(try codec.encode(decoded))
        XCTAssertEqual(reencoded, decoded)
    }

    func testEventPayloadDecodesThroughTypedBridge() throws {
        let payload = SessionMessagePayload(
            role: .assistant,
            text: "hello",
            blocks: [ContentBlock(kind: .text, text: "hello")]
        )
        let value = try codec.encodeToJSONValue(payload)
        let decoded = try codec.decodePayload(SessionMessagePayload.self, from: value)
        XCTAssertEqual(decoded, payload)
    }

    func testNormalizedACPMetadataAndEventsDecode() throws {
        let metadataJSON = """
        {
          "protocol_version": 1,
          "agent_title": "Fake ACP agent",
          "auth_methods": [{"id":"device","name":"Device login","kind":"agent"}],
          "capabilities": {"prompt_image":true,"session_resume":true},
          "modes": {"current_mode_id":"plan","available_modes":[{"id":"plan","name":"Plan"}]}
        }
        """
        let metadata = try codec.decodeJSON(SessionAcpMetadata.self, from: Data(metadataJSON.utf8))
        XCTAssertEqual(metadata.protocolVersion, 1)
        XCTAssertEqual(metadata.authMethods.first?.id, "device")
        XCTAssertTrue(metadata.capabilities.promptImage)
        XCTAssertTrue(metadata.capabilities.sessionResume)
        XCTAssertEqual(metadata.modes?.currentModeId, "plan")

        let planValue: JSONValue = .object([
            ("entries", .array([
                .object([
                    ("content", .string("write tests")),
                    ("priority", .string("high")),
                    ("status", .string("in_progress")),
                ])
            ]))
        ])
        let plan = try codec.decodePayload(SessionPlanPayload.self, from: planValue)
        XCTAssertEqual(plan?.entries.first?.content, "write tests")

        let commandsValue: JSONValue = .object([
            ("commands", .array([
                .object([
                    ("name", .string("compact")),
                    ("description", .string("Compact the conversation")),
                    ("input_hint", .string("scope")),
                ])
            ]))
        ])
        let commands = try codec.decodePayload(SessionCommandsPayload.self, from: commandsValue)
        XCTAssertEqual(commands?.commands.first?.inputHint, "scope")

        let turn = try codec.decodePayload(
            SessionTurnEndedPayload.self,
            from: .object([("stop_reason", .string("cancelled"))])
        )
        XCTAssertEqual(turn?.stopReason, "cancelled")
    }

    func testGapEventSequenceIsReportedAsGap() {
        var journal = EventJournal()
        XCTAssertEqual(journal.observe(Self.event(session: "s", sequence: 1)), .accepted)
        XCTAssertEqual(journal.observe(Self.event(session: "s", sequence: 3)), .gap(expected: 2, received: 3))
        XCTAssertEqual(journal.observe(Self.event(session: "s", sequence: 1)), .duplicate)
        XCTAssertEqual(journal.observe(Self.event(session: "s", sequence: 2)), .accepted)
    }

    private static func event(session: String, sequence: Int) -> GatewayEvent {
        GatewayEvent(sessionId: session, sequence: sequence, event: "session.message")
    }
}
