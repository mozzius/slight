---
# slight-wxpc
title: show tool call identity in fallback title
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:40:30Z
updated_at: 2026-09-14T21:41:03Z
---

The tool-call UI falls back to generic 'Tool result' when the normalized title is blank. Preserve and display the actual call identity, such as the tool kind or raw input command/name, so raw-only cards explain what ran.



- [x] Preserve the original tool title across partial updates.
- [x] Add regression coverage.
- [x] Run Apple package tests.

## Summary of Changes

Partial tool-call updates now retain the original title, kind, and detail when those fields are omitted. Added regression coverage and verified all Apple package tests.
