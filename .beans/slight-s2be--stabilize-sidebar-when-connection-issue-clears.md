---
# slight-s2be
title: stabilize sidebar when connection issue clears
status: completed
type: bug
priority: normal
created_at: 2026-09-14T20:47:17Z
updated_at: 2026-09-14T20:47:32Z
---

Keep the macOS sidebar session list from inheriting a bad scroll offset when the transient connection issue content is removed during startup.

## Summary of Changes

Moved the macOS connection issue view outside the scrollable session List so its startup insertion/removal cannot alter the List's retained scroll offset. The iOS list behavior remains unchanged.

Verified with the macOS Xcode build.
