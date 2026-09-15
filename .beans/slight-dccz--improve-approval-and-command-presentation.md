---
# slight-dccz
title: improve approval and command presentation
status: completed
type: feature
priority: normal
created_at: 2026-09-14T21:48:48Z
updated_at: 2026-09-14T21:49:22Z
---

Compact long tool commands to one line and improve the approval model UI so Guardian Review presents status, action, risk, authorization, and rationale clearly.



- [x] Render long approval actions as one truncated line.
- [x] Create structured Guardian Review approval presentation.
- [x] Preserve fallback rendering for unstructured permission details.
- [x] Run Apple package tests.

## Summary of Changes

Approval prompts now render structured Guardian Review fields with compact status badges, a one-line middle-truncated action, rationale, and decision controls. Long command text no longer expands the card vertically.
