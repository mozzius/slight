---
# slight-aok3
title: clarify permission request actions
status: completed
type: bug
priority: normal
created_at: 2026-09-14T17:07:42Z
updated_at: 2026-09-14T17:09:49Z
parent: slight-y6g1
---

Show the requested permission/action details clearly in the permission prompt and normalize visible decision labels to Allow Once and Deny instead of adapter-specific Cancel wording.

## Summary of Changes

- Added an explicit “Permission requested” heading and retained the concrete action title.
- Propagated ACP raw tool input into the normalized permission detail so commands can show what will run.
- Normalized action labels to Allow Once, Always Allow, Deny, and Always Deny.
- Unified transcript-adjacent surfaces at an 824pt outer width so the composer and permission panel align with the main content.
- Verified with 61 Swift tests and the Rust ACP/session/gateway test suites.
