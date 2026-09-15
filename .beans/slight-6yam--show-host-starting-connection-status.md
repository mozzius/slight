---
# slight-6yam
title: show host starting connection status
status: completed
type: task
priority: normal
created_at: 2026-09-14T19:21:25Z
updated_at: 2026-09-14T19:22:10Z
---

Expose bundled local host startup as an explicit Host starting status in the host settings connection section, including spinner and explanatory copy.



- [x] Expose local host startup state
- [x] Show Host starting with spinner and explanatory text
- [x] Add explicit labels for all connection lifecycle states
- [x] Disable connection actions while host startup is active
- [x] Verify Swift tests

## Summary of Changes

- Added observable local host startup state.
- Host settings now shows `Host starting` distinctly from WebSocket connection states.
- Added clear labels for ready, connecting, handshaking, connected, reconnecting, replaying, resync, failed, and disconnected states.
- Verified `swift test`: 64 tests passed.
