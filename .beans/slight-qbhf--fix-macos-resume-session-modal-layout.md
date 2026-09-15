---
# slight-qbhf
title: fix macOS resume session modal layout
status: completed
type: bug
priority: normal
created_at: 2026-09-15T15:33:50Z
updated_at: 2026-09-15T15:34:41Z
---

The macOS resume session modal is visually broken: session rows overflow/clash with the footer and Resume/Cancel controls. iOS remains correct. Inspect the shared/native Apple UI and make the macOS presentation contain the list and actions correctly.\n\n- [x] Inspect macOS and iOS resume modal implementations\n- [x] Fix macOS layout and preserve scrolling/actions\n- [x] Verify with focused tests or build

## Summary of Changes\n\n- Added a macOS-only bounded ScrollView for resume-session discovery results.\n- Replaced the macOS navigation toolbar cancellation action with a dedicated footer, preventing row/action overlap.\n- Preserved the existing iOS Form and toolbar presentation.\n- `swift test` passed with 66 tests.
