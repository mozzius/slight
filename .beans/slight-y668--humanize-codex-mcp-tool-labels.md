---
# slight-y668
title: humanize Codex MCP tool labels
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:43:55Z
updated_at: 2026-09-14T21:44:57Z
---

Codex tool calls currently render identifiers such as mcp.cua_repl.js instead of the human-readable action label, e.g. Called Find the Weather app or Called Open Weather.



- [x] Prefer human-readable action metadata for MCP calls.
- [x] Render Codex-style Called labels with a safe fallback.
- [x] Run Apple package tests.

## Summary of Changes

MCP tool rows now prefer action-like raw input fields and display Codex-style `Called ...` labels instead of exposing implementation names such as `mcp.cua_repl.js`.
