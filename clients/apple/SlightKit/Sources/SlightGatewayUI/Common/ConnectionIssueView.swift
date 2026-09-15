import SlightGateway
import SwiftUI

struct ConnectionIssueView: View {
    let state: GatewayConnectionState
    let endpoint: String
    let errorMessage: String?
    let onRetry: () -> Void
    let onSettings: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Can't connect to host", systemImage: "exclamationmark.triangle")
                .font(.headline)
                .foregroundStyle(.orange)

            Text(endpoint)
                .font(.caption)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
                .lineLimit(2)

            Text(errorMessage ?? state.shortDescription)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            HStack {
                Button("Retry", action: onRetry)
                    .buttonStyle(.borderedProminent)
                Button("Host Settings", action: onSettings)
                    .buttonStyle(.bordered)
            }
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .contain)
    }
}
