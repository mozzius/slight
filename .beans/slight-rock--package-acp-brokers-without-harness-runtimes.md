---
# slight-rock
title: package ACP brokers without harness runtimes
status: completed
type: task
priority: normal
created_at: 2026-09-14T16:47:23Z
updated_at: 2026-09-14T16:55:23Z
parent: slight-bg6w
---

Remove the dev-only bundle wrapper and revise production ACP packaging so Slight ships the ACP adapter components without bundling Claude or Codex harness runtimes. Resolve and document the required installed harness boundary, replacing the current Claude SDK-runtime assumption where necessary.

## Findings

- Removed the dev-only `scripts/dev-host.sh` wrapper and generated local `vendor/` bundles.
- The current Claude ACP broker is explicitly powered by `@anthropic-ai/claude-agent-sdk`, whose platform package is the Claude Code runtime. The installed `claude` CLI does not expose ACP mode, so the current broker cannot use it as a substitute.
- Production packaging can remove the Codex native binary, but Claude requires either the SDK runtime bundle or a different ACP adapter implementation.

## Summary of Changes

- Removed the incorrect dev-only host wrapper and its documentation.
- Kept production packaging in the existing unified `scripts/package-acp-bundles.sh` flow, which produces both ACP bundles beside the host package.
- Generated the arm64 production-layout bundles beside `target/debug/acp-host` and verified the host discovers Claude Code without bundle environment variables.
- Claude Code is available at bundled adapter version `0.76.0`.
