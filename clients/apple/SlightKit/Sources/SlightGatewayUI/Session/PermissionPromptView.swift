import SlightGateway
import SwiftUI

public struct PermissionPromptView: View {
    @ObservedObject public var viewModel: SessionViewModel

    public init(viewModel: SessionViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        ForEach(viewModel.permissionRequests) { request in
            VStack(alignment: .leading, spacing: 12) {
                Label {
                    Text("Guardian Review")
                        .font(.subheadline.weight(.semibold))
                } icon: {
                    Image(systemName: "lock.shield.fill")
                        .foregroundStyle(.orange)
                }
                let review = ApprovalReview(request: request)
                if review.title != "Guardian Review" {
                    Text(review.title)
                        .font(.headline)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if let status = review.status {
                    HStack(spacing: 8) {
                        ReviewBadge(label: status, color: statusColor(status))
                        if let risk = review.risk {
                            ReviewBadge(label: "Risk \(risk)", color: riskColor(risk))
                        }
                        if let authorization = review.authorization {
                            ReviewBadge(label: "Authorization \(authorization)", color: .blue)
                        }
                    }
                }
                if let action = review.action {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Action")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.secondary)
                        Text(action)
                            .font(.system(.callout, design: .monospaced))
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(10)
                    .background(Color.primary.opacity(0.06), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                }
                if let rationale = review.rationale {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Why")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.secondary)
                        Text(rationale)
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                if review.isUnstructured, let detail = request.detail {
                    Text(detail)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .textSelection(.enabled)
                }
                if let toolCallId = request.toolCallId {
                    Text("Tool call \(toolCallId)")
                        .font(.caption2)
                        .fontDesign(.monospaced)
                        .foregroundStyle(.tertiary)
                }
                actionButtons(for: request)
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background {
                if #available(iOS 26, macOS 26, *) {
                    Color.clear
                } else {
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .fill(.regularMaterial)
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
            .modifier(GlassEffectModifier())
            .overlay {
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .strokeBorder(Color.orange.opacity(0.22), lineWidth: 1)
            }
        }
    }

    @ViewBuilder
    private func actionButtons(for request: PermissionRequest) -> some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                buttons(for: request)
            }
            VStack(spacing: 8) {
                buttons(for: request)
            }
        }
    }

    @ViewBuilder
    private func buttons(for request: PermissionRequest) -> some View {
        ForEach(request.options) { option in
            Button {
                Task { await viewModel.respond(to: request, option: option) }
            } label: {
                Label(label(for: option), systemImage: icon(for: option.kind))
                    .font(.subheadline.weight(.semibold))
                    .lineLimit(2)
                    .minimumScaleFactor(0.85)
                    .frame(maxWidth: .infinity, minHeight: 44)
            }
            .buttonStyle(.plain)
            .modifier(PermissionActionGlass(tint: tint(for: option.kind)))
            .disabled(viewModel.respondingPermissionIds.contains(request.id))
        }

        if viewModel.respondingPermissionIds.contains(request.id) {
            ProgressView()
                .controlSize(.small)
                .padding(.horizontal, 8)
        }
    }

    private func tint(for kind: PermissionOption.Kind) -> Color {
        switch kind {
        case .allow, .allowAlways: return .accentColor
        case .deny, .denyAlways: return .red
        case .custom: return .gray
        }
    }

    private func icon(for kind: PermissionOption.Kind) -> String {
        switch kind {
        case .allow: return "checkmark.circle"
        case .allowAlways: return "checkmark.shield"
        case .deny: return "xmark.circle"
        case .denyAlways: return "hand.raised.slash"
        case .custom: return "ellipsis.circle"
        }
    }

    private func label(for option: PermissionOption) -> String {
        switch option.kind {
        case .allow: return "Allow Once"
        case .allowAlways: return "Always Allow"
        case .deny: return "Deny"
        case .denyAlways: return "Always Deny"
        case .custom: return option.label
        }
    }

    private func statusColor(_ status: String) -> Color {
        status.lowercased() == "approved" ? .green : .orange
    }

    private func riskColor(_ risk: String) -> Color {
        switch risk.lowercased() {
        case "low": return .green
        case "medium": return .orange
        default: return .red
        }
    }
}

private struct ReviewBadge: View {
    let label: String
    let color: Color

    var body: some View {
        Text(label)
            .font(.caption2.weight(.semibold))
            .foregroundStyle(color)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(color.opacity(0.12), in: Capsule())
    }
}

private struct ApprovalReview {
    let title: String
    let status: String?
    let action: String?
    let risk: String?
    let authorization: String?
    let rationale: String?
    let isUnstructured: Bool

    init(request: PermissionRequest) {
        title = request.title
        var fields: [String: String] = [:]
        for line in (request.detail ?? "").split(separator: "\n", omittingEmptySubsequences: true) {
            guard let separator = line.firstIndex(of: ":") else { continue }
            let key = line[..<separator].trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            let value = line[line.index(after: separator)...].trimmingCharacters(in: .whitespacesAndNewlines)
            fields[key] = value
        }
        status = fields["status"]
        action = fields["action"]
        risk = fields["risk"]
        authorization = fields["authorization"]
        rationale = fields["rationale"]
        isUnstructured = fields.isEmpty
    }
}

private struct PermissionActionGlass: ViewModifier {
    let tint: Color

    @ViewBuilder
    func body(content: Content) -> some View {
        if #available(iOS 26, macOS 26, *) {
            content
                .glassEffect(.regular.tint(tint.opacity(0.18)), in: .rect(cornerRadius: 13))
        } else {
            content
                .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 13, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 13, style: .continuous)
                        .strokeBorder(tint.opacity(0.3), lineWidth: 1)
                }
        }
    }
}
