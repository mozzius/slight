---
# slight-bnyv
title: clear stale permission prompts
status: completed
type: bug
priority: normal
created_at: 2026-09-14T17:18:03Z
updated_at: 2026-09-14T17:20:03Z
parent: slight-y6g1
---

Handle permission response errors caused by a stale replayed permission request with no live host pending permission, clearing or refreshing the prompt instead of leaving the session stuck.

## Summary of Changes

- Made `session.inspect` authoritative for pending permissions during snapshot loading, clearing replayed permission requests when the host has no live pending request.
- A stale permission response now removes the stale prompt and refreshes the session instead of leaving it stuck.
- Read host diagnostics: session `8705e250-ba7c-47e6-afc6-69d1b136710f` is idle with no host-side permission log; the stale UI request was replayed history.
- Verified with 61 Swift tests and Rust session/gateway tests.
