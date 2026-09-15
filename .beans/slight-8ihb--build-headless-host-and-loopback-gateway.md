---
# slight-8ihb
title: build headless host and loopback gateway
status: completed
type: feature
priority: normal
created_at: 2026-09-12T08:24:46Z
updated_at: 2026-09-12T09:16:44Z
parent: slight-4yk6
---

Compose host-service, host-cli, and gateway-server into a headless host usable without SwiftUI, Xcode, a logged-in desktop user, or a window server. Start with loopback transport and multiple sessions, exposing serve/status/sessions commands and authenticated protocol plumbing in development mode.

- [x] Compose host service and lifecycle
- [x] Implement loopback gateway listener
- [x] Add acp-host serve/status/sessions commands
- [x] Supervise multiple sessions
- [x] Add integration tests with fake agent and listener

## Admin Boundary

The Rust host remains canonical. Expose host-management API shapes for the macOS app, including service status, session inspection, pairing/revocation, diagnostics, and lifecycle commands. The macOS app presents these controls but does not duplicate process supervision.

## Summary of Changes

Built the headless Rust host workspace under `rust/`, including ACP/domain boundaries, in-memory session store, normalized session core, gateway-v1 protocol, loopback server, host service, CLI, fake-agent support, protocol fixtures, and an architecture ADR. Added macOS admin API shapes and pairing/device operations.

Verification: `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace` with 19 passing tests, and `cargo clippy --workspace --all-targets` all pass.

Deferred follow-ups: conformant ACP JSON-RPC over stdio and real agent subprocesses, SQLite persistence/restart recovery, production TLS/Tailscale authorization, and full lifecycle implementation behind the admin hooks.
