---
# slight-n4hs
title: add host connection debug logging
status: completed
type: bug
priority: normal
created_at: 2026-09-12T18:41:53Z
updated_at: 2026-09-12T18:45:24Z
---

Add console diagnostics around Apple client host connection and gateway decoding: endpoint, lifecycle state changes, handshake, outgoing commands, inbound frame type/request ID, and decode failures with enough context to identify stale-host or malformed-payload issues.

## Summary of Changes

Added `OSLog` diagnostics for gateway lifecycle, connection attempts, transport start/close, WebSocket send/receive failures, handshake identity/version, inbound/outbound frame types and sizes, state transitions, session list/create failures, and frame decoding errors. Decode errors now retain the `DecodingError` coding path in the console instead of only surfacing a generic UI message.

Verification: iOS Simulator `xcodebuild` succeeds.
