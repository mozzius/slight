# Gateway v1 contract

Status: draft, implemented by the Rust host in this repository. This document is
the canonical contract; the Swift types under `clients/apple` are provisional and
must be reconciled with the schemas and fixtures here (bean `slight-3ude`).

## Transport

The MVP transport is a single WebSocket connection per client carrying one
gateway frame per WebSocket message, encoded as UTF-8 JSON. The server performs
an RFC 6455 upgrade at any path (the canonical client uses `/`). The Rust host
uses the synchronous [`tungstenite`](https://crates.io/crates/tungstenite) crate
with its `handshake` feature; no async runtime is required.

`URLSessionWebSocketTask` sends JSON with `.data`, which arrives as a binary
WebSocket message. The server therefore accepts **both** text and binary
messages as long as the payload is UTF-8 JSON. Responses are always sent as text
messages. Invalid UTF-8 or non-JSON payloads produce an `error` frame with code
`malformed_frame` and do not close the connection.

Deferred, but planned without changing the frame types:

- WebSocket over TLS (`wss://`) on the Tailscale interface.
- Binary encoding behind the same `Frame` union.

Each frame is one JSON object in one WebSocket message. There is no newline
terminator: the message boundary delimits the frame. Unknown JSON object fields
are ignored so newer peers can add fields additively.

The loopback listener is intended for local development and the macOS host
boundary. The loopback CLI (`acp-host`) speaks the same WebSocket protocol.

### Liveness and close

- Protocol-level WebSocket `ping` frames are answered automatically with
  `pong` by the transport.
- Application-level `ping` / `pong` frames (see below) are also supported. The
  server sends an application `ping` every `heartbeat_interval_ms` reported in
  `welcome`; clients should reply with `pong` carrying the same nonce.
- A peer may initiate the WebSocket closing handshake at any time. The server
  echoes the close frame and then drops the connection. A connection that sends
  invalid framing receives an `error` frame rather than an abrupt close, so a
  client can recover or re-handshake.

## Versioning

- `PROTOCOL_VERSION` is `1` and is carried as `"v"` on every frame.
- A server that receives a `hello` with an unsupported `v` replies with an
  `error` frame whose code is `invalid_version`.
- Additive fields are allowed within a version. Removing or renaming a field
  requires a new version and a migration note.

## Frames

All frames are discriminated by `"type"`.

| type | direction | purpose |
| --- | --- | --- |
| `hello` | client -> server | handshake, credential, optional resume hint |
| `welcome` | server -> client | connection id, capabilities, resync hint |
| `command` | client -> server | a request with `request_id`, `command`, optional `session_id` and `params` |
| `ack` | server -> client | response to a `command`, with `ok`, `result` or `error` |
| `event` | server -> client | normalized session or host event |
| `ping` / `pong` | both | liveness |
| `resync_required` | server -> client | requested history is unavailable |
| `error` | server -> client | connection-level protocol error |

### Hello

```json
{
  "type": "hello",
  "v": 1,
  "client_id": "…",
  "client_name": "Slight",
  "client_version": "1.0.0",
  "device_id": "…",
  "credential": "…",
  "resume": { "session_id": "…", "last_event_sequence": 12 }
}
```

`credential` is a device token or a one-time pairing code. The host rejects
unauthenticated commands with an `ack` error code `unauthenticated`.

### Welcome

```json
{
  "type": "welcome",
  "v": 1,
  "connection_id": "…",
  "server_name": "slight-host",
  "server_version": "0.1.0",
  "host_id": "slight-local",
  "capabilities": {
    "supports_replay": true,
    "max_replay_events": 256,
    "max_event_journal": 1024,
    "supports_raw_acp": false,
    "permission_options": true,
    "host_admin": true
  },
  "heartbeat_interval_ms": 15000,
  "resync_required": false
}
```

Unknown capability keys are ignored.

### Command and ack

```json
{
  "type": "command",
  "v": 1,
  "request_id": "req-1",
  "command": "session.create",
  "session_id": null,
  "params": {
    "agent": "fake",
    "model": "auto",
    "effort": "medium",
    "working_directory_label": "~/work"
  }
}
```

```json
{
  "type": "ack",
  "v": 1,
  "request_id": "req-1",
  "ok": true,
  "result": { "session": { "id": "…" } },
  "error": null
}
```

`request_id` is client-generated and opaque. Command handling is idempotent per
`request_id` from the client's perspective: a client may retry after reconnect
and correlate the ack.

## Commands

Session commands:

| command | scope | params | result |
| --- | --- | --- | --- |
| `session.list` | read | – | `{ sessions: SessionSummary[] }` |
| `session.working_directories` | read | – | `{ paths: string[] }` (at most 20 distinct paths, newest activity first; includes Slight sessions and paths advertised by available harnesses) |
| `session.create` | create | `CreateSessionParams` | `{ session: SessionSummary }` |
| `session.attach` | read | `{ after_sequence?: u64 }`, `session_id` | `SessionAttachResult` |
| `session.detach` | read | `session_id` | unit |
| `session.input` | input | `{ text }`, `session_id` | unit |
| `session.cancel` | input | `session_id` | unit |
| `session.rename` | input | `{ title }`, `session_id` | `{ session: SessionSummary }` |
| `session.set_mode` | input | `{ mode_id }`, `session_id` | unit |
| `session.set_config_option` | input | `{ config_id, value_id }`, `session_id` | unit |
| `session.permission.respond` | permission | `{ permission_id, option_id }`, `session_id` | unit |
| `session.inspect` | read | `session_id` | `SessionInspectResult` |
| `session.resume` / `events.replay` | read | aliases of attach | `SessionAttachResult` |
| `agent.sessions.list` | read | `{ agent, working_directory_label? }` | `{ sessions: AgentSessionSummary[] }` |
| `agent.sessions.import` | create | `{ agent, agent_session_id, working_directory_label, recovery, title? }` | `{ session: SessionSummary }` |

Host-management commands (admin boundary for the macOS app):

| command | params | result |
| --- | --- | --- |
| `host.status` | – | `HostStatusResult` |
| `host.start` | – | `HostLifecycleResult` |
| `host.stop` / `host.shutdown` | – | `HostLifecycleResult` |
| `host.restart` | – | `HostLifecycleResult` |
| `host.diagnostics` | – | `HostDiagnosticsResult` |
| `host.configuration` | – | `HostStatusResult` (placeholder) |
| `pairing.create` | `{ label }` | `PairingCreateResult` |
| `pairing.list` | – | `PairingListResult` |
| `device.list` | – | `DeviceListResult` |
| `device.revoke` | `{ device_id }` | `DeviceRevokeResult` |

`host.start` / `host.restart` are hooks: the running process cannot restart
itself, so they report the current state and explain that an external supervisor
is required. `host.stop` / `host.shutdown` request an orderly shutdown.

Unknown commands produce an `ack` with `ok: false` and code `unsupported`.

### Scope

Every command maps to a scope: `read`, `input`, `permission`, `create`, or
`host_admin`. A device credential carries a subset. Attempting a command outside
its scopes returns `unauthorized`.

`agent.sessions.list` is `read`; `agent.sessions.import` is `create`.

`session.working_directories` returns distinct working-directory labels from
recent product sessions and native sessions advertised by available harnesses.
The host caps the result at 20; clients may still accept a manually entered path.

### Agent session discovery and import

These commands expose sessions that already exist inside an ACP agent and bring
one under Slight's management. They are the only place a client sees native
agent session identity.

`agent.sessions.list` asks the agent for its retained sessions, optionally
scoped to a working directory. `working_directory_label` accepts the same
`~`-relative or absolute form as `session.create`; the agent may apply the
filter loosely, so clients must treat the returned `cwd` as authoritative.
Discovery requires the agent to advertise `sessionCapabilities.list`; otherwise
it fails with `unsupported_capability`.

`agent.sessions.import` creates a new product session bound to the native
session:

- `recovery: "load"` sends ACP `session/load`, which replays the agent's
  retained history through the normal `session.message` / tool-call event
  pipeline before the ack; the replayed events are appended to Slight's bounded
  journal.
- `recovery: "resume"` sends ACP `session/resume`, which reconnects without
  replaying agent-side history (for a client that already replayed it locally).

`load` requires the agent's `loadSession` capability and `resume` requires
`sessionCapabilities.resume`; each fails with `unsupported_capability` when
absent. Neither is the same as `session.resume` / `events.replay`, which are
aliases of `session.attach` and only replay Slight's local journal.
`agent_session_id` and `working_directory_label` are passed through to the
agent; the host never parses agent-private session storage.

#### AgentSessionSummary

```json
{
  "agent": "opencode",
  "agent_session_id": "ses_abc123",
  "cwd": "/Users/me/code/project",
  "additional_directories": ["/Users/me/code/shared"],
  "title": "Fix flaky test",
  "updated_at": "2026-01-01T00:00:00Z"
}
```

`agent_session_id` is the agent's native identifier. Treat it as untrusted
input from the agent: it may be shown to the user, but clients must not use it
for filesystem access or assume it is stable across agent versions.
`additional_directories` is omitted when empty.

## Events

Events are delivered as they are appended to the session journal. Each carries a
monotonic per-session `sequence`.

| event | payload |
| --- | --- |
| `session.status` | `{ status }` |
| `session.message` | `{ role, text, blocks[] }` |
| `session.tool_call` | `{ id, title, kind, status, detail, content[], locations[], raw_input?, raw_output? }` |
| `session.config_options` | `{ options[] }` (normalized session configuration) |
| `session.info` | `{ title, updated_at }` (agent-reported metadata change) |
| `session.usage` | `{ used, size, cost? }` (context window and cost) |
| `session.permission_request` | `{ id, title, detail, tool_call_id, options[] }` |
| `session.permission_resolved` | `{ id, option_id }` |
| `session.exit` | `{ code, reason }` |
| `session.diagnostics` | `{ level, message }` |
| `session.snapshot` | `{ summary, pending_permission }` |
| `replay.complete` | `{ latest_sequence }` |
| `host.status_changed` | host status |

`SessionStatus` is one of `idle`, `working`, `waiting_permission`, `exited`,
`failed`.

### Content blocks

Every `ContentBlock` carries an `id`, a `kind`, and a plain-text `text`
fallback. `kind` is one of:

- `text`, `code`, `reasoning`, `tool_result` — the pre-existing plain blocks.
- `image` — `data_base64` and `mime_type` are present; `uri` is optional.
- `audio` — `data_base64` and `mime_type` are present.
- `resource_link` — `uri` and `name` are present; `mime_type`, `title`,
  `description`, and `size` are optional.
- `resource_text` — `text`, `uri`, and optional `mime_type`.
- `resource_blob` — `data_base64`, `uri`, and optional `mime_type`.

Media and blob payloads are base64 and are always labelled with a MIME type so a
client can decide whether it understands the format. The host never resolves a
`uri` and never fetches remote content. `data_base64`, `mime_type`, `uri`,
`name`, `title`, `description`, and `size` are omitted when absent.

```json
{
  "role": "assistant",
  "text": "",
  "blocks": [
    {
      "id": "b1",
      "kind": "image",
      "text": "",
      "mime_type": "image/png",
      "data_base64": "iVBORw0KGgo=",
      "uri": "file:///tmp/screenshot.png"
    }
  ]
}
```

### Tool-call content

`session.tool_call` carries `content[]`, `locations[]`, and optional
`raw_input`/`raw_output`. Each `content` entry is tagged by `type`:

- `{"type":"content", ...ContentBlock}` — a standard content block.
- `{"type":"diff","path","old_text","new_text"}` — a file modification.
- `{"type":"terminal","terminal_id"}` — a reference to terminal output; clients
  do not run the command.

`locations` entries are `{ path, line? }`. `raw_input` and `raw_output` are the
untyped tool payloads, kept for an explicit structured/debug view; clients must
treat them as untrusted data and never execute anything inside them. On a
partial tool-call update, absent collections and raw values mean "unchanged" and
a client should retain its previous values.

### SessionSummary

```json
{
  "id": "…",
  "title": "…",
  "agent": "fake",
  "model": "auto",
  "effort": "medium",
  "working_directory_label": "~/work",
  "git_branch": "feature/login-flow",
  "status": "idle",
  "created_at": "2026-01-01T00:00:00Z",
  "last_activity_at": "2026-01-01T00:00:00Z",
  "last_sequence": 5,
  "recovery": { "state": "live" }
}
```

`recovery` describes how the session relates to its native agent session after a
host restart. It is omitted by hosts that predate recovery; clients must treat a
missing value as `{"state":"live"}`. `state` is one of:

- `live` — the session was created on this host and did not come from disk.
- `recovered` — the host reconnected to the persisted native agent session
  (`session/resume`) after a restart; the local journal is intact and the
  session is live again.
- `stale` — the native agent session no longer exists (for example the agent
  deleted it). `reason` carries the agent's explanation. The host retains and
  replays the local journal, but the session is not live and will not accept
  input; the host never silently creates a replacement.
- `unavailable` — the agent is not installed/registered or cannot resume its
  sessions. Same retention rule as `stale`.

### Session ACP metadata

`session.inspect` returns an `acp` object holding the normalized result of ACP
negotiation for the session:

- `protocol_version` — negotiated ACP protocol version;
- `agent` — `{ name, title, version }` identity from `initialize`;
- `auth_methods[]` — normalized authentication methods;
- `capabilities` — flattened agent capability booleans;
- `modes` — initial/current session mode state, when supported;
- `config_options[]` — normalized session configuration options, each with an
  `id`, `name`, `description`, optional `category`, and a `kind` of `select`
  (`current_value_id`, `groups[]`) or `boolean` (`current_value`).

Raw ACP payloads, `_meta`, and unstable feature fields are never exposed.

## Replay and resync

`session.attach` with `after_sequence` replays retained events with a higher
sequence. If the requested point predates the retained journal, the result sets
`resync_required: true` and reports `oldest_available_sequence`. A client that
sees this must discard local session state and restate it from the replay plus a
fresh snapshot.

The journal is bounded (`max_event_journal`). The host does not promise infinite
transcript history.

### Restart recovery

Session metadata and the bounded event journal are persisted in SQLite. When the
host restarts, it rehydrates each persisted session by launching the owning
adapter and calling ACP `session/resume` for the stored native session id.

- A successful resume reconnects the session and marks it `recovery: recovered`.
  The local journal is preserved and replayed exactly as before the restart.
- If the agent rejects the native session, the session is marked
  `recovery: stale` and its local journal remains replayable.
- If the agent is missing, cannot be launched, or does not advertise
  `session/resume`, the session is marked `recovery: unavailable`.

In both failure cases the host reports the state explicitly and never spawns a
replacement session. Recovery always prefers `session/resume`; it does not call
`session/load` on restart, so agent history is not replayed twice into the local
journal.

## Errors

`ack` errors and connection-level `error` frames use stable codes:

`malformed_frame`, `invalid_version`, `unauthenticated`, `unauthorized`,
`unknown_session`, `unsupported_capability`, `resync_required`,
`invalid_command`, `unsupported`, `invalid_params`, `internal`.

A `hello` with a bad credential is answered with an `error` frame; the client is
expected to close and re-pair.

## Conformance

Fixtures live in `protocol/conformance/` and are decoded by
`rust/gateway-protocol/tests/conformance.rs`:

- `hello.json` — handshake with resume hint
- `session-create-command.json` — typed command params
- `ack-ok.json` — typed success result
- `agent-sessions-list-command.json` — discovery command params
- `agent-session-import-command.json` — import command params and recovery mode
- `agent-sessions-list-ack.json` — discovery result with native session ids
- `agent-session-import-ack.json` — import result
- `agent-session-import-unsupported.json` — `unsupported_capability` error ack
- `session-list-recovery.json` — session list with `live`/`recovered`/`stale`/
  `unavailable` recovery states
- `malformed-unknown-type.json` — unknown discriminator must be rejected

Unknown fields are ignored; unknown discriminators are rejected.

The transport itself is covered by
`rust/gateway-server/tests/websocket.rs`, which drives a raw WebSocket client
through the upgrade, hello/welcome handshake, text and binary JSON framing,
malformed and invalid-UTF-8 frames, application and protocol ping/pong,
heartbeats, close handshake, error frames, and event streaming.

## Deferred

- `wss://`/TLS transport and Tailscale interface discovery.
- Real Claude Code and Codex ACP adapters (only the fake in-memory agent ships).
- Server-initiated `host.status_changed` events (application heartbeats ship;
  admin events do not yet push).
- Pairing artifacts persisted across host restarts.
