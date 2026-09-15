# ADR 0006: Session persistence and restart recovery

Status: accepted

## Context

An ACP agent runs as a child process of the Rust host. When the host stops, the
agent process dies with it, so a session that was `idle`, `working`, or
`waiting_permission` cannot survive a restart as a live process. The product
still needs to present those sessions after a restart and, where the agent
supports it, reconnect to the native agent session instead of losing context.

Two different kinds of history must not be confused:

- the agent's native session history, reachable through ACP
  (`session/list`, `session/load`, `session/resume`); and
- Slight's local bounded journal, which is what clients replay on attach.

The prior host slice persisted only a `SessionSummary` and, on restart, reported
every previously live session as `exited`. That silently discarded the native
session identity and gave clients no way to tell a genuinely finished session
from one the host simply could not restore.

## Decisions

### Persist a typed session record

`session-store` keeps `StoredSession.metadata` opaque JSON, so the durable shape
is owned by `session-core`. Each session is persisted as a `PersistedSession`:

- the existing `SessionSummary` (title, agent, model, status, sequence, …);
- the native `agent_session_id`;
- the resolved absolute working directory (the label is for display only); and
- the launch origin (`new`, `load`, or `resume`).

All new fields are `#[serde(default)]`. A record written before this change
still decodes; it simply has no native session id and is reported as
`unavailable`. No SQL migration is required because the metadata column is JSON.

### Rehydrate eagerly on startup, prefer `session/resume`

`SessionManager::new` iterates persisted records before the host serves clients.
For each one it looks up the owning adapter, spawns it, completes `initialize`,
and calls `session/resume` for the stored native id. Recovery deliberately never
calls `session/load`: the agent history was already normalized into the local
journal when the session was imported, and replaying it again would duplicate
events.

The existing local journal is loaded from `session-store` and installed into the
rehydrated session, so attach and history semantics are unchanged.

### Report explicit recovery state; never silently replace

Recovery outcomes are exposed on `SessionSummary.recovery` and therefore on the
gateway `SessionSummaryDto`:

- `live` — created on this host, not restored from disk;
- `recovered` — resumed successfully; the session is live again;
- `stale { reason }` — the agent rejected the native session (for example it was
  deleted);
- `unavailable { reason }` — the agent is not registered, could not launch, or
  does not advertise `session/resume`.

A failed recovery retains the local journal and marks the session `exited`. The
host reports the failure and never spawns a replacement session, so a client can
distinguish "the native session is gone" from "a fresh session exists".

## Consequences

- Restarting the host preserves native session identity and reconnects when the
  agent supports resume; otherwise clients see an explicit state.
- Recovery spawns one agent process per recoverable session during host startup.
  This is acceptable for the single-host MVP; a future slice can make recovery
  lazy or bounded if the session count grows.
- The recovery contract is additive within gateway-v1. Clients must treat a
  missing `recovery` as `live`.

## Remaining gaps

- Recovery does not retry once an agent later becomes available; the state is
  computed once at startup.
- `session/load`-only agents cannot be recovered; they report `unavailable`
  until a resume capability exists.
- Recovery does not re-establish per-client subscriptions, which are inherently
  connection-scoped and are recreated on attach.
