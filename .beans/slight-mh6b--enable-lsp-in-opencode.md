---
# slight-mh6b
title: enable LSP in OpenCode
status: completed
type: task
priority: normal
created_at: 2026-09-12T08:55:13Z
updated_at: 2026-09-12T09:16:30Z
---

Enable the OpenCode LSP subsystem in the global configuration and verify the resolved config.

- [x] Update global OpenCode config
- [x] Verify resolved configuration
- [x] Record restart requirement

## Summary of Changes

Set `lsp` to `true` in `~/.config/opencode/opencode.jsonc`. Verified the resolved configuration reports `lsp: true`. OpenCode must be restarted for the setting to take effect.
