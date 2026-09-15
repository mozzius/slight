---
# slight-cdke
title: restore macos session navigation title
status: completed
type: bug
priority: normal
created_at: 2026-09-14T20:44:55Z
updated_at: 2026-09-14T20:53:46Z
---

The macOS session detail screen no longer visibly presents its navigation title. Restore the title in the desktop navigation surface and verify the Apple client builds.

## Summary of Changes

Added explicit macOS navigation titles at the NavigationSplitView detail boundary for selected sessions and the new-session screen. Verified with `swift test`: 64 tests passed.


Follow-up: the split-view navigation metadata was not visually rendered on macOS; add an explicit visible toolbar title.


Added an explicit macOS principal toolbar title after confirming the navigation metadata alone was not visible. `swift test` still passes all 64 tests.


Replaced the explicit toolbar text with a NavigationStack around the macOS detail column, preserving `.navigationTitle` as the source of the title. `swift test` passes all 64 tests.


Removed the hidden macOS title-bar scene style so the system can display the existing `.navigationTitle` instead of suppressing its native navigation/title surface.


Final direction: apply the hidden title-bar style only while showing the onboarding splash, and remove the session-screen navigation experiments.


Removed the duplicate macOS detail titles and nested NavigationStack. Kept the session screen on its original `.navigationTitle`, and limited splash-only chrome hiding to the onboarding view. The macOS app build succeeds.
