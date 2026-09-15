# Slight Vision

Slight is a native remote control surface for coding agents running on a Mac.
The Mac is the execution host. Phones and desktops are calm, trustworthy
windows into the work: they show what agents are doing, let people respond,
and make it safe to supervise work from anywhere.

Slight should feel less like a chat app and more like a focused operations
desk for active software work.

## Product Promise

From one native client, a person can:

- See every coding session in one place.
- Understand which agent is working, waiting, blocked, or finished.
- Open a session and follow its streamed work in real time.
- Respond to prompts, permissions, and errors without being at the Mac.
- Reconnect without losing the session or silently starting over.
- Administer the host when necessary, without exposing host internals by
  default.

The product should make the state of work legible at a glance. It should not
make users reconstruct state from logs, terminals, or agent-specific details.

## Client Experience

### Primary Surface: All Sessions

The primary app opens to **All Sessions**. This is the center of gravity for
the product, not a split view between user-facing sessions and host controls.

All Sessions should support:

- A clear list of active, waiting, failed, and completed sessions.
- Agent identity, working-directory label, current state, and recent activity.
- Fast filtering and sorting as the number of sessions grows.
- A session detail view with streamed messages, thoughts where appropriate,
  tool progress, permission requests, diagnostics, and input.
- Obvious connection, replay, and resync state.

The session view is the same product experience on iOS and macOS, adapted to
each platform rather than duplicated into separate application architectures.

### Host Administration Is Separate

Host administration is a secondary operation. It should be accessed through a
button, menu item, or command from All Sessions and open in a separate Host
window or sheet appropriate to the platform.

The Host surface may expose:

- Host status and health.
- Start, stop, and restart controls.
- Pairing and device revocation.
- Listener and connection configuration.
- Session diagnostics and retention information.
- Logs and recovery actions.

Host controls are not a peer tab in the primary navigation. Most users should
be able to use Slight entirely from All Sessions without thinking about the
daemon, ACP, sockets, or credentials.

### Visual Direction

Slight's visual identity is grounded in **forest green**: calm, confident,
and associated with healthy ongoing work rather than warning-heavy command
centers. Green is the accent and status language, not a blanket tint over
every surface.

- Use a deep forest green for primary actions, focus, and healthy connection
  state.
- Use restrained neutrals for the main canvas and transcript surfaces.
- Reserve amber and red for waiting-for-action, failure, and destructive
  operations.
- Prefer readable typography, clear state labels, and quiet motion over dense
  dashboards or decorative chrome.
- Preserve native iOS and macOS conventions while sharing the same visual
  language.

## Runtime Architecture

The canonical path is:

```text
Slight iOS/macOS client
            |
            | Gateway v1: WebSocket + JSON
            v
Rust host daemon
            |
            | ACP: JSON-RPC over stdio
            v
Claude / Codex / OpenCode ACP agent
```

The gateway protocol and ACP are different boundaries.

- The gateway is Slight's stable, agent-neutral client protocol.
- ACP is the host-to-agent protocol.
- Rust owns process lifecycle, ACP I/O, normalization, persistence,
  authentication, and reconnect semantics.
- Native clients consume normalized Slight events and never contain
  agent-specific conditionals.
- The macOS app is a gateway client and host administration surface. It is not
  a second host implementation and does not own ACP processes.

The first real agent integrations are:

- Claude through a conformant ACP adapter.
- Codex through `agentclientprotocol/codex-acp`.
- OpenCode through `opencode acp`.

All three must look identical to session-core and to native clients. Their
launch, authentication, capability, and output differences belong in
`acp-adapters`.

## Trust And Reliability

Slight controls access to real coding environments, so the system must be
explicit and conservative.

- Pair devices individually with revocable credentials.
- Authorize reading sessions, sending input, approving permissions, creating
  sessions, and managing the host separately where practical.
- Never expose environment variables, credentials, or arbitrary filesystem
  data by accident.
- Display the target path and relevant command before a permission decision.
- Treat agent output as untrusted data.
- Number events and define replay and resync behavior.
- Make commands request-addressable and idempotent.
- Persist enough state to explain what happened after disconnects or restart.
- Prefer a visible failure over silently spawning a replacement agent.

The real host and real protocol path are the source of truth. Fakes may exist
for isolated unit tests and previews, but they must not sit in the production
composition or be required to validate the primary user flow.

## Product Stages

### Foundation

- A headless Rust host that runs without SwiftUI or a logged-in desktop user.
- Gateway v1 over WebSocket and JSON.
- Conformant ACP v1 client support over stdio.
- One real end-to-end OpenCode session.

### First Useful Product

- All Sessions on macOS and iOS.
- Streaming transcripts, tool progress, permissions, cancellation, and
  reconnect behavior.
- Separate macOS Host administration window.
- Real Claude, Codex, and OpenCode adapter paths.
- Secure pairing and loopback development setup.

### Durable Product

- SQLite-backed session metadata and bounded event journal.
- Restart recovery and explicit retention behavior.
- Tailscale-safe production networking with application authorization.
- Signed and packaged native clients and a headless launchd service.
- Cross-client protocol conformance tests and operational diagnostics.

## Non-Goals

- Electron or a web UI as a substitute for native clients.
- A polished chat interface before session lifecycle and reconnect behavior are
  trustworthy.
- Infinite transcript retention by default.
- Agent-specific behavior embedded in iOS or macOS views.
- A second process supervisor hidden inside the macOS app.

## Keeping This Current

This document is the high-level product north star. Update it when a product
decision changes the primary information hierarchy, visual language, runtime
boundaries, trust model, supported-agent baseline, or delivery sequence.
Implementation details belong in `AGENTS.md`, protocol docs, ADRs, crate
READMEs, and runbooks.
