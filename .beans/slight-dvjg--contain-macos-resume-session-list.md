---
# slight-dvjg
title: contain macOS resume session list
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:59:39Z
updated_at: 2026-09-14T22:00:23Z
---

The macOS resume-session sheet grows to the full height of the session list. Constrain it to a sensible contained height and make the session list scroll within the sheet.

## Summary of Changes

- Added macOS-only width and height bounds to the resume-session sheet.
- Kept the existing Form as the contained scrolling surface for long session lists.
- Verified with an Xcode macOS Debug build.
