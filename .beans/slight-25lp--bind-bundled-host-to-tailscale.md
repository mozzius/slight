---
# slight-25lp
title: bind bundled host to tailscale
status: completed
type: task
priority: normal
created_at: 2026-09-14T18:03:03Z
updated_at: 2026-09-14T18:04:19Z
---

When the macOS app starts its bundled acp-host, bind it to localhost and the discovered Tailscale IPv4 address when Tailscale is available. Fall back to localhost only when no Tailscale interface is available; never bind broadly to 0.0.0.0.



- [x] Discover an available Tailscale IPv4 address safely
- [x] Bind bundled acp-host to localhost and Tailscale when available
- [x] Fall back to localhost without broad interface binding
- [x] Verify Swift compilation and formatting

## Summary of Changes

- Bundled local host startup now passes localhost plus the first address from `tailscale ip -4`.
- If Tailscale is unavailable, the host remains localhost-only.
- The host is never started on `0.0.0.0` by the desktop path.
- `swift test` passes with 64 tests.
