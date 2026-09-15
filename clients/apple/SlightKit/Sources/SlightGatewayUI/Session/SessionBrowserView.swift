import SlightGateway
import OSLog
import SwiftUI

#if DEBUG
private let sidebarLogger = Logger(subsystem: "sh.slight.client", category: "session.sidebar")
#endif

private struct SidebarFirstRowMinYKey: PreferenceKey {
    static let defaultValue: CGFloat = .greatestFiniteMagnitude

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

private struct WorkspaceSessionGroup: Identifiable {
    let path: String
    let name: String
    let sessions: [SessionSummary]

    var id: String { path }
}

private func workspaceDisplayName(for path: String) -> String {
    if path == "~" { return "Home" }
    if path == "/" { return "Root" }
    return path
        .split(separator: "/", omittingEmptySubsequences: true)
        .last
        .map(String.init) ?? path
}

public struct SessionBrowserView: View {
    @ObservedObject private var model: SlightAppModel
    @ObservedObject private var list: SessionListViewModel
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    #endif

    @State private var selectedId: String?
    @State private var searchText = ""
    @State private var showingNewSession = false
    @State private var showingImport = false
    @State private var showingHostSettings = false
    @State private var showingArchived = false

    public init(model: SlightAppModel) {
        self.model = model
        self.list = model.sessionList
    }

    public var body: some View {
        content
            .sheet(isPresented: $showingImport) {
                ImportSessionSheet(list: list) { summary in
                    showingImport = false
                    selectedId = summary.id
                }
            }
            .sheet(isPresented: $showingHostSettings) {
                HostConnectionSheet(model: model)
            }
    }

    private func showHostSettings() {
        showingHostSettings = true
    }

    @ViewBuilder
    private var content: some View {
        #if os(iOS)
        if horizontalSizeClass == .regular {
            splitContent
        } else {
            NavigationStack {
                compactContent
            }
        }
        #else
        splitContent
        #endif
    }

    private var splitContent: some View {
        NavigationSplitView {
            sidebar
                #if os(macOS)
                .frame(minWidth: 260, idealWidth: 300)
                #endif
        } detail: {
            detail
                #if os(macOS)
                .frame(minWidth: 360, maxWidth: .infinity, maxHeight: .infinity)
                #endif
        }
    }

