---
# slight-098z
title: wire Claude and Codex bundles into packaging
status: completed
type: task
priority: normal
created_at: 2026-09-13T14:01:28Z
updated_at: 2026-09-13T14:06:45Z
parent: slight-xnju
---

Make the release/CI packaging path build and validate both Claude Agent ACP and Codex ACP bundles so shipped hosts find both runtimes automatically.\n\n- [x] Add unified bundle packaging entry point\n- [x] Update CI/release workflow for both agents\n- [x] Validate both bundle smoke tests and provenance

## Summary of Changes\n\nAdded
added 104 packages in 953ms as the single packaging entry point for Claude Agent ACP and Codex ACP across one or more macOS target triples. Updated the existing reusable GitHub Actions workflow to build, validate architecture/provenance, smoke-test both brokers, and upload both agents' sidecars. Added the shared ACP bundle runbook and fixed the existing Codex bundler's EXIT cleanup status bug that prevented composition with other bundle steps.

Correction: the unified entry point is scripts/package-acp-bundles.sh. No generated vendor artifacts are retained in the repository; release jobs generate them into staging.
