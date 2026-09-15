---
# slight-nmpb
title: remove loopback controls from iOS UI
status: completed
type: task
priority: normal
created_at: 2026-09-14T17:34:24Z
updated_at: 2026-09-14T17:35:59Z
parent: slight-7ta5
---

Remove loopback-only endpoint controls and defaults from the iOS UI while preserving them for macOS.\n\n- [x] Hide loopback mode and copy from iOS host settings\n- [x] Keep iOS host settings on remote mode instead of exposing loopback\n- [x] Update tests and verify Apple client builds

## Summary of Changes

- Hid the local host mode picker and local-only copy from iOS while preserving the macOS path.
- Renamed user-facing Loopback labels to Local.
- Verified the SlightKit Swift test suite: 61 tests passed.
