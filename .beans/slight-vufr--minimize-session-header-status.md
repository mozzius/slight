---
# slight-vufr
title: minimize session header status
status: completed
type: task
priority: normal
created_at: 2026-09-12T17:09:51Z
updated_at: 2026-09-12T17:11:27Z
---

Replace the session screen's oversized static server-status attachment with a compact connection indicator using the appropriate non-large header presentation. Preserve navigation and accessibility, then verify the Apple client builds and the header renders correctly.

## Summary of Changes

Removed the full-width connection status row from the session detail screen. Added a compact, accessible toolbar status indicator with a menu for connection state, errors, and connect/disconnect actions. iOS uses inline navigation title mode; macOS keeps its native title presentation.

Verification: `xcodebuild -project Slight.xcodeproj -scheme Slight -sdk iphonesimulator -configuration Debug -derivedDataPath /tmp/slight-derived build CODE_SIGNING_ALLOWED=NO` succeeds. `swift test` compiles the UI target, but the existing SessionViewModelBehaviorTests still have seven timeout failures unrelated to this header change.
