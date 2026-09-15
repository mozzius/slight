---
# slight-2vxo
title: remove unused ListedSessionSummary import
status: completed
type: task
priority: normal
created_at: 2026-09-15T05:29:05Z
updated_at: 2026-09-15T05:29:49Z
---

Remove the compiler-reported unused ListedSessionSummary import without changing behavior.



## Summary of Changes

Removed the unused session-core re-export and imported ListedSessionSummary directly from acp-types in gateway-protocol.
