---
# slight-y6g1
title: build Slight client session UI
status: in-progress
type: feature
priority: normal
created_at: 2026-09-12T09:18:54Z
updated_at: 2026-09-12T13:09:29Z
parent: slight-4yk6
---

Advance the existing single SwiftUI Multiplatform Slight app from protocol foundation to a usable client UI. Connect the session browser and transcript to the host gateway, support streaming messages/tool progress/permissions, input/cancel/reconnect states, and expose the macOS host administration surface. Keep iOS and macOS in one app target with platform-specific polish only. Use fake host fixtures until the ACP-backed host path is available.

- [x] Establish a fake-host UI fixture and preview data
- [x] Build session list and attach flow
- [x] Render streaming messages and tool progress
- [x] Implement input, cancel, and permission interactions
- [x] Show reconnect/replay/resync states
- [x] Build macOS host administration screens
- [ ] Add iOS/macOS UI tests or focused view-model tests

## Current State

Added a shared Xcode Debug launch argument named -SlightFakeHost so the app opens with demo sessions, streaming replies, permissions, and host-admin data without a running daemon. Fixed the app-level MainActor wiring in PlatformAdapters. macOS and iOS Simulator builds pass with code signing disabled. Swift package behavior tests still have seven fake-transport timeout failures and remain open.
