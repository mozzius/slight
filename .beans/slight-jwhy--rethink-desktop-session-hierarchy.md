---
# slight-jwhy
title: rethink desktop session hierarchy
status: completed
type: task
priority: normal
created_at: 2026-09-12T17:25:59Z
updated_at: 2026-09-12T17:26:50Z
---

Work out a desktop-first navigation hierarchy: sidebar sessions with search, compact connected-host affordance, host settings formsheet, and a clear session detail header exposing model/effort metadata without duplicating host status.

## Proposed Direction

Use a desktop-first `NavigationSplitView`: the sidebar owns session discovery and a compact host control; the detail column owns one session and its agent configuration. On iOS, the same hierarchy collapses to navigation push.

- Sidebar: search, session list, new-session action, compact host connection toolbar item
- Connected host: show only a small status dot/name in the toolbar; open host settings in a sheet/formsheet
- Session row: title, working-directory label, agent/model summary, status, and relative last activity
- Session detail header: title plus agent/model/effort metadata; no repeated host status bar
- Transcript: primary content area
- Composer: primary input at the bottom, with cancel/permission state integrated above it
- Session controls: model and effort as explicit session-scoped controls, not host settings

Protocol note: `SessionSummary` currently has agent, working directory, status, and activity timestamps, but not model or effort. Those should be added to the session contract before the UI presents them rather than inferred from agent names.

## Summary of Changes

Established the desktop session hierarchy and separated host concerns from session concerns. This is ready to turn into a focused NavigationSplitView and session metadata follow-up.
