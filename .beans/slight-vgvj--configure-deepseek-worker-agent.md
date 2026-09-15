---
# slight-vgvj
title: configure DeepSeek worker agent
status: completed
type: task
priority: normal
created_at: 2026-09-12T08:28:03Z
updated_at: 2026-09-12T08:37:41Z
---

Configure OpenCode so delegated worker subagents use DeepSeek, with a focused implementation prompt and safe permissions. Preserve the orchestrator as the primary agent and document restart requirements.

- [x] Inspect existing project and global OpenCode configuration
- [x] Add or update DeepSeek worker agent configuration
- [x] Validate configuration shape and model identifier
- [x] Verify the resulting agent is discoverable

## Summary of Changes

Configured the global OpenCode `general` subagent to use `opencode-go/deepseek-v4.1-flash`. Verified the resolved configuration and agent details with `opencode debug config` and `opencode debug agent general`.
