---
# slight-7bu0
title: retry local host startup after boot failure
status: completed
type: bug
priority: normal
created_at: 2026-09-15T15:52:53Z
updated_at: 2026-09-15T15:53:03Z
---

Retry from the connection issue must rerun the macOS local host startup gate. Currently the sidebar retry calls the gateway connection directly, bypassing LocalHostServiceController after the bundled acp-host failed to listen on port 8787.

## Summary of Changes\n\n- Routed both session-browser Retry actions through `SlightAppModel.connect()`.\n- Retry now reruns local host startup before opening the gateway connection, so a failed bundled host launch can recover without editing and saving the profile.
