---
# slight-spt4
title: present macos host settings as modal
status: completed
type: task
priority: normal
created_at: 2026-09-14T17:54:28Z
updated_at: 2026-09-14T17:55:19Z
---

Present the macOS host configuration screen as a standard modal sheet like iOS instead of a separate Settings window.



- [x] Replace macOS Settings scene with sheet presentation
- [x] Reuse the shared host sheet wrapper on both platforms
- [x] Preserve host toolbar actions and modal dismissal
- [x] Verify Apple package tests

## Summary of Changes

- Removed the separate macOS Settings scene.
- Host Settings now opens as a standard sheet from the macOS session browser, matching iOS.
- Made `HostConnectionSheet` cross-platform and kept Remove Host dismissal safe for sheets.
- Verified `swift test`: 64 tests passed.
