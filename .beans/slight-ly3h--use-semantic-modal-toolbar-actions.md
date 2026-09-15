---
# slight-ly3h
title: use semantic modal toolbar actions
status: completed
type: task
priority: normal
created_at: 2026-09-14T16:10:20Z
updated_at: 2026-09-14T16:10:32Z
parent: slight-y6g1
---

Ensure modal header actions use SwiftUI semantic button roles and adaptive toolbar placements instead of plain hardcoded cancellation buttons.

## Summary of Changes

- Made the resume sheet cancellation button use SwiftUI’s `.cancel` role alongside `.cancellationAction` placement.
- Confirmed other modal header actions already use adaptive toolbar placements.
- Verified with swift test: 61 tests passed.
