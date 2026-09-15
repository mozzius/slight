---
# slight-3ih4
title: stabilize bundled host connection startup
status: completed
type: bug
priority: normal
created_at: 2026-09-14T18:13:16Z
updated_at: 2026-09-14T19:26:35Z
---

Prevent the desktop client from connecting before the bundled acp-host is ready, and adopt an existing local daemon across app relaunches instead of starting a duplicate process.



- [x] Detect and adopt an already-listening independent daemon
- [x] Restart only the previously app-managed daemon after rebuild/relaunch
- [x] Wait for local host readiness before connecting the gateway
- [x] Surface startup timeout instead of a false connected state
- [x] Verify Swift tests and formatting

## Summary of Changes

- Added managed daemon PID tracking across app launches.
- App-managed daemons are replaced on rebuild; independently launched daemons are adopted.
- Added TCP readiness probing before gateway startup.
- Verified `swift test`: 64 tests passed.


- [x] Remove Swift concurrency warnings from readiness probing

## Warning Cleanup

- Replaced the concurrently mutated local callback state with a locked `Sendable` probe helper.
- `swift test` now passes with no compiler warnings.



## Connection Gate Fix

- `startIfNeeded()` now returns a success result.
- Local gateway connection is skipped when the bundled host cannot start or bind.
- Concurrent startup callers wait for the in-flight startup instead of racing the socket.
- `swift test` passes with 64 tests.



## Probe Timeout Fix

- Added a hard timeout to each local TCP readiness probe.
- Prevented `Host starting` from hanging when no listener exists and the network probe remains pending.
- Verified `swift test`: 64 tests passed.
