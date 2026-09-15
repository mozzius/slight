---
# slight-gada
title: add bottom transcript scroll edge effect
status: completed
type: feature
priority: normal
created_at: 2026-09-15T05:31:37Z
updated_at: 2026-09-15T05:32:23Z
---

Add a native bottom scroll-edge effect to the transcript boundary above the ComposerView, preserving the existing bottom composer layout.



- [x] Add an availability-safe bottom scroll-edge effect to the transcript above ComposerView
- [x] Verify the Apple package build

## Summary of Changes

Added the native soft bottom scroll-edge effect on iOS 26 and macOS 26+, with no-op behavior on older supported OS versions.
