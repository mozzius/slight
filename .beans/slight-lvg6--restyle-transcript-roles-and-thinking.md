---
# slight-lvg6
title: restyle transcript roles and thinking
status: completed
type: feature
priority: normal
created_at: 2026-09-13T01:47:59Z
updated_at: 2026-09-13T01:59:20Z
---

Remove agent labels/bubbles and user labels/timestamps, right-align subtle user prompts with pending/sent styling, and render reasoning as a tappable Thinking duration control with a live popover.

## Summary of Changes

Restyled transcript roles: agent messages no longer render a bubble or Agent label; user messages are right-aligned subtle pending/sent pills without timestamps; reasoning is a tappable Thinking control with a live popover and elapsed display. Markdown remains supported with whitespace-preserving rendering and heading detection.

Verification: iOS Simulator `xcodebuild` succeeds.
