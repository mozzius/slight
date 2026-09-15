---
# slight-54ld
title: remove duplicate starting session labels
status: completed
type: bug
priority: normal
created_at: 2026-09-14T17:18:55Z
updated_at: 2026-09-14T17:19:43Z
---

Remove both textual Starting session labels shown when sending the first message in a new session; retain the existing spinner.\n\n- [x] Locate both labels and the shared loading state\n- [x] Remove labels while preserving spinner behavior\n- [x] Verify with focused tests or build

## Summary of Changes\n\nRemoved the creation-state text from the new-session composer so the existing spinner is the only loading indicator. Preserved the disconnected-host message.\n\nVerification: `swift test` passed with 61 tests.
