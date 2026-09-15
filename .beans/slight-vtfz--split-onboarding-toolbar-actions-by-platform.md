---
# slight-vtfz
title: split onboarding toolbar actions by platform
status: completed
type: bug
priority: normal
created_at: 2026-09-14T21:32:06Z
updated_at: 2026-09-14T21:32:51Z
---

The onboarding toolbar uses iOS 26 semantic confirm styling, which looks poor on macOS. Platform-split the toolbar presentation while preserving X/checkmark semantics and connection gating.

## Checklist

- [x] Use native macOS toolbar presentation
- [x] Preserve iOS confirm role and existing behavior
- [x] Run Apple package tests



## Summary of Changes

- macOS and pre-26 iOS use text Cancel/Done toolbar actions.
- iOS 26+ keeps the compact X/checkmark controls with semantic cancel/confirm roles.
- Verified with 65 passing SlightKit tests.
