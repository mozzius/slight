---
# slight-9bhc
title: add native session discovery and import UI
status: completed
type: task
priority: normal
created_at: 2026-09-13T14:05:25Z
updated_at: 2026-09-13T14:34:19Z
parent: slight-s018
blocked_by:
    - slight-6f27
    - slight-v0aa
---

Expose discovery/import/resume in the Apple client after the gateway contract exists. Show agent, native session identity, cwd, title, timestamp, unavailable states, and preserve existing local replay behavior.

- [x] Add protocol client models and commands
- [x] Add discovery/import UI states
- [x] Add view model tests
- [x] Run Swift tests

## Summary of Changes

Apple client support for discovering, importing, and recovering native agent sessions.

- Protocol models (`SlightGateway/Protocol/AgentSessionModels.swift`): `AgentSessionSummary` (native id, cwd, additional dirs, title, updatedAt), `SessionRecoveryState` (live/recovered/stale/unavailable with reason; tolerant decode defaults to live), `SessionRecovery` (load/resume import mode), typed `AgentSessionsListParams`/`Result` and `ImportAgentSessionParams`/`Result`. `SessionSummary` gained `recovery` (missing decodes to live) and `GatewayCommandName` gained `.agentSessionsList`/`.agentSessionImport`.
- View model state (`SessionDiscoveryState` + `SessionListViewModel`): discovery state machine, `discoverAgentSessions(agent:workingDirectory:)`, `importAgentSession(_:recovery:...)` which upserts the imported session and passes the native identity plus the authoritative discovered cwd, and `clearAgentSessionDiscovery()`. Discovery/import never touch the local journal, preserving attach/replay.
- UI: `ImportSessionSheet` (agent picker, working-directory field, recovery mode, discovery list showing agent, native session id, cwd, title, timestamp, additional dirs, and import progress), toolbar/sidebar entry points, `SessionRecoveryBadge` on session rows, a recovery warning banner in `SessionMetadataView`, and a recovery-aware composer disabled reason. Fake host answers the new commands and seeds stale/unavailable fixtures for previews.
- Tests: conformance decoding for `agent-sessions-list-*`, `agent-session-import-*`, `session-list-recovery`, plus param snake_case encoding and legacy-summary default; new `SessionListDiscoveryTests` covering discovery load/sort/params/failure and import params/failure while asserting local sessions are preserved.
- No Rust or gateway-protocol changes were needed. `swift build` clean; `swift test` green (59 tests, 0 failures).
