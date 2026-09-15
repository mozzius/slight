import Foundation
import SlightGateway

/// In-memory transport used to drive `GatewayConnection` without a socket.
final class FakeGatewayTransport: GatewayTransport, @unchecked Sendable {
    private let lock = NSLock()
    private let codec = GatewayCodec()
    private var incoming: [Data] = []
    private var receiveWaiters: [CheckedContinuation<Data?, Error>] = []
    private var isClosed = false
    private var sentFrames: [GatewayFrame] = []

    /// Called on the sending thread whenever the connection writes a frame.
    var onSend: (@Sendable (GatewayFrame) -> Void)?

    func start() async throws {}

    func send(_ data: Data) async throws {
        let frame = try codec.decode(data)
        lock.lock()
        sentFrames.append(frame)
        lock.unlock()
        onSend?(frame)
    }

    func receive() async throws -> Data? {
        lock.lock()
        if !incoming.isEmpty {
            let data = incoming.removeFirst()
            lock.unlock()
            return data
        }
        if isClosed {
            lock.unlock()
            return nil
        }
        lock.unlock()
        return try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            if !incoming.isEmpty {
                let data = incoming.removeFirst()
                lock.unlock()
                continuation.resume(returning: data)
                return
            }
            if isClosed {
                lock.unlock()
                continuation.resume(returning: nil)
                return
            }
            receiveWaiters.append(continuation)
            lock.unlock()
        }
    }

    func close() async {
        lock.lock()
        isClosed = true
        let waiters = receiveWaiters
        receiveWaiters.removeAll()
        lock.unlock()
        for waiter in waiters {
            waiter.resume(returning: nil)
        }
    }

    func push(_ frame: GatewayFrame) {
        guard let data = try? codec.encode(frame) else { return }
        push(data)
    }

    func push(_ data: Data) {
        lock.lock()
        if !receiveWaiters.isEmpty {
            let waiter = receiveWaiters.removeFirst()
            lock.unlock()
            waiter.resume(returning: data)
            return
        }
        incoming.append(data)
        lock.unlock()
    }

    func frames() -> [GatewayFrame] {
        lock.lock()
        defer { lock.unlock() }
        return sentFrames
    }
}
