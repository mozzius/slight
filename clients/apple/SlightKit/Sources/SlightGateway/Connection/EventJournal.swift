import Foundation

/// Classification of an inbound event against the client's replay cursor.
public enum EventDisposition: Equatable, Sendable {
    case accepted
    case duplicate
    case gap(expected: Int, received: Int)
}

/// Tracks the highest contiguous per-session event sequence the client has seen.
///
/// The gateway promises monotonically increasing per-session sequence numbers.
/// A duplicate is a replay overlap; a gap means the client must ask for replay
/// or accept a resync. On a gap the cursor is intentionally *not* advanced, so
/// repeated gaps keep pointing at the same missing range until replay catches up.
public struct EventJournal: Sendable {
    public private(set) var lastSequenceBySession: [String: Int]

    public init(lastSequenceBySession: [String: Int] = [:]) {
        self.lastSequenceBySession = lastSequenceBySession
    }

    public func lastSequence(for sessionId: String) -> Int {
        lastSequenceBySession[sessionId] ?? 0
    }

    public var highestKnownSequence: Int {
        lastSequenceBySession.values.max() ?? 0
    }

    @discardableResult
    public mutating func observe(_ event: GatewayEvent) -> EventDisposition {
        let last = lastSequence(for: event.sessionId)
        if event.sequence <= last {
            return .duplicate
        }
        if event.sequence > last + 1 {
            return .gap(expected: last + 1, received: event.sequence)
        }
        lastSequenceBySession[event.sessionId] = event.sequence
        return .accepted
    }

    public mutating func remove(sessionId: String?) {
        guard let sessionId else {
            lastSequenceBySession.removeAll()
            return
        }
        lastSequenceBySession.removeValue(forKey: sessionId)
    }

    public mutating func markReplayed(sessionId: String, to sequence: Int) {
        let existing = lastSequence(for: sessionId)
        lastSequenceBySession[sessionId] = max(existing, sequence)
    }
}
