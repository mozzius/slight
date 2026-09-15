import XCTest
@testable import SlightGateway

final class GatewayConnectionTests: XCTestCase {
    func testHandshakeAndCommandAcknowledgement() async throws {
        let transport = FakeGatewayTransport()
        transport.onSend = { frame in
            switch frame {
            case .hello:
                transport.push(.welcome(ServerWelcome(
                    connectionId: "c1",
                    serverName: "host",
                    serverVersion: "1",
                    capabilities: GatewayCapabilities(supportsReplay: true),
                    heartbeatIntervalMs: 60_000
                )))
            case .command(let command):
                transport.push(.ack(GatewayAck(
                    requestId: command.requestId,
                    ok: true,
                    result: .object([("sessions", .array([]))])
                )))
            default:
                break
            }
        }

        let connection = GatewayConnection(transportFactory: { transport })
        await connection.start()
        try await waitUntil { await connection.state.isConnected }

        let ack = try await connection.send(GatewayCommand(name: .sessionList))
        XCTAssertTrue(ack.ok)
        XCTAssertEqual(ack.result?["sessions"], .array([]))
    }

    func testSequenceGapRequestsReplay() async throws {
        let transport = FakeGatewayTransport()
        transport.onSend = { frame in
            switch frame {
            case .hello:
                transport.push(.welcome(ServerWelcome(
                    connectionId: "c1",
                    serverName: "host",
                    serverVersion: "1",
                    heartbeatIntervalMs: 60_000
                )))
            case .command(let command) where command.command == GatewayCommandName.eventsReplay.rawValue:
                transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true)))
            default:
                break
            }
        }

        let connection = GatewayConnection(transportFactory: { transport })
        await connection.start()
        try await waitUntil { await connection.state.isConnected }

        transport.push(.event(GatewayEvent(sessionId: "s1", sequence: 1, event: "session.message")))
        transport.push(.event(GatewayEvent(sessionId: "s1", sequence: 3, event: "session.message")))

        try await waitUntil {
            transport.frames().contains { frame in
                if case .command(let command) = frame {
                    return command.command == GatewayCommandName.eventsReplay.rawValue
                }
                return false
            }
        }

        let snapshot = await connection.journalSnapshot()
        XCTAssertEqual(snapshot["s1"], 1, "A gap must not advance the replay cursor")
    }

    func testCommandTimesOutWithoutAck() async throws {
        let transport = FakeGatewayTransport()
        transport.onSend = { frame in
            if case .hello = frame {
                transport.push(.welcome(ServerWelcome(
                    connectionId: "c1",
                    serverName: "host",
                    serverVersion: "1",
                    heartbeatIntervalMs: 60_000
                )))
            }
        }

        let config = GatewayConnectionConfig(
            reconnect: ReconnectPolicy(maxAttempts: 0),
            handshakeTimeout: .seconds(2),
            commandTimeout: .milliseconds(80),
            heartbeatTimeout: .seconds(60)
        )
        let connection = GatewayConnection(transportFactory: { transport }, config: config)
        await connection.start()
        try await waitUntil { await connection.state.isConnected }

        do {
            _ = try await connection.send(GatewayCommand(name: .hostStatus))
            XCTFail("Expected timeout")
        } catch let error as GatewayConnectionError {
            guard case .commandTimedOut = error else {
                return XCTFail("Unexpected error \(error)")
            }
        }
    }

    func testConnectionFailurePublishesTheUnderlyingReason() async throws {
        let config = GatewayConnectionConfig(
            reconnect: ReconnectPolicy(maxAttempts: 0),
            handshakeTimeout: .seconds(2),
            commandTimeout: .seconds(2),
            heartbeatTimeout: .seconds(60)
        )
        let connection = GatewayConnection(
            transportFactory: {
                throw GatewayConnectionError.handshakeFailed("TLS is unavailable")
            },
            config: config
        )

        await connection.start()
        try await waitUntil {
            if case .failed = await connection.state {
                return true
            }
            return false
        }

        guard case .failed(let failure) = await connection.state else {
            return XCTFail("Expected a failed connection")
        }
        XCTAssertEqual(failure.message, "Disconnected: Handshake failed: TLS is unavailable")
    }

    // MARK: Helpers

    private func waitUntil(
        timeout: TimeInterval = 3,
        _ condition: @escaping () async -> Bool
    ) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if await condition() { return }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
        throw GatewayConnectionError.timedOut
    }
}
