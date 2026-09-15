---
# slight-2e13
title: bundle and manage macos acp-host
status: completed
type: feature
priority: normal
created_at: 2026-09-14T18:04:07Z
updated_at: 2026-09-14T18:04:15Z
---

Bundle acp-host with the macOS app, manage its local lifecycle during onboarding and startup, and offer optional launch-at-login behavior while preserving the standalone CLI.



- [x] Add bundled macOS acp-host build phase
- [x] Manage local host process from the desktop client
- [x] Add local-host setup prompt during onboarding
- [x] Add optional launch-at-login behavior
- [x] Preserve independent acp-host CLI operation
- [x] Verify Rust, Swift package, macOS, and iOS builds

## Summary of Changes

- Added an Xcode build phase that compiles and bundles `acp-host` into macOS app resources.
- Added a local host service controller that starts/stops the bundled daemon and detects launch-at-login state through `SMAppService`.
- Added onboarding UI for local-host setup and optional launch-at-login behavior.
- Existing standalone `acp-host serve` operation remains unchanged.
- `cargo check -p host-cli`, `swift test` with 64 tests, and macOS/iOS Xcode builds pass.
