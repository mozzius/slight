import SlightGateway
import SwiftUI

/// Shared host endpoint editor and connection controls. Used by the macOS
/// Shared host configuration and connection controls. Presented as a sheet on
/// both macOS and iOS.
public struct HostConnectionSettingsView: View {
    @ObservedObject private var model: SlightAppModel
    @Environment(\.dismiss) private var dismiss
    @State private var mode: HostConnectionMode
    @State private var host: String
    @State private var port: String
    @State private var name: String
    #if os(macOS)
    @State private var harnesses: HarnessConfiguration
    #endif
    @State private var error: String?
    #if os(macOS)
    @State private var isHarnessSetupExpanded = false
    #endif
    @State private var isShowingRemoveConfirmation = false

    public init(model: SlightAppModel) {
        self.model = model
        let profile = model.hostProfile
        let isLocal = profile?.isLoopback == true
        #if os(macOS)
        _mode = State(initialValue: isLocal ? .local : .remote)
        #else
        _mode = State(initialValue: .remote)
        #endif
        _host = State(initialValue: isLocal ? "127.0.0.1" : profile?.endpoint.host ?? "")
        _port = State(initialValue: String(profile?.endpoint.port ?? 8787))
        _name = State(initialValue: isLocal ? "" : profile?.displayName ?? "")
        #if os(macOS)
        _harnesses = State(initialValue: HarnessConfigurationStore.load())
        #endif
    }

    public var body: some View {
        Form {
            Section("Host") {
                #if os(macOS)
                Picker("Host type", selection: $mode) {
                    Text("Local").tag(HostConnectionMode.local)
                    Text("Remote").tag(HostConnectionMode.remote)
                }
                .pickerStyle(.segmented)

                if mode == .local {
                    Text("Use the host running on this Mac.")
                        .foregroundStyle(.secondary)
                } else {
                    hostFields
                }
                #else
                hostFields
                #endif

                if let error {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                }

                Button(action: save) {
                    Label(
                        model.isApplyingHostProfile ? "Saving & Connecting…" : "Save & Connect",
                        systemImage: "externaldrive.badge.wifi"
                    )
                }
                    .buttonStyle(.borderedProminent)
                    .foregroundStyle(.white)
                    .disabled(model.isApplyingHostProfile)
            }

            HostConnectionStatusView(
                state: model.connectionState,
                endpoint: model.hostProfile?.endpoint.absoluteString ?? "Not configured",
                errorMessage: model.connectionErrorMessage,
                isApplyingHostProfile: model.isApplyingHostProfile,
                isStartingLocalHost: model.localHostService.isStarting,
                onConnect: model.connect,
                onDisconnect: model.disconnect
            )

            #if os(macOS)
            DisclosureGroup("Harness setup", isExpanded: $isHarnessSetupExpanded) {
                Text("Claude Code and Codex are bundled with Slight. OpenCode is optional and can be configured here.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                harnessField("OpenCode", text: $harnesses.opencode.program, placeholder: HarnessConfigurationStore.detectedOpencodePath ?? "Not detected")
            }
            #endif

            Section {
                Button("Remove host", systemImage: "trash", role: .destructive) {
                    isShowingRemoveConfirmation = true
                }
                .confirmationDialog(
                    "Remove this host?",
                    isPresented: $isShowingRemoveConfirmation,
                    titleVisibility: .visible
                ) {
                    Button("Remove host", role: .destructive) {
                        model.removeHostProfile()
                        closeHostConfiguration()
                    }
                    Button("Cancel", role: .cancel) {}
                } message: {
                    Text("Slight will disconnect and return to host setup.")
                }
            } footer: {
                Text("Remove the saved host from this device. You can configure it again later.")
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Host")
        .onChange(of: mode) { _, newMode in
            if newMode == .local { host = "127.0.0.1" }
            if newMode == .remote, host == "127.0.0.1" { host = "" }
            error = nil
        }
    }

    private func closeHostConfiguration() {
        dismiss()
    }

    private func save() {
        #if os(macOS)
        if mode == .local {
            guard HarnessConfigurationStore.validate(harnesses) == nil else {
                error = HarnessConfigurationStore.validate(harnesses)
                return
            }
            do {
                try HarnessConfigurationStore.save(harnesses)
            } catch {
                self.error = "Could not save harness configuration: \(error.localizedDescription)"
                return
            }
            error = nil
            model.updateHostProfile(.loopback())
            return
        }
        #endif

        guard let port = Int(port), (1...65_535).contains(port),
              !host.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            error = "Enter a host and a port from 1 to 65535."
            return
        }
        let trimmedHost = host.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let endpoint = URL(string: "ws://\(trimmedHost):\(port)/gateway") else {
            error = "Enter a valid host."
            return
        }
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        error = nil
        let existingRemoteID = model.hostProfile?.isLoopback == true ? nil : model.hostProfile?.id
        model.updateHostProfile(HostProfile(
            id: existingRemoteID ?? "configured",
            displayName: trimmedName.isEmpty ? trimmedHost : trimmedName,
            endpoint: endpoint,
            isLoopback: HostEndpoint.isLoopback(endpoint)
        ))
    }

    @ViewBuilder
    private var hostFields: some View {
        TextField("Host", text: $host, prompt: Text("127.0.0.1 or host name"))
        .autocorrectionDisabled()
        #if os(iOS)
        .keyboardType(.URL)
        .textInputAutocapitalization(.never)
        #endif
        TextField("Port", text: $port, prompt: Text("8787"))
            #if os(iOS)
            .keyboardType(.numberPad)
            #endif
        TextField("Name (optional)", text: $name)
    }

    #if os(macOS)
    private func harnessField(_ title: String, text: Binding<String?>, placeholder: String) -> some View {
        TextField(title, text: Binding(
            get: { text.wrappedValue ?? "" },
            set: { text.wrappedValue = $0.isEmpty ? nil : $0 }
        ), prompt: Text(placeholder))
        .textFieldStyle(.roundedBorder)
    }
    #endif
}

/// Presentation wrapper: a dismissable sheet around the shared settings form.
public struct HostConnectionSheet: View {
    @ObservedObject private var model: SlightAppModel
    @Environment(\.dismiss) private var dismiss

    public init(model: SlightAppModel) {
        self.model = model
    }

    public var body: some View {
        NavigationStack {
            HostConnectionSettingsView(model: model)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done", action: dismiss.callAsFunction)
                    }
                }
        }
    }
}
