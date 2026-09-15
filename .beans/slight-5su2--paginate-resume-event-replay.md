---
# slight-5su2
title: paginate resume event replay
status: completed
type: bug
priority: normal
created_at: 2026-09-15T15:59:55Z
updated_at: 2026-09-15T16:00:29Z
---

Resume reconnect currently uses events.replay/session.attach and returns the entire retained journal in one WebSocket ack. Bound replay responses and have the Apple client request subsequent pages until latest_sequence is reached.

## Summary of Changes\n\n- Bound `events.replay` responses to the host-advertised `max_replay_events` value.\n- Added client-side replay paging: after each bounded response, request the next sequence range until `latest_sequence` is reached.
