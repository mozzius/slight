---
# slight-sdsj
title: advertise host agent configurations
status: completed
type: feature
priority: normal
created_at: 2026-09-12T18:37:35Z
updated_at: 2026-09-13T03:00:34Z
parent: slight-y65m
---

Replace hardcoded client agent/model/effort picker values with host-advertised harness descriptors and actual per-harness model/configuration options. Surface custom host configurations, friendly display names, effort levels, and supported values through the gateway and Apple client.

## Summary of Changes

Connected the existing ACP capability matrix to host configuration. The host now probes each registered available harness through ACP initialize/session/new and exposes its display name, version, availability, and normalized model/thought-level/config options through `host.configuration`.

Verification: workspace Rust tests pass.
