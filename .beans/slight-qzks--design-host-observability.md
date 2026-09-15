---
# slight-qzks
title: Design host observability
status: completed
type: task
priority: normal
created_at: 2026-09-15T15:16:52Z
updated_at: 2026-09-15T15:17:07Z
---

Assess the current host observability setup and recommend a practical instrumentation architecture for logs, metrics, and traces.

## Summary of Changes\n\nReviewed the host-service, gateway-server, session-core, configuration, and architecture ADRs. Recommended keeping the existing diagnostics API as a support snapshot while introducing a host-owned observability facade backed by tracing, structured metrics, and optional OpenTelemetry export. The key design is to instrument gateway requests, session lifecycle/ACP calls, persistence, and pump health with correlation fields, while redacting prompts, tokens, paths, and raw ACP payloads. Suggested phased rollout: tracing facade and spans first, metrics second, OTLP exporter behind configuration third.
