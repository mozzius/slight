---
# slight-ukwb
title: keep transcript scroll content full height
status: completed
type: bug
priority: normal
created_at: 2026-09-13T13:03:07Z
updated_at: 2026-09-13T13:03:25Z
---

Prevent the main transcript ScrollView content from collapsing when the transcript is empty or short; content should have a minimum height equal to the scroll view viewport.


## Summary of Changes

- Added a viewport GeometryReader around the transcript ScrollView.
- Applied the viewport height as the LazyVStack content minHeight while retaining the centered 800pt width cap.
- Verified the macOS Xcode build succeeds.
