---
# slight-qi0b
title: extract nested MCP action labels
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:45:26Z
updated_at: 2026-09-14T21:45:56Z
---

The MCP label fallback still displays Called Cua repl because Codex action metadata is nested or uses a different argument key. Extract the natural-language action from nested raw input without exposing the MCP implementation name.



- [x] Extract nested and alternate MCP argument labels.
- [x] Avoid implementation-name fallback where possible.
- [x] Run Apple package tests.

## Summary of Changes

MCP action extraction now traverses nested objects and arrays, recognizes additional natural-language argument keys, and ignores obvious code payloads in favor of comments or human-readable text.
