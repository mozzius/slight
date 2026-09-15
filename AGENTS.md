# Native ACP Session Platform

This repository is a new monorepo for running ACP-compatible coding agents on a
Mac and controlling their sessions from native iOS, Android, and macOS clients.
The product is intentionally native-first. Do not introduce Electron or a web
UI as a substitute for the native clients.

## Product Shape

- The Mac is the execution host. It starts and supervises Claude Code, Codex
  through an ACP adapter, or another ACP agent process.
- The Rust core owns ACP I/O, process lifecycle, session state, persistence,
  networking, authentication, and protocol translation.
- A phone or desktop client is a remote UI and input device. It must not need
  to understand agent-specific ACP extensions.
- macOS ships both a SwiftUI host application and a headless Rust host binary.
  The app is a convenience shell around the same host service, not a second
  implementation.
- Tailscale is the expected network path, but it is not the complete product
  security model. Pairing and application-level authorization are required.

## Design Rules

1. Keep ACP at the host boundary. Agent subprocesses speak ACP; clients speak
   the product's versioned session API.
2. Keep the Rust host usable without SwiftUI, Xcode, a logged-in desktop user,
   or a window server.
3. Treat the network as unreliable: events need sequence numbers, reconnect
   needs a defined replay policy, and commands need request IDs and idempotency
   rules.
4. Prefer explicit protocol types over untyped JSON in Rust and generated or
   hand-maintained equivalent models in each client.
5. Never send credentials, environment variables, or arbitrary filesystem data
   to a client unless a feature explicitly requires it and the user has
   authorized it.
6. Do not put agent-specific conditionals in native clients. Add normalization
   in the Rust ACP adapter layer.
7. Make local development possible without Tailscale by using loopback and a
    development-only pairing path. Do not weaken production authorization to
     make development convenient.
8. Do not add backwards-compatibility code, fallback decoding, migration
   shims, deprecated aliases, or dual protocol paths for hypothetical clients.
   This is pre-release software with no deployed consumers. Change the
   contract directly and update all in-repository implementations, fixtures,
   and tests. Add compatibility only when a real deployed consumer exists or
   the user explicitly requests it.

## Product Vision

Read [`vision.md`](vision.md) for the current high-level product direction,
client hierarchy, visual language, supported-agent baseline, and roadmap
principles. Keep it aligned with decisions that change the product shape;
implementation details belong in the protocol docs, ADRs, runbooks, and crate
documentation below.

## Proposed Repository Layout

The exact package names may change, but the ownership boundaries should not.

```text
.
├── AGENTS.md
├── Cargo.toml                 # Rust workspace
├── Cargo.lock
├── rust/
│   ├── acp-types/             # ACP domain types and codecs
│   ├── acp-adapters/          # Agent launch/config quirks; no networking
│   ├── session-core/          # Session state machine and command handling
│   ├── session-store/         # Durable metadata, event journal, migrations
│   ├── gateway-protocol/      # Versioned client API models and envelopes
│   ├── gateway-server/        # Authenticated network server and connections
│   ├── host-service/          # Composition root, lifecycle, config, logging
│   ├── host-cli/              # Headless executable and service commands
│   └── test-support/          # Fake ACP agents, fixtures, test transports
├── clients/
│   ├── ios/                   # SwiftUI app and iOS-specific integration
│   ├── macos/                 # SwiftUI app, host controls, client UI
│   └── android/               # Compose app and Android-specific integration
├── protocol/
│   ├── gateway-v1.md          # Human-readable wire contract
│   ├── schemas/                # Canonical serialized schemas/fixtures
│   └── conformance/            # Cross-client protocol test vectors
├── docs/
│   ├── architecture/          # ADRs and operational design
│   └── runbooks/               # Pairing, recovery, support procedures
├── scripts/                   # Reproducible generation and dev commands
└── .github/workflows/         # Rust and client CI
```

Use a Rust workspace for the host. Keep native projects as first-class
subprojects rather than hiding them behind a cross-platform wrapper. Shared
protocol schemas and fixtures belong in `protocol/`, not in a client project.

## Runtime Architecture

