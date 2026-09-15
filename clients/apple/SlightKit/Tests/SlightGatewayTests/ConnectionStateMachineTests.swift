import XCTest
@testable import SlightGateway

final class ConnectionStateMachineTests: XCTestCase {
    func testHappyPathConnect() {
        var machine = ConnectionStateMachine()
        XCTAssertEqual(machine.handle(.connectRequested), .openTransport)
        XCTAssertEqual(machine.state, .connecting(attempt: 1))
        XCTAssertEqual(machine.handle(.transportOpened), .startHandshake)
        XCTAssertEqual(machine.state, .handshaking)

        let welcome = ServerWelcome(connectionId: "c", serverName: "host", serverVersion: "1")
        XCTAssertEqual(machine.handle(.handshakeSucceeded(welcome)), .none)
        XCTAssertEqual(machine.state, .connected(welcome))
    }

    func testDisconnectSchedulesBackoffAndEventualFailure() {
        let policy = ReconnectPolicy(initialDelay: .milliseconds(100), multiplier: 2, maxAttempts: 2)
        var machine = ConnectionStateMachine(policy: policy)
        _ = machine.handle(.connectRequested)
        _ = machine.handle(.transportOpened)

        XCTAssertEqual(machine.handle(.disconnected(reason: "drop")), .scheduleReconnect(delay: .milliseconds(100)))
        XCTAssertEqual(machine.state, .reconnecting(attempt: 2, delayMilliseconds: 100))
        XCTAssertEqual(machine.handle(.reconnectDelayElapsed), .openTransport)
        XCTAssertEqual(machine.state, .connecting(attempt: 2))

        _ = machine.handle(.transportOpened)
        // Second consecutive failure exhausts the two-attempt budget.
        XCTAssertEqual(machine.handle(.disconnected(reason: nil)), .giveUp)
        guard case .failed(let failure) = machine.state else {
            return XCTFail("Expected failure state")
        }
        XCTAssertEqual(failure.code, "reconnect_exhausted")
    }

    func testSuccessfulHandshakeResetsBackoff() {
        let policy = ReconnectPolicy(initialDelay: .milliseconds(100), multiplier: 2, maxAttempts: 3)
        var machine = ConnectionStateMachine(policy: policy)
        _ = machine.handle(.connectRequested)
        _ = machine.handle(.transportOpened)
        _ = machine.handle(.disconnected(reason: nil))
        XCTAssertEqual(machine.attempt, 2)

        _ = machine.handle(.reconnectDelayElapsed)
        _ = machine.handle(.transportOpened)
        let welcome = ServerWelcome(connectionId: "c", serverName: "host", serverVersion: "1")
        _ = machine.handle(.handshakeSucceeded(welcome))
        XCTAssertEqual(machine.attempt, 0)
        XCTAssertEqual(machine.state, .connected(welcome))

        _ = machine.handle(.disconnected(reason: nil))
        XCTAssertEqual(machine.state, .reconnecting(attempt: 1, delayMilliseconds: 100))
    }

    func testResyncIsVisibleWhileConnected() {
        var machine = ConnectionStateMachine()
        _ = machine.handle(.connectRequested)
        _ = machine.handle(.transportOpened)
        _ = machine.handle(.handshakeSucceeded(ServerWelcome(connectionId: "c", serverName: "h", serverVersion: "1")))
        _ = machine.handle(.resyncRequired(reason: "journal_evicted"))
        XCTAssertEqual(machine.state, .resyncRequired(reason: "journal_evicted"))
        XCTAssertTrue(machine.state.isConnected)
    }

    func testResetReturnsToIdle() {
        var machine = ConnectionStateMachine()
        _ = machine.handle(.connectRequested)
        _ = machine.handle(.reset)
        XCTAssertEqual(machine.state, .idle)
        XCTAssertEqual(machine.attempt, 0)
    }

    func testBackoffGrowsAndCaps() {
        let policy = ReconnectPolicy(initialDelay: .milliseconds(100), multiplier: 2, maxDelay: .milliseconds(400))
        XCTAssertEqual(policy.delay(forAttempt: 1), .milliseconds(100))
        XCTAssertEqual(policy.delay(forAttempt: 2), .milliseconds(200))
        XCTAssertEqual(policy.delay(forAttempt: 3), .milliseconds(400))
        XCTAssertEqual(policy.delay(forAttempt: 10), .milliseconds(400))
    }
}

final class EventJournalTests: XCTestCase {
    func testJournalIsPerSession() {
        var journal = EventJournal()
        XCTAssertEqual(journal.observe(GatewayEvent(sessionId: "a", sequence: 1, event: "x")), .accepted)
        XCTAssertEqual(journal.observe(GatewayEvent(sessionId: "b", sequence: 1, event: "x")), .accepted)
        XCTAssertEqual(journal.lastSequence(for: "a"), 1)
        XCTAssertEqual(journal.lastSequence(for: "b"), 1)
        XCTAssertEqual(journal.highestKnownSequence, 1)
    }

    func testMarkReplayedAdvancesCursor() {
        var journal = EventJournal()
        journal.markReplayed(sessionId: "a", to: 5)
        XCTAssertEqual(journal.lastSequence(for: "a"), 5)
        XCTAssertEqual(journal.observe(GatewayEvent(sessionId: "a", sequence: 6, event: "x")), .accepted)
    }
}

final class CredentialStoreTests: XCTestCase {
    func testInMemoryStoreRoundTrips() async throws {
        let store = InMemoryCredentialStore()
        let credential = DeviceCredential(deviceId: "d1", hostId: "h1", token: "t1", displayName: "iPhone")
        try await store.save(credential)
        let loaded = try await store.load(hostId: "h1")
        XCTAssertEqual(loaded, credential)

        try await store.delete(hostId: "h1")
        let deleted = try await store.load(hostId: "h1")
        XCTAssertNil(deleted)
    }
}
