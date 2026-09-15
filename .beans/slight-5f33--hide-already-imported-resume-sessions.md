---
# slight-5f33
title: hide already imported resume sessions
status: completed
type: bug
priority: normal
created_at: 2026-09-15T16:05:08Z
updated_at: 2026-09-15T16:06:26Z
---

The Resume UI shows native agent sessions that are already imported into Slight, including archived product sessions. Selecting one calls session/load again and can produce Codex JSON-RPC -32603. Filter discovered sessions by imported native identity.

## Summary of Changes\n\n- Filter `agent.sessions.list` results against native session identities already imported by Slight.\n- Archived Slight sessions remain part of the ownership set, so they no longer reappear in Resume and cannot be opened a second time.\n- Added gateway regression coverage for the filtered discovery result.
