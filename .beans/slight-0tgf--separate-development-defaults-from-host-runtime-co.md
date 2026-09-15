---
# slight-0tgf
title: separate development defaults from host runtime config
status: todo
type: task
created_at: 2026-09-12T18:49:36Z
updated_at: 2026-09-12T18:49:36Z
parent: slight-y65m
---

Audit loopback endpoint, port, dev token, host name, fake host fixtures, and default working-directory values. Keep explicit DEBUG/local development defaults, but make production/runtime configuration come from host configuration or pairing instead of silently using development values.
