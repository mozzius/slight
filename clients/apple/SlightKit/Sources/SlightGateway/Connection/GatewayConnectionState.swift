import Foundation

// MARK: - Transport

/// A framed, message-oriented gateway transport. The transport moves opaque
/// `Data` frames; encoding and decoding stay in `GatewayCodec`.
public protocol GatewayTransport: AnyObject, Sendable {
    func start() async throws
    func send(_ data: Data) async throws
    /// Returns the next frame, or `nil` when the peer closed the connection.
    func receive() async throws -> Data?
    func close() async
}

/// Creates a fresh transport for each connection attempt.
public typealias GatewayTransportFactory = @Sendable () throws -> GatewayTransport

// MARK: - Clock

public protocol GatewayClock: Sendable {
    func now() -> Date
    func sleep(for duration: Duration) async throws
}

public struct SystemGatewayClock: GatewayClock {
    public init() {}
    public func now() -> Date { Date() }
    public func sleep(for duration: Duration) async throws {
        try await Task.sleep(for: duration)
    }
}

// MARK: - Failures and errors

public struct GatewayConnectionFailure: Equatable, Sendable {
    public var code: String
    public var message: String

    public init(code: String, message: String) {
        self.code = code
        self.message = message
    }
}

public enum GatewayConnectionError: Error, Equatable, Sendable {
    case transportClosed
    case handshakeFailed(String)
    case timedOut
    case commandTimedOut(String)
    case commandFailed(command: String, code: String, message: String)
    case notConnected

    public var userMessage: String {
        switch self {
        case .transportClosed: return "The host closed the connection."
        case .handshakeFailed(let reason): return "Handshake failed: \(reason)"
        case .timedOut: return "The host did not respond in time."
        case .commandTimedOut(let id): return "Command \(id) timed out."
        case .commandFailed(_, _, let message): return message
        case .notConnected: return "Not connected to a host."
        }
    }
}

// MARK: - Reconnect policy

public struct ReconnectPolicy: Equatable, Sendable {
    public var initialDelay: Duration
    public var multiplier: Double
    public var maxDelay: Duration
    public var maxAttempts: Int?

    public init(
        initialDelay: Duration = .milliseconds(250),
        multiplier: Double = 2,
        maxDelay: Duration = .seconds(15),
        maxAttempts: Int? = nil
    ) {
        self.initialDelay = initialDelay
        self.multiplier = multiplier
        self.maxDelay = maxDelay
        self.maxAttempts = maxAttempts
    }

    public func delay(forAttempt attempt: Int) -> Duration {
        guard attempt > 1 else { return initialDelay }
        var seconds = initialDelay.secondsValue
        for _ in 1..<attempt {
            seconds *= multiplier
        }
        return .seconds(min(seconds, maxDelay.secondsValue))
    }

    public func shouldRetry(attempt: Int) -> Bool {
        guard let maxAttempts else { return true }
        return attempt <= maxAttempts
    }
}

public extension Duration {
    var secondsValue: Double {
        let (seconds, attoseconds) = components
        return Double(seconds) + Double(attoseconds) / 1_000_000_000_000_000_000
    }

    var millisecondsInt: Int {
        Int((secondsValue * 1000).rounded())
    }
}

// MARK: - Connection state

public enum GatewayConnectionState: Equatable, Sendable {
    case idle
    case connecting(attempt: Int)
    case handshaking
    case connected(ServerWelcome)
    case reconnecting(attempt: Int, delayMilliseconds: Int)
    case replaying
    case resyncRequired(reason: String?)
    case failed(GatewayConnectionFailure)
    case closed

    public var isConnected: Bool {
        switch self {
        case .connected, .replaying, .resyncRequired: return true
        default: return false
        }
    }

    public var isActive: Bool {
        switch self {
        case .idle, .failed, .closed: return false
        default: return true
        }
    }

    public var shortDescription: String {
        switch self {
        case .idle: return "Not connected"
        case .connecting(let attempt): return attempt <= 1 ? "Connecting" : "Connecting (attempt \(attempt))"
        case .handshaking: return "Handshaking"
        case .connected(let welcome): return "Connected to \(welcome.serverName)"
        case .reconnecting(let attempt, _): return "Reconnecting (attempt \(attempt))"
        case .replaying: return "Replaying events"
        case .resyncRequired: return "Resync required"
        case .failed(let failure): return failure.message
        case .closed: return "Disconnected"
        }
    }

    /// Additional detail for states the user needs to act on. `nil` when the
    /// state is healthy or self-explanatory.
    public var failureMessage: String? {
        switch self {
        case .failed(let failure): return failure.message
        case .resyncRequired(let reason): return reason
        default: return nil
        }
    }
}

// MARK: - Pure state machine

/// Deterministic connection lifecycle reducer. `GatewayConnection` executes the
/// returned effects; this type is what the unit tests exercise directly.
public struct ConnectionStateMachine: Sendable {
    public private(set) var state: GatewayConnectionState
    public private(set) var attempt: Int
    public let policy: ReconnectPolicy

    public init(policy: ReconnectPolicy = ReconnectPolicy()) {
        self.state = .idle
        self.attempt = 0
        self.policy = policy
    }

    public enum Event: Equatable, Sendable {
        case connectRequested
        case transportOpened
        case handshakeSucceeded(ServerWelcome)
        case handshakeFailed(reason: String)
        case disconnected(reason: String?)
        case reconnectDelayElapsed
        case resyncRequired(reason: String?)
        case reset
    }

    public enum Effect: Equatable, Sendable {
        case none
        case openTransport
        case startHandshake
        case scheduleReconnect(delay: Duration)
        case fail(GatewayConnectionFailure)
        case giveUp
    }

    public mutating func handle(_ event: Event) -> Effect {
        switch event {
        case .connectRequested:
            guard !state.isActive else { return .none }
            attempt = 1
            state = .connecting(attempt: attempt)
            return .openTransport

        case .transportOpened:
            switch state {
            case .connecting:
                state = .handshaking
                return .startHandshake
            default:
                return .none
            }

        case .handshakeSucceeded(let welcome):
            attempt = 0
            state = .connected(welcome)
            return .none

        case .handshakeFailed(let reason):
            return handleDisconnect(reason: reason)

        case .disconnected(let reason):
            guard state.isActive || state == .idle else { return .none }
            return handleDisconnect(reason: reason)

        case .reconnectDelayElapsed:
            if case .reconnecting = state {
                state = .connecting(attempt: attempt)
                return .openTransport
            }
            return .none

        case .resyncRequired(let reason):
            state = .resyncRequired(reason: reason)
            return .none

        case .reset:
            attempt = 0
            state = .idle
            return .none
        }
    }

    private mutating func handleDisconnect(reason: String?) -> Effect {
        let nextAttempt = attempt + 1
        guard policy.shouldRetry(attempt: nextAttempt) else {
            let failure = GatewayConnectionFailure(
                code: "reconnect_exhausted",
                message: reason.map { "Disconnected: \($0)" } ?? "Unable to reach the host."
            )
            state = .failed(failure)
            return .giveUp
        }
        let delay = policy.delay(forAttempt: attempt)
        attempt = nextAttempt
        state = .reconnecting(attempt: nextAttempt, delayMilliseconds: delay.millisecondsInt)
        return .scheduleReconnect(delay: delay)
    }

    /// Marks an intentional shutdown so a later `disconnected` event does not
    /// trigger a reconnect.
    public mutating func markClosed() {
        attempt = 0
        state = .closed
    }
}
