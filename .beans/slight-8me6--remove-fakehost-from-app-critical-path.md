---
# slight-8me6
title: remove FakeHost from app critical path
status: completed
type: task
priority: normal
created_at: 2026-09-12T13:38:56Z
updated_at: 2026-09-12T14:32:13Z
parent: slight-dags
---

Remove FakeHost construction and fake-host launch behavior from the production Slight app composition. Keep deterministic fixtures only in previews/tests if still useful, clearly isolated from runtime. Make the real host endpoint and connection failure states first-class in the UI.

- [x] Remove FakeHost construction from production app composition
- [x] Remove fake-host launch arguments and environment behavior
- [x] Isolate fixture code to DEBUG/previews/tests
- [x] Add real endpoint settings and visible connection failure states
- [x] Verify macOS and iOS Simulator builds

## Summary of Changes

Removed FakeHost from the production app path and isolated fixture code behind DEBUG-only development support. Added persisted real-host endpoint settings, reconnect controls, and visible endpoint/error states. The shared Xcode scheme uses the real host path.

Verification: macOS and iOS Simulator builds pass with code signing disabled. Existing Swift behavior-test timeouts remain a separate follow-up.
