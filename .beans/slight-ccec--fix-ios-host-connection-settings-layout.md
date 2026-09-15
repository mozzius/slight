---
# slight-ccec
title: fix ios host connection settings layout
status: completed
type: bug
priority: normal
created_at: 2026-09-14T17:50:02Z
updated_at: 2026-09-14T17:50:23Z
---

Improve the iOS host settings connection status layout without changing macOS: remove excess spacing, avoid side-by-side Connect/Disconnect buttons, and reduce mobile information density.



- [x] Split iOS and macOS connection status layouts
- [x] Replace iOS button row with one full-width contextual action
- [x] Reduce redundant mobile status content and spacing
- [x] Verify Apple package tests

## Summary of Changes

- Kept the macOS connection status layout unchanged.
- Simplified iOS to a vertical status block with one contextual Connect or Disconnect action.
- Removed the redundant endpoint row from the mobile status section and tightened spacing.
- Verified `swift test`: 64 tests passed.
