---
# slight-3bqs
title: align permission prompt width
status: completed
type: bug
priority: normal
created_at: 2026-09-14T17:03:03Z
updated_at: 2026-09-14T17:03:24Z
parent: slight-y6g1
---

Make the permission prompt match ComposerView's actual 800pt content width instead of exceeding it.

## Summary of Changes

- Applied horizontal padding before the 800pt frame so the permission panel’s total outer width matches ComposerView.
- Verified with swift test: 61 tests passed.
