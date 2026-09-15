---
# slight-qoxs
title: fix harness config namespace and disclosure
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:25:00Z
updated_at: 2026-09-14T21:25:14Z
---

The app wrote OpenCode config at the wrong JSON level, so the Rust host ignored it. The harness setup DisclosureGroup also needed explicit expansion state. Fix the shared config namespace and make the setup section reliably expandable.\n\n- [x] Fix host/app config namespace\n- [x] Make harness setup expansion explicit\n- [x] Verify Swift and Rust tests

## Summary of Changes

- Namespaced the config as `{"harnesses": {"opencode": ...}}` and made the host read the existing unnamespaced file once for recovery.
- Added explicit collapsed DisclosureGroup state in setup and host settings.
- Swift and Rust test suites pass.
