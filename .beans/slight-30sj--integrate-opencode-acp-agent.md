---
# slight-30sj
title: integrate OpenCode ACP agent
status: completed
type: task
priority: normal
created_at: 2026-09-12T13:40:15Z
updated_at: 2026-09-12T16:34:40Z
parent: slight-dags
blocked_by:
    - slight-eyfw
---

Make the Rust host launch and supervise OpenCode as an ACP stdio agent using opencode acp. Add sanitized executable discovery/configuration, process lifecycle, ACP initialize/session negotiation, prompt streaming, permissions, cancellation, and diagnostics.

- [x] Discover opencode executable from explicit configuration or PATH
- [x] Launch opencode acp with sanitized process configuration
- [x] Complete real ACP initialize and session/new handshake
- [x] Support real prompt streaming and turn completion
- [x] Surface process and protocol failures
- [x] Add real OpenCode integration tests

## Summary of Changes

Added the OpenCode ACP adapter, process configuration, environment sanitization, lifecycle handling, and real handshake/prompt integration tests. The host registers OpenCode as an available agent and launches it through the conformant ACP stdio boundary.

Verification: OpenCode ACP handshake and real prompt round-trip pass with `SLIGHT_OPENCODE_E2E=1`.
