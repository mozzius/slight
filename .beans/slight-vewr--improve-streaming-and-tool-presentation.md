---
# slight-vewr
title: improve streaming and tool presentation
status: completed
type: feature
priority: normal
created_at: 2026-09-12T19:11:51Z
updated_at: 2026-09-14T21:40:02Z
---

Aggregate streamed assistant chunks into one live message instead of separate bubbles, animate incremental updates, render assistant markdown, and improve tool-call visualization in the session transcript.



- [x] Replace expanded raw-result disclosure with click-to-open detail.
- [x] Give raw-only tool calls a compact visible summary.
- [x] Verify Apple package tests.

## Summary of Changes

Replaced the inline Raw result disclosure with a click-to-open bounded popover, added a Tool result fallback for blank titles, and verified the Apple package test suite.
