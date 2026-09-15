---
# slight-0rvz
title: add pairing and authorization
status: todo
type: feature
priority: normal
created_at: 2026-09-12T08:24:46Z
updated_at: 2026-09-12T08:24:57Z
parent: slight-4yk6
blocked_by:
    - slight-8ihb
---

Add production authorization independent of Tailscale. Implement short-lived one-time pairing artifacts, scoped revocable device credentials, operation-level authorization, secure credential storage interfaces, redacted logs, and safe listener binding. Include development-only loopback pairing without weakening production behavior.\n\n- [ ] Define device identity and credential model\n- [ ] Implement pair and revoke CLI flows\n- [ ] Enforce read/input/permission/create/host scopes\n- [ ] Add listener and TLS/Tailscale configuration\n- [ ] Add security and log-redaction tests
