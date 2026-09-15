---
# slight-b78u
title: fix permission mode and prompt layout
status: completed
type: bug
priority: normal
created_at: 2026-09-14T16:59:26Z
updated_at: 2026-09-14T17:01:52Z
parent: slight-y6g1
---

Make the Allow for me permission mode avoid unnecessary permission prompts and constrain the permission prompt to the established 800pt content width.

## Summary of Changes

- Constrained the permission prompt safe-area panel to the shared 800pt content width.
- Verified that “Approve for me” maps to Codex `agent` / `auto_review`, which intentionally still prompts for potentially unsafe actions; `agent-full-access` is the no-prompt mode.
- Swift UI tests and Rust session/gateway tests pass.
