---
# slight-tewi
title: special-case iOS adaptive navigation
status: completed
type: task
priority: normal
created_at: 2026-09-15T20:07:35Z
updated_at: 2026-09-15T20:08:59Z
---

Use an iOS-specific navigation presentation: keep NavigationSplitView for regular-width iPadOS, but use the compact iPhone NavigationStack/list flow explicitly. Preserve shared session state and detail views without duplicating view models.

- [x] Add regular-width iPadOS split and compact iOS stack presentations
- [ ] Keep macOS split presentation unchanged
- [x] Build iOS and macOS targets

Supersedes the fully unified presentation in slight-kslc.

## Summary of Changes

Added an explicit iOS size-class presentation boundary. Regular-width iPadOS uses NavigationSplitView; compact iOS uses NavigationStack with the existing list/detail destinations. macOS remains on the split view. Both iPadOS simulator and macOS builds pass.
