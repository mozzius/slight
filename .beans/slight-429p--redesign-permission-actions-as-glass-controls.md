---
# slight-429p
title: redesign permission actions as glass controls
status: completed
type: task
priority: normal
created_at: 2026-09-14T17:05:06Z
updated_at: 2026-09-14T17:05:52Z
parent: slight-y6g1
---

Replace the permission prompt's sprawling adaptive button grid with a polished Liquid Glass action layout, retaining semantic option colors, loading state, and accessibility.

## Summary of Changes

- Replaced the adaptive grid with a responsive horizontal-or-vertical action group.
- Added clear action icons and equal-width controls with better label wrapping.
- Added Liquid Glass styling for the prompt surface and permission buttons, with a material fallback on older OS versions.
- Preserved semantic tinting, loading state, and accessibility labels.
- Verified with swift test: 61 tests passed.
