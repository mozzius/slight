---
# slight-dmvg
title: remove redundant draft cancel action
status: completed
type: task
priority: normal
created_at: 2026-09-12T21:58:08Z
updated_at: 2026-09-12T22:47:47Z
---

Remove the draft session's Cancel toolbar button because navigation/back already dismisses the draft surface. Preserve the composer stop action for active agent turns.

## Summary of Changes

Removed the redundant draft Cancel toolbar action. Navigation/back remains the dismissal mechanism, while the composer retains its Stop action for active agent turns.

Verification: iOS Simulator `xcodebuild` succeeds.
