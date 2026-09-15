---
# slight-6f27
title: add gateway discovery and import contract
status: completed
type: task
priority: normal
created_at: 2026-09-13T14:05:25Z
updated_at: 2026-09-13T14:21:48Z
parent: slight-s018
blocked_by:
    - slight-d9yd
---

Add gateway-v1 commands and DTOs for discovering agent-owned sessions and importing one into Slight. Keep discovery scoped by agent and working directory, expose native IDs safely, and distinguish import/resume from local journal replay. Coordinate with the ACP boundary bean.

- [x] Define protocol commands, params, results, and errors
- [ ] Implement server dispatch and authorization
- [ ] Add conformance fixtures and Rust tests
- [x] Document the contract

## Summary of Changes

- Added gateway-v1 commands `agent.sessions.list` (read scope) and `agent.sessions.import` (create scope), plus typed params/results (`AgentSessionsListParams`, `AgentSessionsListResult`, `ImportAgentSessionParams`, `ImportAgentSessionResult`) and the `AgentSessionSummaryDto` that exposes native `agent_session_id` only through this discovery/import contract.
- Added a stable `unsupported_capability` error code and `ProtocolError::UnsupportedCapability`; server dispatch maps `SessionError::UnsupportedCapability` through `session_error`.
- `gateway-server` dispatch resolves the working-directory label via `session_core::resolve_working_directory`, lists agent sessions, and imports one; authorization routes discovery to `Scope::ReadSessions` and import to `Scope::CreateSession`.
- `session-core` now raises typed capability errors: `SessionManager::list_agent_sessions` checks `sessionCapabilities.list`, and the shared `open_session` path (refactored to `OpenKind::{New,Load,Resume}`) checks `loadSession` / `sessionCapabilities.resume` before calling the ACP boundary. Exported `resolve_working_directory`.
- Added conformance fixtures `agent-sessions-list-command.json`, `agent-session-import-command.json`, `agent-sessions-list-ack.json`, `agent-session-import-ack.json`, `agent-session-import-unsupported.json` and decoding tests in `gateway-protocol/tests/conformance.rs`.
- Added `session-core/tests/recovery.rs` capability-gating tests using a legacy-only adapter, a `gateway-server` unit test for the error mapping, and loopback tests for discovery/import dispatch plus read-vs-create authorization and unknown-agent handling.
- Documented the commands, `AgentSessionSummary`, import-vs-local-replay distinction, and `unsupported_capability` in `protocol/gateway-v1.md`; updated ADR-0002's remaining-gaps note.
- Scope intentionally excludes durable restart recovery and Apple UI (tracked by sibling beans). `cargo fmt` clean and `cargo test --workspace` green.
