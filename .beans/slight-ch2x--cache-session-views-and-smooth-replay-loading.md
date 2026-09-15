---
# slight-ch2x
title: cache session views and smooth replay loading
status: completed
type: bug
priority: normal
created_at: 2026-09-13T13:13:41Z
updated_at: 2026-09-13T13:14:21Z
---

Switching sessions currently recreates view models, refetches replay history, flashes a requesting missed events banner, and repeatedly changes scroll position. Cache session view models and make attach/replay loading non-jittery.


## Tasks

- [x] Cache session view models by session ID
- [x] Keep attach loading from clearing visible content prematurely
- [x] Stop replay events from repeatedly moving the scroll position
- [x] Verify Apple build


## Summary of Changes

- Added an app-level SessionViewModel cache keyed by session ID, preserving transcripts and observation tasks across navigation.
- Cached view models receive refreshed session summaries without reattaching.
- Normal replay from sequence zero no longer displays the noisy missed-events banner.
- Replay failures now surface through ReplayState.
- Transcript auto-scroll is suppressed during replay and runs once after replay settles.
- Verified the macOS Xcode build succeeds.