```text
Claude Code / Codex adapter / other ACP agent
                    │ ACP (agent transport)
                    ▼
             acp-adapters
                    │ normalized events + commands
                    ▼
             session-core ───── session-store
                    │
                    ▼
             gateway-server
                    │ authenticated gateway protocol
        ┌───────────┼───────────┐
        ▼           ▼           ▼
      iOS       Android       macOS client
```

### ACP and Agent Processes

`acp-adapters` is responsible for locating an executable, constructing its
environment, attaching to its ACP transport, decoding messages, and converting
them into the internal session event model. It may contain adapter-specific
launch configuration, but it must not know about sockets, Tailscale, Swift, or
Compose.

`session-core` owns the state machine: creating sessions, accepting user input,
approving or rejecting permissions, cancelling work, handling agent exits, and
emitting normalized events. It should be testable with an in-memory fake ACP
connection and must not require a running network listener.

The first implementation should support one host process and multiple sessions.
Do not assume one agent process per client connection. A session remains owned
by the host when a client disconnects.

### Gateway Protocol

The gateway is a separate, documented protocol even if its first transport is
WebSocket. Use a single bidirectional connection per client with:

- an explicit protocol version during handshake;
- client and connection IDs;
- request IDs for commands and acknowledgements;
- monotonically increasing per-session event sequence numbers;
- bounded replay from the event journal after reconnect;
- a clear `resync required` response when the requested history is unavailable;
- heartbeats, close reasons, and server capability discovery.

The initial command/event vocabulary should cover session list/create/attach,
text input, cancel, permission response, terminal resize if applicable,
session status, normalized agent messages, tool-call progress, permission
requests, diagnostics, and session exit. Keep raw ACP payloads available for
debugging only behind an explicit capability; normal clients consume normalized
events.

JSON is a reasonable first wire encoding because it is inspectable and easy to
implement on all three platforms. Hide the encoding behind protocol types so a
later binary encoding does not change the session core. Do not use ad hoc JSON
objects without schema/version tests.

### Persistence and Reconnect

Persist enough state to recover after a client disconnect or host restart:

- session ID, creation time, agent kind, working directory label, and status;
- durable event sequence and a bounded event journal;
- whether the agent process is still running and how it was launched;
- protocol/schema version and migration version.

Do not promise infinite transcript history in the MVP. Define retention limits
and expose them to clients. A reconnect should first replay retained events,
then send a fresh session snapshot. If the host restarted and cannot resume an
agent process safely, report that state rather than silently spawning a new one.

The initial store can be SQLite behind `session-store`. All database access
must stay behind that crate's interface so storage decisions do not leak into
clients or session state logic.

## Security Model

- Bind the production listener only to the intended Tailscale interface or a
  narrowly configured address, never `0.0.0.0` by default.
- Use TLS or an equivalent authenticated encrypted channel even on the tailnet;
  Tailscale reduces network exposure but does not replace app authorization.
- Pair a client with a short-lived, one-time code or QR/deep-link flow. Store a
  scoped device credential in iOS Keychain, Android Keystore, and macOS Keychain.
- Give each device a revocable identity. Do not use one shared static token for
  every client.
- Authorize operations separately where practical: read session, send input,
  approve permission, create session, and manage host.
- Redact tokens, prompts that contain secrets, environment values, and command
  arguments from logs by default.
- Require an explicit user action before exposing a working directory or
  granting a tool permission. Display the target path and relevant command in
  native clients.
- Headless mode must have a non-interactive pairing/revocation CLI and a clear
  way to rotate credentials.

Treat ACP agent output as untrusted data. Native clients must render text and
structured content safely and must not execute links, commands, or suggested
actions implicitly.

## Client Responsibilities

All clients implement the same gateway protocol and session UX, with platform
conventions appropriate to each OS.

- `ios`: SwiftUI navigation, Keychain credentials, background/reconnect policy,
  notifications where permitted, and mobile input ergonomics.
- `android`: Compose UI, Keystore credentials, lifecycle-aware connections,
  notifications, and mobile input ergonomics.
- `macos`: SwiftUI session UI plus host lifecycle, pairing, configuration, and
  log/status views. Host controls call the Rust service; they do not duplicate
  process management.

