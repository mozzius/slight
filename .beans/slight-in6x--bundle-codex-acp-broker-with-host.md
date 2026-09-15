---
# slight-in6x
title: bundle codex-acp broker with host
status: completed
type: task
priority: normal
created_at: 2026-09-12T23:26:53Z
updated_at: 2026-09-13T01:52:40Z
parent: slight-xnju
---

Make Codex ACP self-contained for shipped host builds. Determine the supported codex-acp package/version and whether Bun compile, Node SEA, or a Rust/native packaging approach is appropriate. Vendor or reproducibly install the broker, produce macOS architecture artifacts, launch it from acp-adapters without requiring global npm/Bun, preserve auth behavior, record licenses/SBOM, and add CI/release smoke tests. Do not silently fall back to a globally installed codex-acp in production.

## Packaging checklist

- [x] Record packaging decision (ADR) with alternatives comparison
- [x] Pin codex-acp / @openai/codex / Node versions + integrity in-repo
- [x] Add reproducible bundle script with per-arch layout and checksum verification
- [x] Resolve bundled broker first in CodexAdapter; explicit dev-only global fallback
- [x] Collect upstream licenses and emit bundle provenance (SBOM-lite)
- [x] Add focused resolution tests and an opt-in bundle initialize smoke test
- [x] Add CI/release smoke workflow for macOS bundles
- [x] Run focused tests/builds and record summary

## Summary of Changes

Recommendation: ship a **vendored runtime + package bundle, keyed by target
triple**. Bundle the pinned Node runtime, the `codex-acp` `dist/index.js`
esbuild bundle, and the native `@openai/codex` platform tree; launch
`bin/node lib/codex-acp/index.js` with `CODEX_PATH=<bundle>/bin/codex`. This is
the smallest self-contained runtime that needs no global npm/Bun/codex-acp.
Bun `--compile` and Node SEA were rejected because the broker still needs a
separate native Codex tree and neither removes the Node/JS runtime; a native
Rust Codex translator remains the long-term option.

Packaging decision + alternatives: `docs/architecture/adr-0004-codex-acp-packaging.md`.

Version pinning: `packaging/codex-acp/versions.env` pins codex-acp 1.11.0,
`@openai/codex` 0.153.4 (plus the `-darwin-arm64`/`-darwin-x64` platform
packages), and Node 24.19.0 by sha512 (npm integrity) / sha256 (nodejs.org).
All pins were re-verified against the live registries and the full bundle was
built end-to-end for `aarch64-apple-darwin` and `x86_64-apple-darwin`.

Bundle build: `scripts/bundle-codex-acp.sh` downloads from canonical URLs,
verifies every hash before unpacking, and emits `bundle.json` (SBOM-lite),
`NOTICE`, and upstream licenses per triple. `vendor/` is now git-ignored.

Adapter resolution: `rust/acp-adapters/src/codex.rs` resolves explicit override
> bundle > opt-in global fallback, derives the triple from host `(OS, ARCH)`,
and reports a missing bundle as an actionable error. Global fallback is gated on
`allow_global_fallback` / `SLIGHT_CODEX_ALLOW_GLOBAL=1` (host sets it from
`dev_mode`). Auth/env behavior is unchanged: `HOME` is allowlisted for Codex's
credential store and credentials are only forwarded explicitly.

CI/release: `.github/workflows/codex-acp-bundle.yml` builds both macOS arches,
verifies sidecars/architecture/versions, runs the opt-in initialize smoke test
plus the adapter resolution tests, uploads provenance, and is
`workflow_call`-able for release gating. `docs/runbooks/codex-acp-bundle.md`
documents build, smoke, signing, and troubleshooting.

## Verification

- `cargo test -p acp-adapters`: 41 tests pass (29 lib + 5 matrix + 3 opencode +
  3 wire conformance + 1 smoke skip).
- `SLIGHT_CODEX_ACP_SMOKE=1 SLIGHT_CODEX_BUNDLE_DIR=<arm64 bundle> cargo test -p
  acp-adapters --test codex_bundle_smoke -- --nocapture`: bundled broker
  completes ACP v1 `initialize` as `@agentclientprotocol/codex-acp` with auth
  methods `api-key`, `chat-gpt`, no global npm/Bun.
- Bundle script built both triples with checksum verification; `file` confirms
  arm64/x86_64 Mach-O.

Blockers/notes: the produced tree is unsigned (macOS release pipeline must
codesign/notarize `bin/node` and `bin/codex`; see `slight-bnbd`). Linux/Windows
pins are not maintained (macOS-only shipped host); x64 execution smoke is only
covered natively (CI builds both but executes the arm64 bundle on `macos-14`).
