---
# slight-v0aa
title: persist native session identity for recovery
status: completed
type: task
priority: normal
created_at: 2026-09-13T14:05:25Z
updated_at: 2026-09-13T14:28:55Z
parent: slight-s018
blocked_by:
    - slight-d9yd
---

Persist the native agent session ID, resolved working directory, agent kind, launch configuration, and recovery state. Reconnect persisted sessions after host restart through native resume when supported; report explicit unavailable/stale state otherwise. Coordinate with the ACP boundary bean before implementing calls.

- [x] Define durable recovery metadata
- [x] Rehydrate/resume sessions after host restart
- [x] Handle stale or unavailable native sessions
- [x] Add persistence and restart tests

## Summary of Changes

Durable recovery for imported and newly-created native agent sessions.

- `session-core` now persists a typed `PersistedSession` record (in `session-store`'s opaque JSON metadata column): the `SessionSummary`, native `agent_session_id`, resolved absolute working directory, and launch origin (`new`/`load`/`resume`). The new fields are `#[serde(default)]`, so no SQL migration is needed.
- Added `SessionRecoveryState` (`live`/`recovered`/`stale{reason}`/`unavailable{reason}`) on `SessionSummary`, and `SessionManager::new` eagerly rehydrates every persisted session before serving: it looks up the owning adapter, spawns it, initializes, and calls ACP `session/resume`. Recovery never calls `session/load`, so the local journal is not duplicated.
- Failures are explicit: agent not registered / cannot launch / no resume capability -> `unavailable`; agent rejects the native session -> `stale`. In both cases the local journal is retained and replayable (attach/history unchanged), status is `exited`, and no replacement session is spawned.
- `session.inspect` now also works for non-live recovered sessions, returning the stored descriptor, native id, and journal.
- Gateway: `SessionSummaryDto` exposes `recovery` (additive, defaulted to `live`); re-exported `SessionRecoveryState`. Documented in `protocol/gateway-v1.md` and new `docs/architecture/adr-0006-session-recovery.md`; updated ADR-0002 remaining gaps.
- Fake agent now rejects `session/load`/`session/resume` for unknown native ids, matching real agents.

Tests:
- New `rust/session-core/tests/restart_recovery.rs`: created-session resume + journal preserved, imported-session resume + replayed journal preserved, missing-agent `unavailable` without replacement, missing native session `stale` without replacement (uses a flaky-resume adapter).
- New conformance fixture `protocol/conformance/session-list-recovery.json` + `gateway-protocol` decoding test for all four recovery states; existing fixtures default to `live`.
- `cargo fmt --all` clean; `cargo test --workspace` green (32 test binaries, 0 failures), including the real-OpenCode host restart test.
