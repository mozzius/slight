---
# slight-ykly
title: show host connection issue in sidebar
status: completed
type: bug
priority: normal
created_at: 2026-09-12T18:28:19Z
updated_at: 2026-09-12T18:28:59Z
---

When the client cannot connect to the host, show clear endpoint and failure information in the session sidebar with Retry and Host Settings actions. Keep the connected state compact and avoid restoring the old full-width status bar.

## Summary of Changes

Added a connection issue section to the session sidebar for idle, closed, and failed host states. It displays the configured endpoint, the connection error/state, and inline Retry and Host Settings actions. The connected state remains compact and no full-width status bar was restored.

Verification: macOS `xcodebuild` succeeds.
