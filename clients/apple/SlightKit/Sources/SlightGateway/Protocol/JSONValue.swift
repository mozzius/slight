import Foundation

/// A minimal, `Sendable`, order-insensitive JSON value used for protocol fields
/// whose shape is not yet fixed (for example raw ACP debug payloads, unknown
/// params, or tool-call detail blobs).
///
/// The gateway contract avoids untyped JSON for normal flows, but a typed escape
/// hatch is required for forward compatibility. Unknown object keys are retained
/// so a client can round-trip a payload it does not understand.
public enum JSONValue: Equatable, Sendable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])
}

extension JSONValue: Codable {
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(Double.self) {
            self = .number(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode([JSONValue].self) {
            self = .array(value)
        } else if let value = try? container.decode([String: JSONValue].self) {
            self = .object(value)
        } else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Unsupported JSON value"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null: try container.encodeNil()
        case .bool(let value): try container.encode(value)
        case .number(let value): try container.encode(value)
        case .string(let value): try container.encode(value)
        case .array(let value): try container.encode(value)
        case .object(let value): try container.encode(value)
        }
    }
}

public extension JSONValue {
    subscript(key: String) -> JSONValue? {
        guard case .object(let object) = self else { return nil }
        return object[key]
    }

    var stringValue: String? {
        guard case .string(let value) = self else { return nil }
        return value
    }

    var intValue: Int? {
        guard case .number(let value) = self, value.rounded() == value else { return nil }
        return Int(value)
    }

    /// Compact, stable rendering used by diagnostics and debug views.
    var prettyDescription: String {
        switch self {
        case .null: return "null"
        case .bool(let value): return value ? "true" : "false"
        case .number(let value):
            return value.rounded() == value ? String(Int(value)) : String(value)
        case .string(let value): return value
        case .array(let values):
            return "[" + values.map(\.prettyDescription).joined(separator: ", ") + "]"
        case .object(let object):
            let body = object.keys.sorted().map { "\($0): \(object[$0]!.prettyDescription)" }
            return "{" + body.joined(separator: ", ") + "}"
        }
    }
}
