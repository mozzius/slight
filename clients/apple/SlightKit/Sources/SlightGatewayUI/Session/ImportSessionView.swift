import Foundation
import SlightGateway
import SwiftUI

/// Sheet that discovers native sessions owned by an agent and imports one into
/// Slight. Discovery is scoped by agent and working directory; the discovered
/// `cwd` is authoritative and is sent back on import.
struct ImportSessionSheet: View {
    @ObservedObject var list: SessionListViewModel
    let onImported: (SessionSummary) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var agent = ""
    @State private var workingDirectory = "~"
    @State private var discoveryTask: Task<Void, Never>?

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                content

                #if os(macOS)
                Divider()
                HStack {
                    Spacer()
                    Button("Cancel", role: .cancel) { dismiss() }
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 12)
                #endif
            }
            .navigationTitle("Resume Session")
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", role: .cancel) { dismiss() }
                }
            }
            #endif
        }
        #if os(macOS)
        .frame(
            minWidth: 620,
            idealWidth: 760,
            maxWidth: 900,
            minHeight: 420,
            idealHeight: 640,
            maxHeight: 760
        )
        #endif
        .onAppear {
            applyDefaultAgent()
            Task { await discover() }
        }
        .onChange(of: list.agentCatalog) { _, _ in
            applyDefaultAgent()
            Task { await discover() }
        }
        .onChange(of: agent) { _, _ in
            scheduleDiscovery()
        }
        .onChange(of: workingDirectory) { _, _ in
            scheduleDiscovery()
        }
        .onDisappear {
            discoveryTask?.cancel()
            list.clearAgentSessionDiscovery()
        }
    }

    private var content: some View {
        #if os(macOS)
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                sourceSection
                discoverySection

                if let error = list.lastError {
                    Section {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(20)
        }
        #else
        Form {
            sourceSection
            discoverySection

            if let error = list.lastError {
                Section {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
        }
        #endif
    }

    private var sourceSection: some View {
        Section("Source") {
            Picker("Harness", selection: $agent) {
                ForEach(list.agentCatalog) { entry in
                    Text(entry.displayName).tag(entry.id)
                }
            }
            .pickerStyle(.segmented)
            .disabled(list.discoveryState.isLoading)

            if !list.recentWorkingDirectories.isEmpty {
                Menu {
                    ForEach(list.recentWorkingDirectories, id: \.self) { path in
                        Button(path) { workingDirectory = path }
                    }
                } label: {
                    Label("Recent directories", systemImage: "clock.arrow.circlepath")
                }
            }

            TextField("Working directory", text: $workingDirectory)
                .autocorrectionDisabled()
                #if os(iOS)
                .textInputAutocapitalization(.never)
                #endif

        }
    }

    @ViewBuilder
    private var discoverySection: some View {
        switch list.discoveryState {
        case .idle:
            Section {
                Text("Choose a harness to find its sessions.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        case .loading:
            Section {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Asking \(agent) for its sessions…")
                        .foregroundStyle(.secondary)
                }
            }
        case .failed(let message):
            Section {
                Label {
                    Text(message)
                        .fixedSize(horizontal: false, vertical: true)
                } icon: {
                    Image(systemName: "exclamationmark.triangle")
                }
                .font(.caption)
                .foregroundStyle(.orange)
            }
        case .loaded(let sessions):
            if sessions.isEmpty {
                Section {
                    Text("No sessions found for this agent and directory.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            } else {
                Section {
                    ForEach(sessions) { session in
                        AgentSessionRow(
                            session: session,
                            isImporting: list.importingAgentSessionId == session.id,
                            onImport: { Task { await importSession(session) } }
                        )
                    }
                    if list.discoveryNextCursor != nil {
                        Button("Load more sessions") {
                            Task { await list.loadMoreAgentSessions(agent: agent, workingDirectory: workingDirectory) }
                        }
                        .disabled(list.discoveryState.isLoading)
                    }
                }
            }
        }
    }

    private var canDiscover: Bool {
        list.connectionState.isConnected && !agent.isEmpty && !list.discoveryState.isLoading
    }

    private func discover() async {
        guard canDiscover else { return }
        await list.discoverAgentSessions(agent: agent, workingDirectory: workingDirectory)
    }

    private func scheduleDiscovery() {
        discoveryTask?.cancel()
        discoveryTask = Task {
            try? await Task.sleep(nanoseconds: 250_000_000)
            guard !Task.isCancelled else { return }
            await discover()
        }
    }

    private func importSession(_ session: AgentSessionSummary) async {
        guard list.importingAgentSessionId == nil else { return }
        if let imported = await list.importAgentSession(session, recovery: .load) {
            onImported(imported)
            dismiss()
        }
    }

    private func applyDefaultAgent() {
        guard let first = list.agentCatalog.first else { return }
        if !list.agentCatalog.contains(where: { $0.id == agent }) {
            agent = first.id
        }
    }
}

private struct AgentSessionRow: View {
    let session: AgentSessionSummary
    let isImporting: Bool
    let onImport: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(session.title ?? "Untitled session")
                    .font(.subheadline.weight(.semibold))
                    .lineLimit(1)
                Spacer(minLength: 4)
                if isImporting {
                    ProgressView().controlSize(.small)
                        .accessibilityLabel("Importing")
                } else {
                    Button("Resume", action: onImport)
                        .buttonStyle(.bordered)
                }
            }

            Label(session.cwd, systemImage: "folder")
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)

            if !session.additionalDirectories.isEmpty {
                Label(session.additionalDirectories.joined(separator: ", "), systemImage: "folder.badge.plus")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            HStack(spacing: 6) {
                Text(session.agent)
                Text("·")
                Text(session.agentSessionId)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if let updatedAt = session.updatedAt {
                    Text("·")
                    Text(updatedAt, format: .relative(presentation: .named))
                }
            }
            .font(.caption2)
            .foregroundStyle(.tertiary)
        }
        .padding(.vertical, 2)
    }
}
