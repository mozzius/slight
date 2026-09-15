---
# slight-1gh1
title: replace session actions with plus menu
status: completed
type: task
priority: normal
created_at: 2026-09-14T16:06:38Z
updated_at: 2026-09-14T16:07:21Z
parent: slight-y6g1
---

Replace separate New Session and Import Session controls in the Apple session browser with one plus icon menu containing New Session and Resume Session actions. Preserve existing navigation and discovery/import behavior.

## Summary of Changes

- Replaced separate new/import controls with a single plus menu on iOS and macOS.
- Added New Session and Resume Session menu actions while preserving existing flows.
- Updated the resume sheet and action labels to use Resume Session terminology.
- Verified with swift test: 61 tests passed.
