import SlightGateway
import SwiftUI

/// Full-screen first-run surface. It remains visible until a host handshake
/// succeeds, including while the configuration sheet is open.
public struct HostOnboardingSplashView: View {
    @ObservedObject private var model: SlightAppModel
    @State private var isShowingConfiguration = false

    public init(model: SlightAppModel) {
        self.model = model
    }

    public var body: some View {
        ZStack {
            LinearGradient(
                colors: [
                    Color(red: 0.12, green: 0.25, blue: 0.18),
                    Color(red: 0.06, green: 0.15, blue: 0.11),
                    Color(red: 0.02, green: 0.07, blue: 0.05),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            .ignoresSafeArea()

            VStack(spacing: 28) {
                Spacer()
                Image(systemName: "tree.fill")
                    .font(.system(size: 72))
                    .foregroundStyle(.white)
                VStack(spacing: 10) {
                    Text("Welcome to Slight")
                        .font(.largeTitle.bold())
                    Text("Connect to a host to start working with your sessions.")
                        .font(.title3)
                        .multilineTextAlignment(.center)
                        .foregroundStyle(.white.opacity(0.8))
                }
                Spacer()
                Button("Get Started", systemImage: "arrow.right") {
                    isShowingConfiguration = true
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .tint(.white)
                .foregroundStyle(Color(red: 0.06, green: 0.15, blue: 0.11))
                .padding(.bottom, 36)
            }
            .padding(.horizontal, 32)
            .foregroundStyle(.white)
        }
        .sheet(isPresented: $isShowingConfiguration) {
            HostOnboardingConfigurationView(model: model)
        }
        #if os(macOS)
        .toolbar(.hidden, for: .windowToolbar)
        #endif
    }
}

private struct HostOnboardingConfigurationView: View {
    @ObservedObject private var model: SlightAppModel
    @Environment(\.dismiss) private var dismiss
    @State private var mode: HostConnectionMode = .remote
    @State private var host = ""
    @State private var port = "8787"
    @State private var name = ""
    @State private var error: String?
    @State private var hasAttemptedConnection = false
    #if os(macOS)
    @State private var harnesses = HarnessConfigurationStore.load()
    @State private var isHarnessSetupExpanded = false
    #endif

    init(model: SlightAppModel) {
        self.model = model
        #if os(macOS)
        _mode = State(initialValue: .local)
        _host = State(initialValue: "127.0.0.1")
        #endif
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    #if os(macOS)
                    Picker("Host type", selection: $mode) {
                        Text("Local").tag(HostConnectionMode.local)
                        Text("Remote").tag(HostConnectionMode.remote)
                    }
                    .pickerStyle(.segmented)
                    #endif

                    #if os(macOS)
                    if mode == .remote {
                        hostFields
                    } else {
                        Text("Use the host running on this Mac.")
                            .foregroundStyle(.secondary)
                    }
                    #else
                    hostFields
                    #endif
                } header: {
                    Text("Configure host")
                } footer: {
                    Text("Slight connects over WebSocket on port 8787 by default.")
                }

                #if os(macOS)
                if mode == .local {
                    Section("Local host") {
                        Text("Slight can run acp-host for you on this Mac.")
                            .foregroundStyle(.secondary)
                        LabeledContent(
                            "Status",
                            value: model.localHostService.isRunning ? "Running" : "Not running"
                        )
                        Button(
                            model.localHostService.isRunning ? "Local host is running" : "Set up local host",
                            systemImage: model.localHostService.isRunning ? "checkmark.circle.fill" : "play.fill",
                            action: startLocalHost
                        )
                        .disabled(model.localHostService.isRunning)
                        Toggle(
                            "Start Slight at login",
                            isOn: Binding(
                                get: { model.localHostService.startsAtLogin },
                                set: { model.localHostService.setStartsAtLogin($0) }
                            )
                        )
                        if let serviceError = model.localHostService.errorMessage {
                            Text(serviceError)
                                .font(.caption)
                                .foregroundStyle(.red)
                        }
                    }
                    DisclosureGroup("Harness setup", isExpanded: $isHarnessSetupExpanded) {
                        Text("Claude Code and Codex are bundled with Slight. OpenCode is optional and can be configured here.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        harnessField("OpenCode", text: $harnesses.opencode.program, placeholder: HarnessConfigurationStore.detectedOpencodePath ?? "Not detected")
                    }
                }
                #endif

                Section {
                    Button(
                        model.isApplyingHostProfile ? "Connecting…" : "Connect",
                        systemImage: "bolt.horizontal",
                        action: submit
                    )
                    .disabled(model.isApplyingHostProfile || model.connectionState.isConnected || !canSubmit)

                    if hasAttemptedConnection {
                        LabeledContent("Status", value: model.connectionState.shortDescription)
                        if let error = error ?? model.connectionErrorMessage {
                            Text(error)
                                .foregroundStyle(.red)
                        }
                    }
                } header: {
                    Text("Connection")
                }
            }
            .formStyle(.grouped)
            .navigationTitle("Set up Slight")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    #if os(macOS)
                    Button("Cancel", role: .cancel, action: dismiss.callAsFunction)
                        .disabled(model.isApplyingHostProfile)
                    #else
                    if #available(iOS 26.0, *) {
                        Button(role: .cancel) { dismiss() } label: {
                            Image(systemName: "xmark")
                        }
                        .disabled(model.isApplyingHostProfile)
                    } else {
                        Button("Cancel", role: .cancel, action: dismiss.callAsFunction)
                            .disabled(model.isApplyingHostProfile)
                    }
                    #endif
                }
                ToolbarItem(placement: .confirmationAction) {
                    #if os(macOS)
                    Button("Done", action: submit)
                        .disabled(model.isApplyingHostProfile || !model.connectionState.isConnected)
                    #else
                    if #available(iOS 26.0, *) {
                        Button(role: .confirm, action: submit) {
                            Image(systemName: "checkmark")
                        }
                        .disabled(model.isApplyingHostProfile || !model.connectionState.isConnected)
                    } else {
                        Button("Done", action: submit)
                            .disabled(model.isApplyingHostProfile || !model.connectionState.isConnected)
                    }
                    #endif
                }
            }
            .onChange(of: mode) { _, newMode in
                if newMode == .local { host = "127.0.0.1" }
                if newMode == .remote, host == "127.0.0.1" { host = "" }
                error = nil
            }
            .onChange(of: model.connectionState) { _, _ in
                error = nil
            }
        }
        .interactiveDismissDisabled(true)
    }

    private var canSubmit: Bool {
        #if os(macOS)
        if mode == .local { return true }
        #endif
        return !host.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            Int(port) != nil &&
            (1...65_535).contains(Int(port) ?? 0)
    }

    private func submit() {
        hasAttemptedConnection = true
        guard !model.connectionState.isConnected else {
            model.completeOnboarding()
            dismiss()
            return
        }
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

        guard canSubmit, let port = Int(port) else {
            error = "Enter a host and a port from 1 to 65535."
            return
        }
        let trimmedHost = host.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let endpoint = URL(string: "ws://\(trimmedHost):\(port)/gateway") else {
            error = "Enter a valid host."
            return
        }
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let profile = HostProfile(
            id: "configured",
            displayName: trimmedName.isEmpty ? trimmedHost : trimmedName,
            endpoint: endpoint,
            isLoopback: HostEndpoint.isLoopback(endpoint)
        )
        error = nil
        model.updateHostProfile(profile)
    }

    @ViewBuilder
    private var hostFields: some View {
        TextField("Host", text: $host, prompt: Text("127.0.0.1 or host name"))
            .autocorrectionDisabled()
            #if os(iOS)
            .textInputAutocapitalization(.never)
            .keyboardType(.URL)
            #endif
        TextField("Port", text: $port, prompt: Text("8787"))
            #if os(iOS)
            .keyboardType(.numberPad)
            #endif
        TextField("Name (optional)", text: $name)
    }

    #if os(macOS)
    private func startLocalHost() {
        model.localHostService.start()
        if let serviceError = model.localHostService.errorMessage {
            error = serviceError
        }
    }

    private func harnessField(_ title: String, text: Binding<String?>, placeholder: String) -> some View {
        TextField(title, text: Binding(
            get: { text.wrappedValue ?? "" },
            set: { text.wrappedValue = $0.isEmpty ? nil : $0 }
        ), prompt: Text(placeholder))
        .textFieldStyle(.roundedBorder)
    }
    #endif
}
