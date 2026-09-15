---
# slight-ftmj
title: use semantic toolbar actions
status: completed
type: task
priority: normal
created_at: 2026-09-12T22:02:03Z
updated_at: 2026-09-14T21:31:19Z
---

Replace manual Cancel/Done-style toolbar buttons in Apple SwiftUI views with semantic toolbar roles and placements so iOS 26 can adapt them correctly.



## Checklist

- [x] Move onboarding connection into the form flow
- [x] Hide connection status until connection is attempted
- [x] Use semantic cancel/checkmark toolbar actions and gate completion on connection
- [x] Verify Apple package tests



## Summary of Changes

- Moved onboarding connection into the form and deferred connection status until the first attempt.
- Added semantic cancel and confirm toolbar actions with the checkmark disabled until connected.
- Used ButtonRole.confirm on iOS/macOS 26 and retained the existing deployment-target fallback.
- Verified with 65 passing SlightKit tests.
