---
# slight-ue06
title: create new-format app icon
status: completed
type: feature
priority: normal
created_at: 2026-09-14T22:29:46Z
updated_at: 2026-09-14T22:32:12Z
---

Create and integrate a native .icon app icon for the Apple app using the home screen forest-green gradient.

## Summary of Changes

Added `clients/apple/App/AppIcon.icon` as an Icon Composer package with the forest-gradient background and white tree mark. Registered it as the `AppIcon` resource in the Xcode project for macOS and iOS. Verified with macOS and generic iOS `xcodebuild` builds.
