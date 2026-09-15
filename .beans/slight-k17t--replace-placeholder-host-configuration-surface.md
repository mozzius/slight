---
# slight-k17t
title: replace placeholder host configuration surface
status: completed
type: task
priority: normal
created_at: 2026-09-12T18:49:36Z
updated_at: 2026-09-13T03:01:07Z
parent: slight-y65m
blocked_by:
    - slight-sdsj
---

Implement host.configuration as a real typed host configuration/catalog result instead of the documented placeholder alias for HostStatusResult. Expose supported harnesses and their configuration through the gateway and Apple decoding/tests.

## Summary of Changes

Replaced the placeholder host configuration surface with a typed `agent_catalog` result. The gateway now exposes available harness descriptors and normalized ACP configuration options for Apple clients.

Verification: Rust workspace tests pass.
