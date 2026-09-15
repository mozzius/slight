---
# slight-48p7
title: preserve user and agent messages when loading sessions
status: completed
type: bug
priority: normal
created_at: 2026-09-12T23:44:04Z
updated_at: 2026-09-13T02:09:43Z
---

Loading a persisted session drops user messages and renders all agent messages as one merged block. Trace persistence, replay, decoding, and client rendering; preserve message boundaries and roles after reload.\n\n- [x] Reproduce and identify the layer collapsing transcript messages\n- [x] Fix persisted session loading without regressing live streaming aggregation\n- [x] Add or update regression coverage\n- [x] Run focused verification (Swift suite passed before unrelated concurrent SessionModePayload redeclaration appeared; Rust remains blocked by pre-existing set_mode errors)

\n- [x] Prevent attach/replay from showing active streaming UI while history loads

## Summary of Changes\n\n- Persisted each submitted prompt as a normalized user `session.message` event so replay retains user messages and assistant-turn boundaries.\n- Kept live assistant chunk aggregation unchanged.\n- Suppressed the active/streaming state while a session attach is rebuilding its transcript, then restored the actual host status after replay.\n- Added Apple and Rust regression coverage.

\n- [x] Stop replayed historical messages from showing streaming animation

\n## Follow-up Verification\n\n- Replayed messages now have `isStreaming` cleared after attach replay.\n- The session-level working indicator remains hidden while attach/replay is active.\n- Focused Swift regression test passed.

\n- [x] Batch replayed history so attach renders one completed transcript update instead of streaming events

\n## Replay Batching\n\n- Attach events are buffered in the session view model and flushed together at replay completion, preventing history from visually arriving as a live stream.\n- Focused `SessionViewModelBehaviorTests` passed (13 tests).

\n- [x] Stop replay cursor loop and broken-pipe reconnects during session attach

\n## Replay Cursor Fix\n\n- Events arriving while attach is pending are deferred instead of triggering a replay request from sequence 1.\n- Deferred events are applied after the attach response establishes the journal cursor.\n- Gateway connection and session behavior tests pass.

\n- [x] Stop session.list retry loop on broken WebSocket and add connection lifecycle diagnostics

\n## Connection Recovery\n\n- Session listing now runs only in the fully connected state and does not overlap requests.\n- Failed command sends tear down the dead transport so reconnect can create a fresh socket.\n- Added request and send-failure diagnostics.
