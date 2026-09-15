---
# slight-11ro
title: run real Slight host locally
status: completed
type: task
priority: normal
created_at: 2026-09-12T13:15:52Z
updated_at: 2026-09-12T16:43:57Z
---

Run the real Rust host on loopback and connect the Slight app through the actual gateway instead of FakeHost. Verify host startup, status, session listing, and client launch configuration.

- [x] Build and start acp-host
- [x] Verify real host status and gateway listener
- [x] Launch Slight without fake-host mode
- [x] Resolve live transport and development-auth integration blockers
- [x] Verify a real OpenCode session from the client

## Summary of Changes

Started the real Rust host on 127.0.0.1:8787 using WebSocket plus JSON, added the development-only loopback credential to the real client path, and verified the Apple client connects without FakeHost. Argent verified the simulator connection, real OpenCode session creation, prompt submission, and streamed response ending in `pong`.

Deferred follow-ups: production pairing flow, cancellation/permission/error smoke coverage, persistence, and automated end-to-end tests.
