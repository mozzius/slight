import SlightGateway
import SwiftUI

/// A small toolbar control for connection state, with connection actions kept in a menu.
struct CompactConnectionStatus: View {
    let state: GatewayConnectionState
    let errorMessage: String?
    let onConnect: () -> Void
    let onDisconnect: () -> Void
    var onSettings: (() -> Void)? = nil

    var body: some View {
        Menu {
            Label(state.shortDescription, systemImage: "circle.fill")

            if let errorMessage, !errorMessage.isEmpty {
                Text(errorMessage)
            }

            Divider()

            if state.isActive {
                Button("Disconnect", action: onDisconnect)
            } else {
                Button("Connect", action: onConnect)
            }

            if let onSettings {
                Button("Host Settings", action: onSettings)
            }
        } label: {
            Circle()
                .fill(color)
                .frame(width: 10, height: 10)
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())
        }
        .accessibilityLabel("Connection: \(state.shortDescription)")
        .accessibilityHint("Show connection actions")
    }

    private var color: Color {
        switch state {
        case .connected: return .green
        case .connecting, .handshaking, .reconnecting, .replaying: return .yellow
        case .resyncRequired: return .orange
        case .failed: return .red
        case .idle, .closed: return .gray
        }
    }
}
