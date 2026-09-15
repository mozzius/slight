---
# slight-to9w
title: add aggregated recent working directories
status: completed
type: feature
priority: normal
created_at: 2026-09-15T05:18:41Z
updated_at: 2026-09-15T05:21:26Z
---

Expose a bounded list of recent working-directory paths aggregated across all harnesses. Reuse it in new-session and resume flows with picker selection plus manual entry.



## Summary of Changes

Added the read-scoped session.working_directories API with a server-side cap of 20 distinct paths ordered by recent activity. Added Apple client loading and dropdown menus in both new-session and resume flows while preserving manual path entry. Updated protocol docs, fake host support, and loopback coverage.
