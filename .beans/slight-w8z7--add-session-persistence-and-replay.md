---
# slight-w8z7
title: add session persistence and replay
status: completed
type: feature
priority: normal
created_at: 2026-09-12T08:24:46Z
updated_at: 2026-09-12T18:50:42Z
parent: slight-y65m
blocked_by:
    - slight-helr
---

Build session-store behind a crate interface, initially backed by SQLite. Persist session metadata, status, launch information, protocol version, durable event sequence, and a bounded event journal. Support reconnect replay, retention limits, resync-required responses, migrations, and restart recovery states.\n\n- [x] Define storage interface and schema\n- [x] Add SQLite migrations\n- [x] Persist sessions and sequenced events\n- [x] Implement bounded replay and resync required\n- [x] Test restart and migration behavior

## Summary of Changes

- Added SQLite-backed `SessionStore` with schema initialization and migration version tracking.
- Persisted session metadata and sequenced JSON events with bounded retention.
- Added replay, latest-sequence, in-memory, and on-disk restart coverage.
- Verified the complete Rust workspace with `cargo test --workspace`.
