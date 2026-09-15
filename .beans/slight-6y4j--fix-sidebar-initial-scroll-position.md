---
# slight-6y4j
title: fix sidebar initial scroll position
status: completed
type: bug
priority: normal
created_at: 2026-09-14T20:44:23Z
updated_at: 2026-09-14T20:45:18Z
---

Investigate and fix the sidebar content starting at the wrong scroll position on app startup, likely caused by content above the scroll area.

## Summary of Changes

Added a top default scroll anchor to the macOS sidebar session list so startup layout changes from the header and search field do not leave the list at an unintended offset.

Verified with the macOS Xcode build.
