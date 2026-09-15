---
# slight-ffra
title: truncate tool call UI to one line
status: completed
type: bug
priority: normal
created_at: 2026-09-14T22:31:49Z
updated_at: 2026-09-14T22:32:37Z
---

Keep the visible tool-call summary to a single line while preserving full multiline command content, including curl commands.


- [x] Limit visible tool-call title and detail to one line.
- [x] Show full multiline details in the tool popover.
- [x] Run Apple package tests.

## Summary of Changes

Tool-call rows now keep titles and details to one line with middle truncation. Tapping a row opens the complete multiline detail and any raw output in the popover.
