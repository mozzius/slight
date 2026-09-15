---
# slight-7387
title: add durable workspace names and grouped session navigation
status: completed
type: feature
priority: normal
created_at: 2026-09-15T15:28:43Z
updated_at: 2026-09-15T15:44:34Z
---

Persist workspace identities and user-renamed display names, expose workspace rename through gateway, and group the session browser by workspace.



## Work

- [x] Derive readable workspace display names from paths
- [x] Group session lists by workspace on iOS and macOS
- [x] Verify Swift tests



## Summary of Changes

Grouped iOS and macOS session lists by working-directory path, ordered by the existing session recency order. Workspace headers and row labels now use the final directory component, with Home and Root handled explicitly. Persistent user-defined workspace renaming remains tracked in slight-u904.
