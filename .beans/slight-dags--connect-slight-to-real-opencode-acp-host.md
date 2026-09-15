---
# slight-dags
title: connect Slight to real OpenCode ACP host
status: in-progress
type: feature
priority: normal
created_at: 2026-09-12T13:38:21Z
updated_at: 2026-09-12T13:39:52Z
parent: slight-3b0e
---

Deliver the first real end-to-end path with no fake host in the application flow: one Rust host daemon, WebSocket plus JSON gateway, the real Slight Multiplatform client, and OpenCode launched as an ACP stdio agent. Remove FakeHost from app composition and Debug launch behavior. Keep the ACP protocol boundary distinct from the gateway protocol.\n\n- [ ] Replace raw TCP gateway transport with WebSocket plus JSON\n- [ ] Add Apple WebSocket client handshake compatibility and real-host connection\n- [ ] Remove FakeHost from app composition and critical-path UI\n- [ ] Launch opencode acp as a supervised ACP stdio agent\n- [ ] Wire ACP sessions through session-core and gateway events\n- [ ] Create and stream one real OpenCode session\n- [ ] Add end-to-end smoke coverage and document local run commands
