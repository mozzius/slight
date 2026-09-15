---
# slight-9ag8
title: connect composer to draft scroll edge
status: completed
type: feature
priority: normal
created_at: 2026-09-15T15:12:54Z
updated_at: 2026-09-15T15:14:28Z
---

Make the new-session ComposerView participate in the main ScrollView safe-area boundary and add the same native bottom scroll-edge effect used by the transcript screen. Preserve layout and older OS behavior.\n\n- [x] Replace the draft screen overlay composer boundary with safeAreaInset\n- [x] Apply the availability-safe bottom scroll-edge effect\n- [x] Verify the Apple package build

## Summary of Changes\n\nConnected the new-session composer to the main scroll view with a bottom safe-area inset and reused the availability-safe soft bottom scroll-edge effect. Verified with `swift test` (66 tests passed).
