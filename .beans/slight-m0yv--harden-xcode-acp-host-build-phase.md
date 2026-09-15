---
# slight-m0yv
title: harden xcode acp-host build phase
status: completed
type: bug
priority: normal
created_at: 2026-09-14T18:09:00Z
updated_at: 2026-09-14T18:09:04Z
---

Make the macOS Xcode build phase reliably find Cargo under Xcode's restricted PATH and report a useful error when Cargo is unavailable.



- [x] Add standard Cargo and Homebrew paths to the build script
- [x] Emit a clear missing-Cargo error
- [x] Reproduce and verify the macOS Xcode build

## Summary of Changes

- Hardened the bundled `acp-host` build phase against Xcode’s restricted PATH.
- Verified the macOS Xcode build completes successfully.