    #if os(iOS)
    private var compactContent: some View {
        List(selection: $selectedId) {
            if shouldShowConnectionIssue {
                Section {
                    ConnectionIssueView(
                        state: list.connectionState,
                        endpoint: model.hostProfile?.endpoint.absoluteString ?? "Not configured",
                        errorMessage: model.connectionErrorMessage,
                        onRetry: model.connect,
                        onSettings: showHostSettings
                    )
                }
            }
            ForEach(workspaceGroups) { group in
                Section(group.name) {
                    ForEach(group.sessions) { summary in
                        SessionRow(summary: summary)
                            .tag(summary.id)
                            .swipeActions(edge: .trailing) {
                                archiveButton(for: summary)
                            }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .refreshable { await list.refresh() }
        .overlay {
            if visibleSessions.isEmpty && !shouldShowConnectionIssue {
                emptyState
            }
        }
        .searchable(text: $searchText, placement: .navigationBarDrawer(displayMode: .always))
        .navigationDestination(item: $selectedId) { id in
            if let summary = list.sessions.first(where: { $0.id == id }) {
                SessionDetailContainer(session: summary, model: model)
                    .id(summary.id)
            } else {
                ContentUnavailableView("Session unavailable", systemImage: "questionmark.circle")
            }
        }
        .navigationDestination(isPresented: $showingNewSession) {
            NewSessionDraftView(
                list: list,
                onCreated: { summary in
                    showingNewSession = false
                    selectedId = summary.id
                }
            )
        }
        .navigationTitle("Sessions")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Menu {
                    Button {
                        showingNewSession = true
                    } label: {
                        Label("New Session", systemImage: "plus")
                    }

                    Button {
                        showingImport = true
                    } label: {
                        Label("Resume Session", systemImage: "arrow.clockwise")
                    }

                    Toggle(isOn: $showingArchived) {
                        Label("Show Archived", systemImage: "archivebox")
                    }
                } label: {
                    Image(systemName: "plus")
                }
                .disabled(!list.connectionState.isConnected)
                .accessibilityLabel("Session actions")
            }
            ToolbarItem(placement: .primaryAction) {
                Button(action: showHostSettings) {
                    Image(systemName: "server.rack")
                }
                .accessibilityLabel("Host Settings")
            }
        }
    }
    #endif

    private var sidebar: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Text("Sessions")
                    .font(.headline)
                Spacer()
                Menu {
                    Button {
                        showingNewSession = true
                    } label: {
                        Label("New Session", systemImage: "plus")
                    }

                    Button {
                        showingImport = true
                    } label: {
                        Label("Resume Session", systemImage: "arrow.clockwise")
                    }

                    Toggle(isOn: $showingArchived) {
                        Label("Show Archived", systemImage: "archivebox")
                    }
                } label: {
                    Image(systemName: "plus")
                }
                .buttonStyle(.borderless)
                .disabled(!list.connectionState.isConnected)
                .help("Session actions")
                .accessibilityLabel("Session actions")

                Button(action: showHostSettings) {
                    Label("Host Settings", systemImage: "server.rack")
                }
                .labelStyle(.iconOnly)
                .buttonStyle(.borderless)
                .help("Host Settings")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)

            Divider()

            if shouldShowConnectionIssue {
                ConnectionIssueView(
                    state: list.connectionState,
                    endpoint: model.hostProfile?.endpoint.absoluteString ?? "Not configured",
                    errorMessage: model.connectionErrorMessage,
                        onRetry: model.connect,
                    onSettings: showHostSettings
                )
                .padding(12)
            }

            List(selection: $selectedId) {
                ForEach(workspaceGroups) { group in
                    Section(group.name) {
                        ForEach(group.sessions) { summary in
                            SessionRow(summary: summary)
                                .background {
                                    if summary.id == filteredSessions.first?.id {
                                        GeometryReader { geometry in
                                            Color.clear.preference(
                                                key: SidebarFirstRowMinYKey.self,
                                                value: geometry.frame(in: .named("sidebarList")).minY
                                            )
                                        }
                                    }
                                }
                                .tag(summary.id)
                                .contextMenu {
                                    archiveButton(for: summary)
                                }
                        }
                    }
                }
            }
            .searchable(text: $searchText, placement: .sidebar)
            .defaultScrollAnchor(.top)
            .coordinateSpace(name: "sidebarList")
            .onAppear {
                logSidebarState("appeared")
            }
            .onChange(of: list.connectionState) { _, _ in
                logSidebarState("connection state changed")
            }
            .onChange(of: list.sessions.count) { _, _ in
                logSidebarState("session count changed")
            }
            .onChange(of: searchText) { _, _ in
                logSidebarState("search changed")
            }
            .onPreferenceChange(SidebarFirstRowMinYKey.self) { minY in
                #if DEBUG
                sidebarLogger.debug("first row minY=\(minY, privacy: .public)")
                #endif
            }
            .overlay {
                if visibleSessions.isEmpty && !shouldShowConnectionIssue {
                    emptyState
                }
            }
            #if os(macOS)
            .padding(.top, 8)
            #endif
        }
    }

    private func logSidebarState(_ event: String) {
        #if DEBUG
        sidebarLogger.debug(
            "\(event, privacy: .public) state=\(String(describing: list.connectionState), privacy: .public) sessions=\(list.sessions.count, privacy: .public) filtered=\(filteredSessions.count, privacy: .public) issue=\(shouldShowConnectionIssue, privacy: .public)"
        )
        #endif
    }

    private var shouldShowConnectionIssue: Bool {
        if model.connectionErrorMessage != nil { return true }
        switch list.connectionState {
        case .closed, .failed:
            return true
        case .idle, .connected, .connecting, .handshaking, .reconnecting, .replaying, .resyncRequired:
            return false
        }
    }

    private var filteredSessions: [SessionSummary] {
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        let sessions = list.sessions.filter { showingArchived || !$0.archived }
        guard !query.isEmpty else { return sessions }
        return sessions.filter { session in
            [session.title, session.agent, session.model ?? "", session.workingDirectoryLabel]
                .contains { $0.localizedCaseInsensitiveContains(query) }
        }
    }

    private var visibleSessions: [SessionSummary] {
        filteredSessions
    }

    private var workspaceGroups: [WorkspaceSessionGroup] {
        var groups: [WorkspaceSessionGroup] = []
        var indexes: [String: Int] = [:]

        for session in filteredSessions {
            let path = session.workingDirectoryLabel
            if let index = indexes[path] {
                groups[index] = WorkspaceSessionGroup(
                    path: path,
                    name: groups[index].name,
                    sessions: groups[index].sessions + [session]
                )
            } else {
                indexes[path] = groups.count
                groups.append(
                    WorkspaceSessionGroup(
                        path: path,
                        name: workspaceDisplayName(for: path),
                        sessions: [session]
                    )
                )
            }
        }
        return groups
    }

