import SlightGatewayUI
import SwiftUI

@main
struct SlightApp: App {
    @StateObject private var model: SlightAppModel

    init() {
        _model = StateObject(wrappedValue: PlatformSupport.makeAppModel())
    }

    var body: some Scene {
        mainScene
    }

    #if os(macOS)
    @SceneBuilder
    private var mainScene: some Scene {
        WindowGroup {
            rootView
        }
        .commands {
            CommandGroup(after: .appInfo) {
                Button("Refresh Sessions") {
                    Task { await model.sessionList.refresh() }
                }
                .disabled(!model.connectionState.isConnected || model.sessionList.isLoading)
            }
        }
    }
    #else
    @SceneBuilder
    private var mainScene: some Scene {
        WindowGroup {
            rootView
        }
    }
    #endif

    @ViewBuilder
    private var rootView: some View {
        #if os(iOS)
        SlightRootView(model: model)
            .onChange(of: scenePhase) { _, phase in
                guard phase == .active, !model.isOnboardingRequired else { return }
                model.connect()
            }
        #else
        SlightRootView(model: model)
            .onAppear {
                guard !model.isOnboardingRequired else { return }
                model.connect()
            }
        #endif
    }

    #if os(iOS)
    @Environment(\.scenePhase) private var scenePhase
    #endif
}
