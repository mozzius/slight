---
# slight-9u5k
title: bundle Claude ACP broker with host
status: completed
type: task
priority: normal
created_at: 2026-09-13T03:14:40Z
updated_at: 2026-09-13T03:21:50Z
parent: slight-xnju
---

Package the Claude Agent SDK ACP broker with the Slight host binary, matching the existing Codex ACP packaging approach where feasible.\n\n- [x] Inspect current Claude adapter and Codex bundle conventions\n- [x] Add reproducible Claude ACP bundle metadata/build path\n- [x] Launch the bundled broker from Rust with tests\n- [x] Document runtime, licensing, authentication, and limitations

## Summary of Changes\n\nAdded a target-specific Claude Agent ACP bundle builder with pinned npm/Node inputs, SDK native runtime, licenses, NOTICE, and provenance. Updated the Rust Claude adapter and host to prefer bundled Node plus the ACP broker, with explicit/global development overrides. Added bundle resolution tests and an opt-in real initialization smoke test. Documented packaging, signing, authentication, and the runtime-bundle limitation in ADR 0005.

Post-review fixes: bundle validation now requires the host's exact macOS architecture, Linux is not advertised before Linux artifacts exist, and explicit relative paths are canonicalized before child working-directory changes.
