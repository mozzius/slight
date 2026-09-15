---
# slight-xw1i
title: simplify host config editor
status: completed
type: task
priority: normal
created_at: 2026-09-14T17:56:30Z
updated_at: 2026-09-14T17:57:03Z
---

Make the existing host configuration sheet use the same simple host, port, and optional name fields as onboarding, while preserving local/remote macOS behavior and connection controls.



- [x] Replace endpoint URL field with host, port, and optional name fields
- [x] Preserve macOS Local/Remote selection and iOS remote-only behavior
- [x] Keep connection status and host removal controls intact
- [x] Verify Apple package tests

## Summary of Changes

- Simplified the host configuration sheet to match onboarding.
- Host endpoints are rebuilt from host, port, and optional display name.
- macOS local mode defaults to 127.0.0.1; iOS remains remote-only.
- Verified `swift test`: 64 tests passed.
