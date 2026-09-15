# ADR 0004: Self-contained Codex ACP broker packaging

Status: accepted (first packaging slice)

## Context

Codex is not an ACP server. Slight reaches it through the
[`@agentclientprotocol/codex-acp`](https://github.com/agentclientprotocol/codex-acp)
broker, which launches the Codex app-server and translates between the Codex
JSON-RPC protocol and ACP. `acp-adapters::codex` currently locates `codex-acp`
on `PATH` (or via `SLIGHT_CODEX_BIN`) and launches it over stdio.

That is fine for a developer machine with a global Node/npm install, but it is
not acceptable for a shipped macOS host: a fresh Mac has no `node`, no `npm`,
and no `codex-acp`, and a launchd-managed `acp-host` gets no login shell
`PATH`. The shipped host must run Codex without a globally installed
npm/Bun/codex-acp.

This ADR records what the upstream package actually contains, the packaging
strategy chosen for shipped hosts, and why the alternatives were rejected.

## Findings (upstream inspection, codex-acp 1.11.0 / codex 0.153.4)

- `codex-acp` is published to npm as a **single esbuild ESM bundle**,
  `dist/index.js` (~1.3 MB). `build.mjs` bundles every dependency and marks only
  `@openai/codex` external. The only non-builtin runtime reference in the bundle
  is `createRequire(import.meta.url).resolve("@openai/codex/bin/codex.js")`, and
  it is only reached when `CODEX_PATH` is unset.
- The repo can build Bun single-file binaries (`bun build --compile`), but those
  artifacts are **not** in the npm package (`files` ships only `dist/index.js`)
  and **not** published as GitHub release assets for current versions. Release
  assets exist only up to `v0.0.38`; even those are just the compiled broker
  (≈23–25 MB) and still require `CODEX_PATH` to point at a Codex binary.
- `@openai/codex` is a thin Node launcher, `bin/codex.js`, plus platform
  optional dependencies. The per-platform package
  (`@openai/codex@<ver>-darwin-arm64`) ships a `vendor/<triple>/` tree
  containing `bin/codex` (≈220 MB), `bin/codex-code-mode-host` (≈62 MB),
  `codex-path/rg` (≈4 MB), and `codex-resources/zsh`.
- `codex-acp` accepts `CODEX_PATH` to run a specific Codex executable directly,
  bypassing `@openai/codex/bin/codex.js` entirely. Driving the broker this way
  with the vendored native binary completes ACP `initialize` with no auth and no
  npm packages on disk (verified against codex-acp 1.11.0 + codex 0.153.4).
- Both `codex-acp` and `@openai/codex` are Apache-2.0. Node is MIT.

## Decision

Ship a **vendored runtime + package bundle**, per target triple:

```text
codex-acp/<triple>/
├── bin/node                      # pinned Node runtime
├── bin/codex                     # pinned native Codex
├── bin/codex-code-mode-host
├── codex-path/rg                 # Codex's relative search binary
├── codex-resources/zsh
├── lib/codex-acp/index.js        # codex-acp dist bundle
├── licenses/                     # upstream LICENSE files
├── NOTICE                        # human-readable third-party notices
└── bundle.json                   # provenance: versions, integrity, licenses
```

The adapter launches `bin/node lib/codex-acp/index.js` and sets
`CODEX_PATH=<bundle>/bin/codex`. The native Codex tree is kept in its original
relative layout so its own `codex-path/rg` / `codex-resources` lookups keep
working.

Bundles live next to the host executable. Resolution order in
`acp-adapters::codex` is:

1. explicit `CodexConfig.program` / `SLIGHT_CODEX_BIN` (development + tests);
2. a bundle directory from `CodexConfig.bundle_dir` / `SLIGHT_CODEX_BUNDLE_DIR`,
   or discovered relative to the running host:
   `<exe_dir>/codex-acp/<triple>`,
   `<exe_dir>/../Resources/codex-acp/<triple>` (macOS app bundle), or
   `<exe_dir>/../codex-acp/<triple>`;
3. `codex-acp` on `PATH`, **only** when
   `CodexConfig.allow_global_fallback` / `SLIGHT_CODEX_ALLOW_GLOBAL=1` is set.

The host sets `allow_global_fallback` from `HostConfig.dev_mode`, so a shipped
host resolves the bundle and a developer host can opt into the global install.
Nothing silently falls back.

### Version pinning and reproducibility

`packaging/codex-acp/versions.env` pins the codex-acp version, the
`@openai/codex` version, the Node version, and the sha512 integrity of every
npm tarball plus the sha256 of every Node tarball. `scripts/bundle-codex-acp.sh`
downloads from the canonical registry/nodejs.org URLs, verifies each artifact
against the pinned hash before use, and writes `bundle.json` recording what
shipped. Re-running with the same pins produces the same layout.

### Platform and architecture

The bundle is keyed by Rust-style target triple. `codex.rs` maps the host
`(OS, ARCH)` to the codex triple (`aarch64-apple-darwin`,
`x86_64-apple-darwin`, and the Linux/Windows equivalents) and only looks in the
matching directory, so an x64/arm64 mismatch is a clear "missing bundle"
instead of an exec failure. Both shipped macOS architectures
(`aarch64-apple-darwin`, `x86_64-apple-darwin`) are pinned and buildable on
either runner because the script only downloads and unpacks. Linux is mapped in
the resolver but its pins are not maintained, and Windows packaging is
intentionally deferred; both are clear build-time errors rather than silent
fallbacks.

### Authentication and environment

Packaging does not change Codex auth. The broker still advertises `chat-gpt`
and `api-key` methods, reuses Codex's own credential store under `HOME`, and
accepts `CODEX_API_KEY` / `OPENAI_API_KEY`. `acp-adapters` keeps the same
env-clear allowlist, so credentials are only forwarded when a caller puts them
in `AgentLaunchConfig.environment`. The bundle adds exactly one variable,
`CODEX_PATH`, which points at the vendored native binary.

### Licensing and SBOM

The bundler copies the upstream `LICENSE` for codex-acp, `@openai/codex`, and
Node into `licenses/`, writes a `NOTICE`, and records package names, versions,
source URLs, and integrity in `bundle.json`. Because codex-acp's bundle inlines
its JavaScript dependencies, the only separately shipped components are these
three (all Apache-2.0/MIT); `bundle.json` is the SBOM-lite of record.

### macOS signing

The vendored `bin/node` and `bin/codex` are third-party Mach-O binaries. A
signed/notarized host must sign them with the app (or with hardened runtime and
the same team ID) during packaging; the bundler produces an unsigned tree and
leaves signing to the release pipeline. This is called out in the runbook so the
release step is not forgotten.

## Alternatives considered

- **Bun single-file compile.** Upstream's own path, but the resulting broker
  still needs a separate `CODEX_PATH`, current release binaries are not
  published, and it adds Bun to the build toolchain for no runtime benefit over
  a vendored Node + the same `dist/index.js`. Rejected for the first slice.
- **Node SEA.** Embeds the bundle in a Node binary via `postject`, but keeps
  the same Node footprint, adds a fragile post-build injection step, and still
  needs the native Codex tree. No advantage over vendored Node + `index.js`.
  Rejected.
- **Vendoring the full npm tree + npm at runtime.** Requires running `npm
  install`/`npx` on the host or shipping `node_modules`; `dist/index.js` already
  inlines its JavaScript dependencies, so this is strictly more surface for no
  gain. Rejected.
- **Native Rust ACP↔Codex translation.** Drop the JS broker and speak the Codex
  app-server protocol directly from `acp-adapters`. Smallest shipped runtime
  (native Codex only) and no Node, but it reimplements and must track a large,
  moving upstream translation surface. Rejected for now; kept as the long-term
  option in "Follow-ups".

## Consequences

- A shipped macOS host needs no global npm/Bun/codex-acp; the bundle is
  self-contained apart from the user's Codex credentials.
- The bundle is large: the native Codex tree is ≈286 MB per architecture
  (Codex is ≈220 MB, code mode host ≈62 MB, ripgrep ≈4 MB). This is inherent to
  shipping Codex and should be measured per release.
- Discovery is deliberate: a mismatched or missing bundle reports an actionable
  "missing executable" error instead of silently using a global install.
- Development keeps working with `SLIGHT_CODEX_BIN` or an explicit
  `allow_global_fallback`.

## Follow-ups

- Native Rust Codex app-server adapter to remove Node from the shipped tree
  (separate bean; largest long-term reduction).
- Windows bundle extraction (zip) if a Windows host ships.
- Universal (arm64 + x64) macOS distribution, or per-arch app slices.
- Codesign/notarize the vendored binaries as part of the macOS release pipeline
  (`slight-bnbd`).
