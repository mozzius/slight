---
# slight-tr1v
title: rebuild bundled host with archive command
status: in-progress
type: bug
created_at: 2026-09-15T15:30:26Z
updated_at: 2026-09-15T15:30:26Z
---

The embedded macOS acp-host can run an older gateway binary after app restart, causing session.archive to return unsupported even though current source dispatches it.
