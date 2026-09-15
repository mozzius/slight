---
# slight-l7yh
title: move refresh into native refresh surfaces
status: completed
type: task
priority: normal
created_at: 2026-09-12T18:50:34Z
updated_at: 2026-09-12T18:51:15Z
---

Remove the refresh toolbar button from the session browser. Preserve iOS pull-to-refresh and expose Refresh Sessions as a macOS menu bar command.

## Summary of Changes

Removed the session browser refresh toolbar button. iOS retains native pull-to-refresh, and macOS now exposes `Refresh Sessions` in the application menu with connection/loading state handling.

Verification: macOS `xcodebuild` succeeds.
