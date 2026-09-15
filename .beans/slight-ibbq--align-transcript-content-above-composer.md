---
# slight-ibbq
title: align transcript content above composer
status: completed
type: bug
priority: normal
created_at: 2026-09-13T13:19:19Z
updated_at: 2026-09-13T13:20:32Z
---

The transcript viewport sizing still includes the full outer height plus the composer inset. Long-session initial loading can land mid-transcript or below the overlaid composer. Correct the content minHeight/inset relationship and stabilize the post-replay scroll.


## Tasks

- [x] Correct transcript minHeight relative to composer inset
- [x] Stabilize initial bottom placement after replay
- [x] Verify Apple build


## Summary of Changes

- Applied the composer bottom inset as scroll content margin while subtracting it from the transcript minimum height.
- Short transcripts now fill exactly the available area above the composer instead of extending below it.
- Post-replay bottom scrolling waits for additional layout yields so long initial transcripts land at the actual bottom.
- Verified the macOS Xcode build succeeds.


## Follow-up

- [x] Replace the hardcoded composer inset with a dynamic safe-area inset


## Correction

Replaced the fixed composer inset with `safeAreaInset(edge: .bottom)`, so the scroll viewport automatically accounts for the actual composer, permission prompt, and error heights. macOS build reverified.
