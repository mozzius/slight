---
# slight-4tmd
title: remove nested desktop sidebar
status: completed
type: bug
priority: normal
created_at: 2026-09-12T18:23:50Z
updated_at: 2026-09-12T18:25:33Z
---

The macOS root view still renders a top-level Sessions/Host navigation sidebar around the session browser's own session sidebar. Make SessionBrowserView the root desktop surface and remove the outer sidebar while preserving host settings access.

## Summary of Changes

Removed the macOS root `NavigationSplitView` that contained Sessions and Host. `SessionBrowserView` is now the root desktop surface and owns the only sidebar, while host settings remain available from the compact toolbar menu.

Verification: macOS `xcodebuild` succeeds.
