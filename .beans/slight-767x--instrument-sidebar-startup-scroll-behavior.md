---
# slight-767x
title: instrument sidebar startup scroll behavior
status: completed
type: task
priority: normal
created_at: 2026-09-14T20:45:47Z
updated_at: 2026-09-14T20:46:19Z
---

Add focused debug logging around the macOS sidebar list lifecycle and startup data transitions to identify why its initial scroll position is wrong.

## Summary of Changes

Added DEBUG-only sidebar instrumentation for lifecycle/state transitions and the first session row's vertical position in a named coordinate space. The logs include connection state, total/filtered session counts, connection issue visibility, search changes, and first-row minY.

Verified with the macOS Xcode build.
