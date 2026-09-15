# Provisional gateway-v1 Swift boundary

The canonical gateway-v1 contract now lives in `protocol/gateway-v1.md`, the
Rust `gateway-protocol` crate, and `protocol/conformance/` (bean `slight-3ude`).
This Swift module mirrors that contract by hand.

Rules for this boundary:

1. All wire names are `snake_case` and all JSON encoding/decoding goes through
   `GatewayCodec`. No view model or view may hand-roll JSON.
2. Frame types in `GatewayFrames.swift` mirror `gateway-protocol/src/frames.rs`.
3. Event payloads in `SessionEventPayloads.swift` mirror
   `session-core/src/types.rs`, and admin DTOs in `HostAdminModels.swift` mirror
   `gateway-protocol/src/admin.rs`.
4. `GatewayFrame.unknown(type:raw:)` preserves frames whose `type` is not yet
   known, so a newer host does not crash an older client.
5. `Tests/SlightGatewayTests/GatewayConformanceTests.swift` decodes the shared
   `protocol/conformance/*.json` vectors directly, so a contract change breaks
   the client tests until the Swift models are updated.

Remaining provisional shapes (no canonical Rust result type yet):

- `HostConfiguration` (`host.configuration`).

Replace them when the corresponding Rust DTOs land, and keep the conformance
tests green.
