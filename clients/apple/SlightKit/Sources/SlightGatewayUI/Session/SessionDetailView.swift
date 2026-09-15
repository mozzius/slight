import SlightGateway
import SwiftUI

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

public struct SessionDetailView: View {
    @ObservedObject public var viewModel: SessionViewModel
    @State private var showingRename = false
    @State private var renameText = ""

    public init(viewModel: SessionViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        VStack(spacing: 0) {
            if let replay = viewModel.replayState.description {
                ReplayBanner(
                    text: replay,
                    isError: viewModel.replayState.isFailed,
                    onRetry: viewModel.replayState.isFailed
                        ? { Task { await viewModel.loadSnapshot() } }
                        : nil
                )
            }
            SessionTranscriptView(
                items: viewModel.transcript,
                isAgentWorking: viewModel.isAgentWorking,
                isLoadingHistory: viewModel.isLoadingHistory,
                hasOlderHistory: viewModel.hasOlderHistory,
                hasLoadedHistory: viewModel.hasLoadedHistory,
                isReplaying: viewModel.replayState.isActive,
                showsEmptyState: viewModel.permissionRequests.isEmpty,
                onLoadOlder: viewModel.loadOlderHistory
            )
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            VStack(spacing: 0) {
                if !viewModel.permissionRequests.isEmpty {
                    PermissionPromptView(viewModel: viewModel)
                        .padding(.horizontal, 16)
                        .padding(.bottom, 8)
                        .frame(maxWidth: 800)
                        .frame(maxWidth: .infinity)
                }
                if let error = viewModel.lastError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .padding(.horizontal, 12)
                        .padding(.bottom, 4)
                }
                ComposerView(
                    message: $viewModel.draft,
                    model: Binding(
                        get: { viewModel.session?.model ?? "Model" },
                        set: { value in
                            guard let configId = viewModel.modelOptionId else { return }
                            Task { await viewModel.setConfigOption(configId: configId, valueId: value) }
                        }
                    ),
                    effort: .constant(viewModel.session?.effort ?? "Effort"),
                    models: viewModel.modelOptions,
                    efforts: [],
                    configurationIsDisabled: false,
                    isEnabled: viewModel.canSend,
                    canCancel: viewModel.canCancel,
                    disabledReason: sessionComposerDisabledReason,
                    onSend: { Task { await viewModel.send() } },
                    onCancel: { Task { await viewModel.cancel() } }
                )
                    .frame(maxWidth: 800)
                    .frame(maxWidth: .infinity)
            }
        }
        .navigationTitle(viewModel.session?.title ?? "Session")
        #if os(iOS)
        .navigationBarTitleDisplayMode(.inline)
        #endif
         .toolbar {
             #if os(macOS)
             ToolbarItemGroup(placement: .primaryAction) {
                 ViewThatFits(in: .horizontal) {
                     directSessionActions
                    sessionActionsMenu
                }
            }
            #else
            ToolbarItem(placement: .primaryAction) {
                sessionActionsMenu
            }
            #endif
        }
        .alert("Change Session Name", isPresented: $showingRename) {
            TextField("Session name", text: $renameText)
            Button("Cancel", role: .cancel) {}
            Button("Save") {
                Task { await viewModel.rename(to: renameText) }
            }
        } message: {
            Text("Choose a name that helps you find this session later.")
        }
        .task { viewModel.start() }
    }

    private var sessionComposerDisabledReason: String? {
        guard !viewModel.canCancel else { return nil }
        if !viewModel.connectionState.isConnected { return "Connect to a host to send messages." }
        if let recovery = viewModel.session?.recovery, !recovery.acceptsInput {
            switch recovery {
            case .stale:
                return "This session's native agent session no longer exists."
            case .unavailable:
                return "This session's agent is unavailable, so it can't accept input."
            case .live, .recovered:
                break
            }
        }
        if viewModel.session?.status == .exited { return "This session has exited." }
        return nil
    }

    private func copyToPasteboard(_ value: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(value, forType: .string)
        #elseif os(iOS)
        UIPasteboard.general.string = value
        #endif
    }

    private var directSessionActions: some View {
        HStack(spacing: 8) {
            Button("Rename", systemImage: "pencil") {
                renameText = viewModel.session?.title ?? ""
                showingRename = true
            }

            Button("Copy Branch", systemImage: "arrow.triangle.branch") {
                if let branch = viewModel.session?.gitBranch {
                    copyToPasteboard(branch)
                }
            }
            .disabled(viewModel.session?.gitBranch == nil)

            permissionModeMenu
        }
    }

    private var sessionActionsMenu: some View {
        Menu {
            Button("Change Session Name", systemImage: "pencil") {
                renameText = viewModel.session?.title ?? ""
                showingRename = true
            }

            Button("Copy Git Branch Name", systemImage: "arrow.triangle.branch") {
                if let branch = viewModel.session?.gitBranch {
                    copyToPasteboard(branch)
                }
            }
            .disabled(viewModel.session?.gitBranch == nil)

            Divider()

            if let modes = viewModel.acpMetadata?.modes,
               !modes.availableModes.isEmpty {
                Menu("Permission Mode", systemImage: "hand.raised") {
                    modeButtons(modes)
                }
            } else {
                Label("Permission Mode Unavailable", systemImage: "hand.raised.slash")
            }
        } label: {
            Label("Session Actions", systemImage: "ellipsis")
        }
        .help("Session Actions")
    }

    @ViewBuilder
    private var permissionModeMenu: some View {
        if let modes = viewModel.acpMetadata?.modes,
           !modes.availableModes.isEmpty {
            Menu {
                modeButtons(modes)
            } label: {
                Label(currentModeName(in: modes), systemImage: "hand.raised")
            }
            .help("Permission Mode")
        }
    }

    @ViewBuilder
    private func modeButtons(_ modes: AcpModeState) -> some View {
        ForEach(modes.availableModes) { mode in
            Button {
                Task { await viewModel.setMode(mode) }
            } label: {
                HStack {
                    Text(mode.name)
                    if mode.id == modes.currentModeId {
                        Spacer()
                        Image(systemName: "checkmark")
                    }
                }
            }
        }
    }

    private func currentModeName(in modes: AcpModeState) -> String {
        modes.availableModes.first { $0.id == modes.currentModeId }?.name ?? "Permission Mode"
    }

}

