# ADR 0002: Conformant ACP v1 client boundary

Status: accepted (first vertical slice)

## Context

The host must launch and drive real ACP agents (Claude Code, Codex via
`codex-acp`, OpenCode's stdio server) while keeping the native clients and
`session-core` agent-neutral. The first host slice used an in-memory fake agent
and a bespoke `AcpEvent`/`AcpCommand` pair in `acp-types`; that pair describes
normalized events, but it does not model the actual wire protocol and cannot
drive a real process.

ACP is JSON-RPC 2.0 over newline-delimited stdio with bidirectional traffic:
the client sends `initialize`, `session/new`, and `session/prompt`, while the
agent streams `session/update` notifications and calls back with
`session/request_permission` *during* an in-flight prompt. Getting that
correlation and framing wrong is the riskiest part of the host, so it is proved
first, against a wire-level fake agent.

This ADR is separate from `protocol/gateway-v1.md` by design. ACP is the
host-to-agent contract; gateway-v1 is the host-to-client contract. Neither may
leak types into the other.

## Decisions

### Protocol types come from the official schema crate

`acp-types::wire` re-exports the official `agent-client-protocol-schema` v1
types (`InitializeRequest`, `NewSessionRequest`, `PromptRequest`,
`SessionUpdate`, `RequestPermissionRequest`, method-name constants, and so on)
and layers only the JSON-RPC envelope handling Slight needs. Method names,
discriminators, and payload shapes stay conformant by construction instead of
being re-implemented by hand.

The envelope layer classifies any frame as a request, response, or notification
and routes agent-to-client requests/notifications into typed
`AgentToClientRequest`/`AgentToClientNotification` values. Unknown methods are
preserved (`Other`) so the client can answer them with `method_not_found`
rather than silently dropping them.

### Framing is a transport concern

`acp-types::transport` owns newline-delimited framing only:
`FrameReader`/`FrameWriter` plus `Transport<R, W>`. It is generic over
`BufRead`/`Write`, so the same code runs over an in-process pipe pair in tests
and over a child process's `stdin`/`stdout` in production. Blank lines are
skipped, `\r\n` is accepted, exactly one trailing newline is written, and a
frame size limit bounds memory from a misbehaving peer.

### The client owns correlation and the bidirectional half

`acp-types::client::AcpClient` allocates JSON-RPC ids, sends requests, and
waits for the matching response. While waiting it still services the
agent-to-client half through a `ClientHandler`:

- `session/update` notifications are recorded for the caller to drain;
- `session/request_permission` (and unknown requests) are answered by the
  handler;
- a handler may `Defer` a request and the caller answers it later with
  `respond_permission` / `respond_result` / `respond_error`, keyed by the
  request id.

Typed lifecycle helpers (`initialize`, `new_session`, `list_sessions`,
`load_session`, `resume_session`, `prompt`, `cancel`) enforce ordering:
`session/new`, `session/list`, `session/load`, `session/resume`, and
`session/prompt` fail with `NotInitialized` before `initialize`, `prompt`
rejects unknown sessions, and `initialize` validates that the negotiated
version is v1. `session/load`, `session/list`, and `session/resume` first check
the capabilities the agent advertised during `initialize` and return
`UnsupportedCapability` when they are absent, so callers never guess. Permission
requests outstanding during a prompt are answered synchronously by the handler,
which is enough for the first slice.

### Process lifecycle is separate from the connection

`acp-adapters::process` launches a child (`stdin`/`stdout` piped, `stderr`
discarded by default) and returns an `AgentProcess` (id, wait, kill, `Drop`)
plus a `ChildTransport`. Separating the two lets the host supervise the process
while the connection owns the pipes. Adapter-specific launch configuration
(command, args, environment, working directory) stays in `acp-adapters`;
nothing in `acp-types` knows about processes.

### Capabilities and the agent matrix

The client sends `ClientCapabilities` and `ClientInfo` in `initialize` and
records the agent's `AgentCapabilities`. It does not yet implement `fs/*`,
`terminal/*`, or elicitation, so the baseline adapters must either not require
them or the client must decline them explicitly. Agent-advertised `session/list`
(`sessionCapabilities.list`), `session/load` (`loadSession`), and
`session/resume` (`sessionCapabilities.resume`) are negotiated and gated in
`AcpClient`; `session/load` replay flows through the same `session/update`
handler as live updates, so history is normalized into the same `AgentEvent`
stream. This is the seam where Claude Code, `codex-acp`, and OpenCode
launch/auth differences belong — never in `session-core` or the native clients.

### The fake agent speaks the real wire protocol

`acp-adapters::wire_agent` implements the agent side of ACP over any
reader/writer: `initialize`, `session/new`, `session/prompt`, streaming
`session/update`, and the permission round-trip. It runs in-process over
`std::io::pipe` and as the `fake-acp-agent` binary for subprocess tests.
`acp-adapters/tests/wire_conformance.rs` drives the real client against both.

## Consequences

- `acp-types` and `acp-adapters` build and test independently of
  `session-core`; the fake agent can exercise the whole wire path in CI with no
  real agent installed.
- The client is synchronous, matching the existing no-async-runtime choice.
  Concurrent prompt turns and deferred permission replies need the caller to
  pump inbound frames on another thread; that is not yet wired up.
- The old `AcpEvent`/`AcpCommand`/`AcpConnection` model and the in-memory fake
  in `acp-adapters::fake` still exist because `session-core` consumes them.
  They are the placeholder the bean intends to replace, not the conformant
  layer described here.

## Remaining gaps

- The normalized capability, session metadata, plan, mode, available-command,
  and turn-ended surfaces are now exposed through gateway-v1 and decoded by the
  Apple client. The in-memory fake remains intentionally available for tests.
- A connection pump that multiplexes concurrent turns and deferred permission
  replies safely across threads.
- `session/cancel` semantics for a turn whose permission request is deferred.
- Additional client methods as capabilities require them: `fs/read_text_file`,
  `fs/write_text_file`, terminals, elicitation, authenticate.
- Native agent session identity now flows from `session/list` through
  `session-core` to the gateway (`agent.sessions.list` /
  `agent.sessions.import`, see `protocol/gateway-v1.md`). Durable metadata and
  restart recovery are implemented (ADR-0006): the host persists native session
  identity, resolved working directory, and launch origin, then reconnects with
  `session/resume` on restart and reports `stale`/`unavailable` when the native
  session cannot be restored.
- Real adapters for Claude Code, Codex (`codex-acp`), and OpenCode stdio,
  including their launch/auth/configuration quirks.
- Wire conformance fixtures under `protocol/` shared with client-side decoders.
