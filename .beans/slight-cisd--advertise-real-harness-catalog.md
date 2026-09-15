---
# slight-cisd
title: advertise real harness catalog
status: completed
type: feature
priority: normal
created_at: 2026-09-12T18:49:36Z
updated_at: 2026-09-13T03:06:11Z
parent: slight-y65m
---

Add a host/gateway catalog of available harnesses with stable identifiers, friendly display names, versions, executable availability, ACP session configuration options, model choices, effort/thought levels, and custom host-specific settings. Probe or derive this from registered adapters and ACP session/new rather than hardcoded client values.

## Summary of Changes

The host/gateway now exposes a typed agent catalog sourced from real ACP probes, including friendly harness names, availability, versions, and session configuration choices. The Apple client decodes the catalog and uses negotiated model/thought-level options in the draft and live composer menus.

Verification: Rust workspace tests and Apple Swift package tests pass.

Follow-up: normalized custom ACP categories as wire strings so Apple catalog decoding accepts agent-specific configuration categories.
