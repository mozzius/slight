---
# slight-r1fp
title: remove fields from local host flow
status: completed
type: task
priority: normal
created_at: 2026-09-14T19:23:44Z
updated_at: 2026-09-14T19:24:20Z
---

Hide host, port, and name fields from the macOS Local onboarding and host settings flows. Local uses the fixed bundled host endpoint on 127.0.0.1:8787; retain fields for Remote and iOS.



- [x] Hide host, port, and name fields for macOS Local onboarding
- [x] Hide host, port, and name fields for macOS Local settings
- [x] Use the fixed local endpoint 127.0.0.1:8787
- [x] Preserve fields for Remote and iOS
- [x] Verify Swift tests

## Summary of Changes

- Local is now a zero-configuration path in onboarding and host settings.
- Remote and iOS retain host, port, and optional name editing.
- Verified `swift test`: 64 tests passed.
