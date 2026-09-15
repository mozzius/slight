#if os(macOS)
import SlightGateway
import SwiftUI

/// macOS-only host administration surface. Uses the gateway host-management
/// API for status, lifecycle, pairing/revocation, diagnostics, configuration,
/// and session inspection.
public struct HostAdminView: View {
    @ObservedObject private var model: SlightAppModel
    @StateObject private var viewModel: HostAdminViewModel

    public init(model: SlightAppModel) {
        self.model = model
        _viewModel = StateObject(wrappedValue: HostAdminViewModel(service: model.adminService))
    }

    public var body: some View {
        VStack(spacing: 0) {
            ConnectionStatusBar(
                state: model.connectionState,
                endpoint: model.hostProfile?.endpoint.absoluteString ?? "Not configured",
                errorMessage: model.connectionErrorMessage,
                onConnect: { model.connect() },
                onDisconnect: { model.disconnect() }
            )
            if let error = viewModel.error {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 4)
            }
            TabView {
                statusTab
                    .tabItem { Label("Status", systemImage: "server.rack") }
                devicesTab
                    .tabItem { Label("Pairing", systemImage: "person.badge.key") }
                sessionsTab
                    .tabItem { Label("Sessions", systemImage: "rectangle.stack") }
                diagnosticsTab
                    .tabItem { Label("Diagnostics", systemImage: "stethoscope") }
                configurationTab
                    .tabItem { Label("Configuration", systemImage: "gearshape") }
            }
            .padding(12)
        }
        .toolbar {
            ToolbarItem {
                Button {
                    Task { await viewModel.refreshAll() }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(!model.connectionState.isConnected)
            }
        }
        .task { await viewModel.refreshAll() }
    }

