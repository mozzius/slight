---
# slight-s018
title: resume existing agent sessions
status: in-progress
type: feature
priority: normal
created_at: 2026-09-13T14:05:17Z
updated_at: 2026-09-15T15:36:59Z
---

Allow Slight to discover, import, and reconnect existing OpenCode, Codex, and Claude sessions through ACP. Preserve native agent session identity and distinguish agent resumption from replaying Slight's local journal.

Implementation should proceed in slices: ACP boundary, durable metadata/restart recovery, gateway discovery/import contract, adapter conformance, and native UI.



Implementation slices delegated and completed:
- slight-d9yd: ACP discovery/load/resume boundary
- slight-6f27: gateway discovery/import contract
- slight-v0aa: durable native-session recovery
- slight-9bhc: Apple discovery/import UI



Pagination follow-up: threaded ACP session/list cursors through the gateway and Apple resume UI so discovery loads one page at a time.

## Notes\n\nThe native-session UI now always imports with history replay. The reconnect-only choice is hidden; internal `session/resume` remains for host restart recovery.
