---
# slight-eyfw
title: implement conformant ACP v1 agent layer
status: completed
type: feature
priority: normal
created_at: 2026-09-12T09:18:54Z
updated_at: 2026-09-12T16:33:31Z
parent: slight-4yk6
---

Replace the placeholder event model with a conformant ACP v1 client boundary for Slight's host. Implement JSON-RPC 2.0 over stdio, initialize/version negotiation, bidirectional request correlation, session/new and prompt/cancel semantics, streaming session updates, permission requests/responses, structured errors, subprocess lifecycle, and a wire-level fake agent for tests. Keep ACP normalization inside adapters and keep the gateway protocol separate.

- [x] Decide and record ACP v1/process/capability model
- [x] Define conformant ACP v1 JSON-RPC types
- [x] Implement bidirectional stdio transport and subprocess lifecycle
- [x] Implement initialize and session lifecycle correlation
- [x] Normalize streaming updates, tools, permissions, cancellation, and errors
- [x] Add wire-level fake agent and conformance tests
- [x] Document real-agent integration boundary

## Baseline Agent Matrix

The first real adapters must support Claude, Codex, and OpenCode behind the same ACP client boundary. Adapter-specific launch/auth/configuration belongs in acp-adapters; session-core and native clients remain agent-neutral.

## Summary of Changes

Implemented the conformant ACP v1 wire/client boundary with JSON-RPC envelopes, newline-delimited stdio framing, bidirectional request correlation, permission handling, subprocess lifecycle, normalized session events, and wire-level conformance tests. Added ADR-0002 for the ACP client boundary and session-core integration tests.

Verification: workspace format, check, tests, and clippy pass. Remaining agent-specific integrations are tracked separately.
