---
# slight-1fmh
title: bundle real harnesses in macOS app
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:15:16Z
updated_at: 2026-09-14T21:21:32Z
---

The macOS app currently bundles only acp-host even though Claude and Codex packaging scripts and adapter bundle resolution exist. Wire the pinned harness bundles into the app packaging/build flow, ensure bundled agents take precedence over local autodetection, and verify the resulting app contains usable bundles.\n\n- [x] Inspect existing bundle scripts and Xcode packaging constraints\n- [x] Wire Claude and Codex bundles into the macOS app build/package flow\n- [x] Make bundled harnesses take precedence over autodetected global paths\n- [x] Verify build output and adapter resolution

- [x] Fix Xcode User Script Sandboxing for bundle replacement

## Summary of Changes

- Wired the pinned Claude and Codex ACP packaging scripts into the macOS Xcode build phase.
- Declared bundle directories as script outputs so Xcode User Script Sandboxing permits replacement during clean/rebuilds.
- Confirmed the clean Xcode build succeeds and the app Resources directory contains both self-contained runtimes, including their bundled Node binaries.
