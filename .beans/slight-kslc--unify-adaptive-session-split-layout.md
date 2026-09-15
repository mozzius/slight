---
# slight-kslc
title: unify adaptive session split layout
status: completed
type: task
priority: normal
created_at: 2026-09-15T20:05:54Z
updated_at: 2026-09-15T20:06:47Z
---

Use one SwiftUI NavigationSplitView for the shared Apple session browser so iPadOS and macOS get a split layout when space permits while iPhone uses the collapsed navigation presentation.

- [x] Unify SessionBrowserView around shared sidebar/detail columns
- [x] Remove the root NavigationStack wrapper that blocks adaptive ownership
- [x] Build and verify the Apple client

Related feature: slight-y6g1

## Summary of Changes

SessionBrowserView now owns one NavigationSplitView on iOS and macOS. The root no longer wraps it in a NavigationStack, so SwiftUI can present split columns on iPadOS/macOS and collapse them on compact layouts. Verified macOS and iPadOS simulator builds.
