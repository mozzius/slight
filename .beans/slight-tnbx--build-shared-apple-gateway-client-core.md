---
# slight-tnbx
title: build Apple multiplatform gateway client
status: completed
type: feature
priority: normal
created_at: 2026-09-12T08:40:29Z
updated_at: 2026-09-12T09:16:52Z
parent: slight-4yk6
---

Create one SwiftUI Multiplatform app project targeting both iOS and macOS. The app is a gateway client on both platforms and owns shared gateway-v1 models/codecs, connection and reconnect state, replay/resync handling, command acknowledgements, session view models, credential storage abstraction, and shared tests. Use small platform adapters for Keychain, lifecycle, windowing, and platform conventions. The macOS target must connect to the host through the same gateway protocol, including when the host runs locally over loopback; it must not duplicate Rust host or ACP process management.

- [x] Create one multiplatform Xcode project with iOS and macOS destinations
- [x] Implement gateway models and codecs
- [x] Implement connection/reconnect/replay state
- [x] Implement shared session view models and command handling
- [x] Add platform credential and lifecycle adapters
- [x] Build shared session UI with platform-specific polish only where needed
- [x] Add shared conformance and platform tests
- [x] Verify both app destinations build

## macOS Admin Surface

The macOS destination also provides host administration UI shapes. It uses the gateway/host management API for status, start/stop/restart, pairing, device revocation, diagnostics, and session inspection; it does not embed a second Rust runtime or ACP implementation.

## Summary of Changes

Built one `Slight` SwiftUI Multiplatform Xcode project under `clients/apple`, with a single app target for iOS and macOS. Added shared gateway models/codecs, connection state and reconnect backoff, replay handling, command acknowledgements, session view models, credential abstractions, session UI, and macOS host-admin views. Added platform adapters and conformance/unit tests.

Verification: `swift test` passed with 22 tests; `xcodebuild` passed for macOS and generic iOS Simulator with code signing disabled.

Deferred follow-ups: live host integration, production pairing/auth wiring, signed-device runtime testing, and ACP-v1 client model updates as the Rust agent boundary becomes conformant.
