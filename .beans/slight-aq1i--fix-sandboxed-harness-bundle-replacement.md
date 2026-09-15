---
# slight-aq1i
title: fix sandboxed harness bundle replacement
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:22:08Z
updated_at: 2026-09-14T21:22:50Z
---

Xcode still denies the ACP packaging script when it removes nested files under the generated app Resources bundle, despite declared outputs. Adjust the macOS target build configuration so the packaging phase can safely replace generated third-party bundle trees, and verify with the default DerivedData path.\n\n- [x] Update Xcode packaging sandbox configuration\n- [x] Verify clean/default DerivedData build succeeds

## Summary of Changes

- Disabled Xcode User Script Sandboxing for the macOS target because the packaging phase replaces generated third-party bundle trees under app Resources.
- Verified a clean Debug build using the default Xcode DerivedData path succeeds and produces both harness bundles.
