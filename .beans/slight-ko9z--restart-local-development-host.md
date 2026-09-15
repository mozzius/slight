---
# slight-ko9z
title: restart local development host
status: completed
type: task
priority: normal
created_at: 2026-09-14T16:29:14Z
updated_at: 2026-09-14T16:29:40Z
---

Restart the local development host on 127.0.0.1:8787 and verify the gateway responds.

## Summary of Changes

- Confirmed the previous host was stopped and refusing connections.
- Started `acp-host serve` on `127.0.0.1:8787` in development mode.
- Verified the host is running with 5 sessions and supports OpenCode, Claude Code, and Codex.
