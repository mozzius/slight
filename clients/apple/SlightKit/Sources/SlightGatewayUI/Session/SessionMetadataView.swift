import SlightGateway
import SwiftUI

struct SessionMetadataView: View {
    let summary: SessionSummary

    var body: some View {
        VStack(spacing: 0) {
            if let recoveryMessage {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle")
                    Text(recoveryMessage)
                        .font(.caption)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 0)
                }
                .foregroundStyle(summary.recovery.isUnavailable ? Color.red : Color.orange)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background((summary.recovery.isUnavailable ? Color.red : Color.orange).opacity(0.10))
            }
            HStack(spacing: 12) {
                Label(summary.agent, systemImage: "cpu")
                if let model = summary.model { Text(model) }
                if let effort = summary.effort {
                    Label("\(effort) effort", systemImage: "dial.medium")
                }
                Spacer(minLength: 0)
            }
            .font(.caption)
            .foregroundStyle(.secondary)
            .lineLimit(1)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(.bar)
        }
    }

    private var recoveryMessage: String? {
        switch summary.recovery {
        case .live, .recovered:
            return nil
        case .stale(let reason):
            return reason.isEmpty
                ? "The native agent session no longer exists. Review is available, but new input is disabled."
                : "Native session is stale: \(reason) Review is available, but new input is disabled."
        case .unavailable(let reason):
            return reason.isEmpty
                ? "The agent cannot resume this session. Review is available, but new input is disabled."
                : "Native session is unavailable: \(reason) Review is available, but new input is disabled."
        }
    }
}
