import Foundation
import OSLog

#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// `URLSessionWebSocketTask`-backed transport. One instance represents one
/// connection attempt; the factory in `GatewayConnection` creates a new one on
/// reconnect.
public final class WebSocketGatewayTransport: NSObject, GatewayTransport, @unchecked Sendable {
    private static let logger = Logger(subsystem: "sh.slight.client", category: "gateway.websocket")
    private let url: URL
    private let additionalHeaders: [String: String]
    private let lock = NSLock()
    private var session: URLSession?
    private var task: URLSessionWebSocketTask?

    public init(url: URL, additionalHeaders: [String: String] = [:]) {
        self.url = url
        self.additionalHeaders = additionalHeaders
        super.init()
    }

    public func start() async throws {
        Self.logger.info("starting WebSocket transport endpoint=\(self.url.absoluteString, privacy: .public)")
        var request = URLRequest(url: url)
        for (key, value) in additionalHeaders {
            request.setValue(value, forHTTPHeaderField: key)
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.waitsForConnectivity = false
        let session = URLSession(configuration: configuration)
        let task = session.webSocketTask(with: request)
        lock.withLock {
            self.session = session
            self.task = task
        }
        task.resume()
        Self.logger.debug("WebSocket task resumed")
    }

    public func send(_ data: Data) async throws {
        guard let task = currentTask else { throw GatewayConnectionError.transportClosed }
        Self.logger.debug("WebSocket send bytes=\(data.count)")
        try await task.send(.data(data))
    }

    public func receive() async throws -> Data? {
        guard let task = currentTask else { throw GatewayConnectionError.transportClosed }
        let message: URLSessionWebSocketTask.Message
        do {
            message = try await task.receive()
        } catch {
            Self.logger.error("WebSocket receive failed: \(String(describing: error), privacy: .public)")
            throw error
        }
        switch message {
        case .data(let data):
            Self.logger.debug("WebSocket received data bytes=\(data.count)")
            return data
        case .string(let string):
            Self.logger.debug("WebSocket received text bytes=\(string.utf8.count)")
            return Data(string.utf8)
        @unknown default:
            return nil
        }
    }

    public func close() async {
        Self.logger.info("closing WebSocket transport")
        let (task, session) = lock.withLock { (self.task, self.session) }
        lock.withLock {
            self.task = nil
            self.session = nil
        }
        task?.cancel(with: .normalClosure, reason: nil)
        session?.invalidateAndCancel()
    }

    private var currentTask: URLSessionWebSocketTask? {
        lock.withLock { task }
    }
}

/// Builds `WebSocketGatewayTransport` instances for a host endpoint.
public struct WebSocketTransportFactory: Sendable {
    public var url: URL
    public var additionalHeaders: [String: String]

    public init(url: URL, additionalHeaders: [String: String] = [:]) {
        self.url = url
        self.additionalHeaders = additionalHeaders
    }

    public func makeFactory() -> GatewayTransportFactory {
        let url = self.url
        let headers = self.additionalHeaders
        return { WebSocketGatewayTransport(url: url, additionalHeaders: headers) }
    }
}
