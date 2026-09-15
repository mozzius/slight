---
# slight-qrnd
title: replace hardcoded session configuration picker
status: completed
type: task
priority: normal
created_at: 2026-09-12T18:49:36Z
updated_at: 2026-09-13T03:01:00Z
parent: slight-y65m
blocked_by:
    - slight-sdsj
---

Remove hardcoded agent, model, and effort arrays/defaults from NewSessionSheet. Drive picker labels, identifiers, available models, custom config options, and effort/thought-level choices from the host ACP catalog. Update FakeHost previews to consume the same typed catalog fixture.

## Summary of Changes

Removed static harness/model/effort values from the new-session draft. Harnesses now use host-advertised display names, and model/thought-level choices come from the selected harness catalog entry.

Verification: Apple Swift package tests pass.
