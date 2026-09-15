#if DEBUG
import Foundation
import SlightGateway

public extension SlightAppModel {
    /// Builds an app model backed by the in-memory `FakeHost`.
    ///
    /// Development fixture only: this is compiled in `DEBUG` builds for SwiftUI
    /// previews and is never reachable from normal app startup (the production
    /// composition root always builds a real WebSocket-backed model).
    static func fakeHost(_ host: FakeHost) -> SlightAppModel {
        SlightAppModel(
            hostProfile: HostProfile(
                id: "fake-host",
                displayName: "Fake Slight Host",
                endpoint: URL(string: "fake://host/gateway")!,
                isLoopback: true
            ),
            credentialStore: InMemoryCredentialStore(),
            transportFactory: host.makeTransportFactory()
        )
    }
}
#endif
