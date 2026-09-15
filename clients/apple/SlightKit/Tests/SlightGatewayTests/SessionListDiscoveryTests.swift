import XCTest
@testable import SlightGateway

/// Drives `SessionListViewModel` discovery/import through an in-memory
/// transport. Discovery and import must not disturb the local session list or
/// its attach/replay state.
@MainActor
final class SessionListDiscoveryTests: XCTestCase {
    private let codec = GatewayCodec()

    // MARK: - Harness

    private final class JSONBox: @unchecked Sendable {
        private let lock = NSLock()
        private var stored: JSONValue?

        func set(_ value: JSONValue?) {
            lock.lock()
            stored = value
            lock.unlock()
        }

        var value: JSONValue? {
            lock.lock()
            defer { lock.unlock() }
            return stored
        }
    }

    private func makeModel(
        sessions: [SessionSummary] = [],
        catalog: [AgentCatalogEntry] = [],
        respond: @escaping @Sendable (GatewayCommand, FakeGatewayTransport) -> Void
    ) -> (SessionListViewModel, FakeGatewayTransport, GatewayConnection) {
        let transport = FakeGatewayTransport()
        let codec = self.codec
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
                switch command.command {
                case GatewayCommandName.sessionList.rawValue:
                    let result = try? codec.encodeToJSONValue(SessionListResult(sessions: sessions))
                    transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
                case GatewayCommandName.hostConfiguration.rawValue:
                    let status = HostStatus(
                        state: .running,
                        hostName: "host",
                        serverId: "server",
                        serverVersion: "1",
                        protocolVersion: GatewayProtocol.version,
                        supportedAgents: catalog.map(\.id),
                        agentCatalog: catalog
                    )
                    let result = try? codec.encodeToJSONValue(status)
                    transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
                default:
                    respond(command, transport)
                }
            default:
                break
            }
        }
        let connection = GatewayConnection(transportFactory: { transport })
        let model = SessionListViewModel(connection: connection, codec: codec)
        return (model, transport, connection)
    }

    private func start(_ model: SessionListViewModel, _ connection: GatewayConnection) async throws {
        model.start()
        await connection.start()
        try await waitUntil { model.connectionState.isConnected }
    }

    private func waitUntil(timeout: TimeInterval = 3, _ condition: () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
        throw GatewayConnectionError.timedOut
    }

    // MARK: - Discovery

    func testUnavailableAgentCatalogSurfacesHostError() async throws {
        let catalog = [AgentCatalogEntry(
            id: "codex",
            displayName: "Codex",
            version: nil,
            available: false,
            configOptions: []
        )]
        let (model, _, connection) = makeModel(catalog: catalog) { _, _ in }
        try await start(model, connection)

        await model.refreshHostConfiguration()

        XCTAssertEqual(model.agentCatalog, [])
        XCTAssertEqual(
            model.hostAvailabilityError,
            "Host connected, but no agent executable is available. Check the host installation or PATH for: Codex."
        )
    }

    func testDiscoverLoadsAndSortsByRecency() async throws {
        let discovered = AgentSessionsListResult(sessions: [
            AgentSessionSummary(
                agent: "fake",
                agentSessionId: "old",
                cwd: "/a",
                title: "Older",
                updatedAt: Date(timeIntervalSince1970: 100)
            ),
            AgentSessionSummary(
                agent: "fake",
                agentSessionId: "new",
                cwd: "/b",
                title: "Newer",
                updatedAt: Date(timeIntervalSince1970: 200)
            ),
            AgentSessionSummary(agent: "fake", agentSessionId: "untimed", cwd: "/c", title: "Untimed"),
        ])
        let codec = self.codec
        let (model, _, connection) = makeModel { command, transport in
            guard command.command == GatewayCommandName.agentSessionsList.rawValue else { return }
            let result = try? codec.encodeToJSONValue(discovered)
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
        }
        try await start(model, connection)

        await model.discoverAgentSessions(agent: "fake", workingDirectory: "~/work")

        guard case .loaded(let sessions) = model.discoveryState else {
            return XCTFail("Expected loaded discovery state, got \(model.discoveryState)")
        }
        XCTAssertEqual(sessions.map(\.agentSessionId), ["new", "old", "untimed"])
        XCTAssertNil(model.lastError)
    }

    func testLoadMoreAppendsToExistingDiscoveryResults() async throws {
        let codec = self.codec
        let (model, _, connection) = makeModel { command, transport in
            guard command.command == GatewayCommandName.agentSessionsList.rawValue else { return }
            let cursor = command.params?["cursor"]?.stringValue
            let sessions = cursor == nil
                ? [AgentSessionSummary(agent: "fake", agentSessionId: "first", cwd: "/a")]
                : [AgentSessionSummary(agent: "fake", agentSessionId: "second", cwd: "/b")]
            let result = try? codec.encodeToJSONValue(AgentSessionsListResult(
                sessions: sessions,
                nextCursor: cursor == nil ? "page-2" : nil
            ))
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
        }
        try await start(model, connection)

        await model.discoverAgentSessions(agent: "fake")
        await model.loadMoreAgentSessions(agent: "fake")

        guard case .loaded(let sessions) = model.discoveryState else {
            return XCTFail("Expected loaded discovery state, got \(model.discoveryState)")
        }
        XCTAssertEqual(sessions.map { $0.agentSessionId }, ["first", "second"])
    }

    func testDiscoverSendsAgentAndWorkingDirectory() async throws {
        let box = JSONBox()
        let codec = self.codec
        let (model, _, connection) = makeModel { command, transport in
            guard command.command == GatewayCommandName.agentSessionsList.rawValue else { return }
            box.set(command.params)
            let result = try? codec.encodeToJSONValue(AgentSessionsListResult(sessions: []))
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
        }
        try await start(model, connection)

        await model.discoverAgentSessions(agent: "codex", workingDirectory: "  ~/dev  ")

        let params = try XCTUnwrap(box.value)
        XCTAssertEqual(params["agent"]?.stringValue, "codex")
        XCTAssertEqual(params["working_directory_label"]?.stringValue, "~/dev")
        XCTAssertEqual(model.discoveryState, .loaded([]))
    }

    func testDiscoverBlankWorkingDirectorySendsNoLabel() async throws {
        let box = JSONBox()
        let codec = self.codec
        let (model, _, connection) = makeModel { command, transport in
            guard command.command == GatewayCommandName.agentSessionsList.rawValue else { return }
            box.set(command.params)
            let result = try? codec.encodeToJSONValue(AgentSessionsListResult(sessions: []))
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
        }
        try await start(model, connection)

        await model.discoverAgentSessions(agent: "fake", workingDirectory: "   ")

        let params = try XCTUnwrap(box.value)
        XCTAssertNil(params["working_directory_label"])
    }

    func testDiscoverFailureSurfacesUnsupportedCapability() async throws {
        let (model, _, connection) = makeModel { command, transport in
            guard command.command == GatewayCommandName.agentSessionsList.rawValue else { return }
            transport.push(.ack(GatewayAck(
                requestId: command.requestId,
                ok: false,
                error: GatewayErrorBody(code: "unsupported_capability", message: "agent does not support session/list")
            )))
        }
        try await start(model, connection)

        await model.discoverAgentSessions(agent: "fake")

        XCTAssertEqual(model.discoveryState, .failed("agent does not support session/list"))
    }

    func testDiscoveryDoesNotDisturbLocalSessions() async throws {
        let existing = SessionSummary(
            id: "keep",
            title: "Existing",
            agent: "fake",
            workingDirectoryLabel: "~",
            status: .working
        )
        let codec = self.codec
        let (model, _, connection) = makeModel(sessions: [existing]) { command, transport in
            guard command.command == GatewayCommandName.agentSessionsList.rawValue else { return }
            let result = try? codec.encodeToJSONValue(AgentSessionsListResult(sessions: [
                AgentSessionSummary(agent: "fake", agentSessionId: "native", cwd: "/tmp/proj"),
            ]))
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
        }
        try await start(model, connection)

        await model.discoverAgentSessions(agent: "fake")

        XCTAssertEqual(model.sessions.map(\.id), ["keep"])
        model.clearAgentSessionDiscovery()
        XCTAssertEqual(model.discoveryState, .idle)
    }

    // MARK: - Import

    func testImportSendsNativeIdentityAndUpserts() async throws {
        let box = JSONBox()
        let codec = self.codec
        let imported = SessionSummary(
            id: "s-imported",
            title: "Imported",
            agent: "fake",
            workingDirectoryLabel: "/Users/me/proj",
            status: .idle,
            recovery: .recovered
        )
        let (model, _, connection) = makeModel { command, transport in
            guard command.command == GatewayCommandName.agentSessionImport.rawValue else { return }
            box.set(command.params)
            let result = try? codec.encodeToJSONValue(ImportAgentSessionResult(session: imported))
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
        }
        try await start(model, connection)

        let native = AgentSessionSummary(
            agent: "fake",
            agentSessionId: "ses_native",
            cwd: "/Users/me/proj",
            title: "Prior session",
            updatedAt: Date()
        )
        let result = await model.importAgentSession(native, recovery: .resume)

        XCTAssertEqual(result?.id, "s-imported")
        XCTAssertTrue(model.sessions.contains { $0.id == "s-imported" })
        XCTAssertNil(model.importingAgentSessionId)
        XCTAssertNil(model.lastError)

        let params = try XCTUnwrap(box.value)
        XCTAssertEqual(params["agent"]?.stringValue, "fake")
        XCTAssertEqual(params["agent_session_id"]?.stringValue, "ses_native")
        XCTAssertEqual(params["working_directory_label"]?.stringValue, "/Users/me/proj")
        XCTAssertEqual(params["recovery"]?.stringValue, "resume")
        XCTAssertEqual(params["title"]?.stringValue, "Prior session")
    }

    func testImportDefaultsWorkingDirectoryToDiscoveredCwd() async throws {
        let box = JSONBox()
        let codec = self.codec
        let imported = SessionSummary(id: "s-1", title: "I", agent: "fake", workingDirectoryLabel: "/abs", status: .idle)
        let (model, _, connection) = makeModel { command, transport in
            guard command.command == GatewayCommandName.agentSessionImport.rawValue else { return }
            box.set(command.params)
            let result = try? codec.encodeToJSONValue(ImportAgentSessionResult(session: imported))
            transport.push(.ack(GatewayAck(requestId: command.requestId, ok: true, result: result)))
        }
        try await start(model, connection)

        let native = AgentSessionSummary(agent: "fake", agentSessionId: "ses", cwd: "/authoritative/path")
        _ = await model.importAgentSession(native, recovery: .load)

        let params = try XCTUnwrap(box.value)
        XCTAssertEqual(params["working_directory_label"]?.stringValue, "/authoritative/path")
    }

    func testImportFailureKeepsExistingSessionsAndSurfacesError() async throws {
        let existing = SessionSummary(
            id: "keep",
            title: "Existing",
            agent: "fake",
            workingDirectoryLabel: "~",
            status: .idle
        )
        let (model, _, connection) = makeModel(sessions: [existing]) { command, transport in
            guard command.command == GatewayCommandName.agentSessionImport.rawValue else { return }
            transport.push(.ack(GatewayAck(
                requestId: command.requestId,
                ok: false,
                error: GatewayErrorBody(code: "unsupported_capability", message: "agent does not support session/load")
            )))
        }
        try await start(model, connection)

        let native = AgentSessionSummary(agent: "fake", agentSessionId: "ses", cwd: "/tmp")
        let result = await model.importAgentSession(native, recovery: .load)

        XCTAssertNil(result)
        XCTAssertEqual(model.lastError, "agent does not support session/load")
        XCTAssertEqual(model.sessions.map(\.id), ["keep"])
        XCTAssertNil(model.importingAgentSessionId)
    }
}
