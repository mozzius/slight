---
# slight-o0gi
title: preserve bundled host launch logs
status: completed
type: bug
priority: normal
created_at: 2026-09-15T15:54:27Z
updated_at: 2026-09-15T15:54:45Z
---

The macOS local host controller discards bundled acp-host stdout and stderr, leaving startup failures indistinguishable from listener readiness timeouts. Persist process output and point the UI at it.

## Summary of Changes\n\n- Preserve bundled acp-host stdout and stderr in `~/.slight/logs/acp-host-process.log`.\n- Include the log path in the local-host startup timeout error.
