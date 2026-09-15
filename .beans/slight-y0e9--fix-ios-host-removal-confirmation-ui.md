---
# slight-y0e9
title: fix iOS host removal confirmation UI
status: completed
type: bug
priority: normal
created_at: 2026-09-14T19:29:09Z
updated_at: 2026-09-14T19:30:12Z
---

Fix the iOS remove-host presentation and wire the confirmation action to the remove button.\n\n- [x] Find the host removal UI and current confirmation state\n- [x] Improve the remove-host label/layout\n- [x] Connect the button to confirmation and removal behavior\n- [x] Run focused verification

## Summary of Changes

- Attached the host-removal confirmation dialog directly to the destructive remove-host button.
- Updated the iOS remove-host label casing and ensured the save/connect icon and label render with high contrast.
- Verified with `swift test` (64 tests passed).
