---
# slight-acp9
title: reorient sessions around workspaces
status: completed
type: feature
priority: normal
created_at: 2026-09-15T15:25:25Z
updated_at: 2026-09-15T15:28:47Z
---

Group sessions by working-directory workspaces. Workspaces are named from directory basename by default, can be renamed, and session creation selects an existing or new workspace plus a harness.



## Work

- [x] Aggregate workspace paths from Slight and native sessions
- [x] Add searchable native workspace picker to new-session flow
- [x] Verify Rust and Swift tests

## Summary of Changes

Added host workspace-path aggregation across Slight and available native harness sessions, updated the gateway contract and loopback coverage, and replaced inline directory selection with a searchable native workspace picker that supports selecting recent paths or entering a new one. Durable workspace names, rename semantics, and grouped session navigation are tracked separately in slight-7387.
