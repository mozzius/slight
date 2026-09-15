---
# slight-pfx0
title: investigate resuming existing agent sessions
status: completed
type: task
priority: normal
created_at: 2026-09-13T14:00:20Z
updated_at: 2026-09-13T14:02:19Z
---

Investigate what is required for Slight to discover and resume existing OpenCode, Codex, and Claude sessions. Compare ACP support, native CLI/session persistence APIs, current adapter capabilities, and gateway/persistence changes. Produce a recommendation and implementation breakdown without changing product code.

- [x] Inspect current Slight lifecycle and adapter resume support
- [x] Investigate OpenCode, Codex, and Claude session persistence/resume mechanisms
- [x] Identify protocol, storage, process, and UI work
- [x] Write findings and recommendation

## Findings

All three current ACP implementations expose the standardized resume surface: `session/list`, `session/load` (restore and replay), and `session/resume` (restore without replay), subject to negotiated capabilities.

Slight currently only implements `session/new`; `AcpSession` has no load/resume/list methods. `SessionManager` marks persisted live sessions as exited after restart, and persisted metadata does not retain the native agent session ID. The gateway `session.resume` command is only an alias for local journal attach/replay.

- OpenCode: sessions are stored in its SQLite database, keyed by project/cwd. ACP `list` is project-scoped and `load` requires the original cwd.
- Codex: threads are persisted under `~/.codex` and the ACP broker maps resume/load/list to app-server thread methods. Listing is cwd-filtered unless using broader native CLI options.
- Claude: sessions are JSONL files under `~/.claude/projects`; the ACP broker can list/load/resume by native UUID, but cwd/project identity matters and retention/settings can remove sessions.

## Recommended Implementation

1. Extend the agent-neutral ACP boundary with negotiated list, load, resume, and optional close/fork operations.
2. Add host commands for discovery/import, keeping imported agent sessions distinct from Slight-owned sessions. Discovery must accept a working directory and show agent, native ID, title, and timestamp.
3. Persist native agent session ID, resolved cwd, agent kind, launch configuration, and ACP capability metadata.
4. On import, use `session/load` to replay the agent history into Slight's bounded journal, then use `session/resume` for reconnects where replay is already local.
5. On host restart, spawn the adapter in the persisted cwd and call native resume instead of unconditionally marking the session exited. Return an explicit unavailable/stale state when the agent no longer has it.
6. Add native client discovery/import UI and capability-aware error states.

This is a medium-sized cross-layer feature, not an adapter-only change. The lowest-risk first slice is OpenCode import/resume through ACP, followed by Codex and Claude conformance tests. Do not parse agent-private SQLite/JSONL directly; ACP is the compatibility boundary.

References: https://agentclientprotocol.com/protocol/session-setup, https://agentclientprotocol.com/rfds/session-resume, https://opencode.ai/docs/acp/, https://developers.openai.com/codex/cli/reference, https://code.claude.com/docs/en/sessions.md
