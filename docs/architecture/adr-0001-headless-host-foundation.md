# ADR 0001: Headless host foundation and loopback gateway

Status: accepted (MVP)

## Context

The repository needs a buildable Rust host foundation before native clients can
integrate. The riskiest behavior is ACP session lifecycle, normalized events,
reconnect/replay, and secure remote control. None of that requires a polished UI
or a real ACP agent, so the first slice proves the core with a fake in-memory
agent and a loopback listener.

## Decisions

### Workspace and crate boundaries

The workspace under `rust/` follows the ownership boundaries in `AGENTS.md`:

- `acp-types` — normalized ACP domain types and codecs.
- `acp-adapters` — adapter trait, launch configuration, registry, and a fake
  in-memory agent used for development and tests.
- `session-store` — persistence trait plus an in-memory implementation.
- `session-core` — session state machine, bounded journal, replay, and
  broadcast subscriptions; independent of any network listener.
- `gateway-protocol` — versioned frame/command/event/admin wire types and
  conformance fixtures.
- `gateway-server` — loopback listener, authentication and admin traits, and a
  blocking client used by the CLI and tests.
- `host-service` — composition root: config, lifecycle, pairing/revocation,
  diagnostics, and the pump thread.
- `host-cli` — the `acp-host` binary with `serve`, `status`, `sessions`,
  `diagnostics`, `pair`, `pairings`, `devices`, and `revoke`.
- `test-support` — shared fixtures and helpers.

`session-core` depends on `session-store`, and `gateway-protocol` embeds the
normalized session model, so there is no duplicate domain model between the
state machine and the wire contract.

### Transport

The initial transport was newline-delimited JSON over TCP bound to loopback: it
required no async runtime, kept `session-core` synchronous and deterministic,
and was trivial to exercise from integration tests. The `Frame` union was kept
transport-independent for exactly this reason.

Superseded by ADR 0003: the same `Frame` union now travels over WebSocket with
JSON payloads, matching the `URLSessionWebSocketTask` clients. The state
machine, auth hooks, and command/event vocabulary are unchanged.

### State machine and event delivery

`SessionManager` owns all sessions behind a mutex. A background pump thread in
`host-service` drains adapter events into the journal. Each `session.attach`
registers an `mpsc` subscription; the gateway connection drains those receivers
and forwards events. Sequences are monotonic per session, and the journal is
bounded, which produces an explicit `resync_required` result when history has
been evicted.

### Persistence

`SessionStore` is a trait with an in-memory implementation. `SessionManager`
writes session metadata and each event through the trait, so adding a SQLite
backend later does not change session-core or the clients.

### Authentication, pairing, and admin boundary

The Rust host remains canonical. The gateway exposes an `Authenticator` trait
and a `HostAdmin` trait. `host-service` implements both: development
credentials for loopback, one-time pairing codes that mint per-device tokens,
device listing/revocation, host status, lifecycle hooks, diagnostics, and
session inspection. `host.stop` requests an orderly shutdown; `host.start` and
`host.restart` are reported as hooks requiring an external supervisor.

## Consequences

- The host is usable without SwiftUI, Xcode, or a GUI login session.
- Tests cover the fake agent, state transitions, replay/resync, the loopback
  session flow, and the full host pairing flow.
- The wire contract is close to the provisional Apple client boundary, but the
  Apple types must still be reconciled against `protocol/gateway-v1.md`.

## Deferred

- TLS (wss://) and Tailscale interface discovery.
- SQLite-backed `session-store` and restart recovery.
- Real Claude Code and Codex adapters.
- Persisted pairings, server-initiated host status events, and client-side
  heartbeat timeout/reconnect policy.
- Idempotency bookkeeping for replayed command request IDs.
