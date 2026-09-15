---
# slight-j08b
title: show loading state while host starts
status: completed
type: bug
priority: normal
created_at: 2026-09-14T20:48:20Z
updated_at: 2026-09-14T20:49:10Z
---

When the host is expected to come online during app startup, show the connecting/loading state instead of the connection error card. Keep the error presentation for actual failed or closed connections.

## Summary of Changes

Changed the sidebar connection issue condition so the idle state is treated as a startup/loading state. The existing Connecting to host empty state now appears while idle, connecting, or handshaking; the error card remains for failed and closed connections.

Verified with the macOS Xcode build.
