---
# slight-kebf
title: fix app icon symbol scale and tint
status: completed
type: bug
priority: normal
created_at: 2026-09-14T22:35:42Z
updated_at: 2026-09-14T22:38:34Z
---

The app icon tree symbol renders black and oversized; make it match the white, smaller splash-screen symbol.

## Summary of Changes

Regenerated the `tree.fill` asset at a smaller 480-point footprint and forced its alpha mask to white, fixing the oversized black rendering. Verified macOS and iOS builds.
