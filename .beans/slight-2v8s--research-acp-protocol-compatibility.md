---
# slight-2v8s
title: research ACP protocol compatibility
status: completed
type: task
priority: normal
created_at: 2026-09-12T08:55:13Z
updated_at: 2026-09-12T09:16:26Z
---

Digest the authoritative ACP protocol specification and compare it with the current Rust host foundation and gateway normalization boundary. Identify transport/message/lifecycle/permission semantics, missing requirements, compatibility risks, and concrete follow-up beans. Research only; do not modify implementation files.

- [x] Locate authoritative ACP specification sources
- [x] Summarize protocol model and lifecycle
- [x] Compare against current host implementation
- [x] Identify gaps and risks
- [x] Propose follow-up implementation work

## Summary of Changes

Research confirmed that ACP and the Slight gateway are distinct protocols. The current gateway MVP is coherent, but the ACP-facing layer is still a bespoke fake event model rather than conformant ACP JSON-RPC over stdio. The main blockers are bidirectional request/response correlation, subprocess transport, initialize/session negotiation, agent-issued session IDs, streaming update/tool-call semantics, permission cancellation, filesystem/terminal capability handling, structured errors, and process lifecycle.

Authoritative sources reviewed:

- https://agentclientprotocol.com/protocol/v1/overview
- https://agentclientprotocol.com/protocol/v1/initialization
- https://agentclientprotocol.com/protocol/v1/session-setup
- https://agentclientprotocol.com/protocol/v1/prompt-turn
- https://agentclientprotocol.com/protocol/v1/tool-calls
- https://agentclientprotocol.com/protocol/v1/cancellation
- https://agentclientprotocol.com/protocol/v1/terminals
- https://agentclientprotocol.com/protocol/v1/file-system
- https://agentclientprotocol.com/protocol/v1/elicitation
- https://agentclientprotocol.com/protocol/v1/transports
- https://agentclientprotocol.com/protocol/v2/migration

Recommended next decisions are an ACP v1 target/process model ADR, conformant ACP JSON-RPC types, bidirectional stdio transport, and corrected session-core lifecycle semantics. The current fake-agent tests remain useful but must not be treated as real-agent compatibility coverage.
