---
# slight-qcai
title: match permission and composer surface edges
status: completed
type: bug
priority: normal
created_at: 2026-09-14T17:16:05Z
updated_at: 2026-09-14T17:16:25Z
parent: slight-y6g1
---

Match the permission card's visible Liquid Glass edges to ComposerView by using the same outer inset and max-width treatment.

## Summary of Changes

- Matched the permission card’s outer horizontal inset to ComposerView’s 16pt inset.
- This aligns the visible Liquid Glass surfaces while keeping the shared 824pt outer width.
- Verified with swift test: 61 tests passed.
