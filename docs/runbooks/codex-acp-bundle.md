# Runbook: Codex ACP bundle

The shipped macOS host runs Codex without a global Node/npm/Bun install by
launching a self-contained bundle produced from the pins in
[`packaging/codex-acp/versions.env`](../../packaging/codex-acp/versions.env).
The packaging decision is recorded in
[`docs/architecture/adr-0004-codex-acp-packaging.md`](../architecture/adr-0004-codex-acp-packaging.md).

## Layout

```text
vendor/codex-acp/<target-triple>/
├── bin/node                      # pinned Node runtime
├── bin/codex                     # pinned native Codex
├── codex-path/  codex-resources/ # Codex's relative lookups, kept whole
├── lib/codex-acp/index.js        # codex-acp broker (esbuild bundle)
├── licenses/                     # upstream LICENSE files
├── NOTICE                        # human-readable third-party notices
└── bundle.json                   # provenance: versions, URLs, integrity
```

`vendor/` is a build output and is git-ignored; only `versions.env` and the
tracked Codex license are committed.

## Build

```bash
# Host architecture.
scripts/bundle-codex-acp.sh

# Both shipped macOS architectures.
scripts/bundle-codex-acp.sh --target aarch64-apple-darwin --target x86_64-apple-darwin

# Drop the optional ~62 MB code-mode host binary.
scripts/bundle-codex-acp.sh --minimal
```

The script downloads from canonical npm/nodejs.org URLs and verifies every
artifact against `versions.env` **before** unpacking. A hash mismatch aborts.
To bump a component, update its version and integrity hash in
`versions.env`, rebuild, and run the smoke test below.

## Smoke test

```bash
SLIGHT_CODEX_ACP_SMOKE=1 \
SLIGHT_CODEX_BUNDLE_DIR="$PWD/vendor/codex-acp/aarch64-apple-darwin" \
cargo test -p acp-adapters --test codex_bundle_smoke -- --nocapture
```

This completes the ACP `initialize` handshake with the bundled Node + broker +
native Codex. It needs no credentials and never falls back to a global install.
The `codex-acp bundle` GitHub workflow runs the same build and smoke test for
`packaging/**`, `scripts/bundle-codex-acp.sh`, and `rust/acp-adapters/**`
changes, and is `workflow_call`-able for release gating.

## Release and signing

The bundler produces an **unsigned** tree. Because `bin/node` and `bin/codex`
are third-party Mach-O binaries, the macOS release pipeline must codesign (and
notarize) them with the host, or with hardened runtime and the same Team ID,
before shipping. Do not forget these nested binaries; the outer app signature
alone will not cover them. (See `slight-bnbd`.)

## Troubleshooting

- **`no Codex ACP broker found`** — no bundle next to the host, no
  `SLIGHT_CODEX_BIN`. Build the bundle or set `SLIGHT_CODEX_BUNDLE_DIR`. The
  global fallback is intentionally disabled outside dev mode.
- **hash mismatch** — an upstream artifact changed or `versions.env` drifted.
  Re-verify the registry integrity and commit the fix; never bypass the check.
- **architecture mismatch** — the bundle triple is derived from the host
  `(OS, ARCH)`. Building only `aarch64-apple-darwin` and running on an Intel
  host (or a Rosetta shell) reports a missing bundle rather than an exec error.
