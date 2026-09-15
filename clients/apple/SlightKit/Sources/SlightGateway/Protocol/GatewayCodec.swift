import Foundation

/// Single entry point for gateway JSON encoding/decoding.
///
/// The provisional contract uses a `snake_case` wire format. Keeping the codecs
/// here means a future binary encoding or schema-generated model layer only has
/// to replace this type.
public struct GatewayCodec: @unchecked Sendable {
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder

    public init() {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.withoutEscapingSlashes]
        self.encoder = encoder

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        // The Rust host emits RFC 3339 timestamps with millisecond precision
        // whenever the millisecond component is non-zero. `JSONDecoder`'s
        // `.iso8601` strategy rejects fractional seconds, which would fail the
        // entire frame, so accept both forms explicitly.
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let string = try container.decode(String.self)
            if let date = GatewayCodec.parseTimestamp(string) {
                return date
            }
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Expected an RFC 3339 date, got \(string)"
            )
        }
        self.decoder = decoder
    }

    /// Parses RFC 3339 timestamps with or without a fractional-seconds component.
    /// `Date.ISO8601FormatStyle` raises internally on a mismatch, so try the
    /// fractional form first and fall back to whole seconds.
    static func parseTimestamp(_ string: String) -> Date? {
        if let date = try? Date(string, strategy: Date.ISO8601FormatStyle(includingFractionalSeconds: true)) {
            return date
        }
        return try? Date(string, strategy: Date.ISO8601FormatStyle(includingFractionalSeconds: false))
    }

    public func encode(_ frame: GatewayFrame) throws -> Data {
        try encoder.encode(frame)
    }

    public func decode(_ data: Data) throws -> GatewayFrame {
        try decoder.decode(GatewayFrame.self, from: data)
    }

    public func encodeJSON<T: Encodable>(_ value: T) throws -> Data {
        try encoder.encode(value)
    }

    public func decodeJSON<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        try decoder.decode(type, from: data)
    }

    /// Decodes a typed payload out of an untyped frame field. This is the one
    /// place that crosses the provisional `JSONValue` boundary; replace it when
    /// canonical schemas exist.
    public func decodePayload<T: Decodable>(_ type: T.Type, from value: JSONValue?) throws -> T? {
        guard let value else { return nil }
        if case .null = value { return nil }
        let data = try encoder.encode(value)
        return try decoder.decode(type, from: data)
    }

    public func encodeToJSONValue<T: Encodable>(_ value: T) throws -> JSONValue {
        let data = try encoder.encode(value)
        return try decoder.decode(JSONValue.self, from: data)
    }
}

public extension JSONValue {
    /// Builds a JSON object from already-encoded values. Convenience for
    /// constructing command params without stringly-typed dictionaries.
    static func object(_ pairs: [(String, JSONValue?)]) -> JSONValue {
        var result: [String: JSONValue] = [:]
        for (key, value) in pairs {
            result[key] = value ?? .null
        }
        return .object(result)
    }
}
