#if DEBUG
import SlightGateway
import SwiftUI

/// Connects a fake-host-backed model when a preview appears so the session UI
/// exercises the same attach/stream path as the shipping app.
private struct PreviewHarness<Content: View>: View {
    @StateObject private var model: SlightAppModel
    private let content: (SlightAppModel) -> Content

    init(model: SlightAppModel, @ViewBuilder content: @escaping (SlightAppModel) -> Content) {
        _model = StateObject(wrappedValue: model)
        self.content = content
    }

    var body: some View {
        content(model)
            .task { model.connect() }
    }
}

#Preview("Session browser") {
    PreviewHarness(model: .fakeHost(FakeHostFixture.demoHost())) { model in
        NavigationStack {
            SessionBrowserView(model: model)
        }
    }
}

#Preview("Session detail") {
    PreviewHarness(model: .fakeHost(FakeHostFixture.demoHost())) { model in
        NavigationStack {
            SessionDetailContainer(session: FakeHostFixture.sessions()[0], model: model)
        }
    }
}

#if os(macOS)
#Preview("Host admin") {
    PreviewHarness(model: .fakeHost(FakeHostFixture.demoHost())) { model in
        HostAdminView(model: model)
    }
    .frame(minWidth: 900, minHeight: 600)
}
#endif
#endif
