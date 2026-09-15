---
# slight-rdtj
title: start local development host
status: completed
type: task
priority: normal
created_at: 2026-09-14T17:27:16Z
updated_at: 2026-09-14T17:27:58Z
---

Start the real acp-host development server on 127.0.0.1:8787 and verify its gateway responds.

- [x] Start acp-host serve
- [x] Verify host status and listener

## Summary of Changes

Started the real `acp-host serve` process on `127.0.0.1:8787` in development mode. Verified the listener and `acp-host status --json`: the host is running with 5 sessions and supports OpenCode, Claude Code, and Codex.
