---
# slight-yl5o
title: fix iOS host connection fail loop
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:52:41Z
updated_at: 2026-09-15T15:49:01Z
---

Investigate the iOS host connection repeatedly failing/retrying and fix the connection lifecycle without breaking macOS behavior.



- [x] Add host WebSocket command and termination logging
- [x] Rebuild and verify the host with tests


- [x] Reproduce the broken-session loop and diagnose the logged termination

## Summary of Changes

- Treat transient WebSocket write backpressure (EAGAIN/WouldBlock) as retryable and retain the queued frame instead of disconnecting the client.
- Add an explicit connection reconnect path for Retry actions; terminal or closed states now stop the old run and start a fresh attempt.
- Confirmed host logs showed authenticated connections being dropped on Resource temporarily unavailable, matching the reconnect/refusal behavior.
