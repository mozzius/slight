---
# slight-helr
title: implement ACP normalization and session core
status: completed
type: feature
priority: normal
created_at: 2026-09-12T08:24:46Z
updated_at: 2026-09-12T18:47:09Z
parent: slight-4yk6
---

Implement acp-types, acp-adapters boundaries, and session-core state transitions using a fake ACP connection. Normalize agent messages, tool progress, permission requests, diagnostics, exits, input, cancellation, and permission responses without exposing agent-specific behavior to clients.\n\n- [x] Define normalized session/event types\n- [x] Add fake ACP agent and in-memory transport\n- [x] Implement session lifecycle transitions\n- [x] Implement command idempotency and request acknowledgements\n- [x] Add transition and normalization tests

## Summary of Changes

- Confirmed normalized ACP event types, fake session transport, lifecycle transitions, permission handling, and normalization coverage.
- Added a bounded per-connection request acknowledgement cache so duplicate request IDs replay the original result without re-running commands.
- Added a WebSocket regression test proving duplicate session creation is idempotent.
