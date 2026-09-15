---
# slight-vgs0
title: polish macOS target of Apple client
status: todo
type: feature
priority: normal
created_at: 2026-09-12T08:24:46Z
updated_at: 2026-09-12T08:41:54Z
parent: slight-4yk6
blocked_by:
    - slight-3ude
    - slight-0rvz
    - slight-tnbx
---

Complete the macOS destination of the shared Apple Multiplatform gateway client. Do not create a separate client architecture or duplicate protocol/connection logic. Add macOS-specific windowing, menu/keyboard behavior, loopback host discovery/configuration, Keychain integration, and macOS UI polish while consuming the shared app target.

- [ ] Add macOS target integration
- [ ] Add loopback host connection UX
- [ ] Add macOS lifecycle, menus, and keyboard behavior
- [ ] Add macOS credential integration
- [ ] Build and test the macOS destination

\n## Host Administration\n\nAdd the macOS administration surface over the Rust host API: host status, lifecycle controls, pairing and device revocation, diagnostics, configuration visibility, and session inspection. Keep process supervision in the Rust host service.
