---
# slight-7nk9
title: add bottom transcript breathing room
status: completed
type: task
priority: normal
created_at: 2026-09-13T14:14:14Z
updated_at: 2026-09-13T14:16:05Z
---

Ensure newly appended iOS transcript content does not sit directly against the composer; preserve the existing prepend anchor behavior.


## Summary of Changes

- Added 16pt bottom padding after transcript viewport sizing so newly appended content sits above the composer with visible breathing room.
- Preserved pagination and anchor behavior.
- Apple build passes.


## Follow-up

- [x] Scroll to a bottom spacer instead of aligning the last message edge


## Correction

Auto-scroll now targets a dedicated 16pt bottom spacer instead of the last message, so new content retains the intended gap above the composer. Apple build reverified.
