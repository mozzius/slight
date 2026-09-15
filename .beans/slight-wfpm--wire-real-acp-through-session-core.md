---
# slight-wfpm
title: wire real ACP through session core
status: completed
type: task
priority: normal
created_at: 2026-09-12T13:40:15Z
updated_at: 2026-09-12T16:36:13Z
parent: slight-dags
---

Replace session-core placeholder ACP events with the conformant bidirectional ACP client. Correlate product and agent session IDs, normalize updates/tool calls/permissions/stop reasons, handle cancellation, and emit gateway-v1 events.

- [x] Store the agent session ID alongside the product session
- [x] Use the bidirectional ACP session boundary from session-core
- [x] Normalize messages, thoughts, tool calls, permissions, exits, failures, and stop reasons
- [x] Preserve gateway-facing session events and replay behavior
- [x] Add real-wire session-core tests

## Summary of Changes

Migrated session-core to the normalized ACP session boundary, including product-to-agent session correlation, real initialize/session/new handshakes, streaming event normalization, permission handling, cancellation stop reasons, and structured failures.

Verification: workspace tests pass, including wire session streaming, permission, cancellation, and handshake-failure tests.
