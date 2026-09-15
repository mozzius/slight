---
# slight-lvwb
title: avoid false local host readiness timeout
status: completed
type: bug
priority: normal
created_at: 2026-09-15T15:55:36Z
updated_at: 2026-09-15T15:55:43Z
---

The bundled acp-host logs that it is listening, but LocalHostServiceController's NWConnection readiness probe times out after 300ms and kills the healthy process. Make readiness probing tolerate first-use network startup latency.

## Summary of Changes\n\n- Increased the bundled host readiness window from 5 seconds to 15 seconds.\n- Increased each NWConnection probe timeout from 300ms to 2 seconds so first-use Network framework latency cannot kill a healthy listener.
