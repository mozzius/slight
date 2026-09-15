import Combine
import Foundation
import SlightGateway

/// Composition root for the shared SwiftUI client. Owns the single
/// `GatewayConnection`, the credential store, and the shared view models.
@MainActor
public final class SlightAppModel: ObservableObject {
    @Published public private(set) var hostProfile: HostProfile?
    @Published public private(set) var isOnboardingRequired: Bool
    @Published public private(set) var isApplyingHostProfile = false

    public let codec: GatewayCodec
    public let credentialStore: DeviceCredentialStore
    public let localHostService: LocalHostServiceController
    public let connection: GatewayConnection
    public let adminService: HostAdminService
    public let sessionList: SessionListViewModel

    private let hostProfileStore: HostProfileStore
    private let transportFactoryOverride: GatewayTransportFactory?
    private var sessionViewModels: [String: SessionViewModel] = [:]
    private var cancellables: Set<AnyCancellable> = []

    public init(
        hostProfile: HostProfile?,
        credentialStore: DeviceCredentialStore,
        connectionConfig: GatewayConnectionConfig = GatewayConnectionConfig(),
        transportFactory: GatewayTransportFactory? = nil,
        hostProfileStore: HostProfileStore = UserDefaultsHostProfileStore(),
        localHostService: LocalHostServiceController? = nil
    ) {
        self.hostProfile = hostProfile
        self.isOnboardingRequired = hostProfile == nil
        self.credentialStore = credentialStore
        self.localHostService = localHostService ?? LocalHostServiceController()
        self.hostProfileStore = hostProfileStore
        self.transportFactoryOverride = transportFactory

        let codec = GatewayCodec()
        self.codec = codec

        let hostId = hostProfile?.id
        let factory: GatewayTransportFactory
        if let hostProfile {
            factory = transportFactory ?? WebSocketTransportFactory(url: hostProfile.endpoint).makeFactory()
        } else if let transportFactory {
            factory = transportFactory
        } else {
            factory = { throw GatewayConnectionError.notConnected }
        }
        let connection = GatewayConnection(
            transportFactory: factory,
            codec: codec,
            config: connectionConfig,
            credentialProvider: {
                 guard let hostId else { return nil }
                 return try? await credentialStore.load(hostId: hostId)
            }
        )
        self.connection = connection
        self.adminService = GatewayHostAdminService(connection: connection, codec: codec)
        self.sessionList = SessionListViewModel(connection: connection, codec: codec)
        self.sessionList.objectWillChange
            .sink { [weak self] in self?.objectWillChange.send() }
            .store(in: &cancellables)
        self.sessionList.start()
    }

    public var connectionState: GatewayConnectionState {
        sessionList.connectionState
    }

    /// The error the user should see for the current connection. Prefers the
    /// connection-level failure over the last command error.
    public var connectionErrorMessage: String? {
        connectionState.failureMessage ?? sessionList.hostAvailabilityError ?? sessionList.lastError
    }

    public func connect() {
        Task {
            if hostProfile?.isLoopback == true {
                guard await localHostService.startIfNeeded() else {
                    sessionList.lastError = localHostService.errorMessage ?? "The local host could not be started."
                    return
                }
            }
            sessionList.connect()
        }
    }

    public func disconnect() {
        sessionList.disconnect()
    }

    /// Points the client at a new host endpoint, persists it, and connects
    /// against it. The shared `GatewayConnection` and view models are reused, so
    /// open screens keep observing the same connection after the switch.
    public func updateHostProfile(_ profile: HostProfile) {
        hostProfile = profile
        hostProfileStore.save(profile)
        sessionList.lastError = nil
        isApplyingHostProfile = true

        let factory = transportFactoryOverride
            ?? WebSocketTransportFactory(url: profile.endpoint).makeFactory()
        let credentialStore = self.credentialStore
        let hostId = profile.id
        Task {
            if profile.isLoopback {
                guard await localHostService.startIfNeeded() else {
                    isApplyingHostProfile = false
                    sessionList.lastError = localHostService.errorMessage ?? "The local host could not be started."
                    return
                }
            }
            await connection.reconfigure(
                transportFactory: factory,
                credentialProvider: { try? await credentialStore.load(hostId: hostId) }
            )
            await connection.start()
            isApplyingHostProfile = false
        }
    }

    /// Leaves onboarding only after the configured host has completed its
    /// gateway handshake. A disconnected saved host does not re-enter onboarding.
    public func completeOnboarding() {
        guard connectionState.isConnected else { return }
        isOnboardingRequired = false
    }

    /// Removes the saved host and returns the app to its unconfigured state.
    public func removeHostProfile() {
        hostProfileStore.clear()
        hostProfile = nil
        isOnboardingRequired = true
        Task { await connection.stop() }
    }

    /// Restores the local development default (loopback host on this Mac).
    public func resetToDefaultHost() {
        updateHostProfile(.loopback())
    }

    public func makeSessionViewModel(summary: SessionSummary) -> SessionViewModel {
        if let viewModel = sessionViewModels[summary.id] {
            viewModel.updateSummary(summary)
            return viewModel
        }

        let viewModel = SessionViewModel(
            sessionId: summary.id,
            connection: connection,
            codec: codec,
            initialSummary: summary
        )
        sessionViewModels[summary.id] = viewModel
        return viewModel
    }
}
