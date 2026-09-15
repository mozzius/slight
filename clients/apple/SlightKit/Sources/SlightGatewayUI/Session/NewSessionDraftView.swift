import Foundation
import SlightGateway
import SwiftUI

struct NewSessionDraftView: View {
    @ObservedObject var list: SessionListViewModel
    let onCreated: (SessionSummary) -> Void

    @State private var agent = ""
    @State private var model = "Loading model..."
    @State private var effort = "Loading effort..."
    @State private var workingDirectory = "~"
    @State private var showingWorkspacePicker = false
    @State private var message = ""
    @State private var isCreating = false

    var body: some View {
        GeometryReader { _ in
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Session setup")
                            .font(.headline)

                        Picker("Harness", selection: $agent) {
                            ForEach(list.agentCatalog) { entry in
                                Text(entry.displayName).tag(entry.id)
                            }
                        }

                        Button {
                            showingWorkspacePicker = true
                        } label: {
                            HStack {
                                Label("Workspace", systemImage: "folder")
                                Spacer()
                                Text(workspaceName)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                                Image(systemName: "chevron.up.chevron.down")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                    .frame(maxWidth: 800, alignment: .leading)
                    .frame(maxWidth: .infinity)

                    if let error = list.lastError {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
                    }
                    if list.agentCatalog.isEmpty && list.connectionState.isConnected {
                        ProgressView("Loading harness configuration...")
                    }
                }
                .padding(24)
                .padding(.bottom, 150)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .modifier(BottomScrollEdgeEffect())
            .safeAreaInset(edge: .bottom, spacing: 0) {
                VStack(spacing: 0) {
                    ComposerView(
                        message: $message,
                        model: $model,
                        effort: $effort,
                        models: modelOptions,
                        efforts: effortOptions,
                        isEnabled: canCreate,
                        disabledReason: disabledReason,
                        onSend: createSession
                    )
                    if let disabledReason {
                        Text(disabledReason)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: 800)
                .frame(maxWidth: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .navigationTitle("New Session")
        #if os(iOS)
        .navigationBarTitleDisplayMode(.inline)
        #endif
        .overlay {
            if isCreating { ProgressView() }
        }
        .onAppear(perform: applyCatalogDefaults)
        .onChange(of: list.agentCatalog) { _, _ in applyCatalogDefaults() }
        .onChange(of: agent) { _, _ in applyCatalogDefaults() }
        .sheet(isPresented: $showingWorkspacePicker) {
            WorkspacePickerSheet(
                paths: list.recentWorkingDirectories,
                selection: workingDirectory,
                onSelect: { path in
                    workingDirectory = path
                    showingWorkspacePicker = false
                },
                onCancel: { showingWorkspacePicker = false }
            )
        }
    }

    private var canCreate: Bool {
        list.connectionState.isConnected
            && !isCreating
            && !agent.isEmpty
            && (modelOptions.isEmpty || modelOptions.contains(model))
            && (effortOptions.isEmpty || effortOptions.contains(effort))
            && !message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var workspaceName: String {
        let path = workingDirectory.trimmingCharacters(in: .whitespacesAndNewlines)
        return path.split(separator: "/").last.map(String.init) ?? path
    }

    private var selectedCatalog: AgentCatalogEntry? {
        list.agentCatalog.first { $0.id == agent }
    }

    private var modelOptions: [String] {
        selectedCatalog?.configOptions
            .filter { $0.category == .model }
            .flatMap { $0.kind.choices.map(\.valueId) } ?? []
    }

    private var effortOptions: [String] {
        selectedCatalog?.configOptions
            .filter { $0.category == .thoughtLevel }
            .flatMap { $0.kind.choices.map(\.valueId) } ?? []
    }

    private func applyCatalogDefaults() {
        guard let first = list.agentCatalog.first else { return }
        if !list.agentCatalog.contains(where: { $0.id == agent }) { agent = first.id }
        guard let entry = list.agentCatalog.first(where: { $0.id == agent }) else { return }
        let models = entry.configOptions.filter { $0.category == .model }
        let efforts = entry.configOptions.filter { $0.category == .thoughtLevel }
        if let option = models.first {
            model = option.kind.currentValueId ?? option.kind.choices.first?.valueId ?? "Loading model..."
        }
        if let option = efforts.first {
            effort = option.kind.currentValueId ?? option.kind.choices.first?.valueId ?? "Loading effort..."
        }
    }

    private var disabledReason: String? {
        if !list.connectionState.isConnected { return "Connect to a host to start a session." }
        return nil
    }

    private func createSession() {
        guard canCreate else { return }
        isCreating = true
        Task {
            let summary = await list.createSession(
                title: titleForMessage(message),
                agent: agent,
                model: modelOptions.contains(model) ? model : nil,
                effort: effortOptions.contains(effort) ? effort : nil,
                workingDirectory: workingDirectory.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "~"
                    : workingDirectory,
                initialPrompt: message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
            isCreating = false
            if let summary { onCreated(summary) }
        }
    }

    private func titleForMessage(_ message: String) -> String {
        let normalized = message
            .split(whereSeparator: { $0.isWhitespace || $0.isNewline })
            .joined(separator: " ")
        guard normalized.count > 64 else { return normalized }
        return String(normalized.prefix(61)) + "..."
    }
}
