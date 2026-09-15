---
# slight-8cyg
title: automate resume session discovery
status: completed
type: task
priority: normal
created_at: 2026-09-14T16:08:16Z
updated_at: 2026-09-14T16:08:35Z
parent: slight-y6g1
---

Make the Resume Session sheet discover agent sessions automatically for the selected harness, including on initial presentation and harness changes. Replace the harness picker with a segmented control and remove the manual Find Sessions action.

## Summary of Changes

- Changed harness selection to a segmented picker.
- Automatically discovers sessions when the sheet opens, when the catalog arrives, and when the harness changes.
- Removed the manual Find Sessions button and serialized selection during discovery.
- Verified with swift test: 61 tests passed.
