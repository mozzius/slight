---
# slight-lror
title: avoid broadcasting full attach history
status: completed
type: bug
priority: normal
created_at: 2026-09-13T13:26:22Z
updated_at: 2026-09-13T13:34:21Z
---

A full session.attach replay is returned inline and consumed by the requesting SessionViewModel, but GatewayConnection also rebroadcasts every historical event to all subscribers. Long chats eventually overflow any fixed subscriber buffer and trigger resyncs. Keep attach replay inline and only broadcast incremental live/reconnect events.


## Tasks

- [x] Stop rebroadcasting explicit attach history
- [x] Preserve journal cursors and replay completion state
- [x] Verify gateway and Apple tests


## Expanded Scope

- [x] Define paginated history request/response types and retention metadata
- [x] Add host history pagination endpoint
- [x] Make attach responses lightweight and keep live WebSocket replay bounded
- [x] Add Apple transcript pagination and older-history loading
- [x] Verify Rust and Apple tests


## Summary of Changes

- Added bounded `session.history` pages with `before_sequence`, `limit`, `has_more`, and retention metadata.
- Changed `session.attach` acknowledgements to return session state and cursors without the full transcript.
- Stopped rebroadcasting full attach history through every WebSocket subscriber.
- Added Apple client history loading, older-history pagination, and transcript reconstruction.
- Added gateway, host, and session-core coverage for paginated history.
- Rust workspace and macOS Apple builds pass.
