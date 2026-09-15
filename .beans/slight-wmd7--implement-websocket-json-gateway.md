---
# slight-wmd7
title: implement WebSocket JSON gateway
status: completed
type: task
priority: normal
created_at: 2026-09-12T13:40:15Z
updated_at: 2026-09-12T14:31:47Z
parent: slight-dags
---

Replace the raw TCP newline-JSON gateway with a WebSocket plus JSON server compatible with URLSessionWebSocketTask. Preserve gateway-v1 frames, authentication hooks, heartbeats, event streaming, and admin/session commands. Add handshake, close/error behavior, and Rust integration tests.

- [x] Replace raw TCP gateway transport with WebSocket plus JSON
- [x] Preserve gateway-v1 frame and admin/session behavior
- [x] Add handshake, framing, close/error, heartbeat, and event tests
- [x] Update protocol documentation and ADRs

## Summary of Changes

Replaced the gateway listener and Rust client with WebSocket transport using tungstenite, while retaining gateway-v1 JSON frame semantics. Added text and binary UTF-8 JSON handling, handshake/error/close behavior, heartbeat coverage, streamed event tests, and ADR-0003. The Apple URLSessionWebSocketTask transport now matches the host transport.

Verification: workspace format, check, tests, and clippy pass; 11 WebSocket integration tests pass.
