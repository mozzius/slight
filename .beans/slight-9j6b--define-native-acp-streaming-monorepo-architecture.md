---
# slight-9j6b
title: define native ACP streaming monorepo architecture
status: completed
type: task
priority: normal
created_at: 2026-09-12T08:22:52Z
updated_at: 2026-09-12T08:23:33Z
---

Create AGENTS.md for a new monorepo that streams ACP sessions from a macOS host to native iOS, Android, and macOS clients. Include repository layout, Rust host/core boundaries, transport/security, protocol contracts, client responsibilities, headless operation, testing, development workflow, and independent-agent ownership.\n\n- [x] Inspect the empty workspace and establish assumptions\n- [x] Write AGENTS.md with an actionable architecture plan\n- [x] Verify the document and mark this bean complete

## Summary of Changes\n\nAdded AGENTS.md defining the native ACP session platform architecture, Rust crate boundaries, versioned gateway protocol, persistence and reconnect behavior, Tailscale-aware security model, native client responsibilities, headless/macOS operation, testing strategy, agent ownership boundaries, delivery sequence, and initial ADRs.
