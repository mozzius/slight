---
# slight-8f7x
title: remove fake harness from production catalog
status: completed
type: bug
priority: normal
created_at: 2026-09-13T03:06:34Z
updated_at: 2026-09-13T03:09:52Z
parent: slight-y65m
---

Prevent the Fake ACP adapter from being registered or advertised by the production host. Keep fake adapters available only through explicit test support and development fixtures.

## Summary of Changes

Removed Fake from the production adapter registry. Fake remains available only through explicit test-support registries and development fixtures. Updated production host tests to require real adapters.

Claude Code remains absent from the available picker because this host has `claude` installed but not the required `claude-code-acp` broker. The catalog correctly reports it unavailable rather than pretending it is launchable.

Verification: workspace Rust tests pass. Host restarted with the updated binary.
