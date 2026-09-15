---
# slight-v63e
title: fix app-managed host agent resolution
status: in-progress
type: bug
created_at: 2026-09-14T20:57:02Z
updated_at: 2026-09-14T20:57:02Z
---

The Xcode-launched acp-host inherits a restricted PATH and cannot locate installed real agent executables, causing all persisted sessions to recover as exited/unavailable. Make the app-managed host resolve agent tooling reliably and verify with tests.\n\n- [ ] Determine installed agent locations and appropriate resolution strategy\n- [ ] Pass a usable environment or explicit paths to the managed host\n- [ ] Add regression coverage and verify host/client tests
