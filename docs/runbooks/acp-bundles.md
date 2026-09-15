# ACP Runtime Bundles

The shipped host expects both real-agent ACP runtimes beside its executable:

```text
vendor/
├── codex-acp/<target-triple>/
└── claude-agent-acp/<target-triple>/
```

Build both runtimes with one command:

```bash
scripts/package-acp-bundles.sh \
  --target aarch64-apple-darwin \
  --target x86_64-apple-darwin
```

The same command accepts `--out` for a release staging directory. Copy both
directories into the host app or daemon package without renaming them. The Rust
adapters select the matching target triple automatically; they do not require
global Node, npm, or ACP broker installations.

The CI workflow builds and smoke-tests both brokers, validates their pinned
provenance, and uploads license/provenance sidecars. macOS release packaging
must still codesign the bundled Node and native agent executables before
notarization.
