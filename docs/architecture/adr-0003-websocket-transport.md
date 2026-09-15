# ADR 0003: WebSocket transport for gateway v1

Status: accepted

## Context

The first host slice carried gateway-v1 frames as newline-delimited JSON over a
raw TCP connection (ADR 0001). The native clients, however, are built on
`URLSessionWebSocketTask` (`clients/apple/.../WebSocketGatewayTransport.swift`),
and the deployment target is remote control over Tailscale rather than a sibling
process on loopback. A raw TCP line protocol does not survive that boundary: it
has no standard upgrade, no frame boundaries, no close handshake, and no
protocol-level liveness, so every client would have to re-implement framing over
TCP.

The frame union, command vocabulary, authentication hooks, and event/replay
semantics are transport-independent and were deliberately kept that way. Only
the byte transport needed to change.

## Decisions

### WebSocket with JSON payloads, `tungstenite`

The gateway now performs an RFC 6455 upgrade and carries one gateway frame per
WebSocket message. The Rust implementation is the maintained synchronous
[`tungstenite`](https://crates.io/crates/tungstenite) crate with
`default-features = false, features = ["handshake"]` (no TLS backend, no async
runtime). This preserves the existing thread-per-connection model and the
`session-core` state machine's synchronous, deterministic behavior.

TLS (`wss://`) is a separate, later decision; the `rustls`/`native-tls` features
of the same crate can be enabled behind listener configuration without changing
the protocol types.

### Text and binary UTF-8 JSON are both accepted

`URLSessionWebSocketTask.send(.data(...))` produces a binary WebSocket message,
while other clients and the Rust `GatewayClient` send text. Both are the same
JSON encoding, so the server decodes text and binary payloads the same way.
Responses are always emitted as text. Invalid UTF-8 or non-JSON payloads produce
a `malformed_frame` error frame instead of a disconnect.

### Polling preserves subscription and heartbeat behavior

The server keeps a short (50 ms) socket read timeout and polls on each tick to
drain session subscriptions and emit application-level `ping` frames at
`heartbeat_interval_ms`. `tungstenite`'s frame codec preserves partially-read
frame bytes across a `WouldBlock` return, so a timeout mid-frame is safe.
Protocol-level `ping` frames are answered automatically by the transport, and
the close handshake is completed before the connection is dropped.

## Consequences

- The Rust host and the Apple `URLSessionWebSocketTask` clients speak the same
  wire protocol. `protocol/gateway-v1.md` documents text/binary acceptance,
  heartbeats, and close behavior.
- `gateway-server` gains a `tungstenite` dependency (plus its `handshake`
  features: `data-encoding`, `http`, `httparse`, `sha1`). It remains a
  synchronous, non-async crate.
- `gateway-protocol` gains `encode_text`/`decode_text` for message framing while
  keeping `encode_line`/`decode_line` for the existing conformance fixtures.
- The newline-delimited line codec is retained but unused by the live server.

## Remaining gaps

- `wss://`/TLS and Tailscale interface selection.
- Per-connection backpressure policy (the current write path bounds a single
  write with a timeout but does not yet apply flow control to slow readers).
- Client-side heartbeat timeout and reconnect policy on each platform.
