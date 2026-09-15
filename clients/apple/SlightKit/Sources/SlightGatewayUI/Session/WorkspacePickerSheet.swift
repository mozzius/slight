import SwiftUI

struct WorkspacePickerSheet: View {
    let paths: [String]
    let selection: String
    let onSelect: (String) -> Void
    let onCancel: () -> Void

    @State private var searchText = ""
    @State private var customPath = ""

    private var filteredPaths: [String] {
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return paths }
        return paths.filter { $0.localizedCaseInsensitiveContains(query) || name(for: $0).localizedCaseInsensitiveContains(query) }
    }

    var body: some View {
        NavigationStack {
            List {
                if !filteredPaths.isEmpty {
                    Section("Recent workspaces") {
                        ForEach(filteredPaths, id: \.self) { path in
                            Button { onSelect(path) } label: {
                                HStack(spacing: 12) {
                                    Image(systemName: "folder")
                                        .foregroundStyle(.tint)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(name(for: path))
                                            .foregroundStyle(.primary)
                                        Text(path)
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                            .lineLimit(1)
                                    }
                                    Spacer()
                                    if path == selection {
                                        Image(systemName: "checkmark")
                                            .foregroundStyle(.tint)
                                    }
                                }
                            }
                        }
                    }
                }

                Section("New workspace") {
                    TextField("Path", text: $customPath)
                        .autocorrectionDisabled()
                        #if os(iOS)
                        .textInputAutocapitalization(.never)
                        #endif
                    Button("Use this path") {
                        let path = customPath.trimmingCharacters(in: .whitespacesAndNewlines)
                        if !path.isEmpty { onSelect(path) }
                    }
                    .disabled(customPath.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .searchable(text: $searchText, prompt: "Search workspaces")
            .navigationTitle("Workspace")
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: onCancel)
                }
            }
        }
    }

    private func name(for path: String) -> String {
        path.split(separator: "/").last.map(String.init) ?? path
    }
}
