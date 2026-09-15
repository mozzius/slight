---
# slight-u2nh
title: make composer scroll edge visible
status: scrapped
type: bug
priority: normal
created_at: 2026-09-15T15:15:59Z
updated_at: 2026-09-15T15:17:42Z
---

The native scrollEdgeEffectStyle changes do not produce a visible interaction between the main scroll views and ComposerView. Replace the non-visible-only behavior with a reliable boundary treatment that responds as content scrolls toward the composer on both session detail and new-session screens.\n\n- [x] Identify why the current native edge effect is not visible\n- [x] Implement a visible scroll-linked composer boundary effect on both screens\n- [x] Verify the Apple package build and tests

## Summary of Changes\n\nAdded a cross-platform visible gradient boundary between the main scroll views and ComposerView on session detail and new-session screens. Retained the native iOS/macOS 26 scroll edge effect where available. Verified with `swift test` (66 tests passed).

## Reasons for Scrapping\n\nThe fallback gradient was based on an incorrect availability diagnosis. The target runtime is iOS 26.5, so the native scroll edge effect remains the intended implementation.
