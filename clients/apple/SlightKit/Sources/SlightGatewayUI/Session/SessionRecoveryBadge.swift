import SlightGateway
import SwiftUI

/// Compact badge that flags a session whose native agent session is no longer
/// live. Live and recovered sessions are intentionally not badged; stale and
/// unavailable sessions keep their replayable local journal but cannot accept
/// input, so the badge is a warning rather than an error.
struct SessionRecoveryBadge: View {
    let recovery: SessionRecoveryState

    var body: some View {
        if let presentation = RecoveryPresentation(recovery) {
            Label(presentation.label, systemImage: presentation.icon)
                .font(.caption2.weight(.semibold))
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(presentation.color.opacity(0.18))
                .foregroundStyle(presentation.color)
                .clipShape(Capsule())
                .accessibilityLabel(presentation.accessibilityLabel)
                .help(presentation.help)
        }
    }
}

/// View-facing projection of a non-live recovery state. Returns `nil` for
/// states that should render as normal sessions.
struct RecoveryPresentation {
    let label: String
    let icon: String
    let color: Color
    let help: String
    let accessibilityLabel: String

    init?(_ recovery: SessionRecoveryState) {
        switch recovery {
        case .live, .recovered:
            return nil
        case .stale(let reason):
            label = "Stale"
            icon = "clock.badge.exclamationmark"
            color = .orange
            help = reason.isEmpty ? "The native agent session no longer exists." : reason
            accessibilityLabel = "Stale native session. \(help)"
        case .unavailable(let reason):
            label = "Unavailable"
            icon = "exclamationmark.triangle.fill"
            color = .red
            help = reason.isEmpty ? "The agent cannot resume this session." : reason
            accessibilityLabel = "Unavailable native session. \(help)"
        }
    }
}
