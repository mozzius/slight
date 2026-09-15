import SlightGateway
import SwiftUI

/// Shared root. The session browser owns the adaptive navigation layout; host
/// controls are presented from its toolbar instead of taking a second column.
public struct SlightRootView: View {
    @ObservedObject private var model: SlightAppModel

    public init(model: SlightAppModel) {
        self.model = model
    }

    public var body: some View {
        if model.isOnboardingRequired {
            HostOnboardingSplashView(model: model)
        } else {
            connectedRootView
        }
    }

    @ViewBuilder
    private var connectedRootView: some View {
        SessionBrowserView(model: model)
    }

}
