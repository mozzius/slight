---
# slight-2n24
title: open host sheet from server toolbar icon
status: completed
type: task
priority: normal
created_at: 2026-09-12T18:52:37Z
updated_at: 2026-09-12T18:53:24Z
---

Replace the session browser's compact green-dot host control with the existing server.rack toolbar icon. Pressing it should directly open the host settings sheet on iOS or settings surface on macOS.

## Summary of Changes

Replaced the session browser connection dot/menu with the existing `server.rack` toolbar icon. Pressing it directly opens host settings on iOS or the macOS settings surface.

Verification: iOS Simulator `xcodebuild` succeeds.
