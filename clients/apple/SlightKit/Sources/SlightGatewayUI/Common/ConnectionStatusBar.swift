import SlightGateway
import SwiftUI

/// Compact connection indicator used on both platforms. Shows the real host
/// endpoint and surfaces failure state with a retry action.
public struct ConnectionStatusBar: View {
    public let state: GatewayConnectionState
    public var endpoint: String?
    public var errorMessage: String?
    public var onConnect: () -> Void
    public var onDisconnect: () -> Void
    public var onRetry: (() -> Void)?

    public init(
        state: GatewayConnectionState,
        endpoint: String? = nil,
        errorMessage: String? = nil,
        onConnect: @escaping () -> Void,
        onDisconnect: @escaping () -> Void,
        onRetry: (() -> Void)? = nil
    ) {
        self.state = state
        self.endpoint = endpoint
        self.errorMessage = errorMessage
        self.onConnect = onConnect
        self.onDisconnect = onDisconnect
        self.onRetry = onRetry
    }

    public var body: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(color)
                .frame(width: 8, height: 8)
            VStack(alignment: .leading, spacing: 1) {
                Text(state.shortDescription)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                if let endpoint, !endpoint.isEmpty {
                    Text(endpoint)
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                if let errorMessage, !errorMessage.isEmpty {
                    Text(errorMessage)
                        .font(.caption2)
                        .foregroundStyle(.red)
                        .lineLimit(2)
                }
            }
            Spacer()
            if let onRetry {
                Button("Retry", action: onRetry)
                    .font(.caption)
            } else if state.isActive {
                Button("Disconnect", action: onDisconnect)
                    .font(.caption)
            } else {
                Button("Connect", action: onConnect)
                    .font(.caption)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(.thinMaterial)
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
