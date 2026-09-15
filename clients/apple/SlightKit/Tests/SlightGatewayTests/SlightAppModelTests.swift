import Foundation
import SlightGateway
import SlightGatewayUI
import XCTest

@MainActor
final class SlightAppModelTests: XCTestCase {
    func testMissingHostRequiresOnboardingWithoutStartingAConnection() async {
        let model = SlightAppModel(
            hostProfile: nil,
            credentialStore: InMemoryCredentialStore()
        )

        XCTAssertTrue(model.isOnboardingRequired)
        XCTAssertNil(model.hostProfile)
        XCTAssertEqual(model.connectionState, .idle)
    }

    func testConfiguredDisconnectedHostDoesNotRequireOnboarding() {
        let model = SlightAppModel(
            hostProfile: .loopback(),
            credentialStore: InMemoryCredentialStore()
        )

        XCTAssertFalse(model.isOnboardingRequired)
        XCTAssertEqual(model.connectionState, .idle)
    }

    func testRemovingHostClearsConfigurationAndRequiresOnboarding() {
        let suiteName = "SlightAppModelTests.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            XCTFail("Could not create isolated user defaults")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let store = UserDefaultsHostProfileStore(defaults: defaults)
        let profile = HostProfile.loopback()
        store.save(profile)
        let model = SlightAppModel(
            hostProfile: profile,
            credentialStore: InMemoryCredentialStore(),
            hostProfileStore: store
        )

        model.removeHostProfile()

        XCTAssertNil(model.hostProfile)
        XCTAssertTrue(model.isOnboardingRequired)
        XCTAssertNil(store.load())
    }

    func testUpdatingHostProfilePersistsAndConnectsToTheSelection() async throws {
        let suiteName = "SlightAppModelTests.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            XCTFail("Could not create isolated user defaults")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let store = UserDefaultsHostProfileStore(defaults: defaults)
        let transport = FakeGatewayTransport()
        transport.onSend = { frame in
            guard case .hello = frame else { return }
            transport.push(.welcome(ServerWelcome(
                connectionId: "connection",
                serverName: "remote-host",
                serverVersion: "1",
                heartbeatIntervalMs: 60_000
            )))
        }
        let model = SlightAppModel(
            hostProfile: .loopback(),
            credentialStore: InMemoryCredentialStore(),
            transportFactory: { transport },
            hostProfileStore: store
        )
        guard let remoteURL = URL(string: "wss://slight-host.example.ts.net/gateway") else {
            XCTFail("Could not create remote host URL")
            return
        }
        let remoteProfile = HostProfile(
            id: "configured",
            displayName: "slight-host.example.ts.net",
            endpoint: remoteURL,
            isLoopback: false
        )

        model.updateHostProfile(remoteProfile)

        XCTAssertEqual(model.hostProfile, remoteProfile)
        XCTAssertEqual(store.load(), remoteProfile)
        try await waitUntil { await model.connection.state.isConnected }
        model.completeOnboarding()
        XCTAssertFalse(model.isOnboardingRequired)
        await model.connection.stop()
    }

    private func waitUntil(
        timeout: TimeInterval = 3,
        _ condition: @escaping () async -> Bool
    ) async throws {
        let deadline = Date.now.addingTimeInterval(timeout)
        while Date.now < deadline {
            if await condition() { return }
            try await Task.sleep(for: .milliseconds(5))
        }
        throw GatewayConnectionError.timedOut
    }
}
