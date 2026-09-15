---
# slight-h0r1
title: implement full ACP negotiation and session metadata
status: completed
type: task
priority: normal
created_at: 2026-09-12T19:15:34Z
updated_at: 2026-09-12T23:24:11Z
parent: slight-xnju
---

For real agents, advertise complete client capabilities and preserve negotiated initialize/session-new data: config options, session info, usage, auth methods, modes, and agent metadata. Normalize these through acp-types and session-core without exposing raw ACP to clients. Add wire fixtures and transition tests.

## Summary of Changes

Advertised full host ACP client capabilities and preserved negotiated
initialize/session-new metadata through the normalized types, session-core, and
the gateway contract.

- `acp-types::client::host_client_capabilities()` advertises boolean session
  config options and nothing the host cannot service (no fs/terminal). It is
  wired into `ClientSession::initialize`.
- New normalized metadata types in `acp-types::metadata`: `AcpConfigOption`
  (select/boolean, grouped choices, categories), `AcpSessionInfoUpdate`,
  `AcpUsage`/`AcpCost`, and `AgentMetadataSummary`. `SessionMetadata` now
  carries `config_options`.
- `AgentEvent` gains `ConfigOptions`, `SessionInfo`, and `Usage`; the real
  client normalizes `config_option_update`, `session_info_update`, and
  `usage_update` session updates.
- `session-core` stores current config options, applies agent title updates,
  and emits `session.config_options`, `session.info`, and `session.usage`
  events. `SessionAcpMetadata` exposes the negotiated `agent` identity and
  `config_options` through `session.inspect`.
- Gateway: added event-name constants and docs, and extended the conformance
  inspect fixture with agent metadata, config options, and usage events.
- Tests: acp-types unit tests for capability advertisement and each
  normalization; session-core wire tests for initial config options and
  streamed config/info/usage updates; gateway conformance decoding.
  `cargo test --workspace` passes (28 test targets, exit 0).

Deferred: sending `session/set_config_option` or `session/set_mode` back to the
agent, and parsing agent-reported `updated_at` into the product summary
timestamp.
