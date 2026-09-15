---
# slight-nzv2
title: sort and archive sessions
status: completed
type: feature
priority: normal
created_at: 2026-09-15T15:21:23Z
updated_at: 2026-09-15T15:26:12Z
---

Implement session list improvements:

- [x] Sort sessions by most recently used.
- [x] Allow archiving without deleting, via swipe action or peek interaction option.
- [x] Hide archived sessions from the main list and preserve them for future access.

## Summary of Changes

Added recency sorting across the Rust host and Apple client, persisted session archive state, added the session.archive gateway command, and added reversible swipe/context actions with an archived-session reveal.
