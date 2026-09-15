---
# slight-mi5t
title: polish onboarding and host removal
status: completed
type: bug
priority: normal
created_at: 2026-09-14T17:53:11Z
updated_at: 2026-09-14T17:53:37Z
---

Refine onboarding visuals with a subtle forest-green gradient and tree/plant icon, and ensure Remove Host closes the macOS host configuration window.



- [x] Restyle onboarding with subtle forest-green gradient and tree icon
- [x] Hide the macOS main window title bar for onboarding presentation
- [x] Close the host configuration window after removal
- [x] Verify Apple package tests

## Summary of Changes

- Replaced the onboarding purple gradient and bolt icon with a subtle forest-green treatment and tree icon.
- Applied the hidden title bar style to the macOS main window.
- Explicitly close the macOS host configuration window after Remove Host; iOS continues to dismiss its sheet.
- Verified `swift test`: 64 tests passed.