struct ReplayBanner: View {
    let text: String
    let isError: Bool
    var onRetry: (() -> Void)?

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: isError ? "exclamationmark.triangle" : "arrow.triangle.2.circlepath")
            Text(text)
                .font(.caption)
            Spacer()
            if let onRetry {
                Button("Retry", action: onRetry)
                    .font(.caption)
                    .buttonStyle(.borderless)
            }
        }
        .foregroundStyle(isError ? Color.orange : Color.secondary)
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background((isError ? Color.orange : Color.secondary).opacity(0.12))
    }
}

struct NoticeRowView: View {
    let notice: TranscriptNotice

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: icon)
            Text(notice.text)
                .font(.caption)
                .textSelection(.enabled)
            Spacer()
        }
        .foregroundStyle(color)
        .padding(8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(color.opacity(0.08))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var icon: String {
        switch notice.kind {
        case .info: return "info.circle"
        case .warning: return "exclamationmark.triangle"
        case .error: return "xmark.octagon"
        }
    }

    private var color: Color {
        switch notice.kind {
        case .info: return .secondary
        case .warning: return .orange
        case .error: return .red
        }
    }
}

public struct ToolCallRowView: View {
    public let toolCall: ToolCall
    @State private var showingRawOutput = false

    public init(toolCall: ToolCall) {
        self.toolCall = toolCall
    }

