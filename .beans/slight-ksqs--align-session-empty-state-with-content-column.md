---
# slight-ksqs
title: align session empty state with content column
status: completed
type: bug
priority: normal
created_at: 2026-09-15T15:44:56Z
updated_at: 2026-09-15T15:45:21Z
---

The empty state component in the session view is aligned to the wrong horizontal edge; align it with the session view's 800pt content column.



## Checklist

- [x] Align the session empty state with the 800pt content column
- [x] Run the Apple package test suite

## Summary of Changes

Added a leading full-width frame to the session transcript empty state so it aligns with the left edge of the constrained 800pt content column.