Clients should use a small platform-neutral protocol layer per platform, then
map protocol models into observable UI state. Avoid making the UI itself the
network client. Connection state, replay, and command acknowledgement handling
must be independently testable.

## Headless and macOS Operation

The canonical service is `host-cli`/`host-service`, not the macOS app. Support:

```text
acp-host serve                 # run the gateway and supervise sessions
acp-host status                # show host, listener, and session state
acp-host pair                  # create a short-lived pairing artifact
acp-host revoke <device-id>   # revoke one device credential
acp-host sessions              # list persisted sessions
```

The macOS SwiftUI app may start, stop, configure, and observe the service, or
embed the Rust service through a deliberate FFI boundary. Pick one hosting model
early and document it in an ADR; do not create two independent Rust runtimes.
The headless binary must work under a launchd service and must not assume a GUI
login session.

## Testing Strategy

- Rust unit tests for ACP normalization, session transitions, authorization,
  replay, idempotency, and persistence migrations.
- Rust integration tests using a fake ACP agent and an in-memory or temporary
  gateway listener.
- Protocol conformance fixtures in `protocol/conformance/` consumed by all
  clients. Include malformed frames, unknown fields, version negotiation,
  reconnect, replay gaps, duplicate commands, and permission events.
- Swift unit tests for decoding, connection state, credential storage wrappers,
  and session view models. Use SwiftUI previews only for visual iteration, not
  behavioral coverage.
- Kotlin unit tests for decoding, connection state, credential storage wrappers,
  and view models. Add Compose UI tests for critical interaction flows.
- End-to-end tests should run a fake host and real client protocol layers before
  requiring a real Claude Code or Codex installation.
- Security tests must verify unauthenticated connections, revoked devices,
  unauthorized commands, replay limits, and log redaction.

Every protocol change must add or update a schema, a fixture, Rust coverage,
and client decoding coverage. Every session-state change must include a state
transition test.

## Agent Work Model

Agents should work in small, independently verifiable slices. Before editing,
read this file and the relevant protocol/ADR documents. Keep commits focused and
do not silently change the gateway contract to unblock one client.

Suggested ownership boundaries:

- Host foundation: `rust/acp-types`, `rust/acp-adapters`, `rust/session-core`.
- Storage and recovery: `rust/session-store` and persistence ADRs.
- Network and security: `rust/gateway-protocol`, `rust/gateway-server`, pairing.
- Headless host: `rust/host-service`, `rust/host-cli`, launchd/runbooks.
- Apple clients: `clients/ios`, `clients/macos`, Apple protocol conformance.
- Android client: `clients/android`, Android protocol conformance.
- Cross-platform contract: `protocol/`, fixtures, compatibility tests.

Agents may modify shared protocol files only when they also update all affected
implementations or leave the repository in a clearly documented, buildable
intermediate state. Prefer additive protocol changes. Removing or renaming a
field requires a migration/versioning decision.

## Delivery Sequence

1. Define gateway-v1 types, envelopes, error model, and conformance fixtures.
2. Implement a fake ACP agent plus `session-core` state machine.
3. Implement one headless Rust host with loopback transport and SQLite journal.
4. Add authenticated pairing and Tailscale-safe listener configuration.
5. Build a minimal macOS client and use it as the first protocol client.
6. Add iOS and Android clients against the same fixtures and session flows.
7. Add real Claude Code and Codex ACP adapters behind the common interface.
8. Add launchd installation, background/reconnect behavior, observability, and
   release packaging.

Do not start by building polished chat UI. The riskiest work is ACP behavior,
session lifecycle, reconnect semantics, and secure remote control; prove those
with the fake agent and protocol fixtures first.

## Initial ADRs To Write

- Gateway transport and encoding: WebSocket/JSON initially, or an alternative.
- Rust-to-Swift integration model for the macOS host app.
- ACP process launch and permission model for Claude Code and Codex adapters.
- Event journal retention, restart recovery, and transcript privacy.
- Pairing, credential storage, TLS, and Tailscale interface discovery.
- Versioning policy for gateway-v1 and client compatibility.

When a decision changes this plan, update the relevant ADR and this file in the
same change. Keep this document architectural: implementation details belong in
crate READMEs, protocol docs, runbooks, and ADRs.