    public var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: icon)
                .foregroundStyle(color)
                .frame(width: 18)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(displayTitle)
                        .font(.callout.weight(.medium))
                        .lineLimit(1)
                        .truncationMode(.middle)
                    if let kind = toolCall.kind {
                        Text(kind)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(Color.primary.opacity(0.06))
                            .clipShape(Capsule())
                    }
                }
                if let detail = toolCall.detail, !detail.isEmpty {
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .textSelection(.enabled)
                }
                if !toolCall.content.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(toolCall.content.enumerated()), id: \.offset) { _, content in
                            ToolCallContentView(content: content)
                        }
                    }
                }
                if !toolCall.locations.isEmpty {
                    VStack(alignment: .leading, spacing: 1) {
                        ForEach(Array(toolCall.locations.enumerated()), id: \.offset) { _, location in
                            Label(locationLabel(location), systemImage: "doc.text")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                    }
                }
            }
            Spacer()
            if toolCall.status == .inProgress {
                ProgressView().controlSize(.mini)
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.primary.opacity(0.04))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .onTapGesture {
            guard hasPopoverContent else { return }
            showingRawOutput = true
        }
        .accessibilityAddTraits(hasPopoverContent ? .isButton : [])
        .accessibilityHint(hasPopoverContent ? "Opens the full tool details" : "")
        .popover(isPresented: $showingRawOutput) {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    if let detail = toolCall.detail, !detail.isEmpty {
                        Text(detail)
                            .font(.system(.caption, design: .monospaced))
                            .textSelection(.enabled)
                    }
                    if let rawOutput = toolCall.rawOutput {
                        Text(rawOutput.prettyDescription)
                            .font(.system(.caption, design: .monospaced))
                            .textSelection(.enabled)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
            }
            .frame(minWidth: 280, maxWidth: 560, maxHeight: 420)
            .presentationCompactAdaptation(.popover)
        }
    }

    private var hasPopoverContent: Bool {
        toolCall.rawOutput != nil || !(toolCall.detail?.isEmpty ?? true)
    }

    private var displayTitle: String {
        let title = toolCall.title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty else { return "Tool result" }

        if title.hasPrefix("mcp.") {
            return "Called \(rawAction ?? humanizedMCPName(title))"
        }
        return title
    }

    private var rawAction: String? {
        let keys = [
            "action", "description", "label", "title", "query", "prompt", "instruction",
            "task", "intent", "goal", "request", "input", "text", "target", "app"
        ]

        guard let rawInput = toolCall.rawInput else { return nil }
        if case .object(let object) = rawInput {
            for key in keys {
                if let action = naturalAction(in: object[key], key: key) {
                    return action
                }
            }
            for (key, value) in object {
                if let action = naturalAction(in: value, key: key) {
                    return action
                }
            }
        }
        return nil
    }

    private func naturalAction(in value: JSONValue?, key: String) -> String? {
        guard let value else { return nil }
        switch value {
        case .string(let string):
            return actionText(from: string, key: key)
        case .object(let object):
            for (nestedKey, nestedValue) in object {
                if let action = naturalAction(in: nestedValue, key: nestedKey) {
                    return action
                }
            }
        case .array(let values):
            for value in values {
                if let action = naturalAction(in: value, key: key) {
                    return action
                }
            }
        case .null, .bool, .number:
            break
        }
        return nil
    }

    private func actionText(from value: String, key: String) -> String? {
        let lines = value.split(whereSeparator: \.isNewline).map(String.init)
        let candidates: [String]
        if key == "script" || key == "code" || value.contains("await ") {
            candidates = lines.compactMap { line in
                let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
                if trimmed.hasPrefix("//") { return String(trimmed.dropFirst(2)).trimmingCharacters(in: .whitespaces) }
                if trimmed.hasPrefix("#") { return String(trimmed.dropFirst()).trimmingCharacters(in: .whitespaces) }
                return nil
            }
        } else {
            candidates = [value.trimmingCharacters(in: .whitespacesAndNewlines)]
        }

        return candidates.first { candidate in
            !candidate.isEmpty
                && candidate.count <= 120
                && !candidate.contains("mcp.")
                && !candidate.contains("=>")
                && !candidate.contains("{")
                && !candidate.contains("}")
        }
    }

    private func humanizedMCPName(_ title: String) -> String {
        let name = title
            .replacingOccurrences(of: "mcp.", with: "")
            .replacingOccurrences(of: ".js", with: "")
            .replacingOccurrences(of: "_", with: " ")
            .replacingOccurrences(of: ".", with: " ")
        return name.prefix(1).uppercased() + name.dropFirst()
    }

    private var icon: String {
        switch toolCall.status {
        case .pending: return "clock"
        case .inProgress: return "gearshape.2"
        case .completed: return "checkmark.circle"
        case .failed: return "xmark.circle"
        case .cancelled: return "slash.circle"
        }
    }

    private var color: Color {
        switch toolCall.status {
        case .pending: return .secondary
        case .inProgress: return .yellow
        case .completed: return .green
        case .failed: return .red
        case .cancelled: return .secondary
        }
    }

    private func locationLabel(_ location: ToolLocation) -> String {
        if let line = location.line {
            return "\(location.path):\(line)"
        }
        return location.path
    }
}

/// Renders one piece of rich tool-call output: a content block, a file diff, or
/// a terminal reference. Diffs are shown as text; nothing is applied to disk.
struct ToolCallContentView: View {
    let content: ToolCallContent

    var body: some View {
        switch content {
        case .content(let block):
            if block.kind == .text || block.kind == .toolResult || block.kind == .code {
                Text(block.text)
                    .font(.system(.caption, design: .monospaced))
                    .lineLimit(8)
                    .textSelection(.enabled)
                    .padding(6)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color.primary.opacity(0.05))
                    .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
            } else {
                Label(block.title ?? block.name ?? block.mimeType ?? "content", systemImage: "paperclip")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        case .diff(let diff):
            VStack(alignment: .leading, spacing: 2) {
                Text(diff.path)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(.secondary)
                Text(diff.newText)
                    .font(.system(.caption2, design: .monospaced))
                    .lineLimit(10)
                    .textSelection(.enabled)
            }
            .padding(6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.primary.opacity(0.05))
            .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        case .terminal(let terminal):
            Label("Terminal \(terminal.terminalId)", systemImage: "terminal")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }
}
