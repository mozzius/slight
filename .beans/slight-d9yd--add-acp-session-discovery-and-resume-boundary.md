---
# slight-d9yd
title: add ACP session discovery and resume boundary
status: completed
type: task
priority: normal
created_at: 2026-09-13T14:05:25Z
updated_at: 2026-09-13T14:13:57Z
parent: slight-s018
---

Extend the normalized ACP types and session adapter boundary with negotiated session/list, session/load, and session/resume support. Preserve replayed history as normalized events and add fake-agent coverage. Do not add gateway or UI changes in this slice.

- [x] Add typed ACP requests/responses and capability handling
- [x] Extend AcpSession and worker implementation
- [x] Add fake/session-core coverage
- [x] Run Rust tests

## Summary of Changes

- Extended the `acp-types` wire layer with typed `session/list`, `session/load`, and `session/resume` request/response re-exports plus encoders.
- Added negotiated capability gating in `AcpClient` (stored `agentCapabilities`, new `UnsupportedCapability` error) and typed `list_sessions`/`load_session`/`resume_session` helpers that remember loaded sessions.
- Extended the `AcpSession` boundary and the `ClientSession` worker with list/load/resume; `session/load` replay flows through the existing normalizing handler and surfaces as `AgentEvent`s. `StdioSession` and `LegacyAcpSession` updated.
- Added `ListedSessionSummary` normalization in `acp-types::metadata`.
- The in-memory fake and the wire-level fake agent now advertise and implement list/load/resume, including replayed `session/update` history.
- Added `SessionManager::list_agent_sessions` and `SessionManager::import_session` (`load` vs `resume`), sharing one open path with `create_session`.
- Updated ADR-0002 to record the negotiated discovery/resume boundary.
- Tests: `acp-types` capability/metadata units, `acp-adapters` wire round-trip, new `session-core` `recovery.rs`, and wire-session import-replay coverage. `cargo test --workspace` is green.
