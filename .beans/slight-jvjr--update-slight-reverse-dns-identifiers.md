---
# slight-jvjr
title: update Slight reverse DNS identifiers
status: completed
type: task
priority: normal
created_at: 2026-09-12T13:25:02Z
updated_at: 2026-09-12T13:26:24Z
---

Update Apple bundle identifiers and keychain service identifiers to the reverse-DNS namespace for slight.sh: sh.slight. Verify no legacy reverse-DNS identifiers remain.

- [x] Update bundle identifiers
- [x] Update keychain service identifier
- [x] Verify reverse-DNS references

## Summary of Changes

Updated both Xcode build configurations to bundle identifier `sh.slight.client` and changed the shared Keychain service to `sh.slight.gateway`. Verified that no legacy reverse-DNS identifiers remain in product files.

Both macOS and iOS Simulator Xcode builds pass with code signing disabled.
