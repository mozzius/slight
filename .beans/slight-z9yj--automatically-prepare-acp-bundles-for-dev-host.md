---
# slight-z9yj
title: automatically prepare ACP bundles for dev host
status: scrapped
type: task
priority: normal
created_at: 2026-09-14T16:37:25Z
updated_at: 2026-09-14T16:55:15Z
parent: slight-bg6w
---

Make the local development host automatically build and use the target-specific Codex and Claude ACP bundles, rather than relying on global broker installations or manually set bundle environment variables.

## Summary of Changes

- Added `scripts/dev-host.sh`, which detects the host target, builds missing Codex and Claude ACP bundles through the unified packaging script, sets both bundle roots, and starts the host.
- Documented the automatic local-development flow in `docs/runbooks/acp-bundles.md`.
- Built the current arm64 bundles and restarted the local host with Claude Code available.
- Verified Claude and Codex bundle smoke tests plus all acp-adapters tests passed.

## Reasons for Scrapping

The dev-only wrapper was the wrong solution. Production packaging remains in `scripts/package-acp-bundles.sh`; no special dev launcher is retained.
