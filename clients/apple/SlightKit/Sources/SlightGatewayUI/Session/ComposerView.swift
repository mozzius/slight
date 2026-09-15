import SlightGateway
import SwiftUI

public struct ComposerView: View {
    @Environment(\.colorScheme) private var colorScheme

    @Binding public var message: String
    @Binding public var model: String
    @Binding public var effort: String
    public let models: [String]
    public let efforts: [String]
    public let configurationIsDisabled: Bool
    public let isEnabled: Bool
    public let canCancel: Bool
    public let disabledReason: String?
    public let onSend: () -> Void
    public let onCancel: () -> Void

    public init(
        message: Binding<String>,
        model: Binding<String>,
        effort: Binding<String>,
        models: [String] = [],
        efforts: [String] = [],
        configurationIsDisabled: Bool = false,
        isEnabled: Bool = true,
        canCancel: Bool = false,
        disabledReason: String? = nil,
        onSend: @escaping () -> Void,
        onCancel: @escaping () -> Void = {}
    ) {
        _message = message
        _model = model
        _effort = effort
        self.models = models
        self.efforts = efforts
        self.configurationIsDisabled = configurationIsDisabled
        self.isEnabled = isEnabled
        self.canCancel = canCancel
        self.disabledReason = disabledReason
        self.onSend = onSend
        self.onCancel = onCancel
    }

    public var body: some View {
        LiquidGlassContainer {
            VStack(alignment: .leading, spacing: 12) {
                TextField("Message the agent…", text: $message, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1...6)
                    .onSubmit { submit() }
                    #if os(iOS)
                    .submitLabel(.send)
                    #endif

                HStack(spacing: 12) {
                    if !models.isEmpty || !efforts.isEmpty || configurationIsDisabled {
                        ComposerConfigurationMenu(
                            model: $model,
                            effort: $effort,
                            models: models,
                            efforts: efforts,
                            isDisabled: configurationIsDisabled
                        )
                    }
                    Spacer()
                    Button(action: primaryAction) {
                        Image(systemName: canCancel ? "stop.fill" : "arrow.up")
                            .font(.subheadline)
                            .frame(width: 24, height: 24)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(buttonForeground)
                    .background(buttonBackground, in: Circle())
                    .disabled(!canCancel && !canSend)
                    .accessibilityLabel(canCancel ? "Stop agent" : "Send")
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 12)
        .overlay(alignment: .topLeading) {
            if let reason = disabledReason {
                Text(reason)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity)
                    .multilineTextAlignment(.center)
                    .padding(.top, 4)
                    .padding(.bottom, 4)
                    .offset(y: -18)
            }
        }
    }

    private var canSend: Bool {
        isEnabled && !canCancel && !message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func primaryAction() {
        if canCancel {
            onCancel()
        } else {
            onSend()
        }
    }

    private func submit() {
        guard canSend else { return }
        onSend()
    }

    private var buttonBackground: Color {
        if canCancel { return .clear }
        guard canCancel || canSend else { return .secondary.opacity(0.35) }
        return colorScheme == .dark ? .white : .black
    }

    private var buttonForeground: Color {
        if canCancel { return .primary }
        guard canCancel || canSend else { return .white }
        return colorScheme == .dark ? .black : .white
    }
}
