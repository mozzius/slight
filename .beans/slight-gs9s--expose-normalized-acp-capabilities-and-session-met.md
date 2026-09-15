---
# slight-gs9s
title: expose normalized ACP capabilities and session metadata through gateway-v1
status: completed
type: task
priority: high
created_at: 2026-09-12T17:37:18Z
updated_at: 2026-09-12T19:12:06Z
parent: slight-4yk6
---

Audit the ACP implementation against the full ACP v1 contract and expose the currently missing/lossy normalized capability and metadata surface through gateway-v1 and the Apple protocol layer.

## Findings
- InitializeSummary drops agent title and auth methods; ClientCapabilities is not advertised.
- new_session returns only the agent session id; NewSessionResponse modes and config options are dropped.
- SessionUpdate::Plan, AvailableCommandsUpdate, CurrentModeUpdate, ConfigOptionUpdate, SessionInfoUpdate, UsageUpdate are silently dropped.
- Turn stop reasons are only surfaced as a debug diagnostic, not a typed event.
- Unknown agent notifications are downgraded to debug diagnostics; unknown requests get method_not_found.
- ContentBlock variants (image/audio/resource/resource_link) and tool-call content (diffs, terminals, raw IO, locations) are dropped; filesystem, terminal, elicitation, auth, session load/resume/fork/delete/close are not modeled.

## Scope
- Add normalized capability + metadata types in acp-types.
- Extend AcpSession::new_session to return SessionMetadata (agent session id + modes).
- Normalize plan, mode, and available-command updates; add typed turn-ended event.
- Store negotiation metadata in session-core and expose it via session.inspect.
- Add gateway-v1 DTO/event surface + conformance fixtures.
- Add Apple decoding models + tests.
- Document deferred ACP surfaces.

## Todos
- [x] Extend acp-types with capability/metadata summaries and session metadata
- [x] Normalize plan, mode, available-commands, turn-ended events
- [x] Store and expose metadata in session-core
- [x] Expose acp metadata/events through gateway-protocol + gateway-server
- [x] Add conformance fixtures and Rust decode tests
- [x] Add Apple models, decoders, and tests
- [x] Update gateway-v1 docs and ADR gaps
- [x] Run cargo + swift tests

## Summary of Changes

- Confirmed normalized ACP capability and session metadata types flow from initialize/session-new through session-core and `session.inspect`.
- Confirmed typed plan, mode, available-command, and turn-ended events across ACP normalization and gateway-v1.
- Added Apple codec coverage for negotiated metadata and normalized event payloads.
- Updated the ACP ADR to distinguish completed normalized surfaces from deferred ACP features.
- `cargo test --workspace` and `swift test` pass.
