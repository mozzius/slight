#if os(macOS)
import Combine
import Foundation
import SlightGateway

/// Backing model for the macOS host administration surface. Every action maps
/// to a `HostAdminService` (gateway) call; no process supervision happens here.
@MainActor
final class HostAdminViewModel: ObservableObject {
    @Published var status: HostStatus?
    @Published var configuration: HostConfiguration?
    @Published var devices: [PairedDevice] = []
    @Published var diagnostics: HostDiagnostics?
    @Published var pairing: PairingArtifact?
    @Published var sessions: [SessionSummary] = []
    @Published var selectedSessionId: String?
    @Published var inspectedSession: SessionInspectResult?
    @Published var isLoading = false
    @Published var error: String?

    let service: HostAdminService

    init(service: HostAdminService) {
        self.service = service
    }

    func refreshAll() async {
        isLoading = true
        defer { isLoading = false }
        await loadStatus()
        await loadConfiguration()
        await loadDevices()
        await loadSessions()
    }

    func loadStatus() async {
        do { status = try await service.status() } catch { report(error) }
    }

    func loadConfiguration() async {
        do { configuration = try await service.configuration() } catch { report(error) }
    }

    func loadDevices() async {
        do { devices = try await service.devices() } catch { report(error) }
    }

    func loadDiagnostics() async {
        do { diagnostics = try await service.diagnostics() } catch { report(error) }
    }

    func loadSessions() async {
        do { sessions = try await service.sessions() } catch { report(error) }
    }

    func startHost() async {
        do { status = try await service.startHost(); error = nil } catch { report(error) }
    }

    func stopHost() async {
        do { status = try await service.stopHost(); error = nil } catch { report(error) }
    }

    func restartHost() async {
        do { status = try await service.restartHost(); error = nil } catch { report(error) }
    }

    func createPairing(deviceName: String?) async {
        do { pairing = try await service.createPairing(deviceName: deviceName); error = nil } catch { report(error) }
    }

    func revoke(_ device: PairedDevice) async {
        do {
            try await service.revoke(deviceId: device.deviceId)
            await loadDevices()
        } catch {
            report(error)
        }
    }

    func inspect(sessionId: String) async {
        selectedSessionId = sessionId
        do { inspectedSession = try await service.inspect(sessionId: sessionId) } catch { report(error) }
    }

    private func report(_ error: Error) {
        if let adminError = error as? HostAdminError {
            self.error = adminError.userMessage
        } else if let connectionError = error as? GatewayConnectionError {
            self.error = connectionError.userMessage
        } else {
            self.error = error.localizedDescription
        }
    }
}
#endif