    private var statusTab: some View {
        Form {
            Section("Host") {
                LabeledContent("State", value: viewModel.status?.state.rawValue.capitalized ?? "—")
                LabeledContent("Name", value: viewModel.status?.hostName ?? "—")
                LabeledContent("Version", value: viewModel.status?.serverVersion ?? "—")
                LabeledContent("Server ID", value: viewModel.status?.serverId ?? "—")
                LabeledContent("Listener", value: viewModel.status?.listener ?? "—")
                LabeledContent("Sessions", value: "\(viewModel.status?.sessionCount ?? 0)")
                LabeledContent("Active sessions", value: "\(viewModel.status?.activeSessionCount ?? 0)")
                LabeledContent("Paired devices", value: "\(viewModel.status?.pairedDeviceCount ?? 0)")
                if let uptime = viewModel.status?.uptimeMs {
                    LabeledContent("Uptime", value: Self.format(uptimeMs: uptime))
                }
                if let agents = viewModel.status?.supportedAgents, !agents.isEmpty {
                    LabeledContent("Agents", value: agents.joined(separator: ", "))
                }
            }
            Section("Lifecycle") {
                HStack {
                    Button("Start") { Task { await viewModel.startHost() } }
                    Button("Stop") { Task { await viewModel.stopHost() } }
                    Button("Restart") { Task { await viewModel.restartHost() } }
                }
                Text("Lifecycle commands are sent to the Rust host service; this app does not supervise processes.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }

    private var devicesTab: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Button("Create Pairing Code") {
                    Task { await viewModel.createPairing(deviceName: ProcessInfo.processInfo.hostName) }
                }
                Button("Refresh Devices") {
                    Task { await viewModel.loadDevices() }
                }
            }
            if let pairing = viewModel.pairing {
                GroupBox("Pairing Code") {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(pairing.code)
                            .font(.system(.title2, design: .monospaced))
                            .textSelection(.enabled)
                        Text("Label: \(pairing.label)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text("Expires \(pairing.expiresAt, style: .relative)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text("Enter this one-time code on the client device. It cannot be reused.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            Table(viewModel.devices) {
                TableColumn("Device") { device in Text(device.label) }
                TableColumn("Created") { device in Text(device.createdAt, style: .date) }
                TableColumn("Last seen") { device in Text(device.lastSeenAt, style: .date) }
                TableColumn("Status") { device in
                    Text(device.revoked ? "Revoked" : "Active")
                }
                TableColumn("") { device in
                    Button("Revoke") { Task { await viewModel.revoke(device) } }
                        .disabled(device.revoked)
                }
            }
            .frame(minHeight: 200)
        }
    }

    private var sessionsTab: some View {
        HSplitView {
            List(viewModel.sessions, selection: $viewModel.selectedSessionId) { session in
                VStack(alignment: .leading, spacing: 2) {
                    Text(session.title).font(.headline)
                    Text("\(session.agent) · \(session.status.rawValue)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .tag(session.id)
            }
            .onChange(of: viewModel.selectedSessionId) { _, newValue in
                guard let newValue else { return }
                Task { await viewModel.inspect(sessionId: newValue) }
            }
            sessionInspector
                .frame(minWidth: 320)
        }
    }

    @ViewBuilder
    private var sessionInspector: some View {
        if let inspected = viewModel.inspectedSession {
            List {
                Section("Summary") {
                    LabeledContent("ID", value: inspected.session.id)
                    LabeledContent("Agent", value: inspected.agent.displayName)
                    LabeledContent("Directory", value: inspected.session.workingDirectoryLabel)
                    LabeledContent("Status", value: inspected.session.status.rawValue)
                    LabeledContent("Running", value: inspected.running ? "Yes" : "No")
                    LabeledContent("Last sequence", value: "\(inspected.session.lastSequence)")
                }
                if let pending = inspected.pendingPermission {
                    Section("Pending permission") {
                        Text(pending.title).font(.callout)
                        if let detail = pending.detail {
                            Text(detail).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
                Section("Recent events (\(inspected.recentEvents.count))") {
                    ForEach(Array(inspected.recentEvents.enumerated()), id: \.offset) { _, event in
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(event.event) · #\(event.sequence)")
                                .font(.caption.weight(.semibold))
                            if let payload = event.payload {
                                Text(payload.prettyDescription)
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.secondary)
                                    .lineLimit(4)
                            }
                        }
                    }
                }
            }
        } else {
            ContentUnavailableView("Select a session", systemImage: "rectangle.stack")
        }
    }

    private var diagnosticsTab: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Button("Load Diagnostics") { Task { await viewModel.loadDiagnostics() } }
                if let diagnostics = viewModel.diagnostics {
                    Text("Host \(diagnostics.serverVersion) · protocol v\(diagnostics.protocolVersion) · log \(diagnostics.logLevel)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            Table(viewModel.diagnostics?.recentLogs ?? []) {
                TableColumn("Time") { entry in Text(entry.timestamp, style: .time) }
                TableColumn("Level") { entry in Text(entry.level.uppercased()) }
                TableColumn("Message") { entry in Text(entry.message) }
            }
            .frame(minHeight: 160)
            Text("Session diagnostics")
                .font(.caption.weight(.semibold))
            Table(viewModel.diagnostics?.sessions ?? []) {
                TableColumn("Session") { item in Text(item.sessionId) }
                TableColumn("Status") { item in Text(item.status) }
                TableColumn("Agent") { item in Text(item.agent) }
                TableColumn("Events") { item in Text("\(item.eventCount)") }
                TableColumn("Running") { item in Text(item.running ? "Yes" : "No") }
            }
            .frame(minHeight: 140)
        }
    }

    private var configurationTab: some View {
        Form {
            if let configuration = viewModel.configuration {
                Section("Listener") {
                    LabeledContent("Address", value: configuration.listenerAddress)
                    LabeledContent("Transport", value: configuration.transport)
                    LabeledContent("Local access allowed", value: configuration.allowLoopback ? "Yes" : "No")
                    LabeledContent("Pairing required", value: configuration.pairingRequired ? "Yes" : "No")
                }
                Section("Storage") {
                    LabeledContent("Data directory", value: configuration.dataDirectory ?? "—")
                    LabeledContent("Journal limit", value: configuration.maxJournalEvents.map(String.init) ?? "—")
                    LabeledContent("Log level", value: configuration.logLevel)
                }
            } else {
                Text("No configuration loaded.")
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }

    private static func format(uptimeMs: Int) -> String {
        let formatter = DateComponentsFormatter()
        formatter.allowedUnits = [.day, .hour, .minute]
        formatter.unitsStyle = .abbreviated
        return formatter.string(from: Double(uptimeMs) / 1000) ?? "\(uptimeMs / 1000)s"
    }
}
#endif
