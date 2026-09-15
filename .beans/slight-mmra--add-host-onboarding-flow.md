---
# slight-mmra
title: add host onboarding flow
status: completed
type: feature
priority: normal
created_at: 2026-09-14T17:45:21Z
updated_at: 2026-09-14T17:49:13Z
---

Add initial host setup onboarding for Apple clients. Show a full-screen splash and Get Started action when no host is preconfigured, present host configuration in a sheet, support local or remote on macOS and remote on iOS, collect host/port/name, and only complete onboarding after a successful connection.



- [x] Track whether a host was preconfigured and gate startup/main UI
- [x] Add splash screen and configuration sheet
- [x] Connect before completing onboarding and persist the profile
- [x] Add tests and verify Apple client builds


- [x] Add a confirmed remove-host action to host settings

## Summary of Changes

- Added full-screen first-run onboarding with a Get Started button and configuration sheet.
- Removed the synthetic local profile fallback; unconfigured apps have no host profile and do not auto-connect.
- Added macOS Local/Remote selection and host, port, and optional name fields; iOS remains remote-only.
- Onboarding stays in place until the host connects, while saved disconnected hosts use the normal app shell.
- Added confirmed Remove Host settings action that clears persistence, disconnects, and returns to onboarding.
- Verified the SlightKit Swift test suite: 64 tests passed.