    @ViewBuilder
    private func archiveButton(for summary: SessionSummary) -> some View {
        Button {
            Task { await list.setArchived(!summary.archived, for: summary) }
        } label: {
            Label(summary.archived ? "Unarchive" : "Archive", systemImage: summary.archived ? "tray.and.arrow.up" : "archivebox")
        }
        .tint(summary.archived ? .blue : .orange)
    }

    @ViewBuilder
    private var emptyState: some View {
        if list.connectionState.isConnected {
            ContentUnavailableView(
                "No sessions",
                systemImage: "rectangle.stack",
                description: Text("Create a session to get started.")
            )
        } else {
            ContentUnavailableView {
                Label("Connecting to host", systemImage: "bolt.horizontal.circle")
            } description: {
                Text("Waiting for the host connection.")
            }
        }
    }

    @ViewBuilder
    private var detail: some View {
        if showingNewSession {
            NewSessionDraftView(
                list: list,
                onCreated: { summary in
                    showingNewSession = false
                    selectedId = summary.id
                }
            )
        } else if let id = selectedId, let summary = list.sessions.first(where: { $0.id == id }) {
            SessionDetailContainer(session: summary, model: model)
                .id(summary.id)
        } else {
            ContentUnavailableView(
                "Select a session",
                systemImage: "rectangle.stack"
            )
        }
    }
}

public struct SessionDetailContainer: View {
    @StateObject private var viewModel: SessionViewModel

    public init(session: SessionSummary, model: SlightAppModel) {
        _viewModel = StateObject(wrappedValue: model.makeSessionViewModel(summary: session))
    }

    public var body: some View {
        SessionDetailView(viewModel: viewModel)
    }
}

struct SessionRow: View {
    let summary: SessionSummary

    var body: some View {
        HStack(spacing: 11) {
            Image(systemName: statusIcon)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(statusColor)
                .frame(width: 34, height: 34)
                .background(statusColor.opacity(0.13), in: RoundedRectangle(cornerRadius: 10))

            VStack(alignment: .leading, spacing: 5) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(summary.title)
                        .font(.subheadline.weight(.semibold))
                        .lineLimit(1)
                    Spacer(minLength: 4)
                    SessionRecoveryBadge(recovery: summary.recovery)
                    StatusBadge(status: summary.status)
                }

                HStack(spacing: 5) {
                    Text(summary.agent)
                    if let model = summary.model {
                        Text("·")
                        Text(model)
                    }
                    if let effort = summary.effort {
                        Text("·")
                        Text("\(effort) effort")
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)

                Label(workspaceDisplayName(for: summary.workingDirectoryLabel), systemImage: "folder")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(summary.title), \(statusLabel), \(summary.agent)\(recoverySuffix)")
    }

    private var recoverySuffix: String {
        switch summary.recovery {
        case .live, .recovered:
            return ""
        case .stale:
            return ", native session stale"
        case .unavailable:
            return ", native session unavailable"
        }
    }

    private var statusLabel: String {
        switch summary.status {
        case .idle: return "Idle"
        case .working: return "Working"
        case .waitingPermission: return "Waiting for permission"
        case .exited: return "Exited"
        case .failed: return "Failed"
        }
    }

    private var statusIcon: String {
        switch summary.status {
        case .idle: return "pause.fill"
        case .working: return "bolt.fill"
        case .waitingPermission: return "hand.raised.fill"
        case .exited: return "checkmark"
        case .failed: return "exclamationmark.triangle.fill"
        }
    }

    private var statusColor: Color {
        switch summary.status {
        case .idle: return .secondary
        case .working: return Color(red: 0.12, green: 0.48, blue: 0.29)
        case .waitingPermission: return .orange
        case .exited: return .gray
        case .failed: return .red
        }
    }
}

struct StatusBadge: View {
    let status: SessionStatus

    var body: some View {
        Text(label)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.18))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }

    private var label: String {
        switch status {
        case .idle: return "Idle"
        case .working: return "Working"
        case .waitingPermission: return "Permission"
        case .exited: return "Exited"
        case .failed: return "Failed"
        }
    }

    private var color: Color {
        switch status {
        case .idle: return .secondary
        case .working: return Color(red: 0.12, green: 0.48, blue: 0.29)
        case .waitingPermission: return .orange
        case .exited: return .gray
        case .failed: return .red
        }
    }
}
