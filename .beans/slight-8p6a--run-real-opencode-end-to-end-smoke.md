---
# slight-8p6a
title: run real OpenCode end-to-end smoke
status: in-progress
type: task
priority: normal
created_at: 2026-09-12T13:38:57Z
updated_at: 2026-09-12T16:46:10Z
parent: slight-dags
blocked_by:
    - slight-wmd7
    - slight-wfpm
    - slight-30sj
    - slight-8me6
---

Run the actual acp-host, connect the real Slight client over WebSocket plus JSON, launch or attach to opencode acp, create a session, send a prompt, observe streaming events, and verify cancellation/permission/error behavior. Add an automated smoke test and local runbook once the path works.

- [x] Run the actual acp-host
- [x] Connect the real Slight client over WebSocket plus JSON
- [x] Launch OpenCode through opencode acp
- [x] Create an OpenCode session from the client
- [x] Send a prompt and observe streamed output
- [ ] Verify cancellation, permission, and error behavior
- [ ] Add an automated end-to-end smoke test and local runbook

## Current State

Argent verified the real iOS Simulator client connected to `slight-host`, created an `opencode` session, submitted `Reply with exactly one word: pong`, and rendered the streamed response ending in `pong`. Remaining work is hardening the smoke path and covering negative/interactive cases.
