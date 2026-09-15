import Foundation

#if os(macOS)

struct HarnessConfiguration: Codable, Equatable {
    var opencode: HarnessEntry

    init(
        opencode: HarnessEntry = HarnessEntry()
    ) {
        self.opencode = opencode
    }
}

struct HarnessEntry: Codable, Equatable {
    var program: String?

    init(program: String? = nil) {
        self.program = program
    }
}

enum HarnessConfigurationStore {
    static var fileURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".slight", isDirectory: true)
            .appendingPathComponent("config.json")
    }

    static func load() -> HarnessConfiguration {
        guard let data = try? Data(contentsOf: fileURL),
              let decoded = decode(data)
        else {
            return HarnessConfiguration()
        }
        return decoded
    }

    static func save(_ configuration: HarnessConfiguration) throws {
        let directory = fileURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try encoder.encode(PersistedConfiguration(harnesses: configuration)).write(to: fileURL, options: .atomic)
    }

    static func validate(_ configuration: HarnessConfiguration) -> String? {
        guard let path = configuration.opencode.program,
              !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return nil
        }
        guard FileManager.default.isExecutableFile(atPath: expanded(path)) else {
            return "The OpenCode executable was not found at \(path)."
        }
        return nil
    }

    static var detectedOpencodePath: String? {
        autodetectOpencode()
    }

    private static func autodetectOpencode() -> String? {
        autodetect(["~/.opencode/bin/opencode", "/opt/homebrew/bin/opencode"])
    }

    private static func decode(_ data: Data) -> HarnessConfiguration? {
        let decoder = JSONDecoder()
        if let persisted = try? decoder.decode(PersistedConfiguration.self, from: data) {
            return persisted.harnesses
        }
        // Read the pre-namespaced file written by the initial settings build.
        return try? decoder.decode(HarnessConfiguration.self, from: data)
    }

    private static func autodetect(_ candidates: [String]) -> String? {
        candidates
            .map(expanded)
            .first { FileManager.default.isExecutableFile(atPath: $0) }
    }

    static func expanded(_ path: String) -> String {
        (path as NSString).expandingTildeInPath
    }
}

private struct PersistedConfiguration: Codable {
    var harnesses: HarnessConfiguration
}

#endif
