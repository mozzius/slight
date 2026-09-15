---
# slight-g0ps
title: constrain desktop session content width
status: completed
type: task
priority: normal
created_at: 2026-09-12T23:25:02Z
updated_at: 2026-09-13T03:10:19Z
---

Center the macOS session detail content and cap its main content width at approximately 600 points, while preserving full-width mobile layout and the existing sidebar.

## Summary of Changes

Centered the macOS session detail content and capped its main content width at 600 points. iOS remains full-width.

Verification: macOS `xcodebuild` succeeds.

Follow-up: increase the desktop content measure from 600pt to 800pt.

Follow-up verification: desktop measure is now 800pt and the whitespace-preserving Markdown renderer compiles on iOS.

Follow-up: applied the same centered 800pt measure to the New Session draft surface and removed its narrower setup cap.
