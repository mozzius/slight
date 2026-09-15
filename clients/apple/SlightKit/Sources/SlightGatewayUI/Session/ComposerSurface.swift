import SwiftUI

struct LiquidGlassContainer<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        content()
            .padding(16)
            .background {
                if #available(iOS 26, macOS 26, *) {
                    Color.clear
                } else {
                    RoundedRectangle(cornerRadius: 24)
                        .fill(.regularMaterial)
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 24))
            .modifier(GlassEffectModifier())
            .shadow(color: .black.opacity(0.12), radius: 18, y: 8)
    }
}

struct GlassEffectModifier: ViewModifier {
    @ViewBuilder
    func body(content: Content) -> some View {
        if #available(iOS 26, macOS 26, *) {
            content.glassEffect(.regular, in: .rect(cornerRadius: 24))
        } else {
            content
        }
    }
}

struct ComposerConfigurationMenu: View {
    @Binding var model: String
    @Binding var effort: String
    let models: [String]
    let efforts: [String]
    var isDisabled = false

    init(
        model: Binding<String>,
        effort: Binding<String>,
        models: [String],
        efforts: [String],
        isDisabled: Bool = false
    ) {
        _model = model
        _effort = effort
        self.models = models
        self.efforts = efforts
        self.isDisabled = isDisabled
    }

    var body: some View {
        Menu {
            if !models.isEmpty {
                if efforts.isEmpty {
                    // no menu nesting if only one item
                    ForEach(models, id: \.self) { option in
                        Button {
                            model = option
                        } label: {
                            if option == model {
                                Label(option, systemImage: "checkmark")
                            } else {
                                Text(option)
                            }
                        }
                    }
                } else {
                    Menu(model, systemImage: "cpu") {
                        ForEach(models, id: \.self) { option in
                            Button {
                                model = option
                            } label: {
                                if option == model {
                                    Label(option, systemImage: "checkmark")
                                } else {
                                    Text(option)
                                }
                            }
                        }
                    }
                }
            }
            if !efforts.isEmpty {
                Menu(effort, systemImage: "dial.medium") {
                    ForEach(efforts, id: \.self) { option in
                        Button {
                            effort = option
                        } label: {
                            if option == effort {
                                Label(option, systemImage: "checkmark")
                            } else {
                                Text(option)
                            }
                        }
                    }
                }
            }
        } label: {
            Text(label)
                .font(.subheadline.weight(.medium))
        }
        .menuStyle(.borderlessButton)
        .menuOrder(.fixed)
        .disabled(isDisabled)
        .accessibilityLabel("Model and effort")
    }

    private var label: String {
        switch (models.isEmpty, efforts.isEmpty) {
        case (false, false): return "\(model) / \(effort)"
        case (false, true): return model
        case (true, false): return effort
        case (true, true): return model
        }
    }
}
