---
# slight-bg6w
title: integrate real ACP agent adapters
status: todo
type: feature
priority: normal
created_at: 2026-09-12T08:24:46Z
updated_at: 2026-09-12T18:49:46Z
parent: slight-y65m
blocked_by:
    - slight-helr
    - slight-eyfw
---

Add the first production ACP adapters behind the conformant ACP v1 client boundary. Baseline support is Claude, Codex, and OpenCode, with no agent-specific conditionals in session-core or native clients.

- [ ] Choose and document the exact Claude ACP adapter and launch contract
- [ ] Integrate Codex through https://github.com/agentclientprotocol/codex-acp
- [ ] Integrate OpenCode through the opencode acp stdio server
- [ ] Implement executable discovery and sanitized launch policy
- [ ] Translate ACP capabilities, permissions, updates, and failures
- [ ] Handle authentication and process lifecycle per adapter
- [ ] Add adapter fixtures and end-to-end tests

## Boundary

Each agent is an ACP stdio server from the host's perspective. The Rust host owns process supervision and normalization; clients consume only the versioned Slight gateway protocol.
