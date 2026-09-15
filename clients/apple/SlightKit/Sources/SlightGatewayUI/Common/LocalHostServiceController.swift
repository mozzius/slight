import Combine
import Foundation

#if os(macOS)
import Darwin
import Network
import ServiceManagement
#endif

/// Owns the app-managed local `acp-host` process. The binary remains usable as
/// a standalone CLI; this controller only provides the desktop convenience path.
@MainActor
public final class LocalHostServiceController: ObservableObject {
    @Published public private(set) var isRunning = false
    @Published public private(set) var isStarting = false
    @Published public private(set) var startsAtLogin = false
    @Published public private(set) var errorMessage: String?

    #if os(macOS)
    private var process: Process?
    private let managedPIDKey = "SlightManagedHostPID"
    #endif

    public init() {
        #if os(macOS)
        startsAtLogin = SMAppService.mainApp.status == .enabled
        #endif
    }

    public func start() {
        #if os(macOS)
        Task { await startIfNeeded() }
        #endif
    }

    public func startIfNeeded() async -> Bool {
        #if os(macOS)
        if isRunning { return true }
        while isStarting {
            try? await Task.sleep(for: .milliseconds(100))
        }
        guard !isRunning else { return true }
        isStarting = true
        defer { isStarting = false }

        // A previous app instance may have left its managed daemon alive after
        // an Xcode rebuild. Replace only that process; never disturb a host
        // started independently from the CLI.
        if stopPreviousManagedProcess() {
            _ = await waitForLocalHost(expected: false, timeout: .seconds(2))
        }
        if await waitForLocalHost(expected: true, timeout: .milliseconds(500)) {
            isRunning = true
            errorMessage = nil
            return true
        }

        guard let executableURL = bundledExecutableURL else {
            errorMessage = "The bundled acp-host executable is missing from this app."
            return false
        }

        let process = Process()
        process.executableURL = executableURL
        let binds = ["127.0.0.1:8787", tailscaleIPv4().map { "\($0):8787" }]
            .compactMap { $0 }
            .joined(separator: ",")
        process.arguments = ["serve", "--bind", binds]
        let processLogURL = self.launchLogURL
        process.standardOutput = launchLogHandle(at: processLogURL)
        process.standardError = launchLogHandle(at: processLogURL)
        process.terminationHandler = { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.isRunning = false
                self?.process = nil
            }
        }

        do {
            try process.run()
            self.process = process
            UserDefaults.standard.set(Int(process.processIdentifier), forKey: managedPIDKey)
            guard await waitForLocalHost(expected: true, timeout: .seconds(15)) else {
                process.terminate()
                self.process = nil
                UserDefaults.standard.removeObject(forKey: managedPIDKey)
                errorMessage = "The local host did not start listening on port 8787. See \(processLogURL.path) for the host error."
                return false
            }
            isRunning = true
            errorMessage = nil
            return true
        } catch {
            errorMessage = "Could not start the local host: \(error.localizedDescription)"
            return false
        }
        #else
        return true
        #endif
    }

    public func stop() {
        #if os(macOS)
        process?.terminate()
        process = nil
        UserDefaults.standard.removeObject(forKey: managedPIDKey)
        isRunning = false
        #endif
    }

    public func setStartsAtLogin(_ enabled: Bool) {
        #if os(macOS)
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            startsAtLogin = enabled
            errorMessage = nil
        } catch {
            errorMessage = "Could not update startup behavior: \(error.localizedDescription)"
        }
        #else
        _ = enabled
        #endif
    }

    #if os(macOS)
    private func stopPreviousManagedProcess() -> Bool {
        let pid = UserDefaults.standard.integer(forKey: managedPIDKey)
        guard pid > 0, kill(pid_t(pid), 0) == 0 else {
            UserDefaults.standard.removeObject(forKey: managedPIDKey)
            return false
        }
        kill(pid_t(pid), SIGTERM)
        UserDefaults.standard.removeObject(forKey: managedPIDKey)
        return true
    }

    private func waitForLocalHost(expected: Bool, timeout: Duration) async -> Bool {
        let deadline = ContinuousClock.now + timeout
        while ContinuousClock.now < deadline {
            if await localHostPortIsReachable() == expected { return true }
            try? await Task.sleep(for: .milliseconds(100))
        }
        return await localHostPortIsReachable() == expected
    }

    private func localHostPortIsReachable() async -> Bool {
        let probe = PortProbe()
        return await withCheckedContinuation { continuation in
            probe.continuation = continuation
            probe.connection.stateUpdateHandler = { [weak probe] state in
                guard let probe else { return }
                switch state {
                case .ready:
                    probe.resolve(true)
                case .failed, .cancelled:
                    probe.resolve(false)
                default:
                    break
                }
            }
            probe.connection.start(queue: DispatchQueue.global(qos: .utility))
            DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + .seconds(2)) {
                probe.resolve(false)
            }
        }
    }

    private func tailscaleIPv4() -> String? {
        let candidates = [
            "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
            "/opt/homebrew/bin/tailscale",
            "/usr/local/bin/tailscale",
            "/usr/bin/tailscale",
        ]
        guard let executable = candidates.first(where: { FileManager.default.isExecutableFile(atPath: $0) }) else {
            return nil
        }

        let command = Process()
        let output = Pipe()
        command.executableURL = URL(fileURLWithPath: executable)
        command.arguments = ["ip", "-4"]
        command.standardOutput = output
        command.standardError = FileHandle.nullDevice
        do {
            try command.run()
            command.waitUntilExit()
        } catch {
            return nil
        }
        guard command.terminationStatus == 0 else { return nil }

        return String(data: output.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
            .split(whereSeparator: \.isNewline)
            .map(String.init)
            .first(where: Self.isIPv4)
    }

    private static func isIPv4(_ value: String) -> Bool {
        let octets = value.split(separator: ".")
        return octets.count == 4 && octets.allSatisfy { Int($0).map { (0...255).contains($0) } ?? false }
    }

    private var bundledExecutableURL: URL? {
        let resources = Bundle.main.resourceURL
        let candidates = [
            resources?.appendingPathComponent("acp-host"),
            Bundle.main.privateFrameworksURL?.appendingPathComponent("acp-host"),
        ]
        return candidates.compactMap { $0 }.first { FileManager.default.isExecutableFile(atPath: $0.path) }
    }

    private var launchLogURL: URL {
        let directory = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".slight/logs", isDirectory: true)
        return directory.appendingPathComponent("acp-host-process.log")
    }

    private func launchLogHandle(at url: URL) -> FileHandle {
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            if !FileManager.default.fileExists(atPath: url.path) {
                FileManager.default.createFile(atPath: url.path, contents: nil)
            }
            let handle = try FileHandle(forWritingTo: url)
            try handle.seekToEnd()
            return handle
        } catch {
            return FileHandle.nullDevice
        }
    }
    #endif
}

#if os(macOS)
private final class PortProbe: @unchecked Sendable {
    let connection = NWConnection(
        host: NWEndpoint.Host("127.0.0.1"),
        port: NWEndpoint.Port(rawValue: 8787)!,
        using: .tcp
    )

    var continuation: CheckedContinuation<Bool, Never>?
    private let lock = NSLock()
    private var didResolve = false

    func resolve(_ value: Bool) {
        lock.lock()
        guard !didResolve else {
            lock.unlock()
            return
        }
        didResolve = true
        let continuation = self.continuation
        self.continuation = nil
        lock.unlock()

        connection.cancel()
        continuation?.resume(returning: value)
    }
}
#endif
