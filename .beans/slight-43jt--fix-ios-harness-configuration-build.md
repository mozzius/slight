---
# slight-43jt
title: fix iOS harness configuration build
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:50:35Z
updated_at: 2026-09-14T21:51:22Z
---

Investigate and fix macOS-only HarnessConfiguration code being compiled into the iOS target, including verification with the Apple package tests/build.



- [x] Guard macOS-only harness configuration from iOS compilation
- [x] Verify iOS and macOS Apple builds


## Summary of Changes

Wrapped HarnessConfiguration and its filesystem-backed store in #if os(macOS), so iOS compiles the shared UI target without macOS-only APIs. Verified Swift package, SlightGatewayUI on iOS and macOS, and the full Slight iOS app builds.
