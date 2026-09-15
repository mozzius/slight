import SlightGateway
import SwiftUI

struct HostConnectionStatusView: View {
    let state: GatewayConnectionState
    let endpoint: String
    let errorMessage: String?
    let isApplyingHostProfile: Bool
    let isStartingLocalHost: Bool
    let onConnect: () -> Void
    let onDisconnect: () -> Void

    var body: some View {
        #if os(iOS)
        mobileBody
        #else
        desktopBody
        #endif
    }

    #if os(macOS)
    private var desktopBody: some View {
        Section("Connection") {
            LabeledContent("Status") {
                HStack {
                    if isBusy {
                        ProgressView()
                            .controlSize(.small)
                    }
                    Label(statusDescription, systemImage: statusIcon)
                        .foregroundStyle(statusColor)
                }
            }

            LabeledContent("Endpoint", value: endpoint)

            if let progressDescription {
                Text(progressDescription)
                    .foregroundStyle(.secondary)
            }

            if let errorMessage, !errorMessage.isEmpty {
                Label(errorMessage, systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red)
            }

            HStack {
                Button("Connect", systemImage: "bolt.horizontal", action: onConnect)
                    .disabled(state.isActive || isApplyingHostProfile || isStartingLocalHost)
                Button("Disconnect", systemImage: "xmark.circle", action: onDisconnect)
                    .disabled(!state.isActive || isApplyingHostProfile || isStartingLocalHost)
            }
        }
    }
    #endif

    #if os(iOS)
    private var mobileBody: some View {
        Section("Connection") {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 10) {
                    if isBusy {
                        ProgressView()
                            .controlSize(.small)
                    }
                    Label(statusDescription, systemImage: statusIcon)
                        .foregroundStyle(statusColor)
                        .font(.headline)
                }

                if let progressDescription {
                    Text(progressDescription)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }

                if let errorMessage, !errorMessage.isEmpty {
                    Label(errorMessage, systemImage: "exclamationmark.triangle.fill")
                        .font(.subheadline)
                        .foregroundStyle(.red)
                }

                Button(
                    state.isActive ? "Disconnect" : "Connect",
                    systemImage: state.isActive ? "xmark.circle" : "bolt.horizontal",
                    action: state.isActive ? onDisconnect : onConnect
                )
                .buttonStyle(.bordered)
                .frame(maxWidth: .infinity)
                .disabled(isApplyingHostProfile || isStartingLocalHost)
            }
            .padding(.vertical, 4)
        }
    }
    #endif

    private var isBusy: Bool {
        if isApplyingHostProfile || isStartingLocalHost {
            return true
        }
        switch state {
        case .connecting, .handshaking, .reconnecting, .replaying:
            return true
        case .idle, .connected, .resyncRequired, .failed, .closed:
            return false
        }
    }

    private var progressDescription: String? {
        if isStartingLocalHost {
            return "Starting the local host…"
        }
        if isApplyingHostProfile {
            return "Saving the host and starting a new connection…"
        }
        switch state {
        case .connecting:
            return "Opening the WebSocket connection…"
        case .handshaking:
            return "Connected to the socket; waiting for the host handshake…"
        case .reconnecting(_, let delayMilliseconds):
            return "The last attempt failed. Retrying in \(delayMilliseconds) ms."
        case .replaying:
            return "Restoring retained session events…"
        case .idle:
            return "Ready to connect."
        case .closed:
            return "Disconnected by this client."
        case .connected, .resyncRequired, .failed:
            return nil
        }
    }

    private var statusIcon: String {
        if isStartingLocalHost {
            return "arrow.trianglehead.2.clockwise.rotate.90"
        }
        switch state {
        case .connected:
            return "checkmark.circle.fill"
        case .connecting, .handshaking, .reconnecting, .replaying:
            return "arrow.trianglehead.2.clockwise.rotate.90"
        case .resyncRequired:
            return "arrow.clockwise.circle.fill"
        case .failed:
            return "exclamationmark.triangle.fill"
        case .idle, .closed:
            return "circle"
        }
    }

    private var statusColor: Color {
        if isStartingLocalHost {
            return .orange
        }
        switch state {
        case .connected:
            return .green
        case .connecting, .handshaking, .reconnecting, .replaying:
            return .orange
        case .resyncRequired:
            return .yellow
        case .failed:
            return .red
        case .idle, .closed:
            return .secondary
        }
    }

    private var statusDescription: String {
        if isStartingLocalHost { return "Host starting" }
        switch state {
        case .idle: return "Ready to connect"
        case .connecting: return "Connecting"
        case .handshaking: return "Handshaking"
        case .connected: return "Connected"
        case .reconnecting: return "Reconnecting"
        case .replaying: return "Replaying"
        case .resyncRequired: return "Resync required"
        case .failed: return "Failed"
        case .closed: return "Disconnected"
        }
    }
}
