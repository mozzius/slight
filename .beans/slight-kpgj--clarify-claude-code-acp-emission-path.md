---
# slight-kpgj
title: clarify Claude Code ACP emission path
status: completed
type: task
priority: normal
created_at: 2026-09-13T03:14:23Z
updated_at: 2026-09-13T03:14:35Z
---

Answer whether Claude Code natively emits ACP and what Slight should launch for Claude-backed ACP sessions.\n\n- [x] Establish current native versus adapter status\n- [x] Explain required process and protocol boundary\n- [x] Give concrete recommendation for Slight

## Summary of Changes\n\nConfirmed that Claude Code's CLI is not the process Slight should treat as an ACP server. The current practical path is the Agent Client Protocol adapter built on the official Claude Agent SDK: launch claude-agent-acp over stdio and keep Slight's Rust ACP client boundary unchanged. Native ACP would require Anthropic to add an ACP JSON-RPC server mode to the Claude Code CLI itself.
