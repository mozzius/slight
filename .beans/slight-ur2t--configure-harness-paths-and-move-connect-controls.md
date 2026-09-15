---
# slight-ur2t
title: configure harness paths and move connect controls
status: completed
type: feature
priority: normal
created_at: 2026-09-14T21:09:14Z
updated_at: 2026-09-14T21:13:04Z
---

Add explicit persisted harness executable configuration at ~/.slight/config.json, with autodetection where possible but requiring a resolved path before use. Wire the managed host and settings flow to the configuration, and remove the awkward toolbar connect control in favor of host settings.\n\n- [x] Define shared config file shape and host CLI/config loading\n- [x] Add harness path autodetection and explicit settings UI\n- [x] Use configured paths when launching the managed host\n- [x] Move connection controls out of the toolbar\n- [x] Add tests and verify Rust/Swift builds

## Summary of Changes\n\n- Added the `~/.slight/config.json` harness contract and host CLI loading for explicit OpenCode, Claude Code, and Codex executable paths.\n- Added macOS autodetection, validation, persistence, and editable harness fields to both first-run setup and host settings.\n- Removed the macOS app-level reconnect/disconnect commands from the toolbar command group; connection actions remain in Host Settings.\n- Verified Swift package tests, the macOS Xcode build, Rust formatting, and host CLI/service tests.
