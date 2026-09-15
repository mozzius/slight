---
# slight-l0ki
title: investigate sessions showing exited
status: completed
type: bug
priority: normal
created_at: 2026-09-14T20:49:50Z
updated_at: 2026-09-14T20:55:48Z
---

Determine why all persisted sessions are shown as exited and reject input with the agent unavailable message. Reproduce or trace host lifecycle, session persistence, and client status mapping.\n\n- [x] Identify the code path that marks sessions exited or unavailable\n- [x] Determine the immediate root cause from current implementation/state\n- [x] Record findings and any recommended fix

\n- [x] Surface host startup/agent availability errors in the native app

## Summary of Changes\n\n- Confirmed host startup cannot locate the configured real-agent executables under the Xcode-launched PATH, so persisted sessions are intentionally recovered as exited/unavailable.\n- Surfaced a host availability error in the native session browser and host settings when the connected host reports no available agent executables.\n- Added a Swift regression test for the unavailable-agent catalog.
