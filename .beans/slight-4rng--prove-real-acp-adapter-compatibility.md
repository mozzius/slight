---
# slight-4rng
title: prove real ACP adapter compatibility
status: completed
type: task
priority: normal
created_at: 2026-09-12T19:16:05Z
updated_at: 2026-09-12T23:15:16Z
parent: slight-xnju
---

Turn real OpenCode, Claude Code, and Codex ACP processes into a capability matrix. Implement adapter-specific launch/auth configuration behind the common ACP boundary, add opt-in integration tests when binaries are installed, and document unsupported capability results instead of claiming full support.

## Checklist

- [x] Add shared stdio launch/session helper in `acp-adapters` (executable discovery, env allowlist, process session)
- [x] Refactor OpenCode adapter onto the shared helper without changing its behavior
- [x] Add Claude Code adapter with launch/auth config
- [x] Add Codex adapter with launch/auth config
- [x] Register Claude Code and Codex in the default adapter registry / host config
- [x] Add capability matrix + probe reporting types behind `acp-adapters`
- [x] Add opt-in integration tests that report unsupported/missing binaries clearly
- [x] Run `cargo test -p acp-adapters` (36 pass, real OpenCode + Codex probes) and `cargo build -p host-service -p host-cli`; workspace-wide test compile transiently blocked by concurrent `slight-m43h` rich-content work
- [x] Record summary of changes

## Summary of Changes

Proved real ACP adapter compatibility for OpenCode, Claude Code, and Codex entirely behind the `acp-adapters` boundary, and added an inspectable capability matrix that reports gaps instead of assuming support.

### Launch/auth configuration (behind `acp-adapters`)
- `stdio.rs` (new): shared executable discovery (`resolve_program`/`which`/`resolve_candidate`), the sanitized environment allowlist, bounded stderr capture, and a reusable `StdioSession` that wraps the child process + agent-neutral `ClientSession`.
- `opencode.rs`: refactored onto the shared helper; behavior (mandatory `acp` arg, `SLIGHT_OPENCODE_BIN`, sanitized env, working directory) unchanged. Added a capability entry (auth: `opencode auth login`, provider env vars).
- `claude_code.rs` (new): `ClaudeCodeAdapter` launching the `claude-code-acp` broker (`AgentKind::ClaudeCode`), `SLIGHT_CLAUDE_CODE_BIN` override, Anthropic auth env passthrough documented.
- `codex.rs` (new): `CodexAdapter` launching the `codex-acp` broker (`AgentKind::Codex`), `SLIGHT_CODEX_BIN` override, `codex login`/`OPENAI_API_KEY` documented.
- `lib.rs`: `AgentAdapter::capability()` default plus `DefaultAgentConfig` and `with_configured_agents()`; `with_default_agents()` now registers fake + all three real adapters.
- `host-service`: `HostConfig` gained `claude_code_program`/`codex_program`; `Host::bind` now builds the registry via `with_configured_agents`.

### Capability matrix / reporting
- `matrix.rs` (new): serializable `CapabilityMatrix`, `AdapterCapability`, `AdapterAvailability`, `AuthRequirement`, `BoundarySupport`, `ProbeOutcome`, and `ProbeSummary::unsupported_capabilities()` which names every ACP feature the negotiated agent did not advertise. `AdapterRegistry::capability_matrix()` (static) and `probe_capability_matrix()` (live `initialize`) plus `probe_adapter()`. Missing binaries become `unavailable`, failed handshakes become `failed`, never claimed support.

### Tests
- `tests/adapter_matrix.rs` (new): static coverage of every adapter; deterministic missing-binary reporting; live probe of installed agents printing the full JSON matrix and unsupported capabilities; `SLIGHT_ADAPTER_E2E=1`-gated real prompt turns with clear skip reasons for auth/install gaps.
- `cargo test -p acp-adapters`: 36 passed (unit + adapter_matrix + opencode_acp + wire_conformance). Live probe negotiated OpenCode 1.18.30 and `@agentclientprotocol/codex-acp` 1.11.0; `claude-code-acp` correctly reported `missing_executable`.
- `cargo build -p host-service -p host-cli`: passes.

### Blockers / notes
- Workspace-wide `cargo test` compilation was transiently blocked by concurrent sibling work (`slight-m43h` rich content and `slight-h0r1` negotiation) in `session-core`, `acp-types`, `gateway-protocol`, and `test-support`; none of those errors are in `acp-adapters`. Re-run the workspace suite once those land.
- Claude Code ACP uses the external `claude-code-acp` broker; `claude --acp` is not a thing. The adapter reports this clearly when the broker is absent.
