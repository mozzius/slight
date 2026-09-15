import Foundation

/// Observable state for agent session discovery inside a single agent and
/// working directory. Import is tracked separately so the discovery list stays
/// visible while one row is being imported.
public enum SessionDiscoveryState: Equatable, Sendable {
    case idle
    case loading
    case loaded([AgentSessionSummary])
    case failed(String)

    /// The discovered sessions, or an empty list when not loaded.
    public var sessions: [AgentSessionSummary] {
        if case .loaded(let sessions) = self { return sessions }
        return []
    }

    public var isLoading: Bool {
        if case .loading = self { return true }
        return false
    }

    public var error: String? {
        if case .failed(let message) = self { return message }
        return nil
    }
}
