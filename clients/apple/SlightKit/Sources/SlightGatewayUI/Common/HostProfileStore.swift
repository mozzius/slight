import Foundation
import SlightGateway

/// Persists the host endpoint the user configured. Kept behind a small boundary
/// so the shared model does not depend on `UserDefaults` directly and tests can
/// supply an in-memory store.
public protocol HostProfileStore {
    func load() -> HostProfile?
    func save(_ profile: HostProfile)
    func clear()
}

/// `UserDefaults`-backed store shared by iOS and macOS. Also understands the
/// legacy `SlightHostEndpoint` string key from the first development builds.
public struct UserDefaultsHostProfileStore: HostProfileStore {
    public static let profileKey = "SlightHostProfile"
    public static let legacyEndpointKey = "SlightHostEndpoint"

    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public func load() -> HostProfile? {
        if let data = defaults.data(forKey: Self.profileKey),
           let profile = try? JSONDecoder().decode(HostProfile.self, from: data) {
            return profile
        }
        if let endpoint = defaults.string(forKey: Self.legacyEndpointKey),
           let url = URL(string: endpoint) {
            return HostProfile(
                id: "configured",
                displayName: url.host ?? "Configured host",
                endpoint: url,
                isLoopback: HostEndpoint.isLoopback(url)
            )
        }
        return nil
    }

    public func save(_ profile: HostProfile) {
        guard let data = try? JSONEncoder().encode(profile) else { return }
        defaults.set(data, forKey: Self.profileKey)
    }

    public func clear() {
        defaults.removeObject(forKey: Self.profileKey)
        defaults.removeObject(forKey: Self.legacyEndpointKey)
    }
}

/// Validation and classification shared by the endpoint editor and the
/// platform default. The gateway transport only speaks WebSocket, so an endpoint
/// must be a `ws`/`wss` URL with a host.
public enum HostEndpoint {
    public static func normalizedURL(from text: String) -> URL? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              let url = URL(string: trimmed),
              let scheme = url.scheme?.lowercased(),
              scheme == "ws" || scheme == "wss",
              url.host != nil else {
            return nil
        }
        return url
    }

    public static func isLoopback(_ url: URL) -> Bool {
        switch url.host?.lowercased() {
        case "127.0.0.1", "localhost", "::1", "[::1]":
            return true
        default:
            return false
        }
    }

    public static func profile(for url: URL, preserving id: String?) -> HostProfile {
        let host = url.host ?? "Host"
        return HostProfile(
            id: id ?? "configured",
            displayName: host,
            endpoint: url,
            isLoopback: isLoopback(url)
        )
    }
}
