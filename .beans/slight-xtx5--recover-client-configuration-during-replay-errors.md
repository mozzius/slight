---
# slight-xtx5
title: recover client configuration during replay errors
status: completed
type: bug
priority: normal
created_at: 2026-09-13T13:24:43Z
updated_at: 2026-09-13T13:25:00Z
---

During gateway replay/resync states the Apple client can skip host configuration refresh, causing model selectors to disappear. Large session replays can also overflow the inbound subscriber buffer.


## Tasks

- [x] Refresh host configuration in all usable connection states
- [x] Increase replay event subscriber capacity
- [x] Verify Apple build


## Summary of Changes

- Host configuration and session list refresh now run for all connection states that report `isConnected`, including replay/resync states.
- Increased the gateway subscriber replay buffer from 1,024 to 8,192 inbound frames, covering the observed 1,121-event transcript replay.
- Verified the macOS Xcode build succeeds.
