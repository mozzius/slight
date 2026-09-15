# Third-party notices: bundled Codex ACP runtime

The shipped Slight host bundles a self-contained Codex ACP runtime under
`codex-acp/<target-triple>/`. The bundler
(`scripts/bundle-codex-acp.sh`) copies each component's upstream `LICENSE` into
the bundle's `licenses/` directory and records versions, source URLs, and
integrity hashes in `bundle.json`. This file is the human-readable summary.

The runtime currently ships three components per architecture:

| Component | Package | Version | License |
| --- | --- | --- | --- |
| Codex ACP broker | `@agentclientprotocol/codex-acp` | 1.11.0 | Apache-2.0 |
| Codex | `@openai/codex` | 0.153.4 | Apache-2.0 |
| Node.js | `node` | 24.19.0 | MIT |

`dist/index.js` from `codex-acp` is an esbuild bundle that inlines its
JavaScript dependencies (`@agentclientprotocol/sdk`, `diff`, `open`,
`vscode-jsonrpc`, `zod`); those dependencies are therefore not separate
components in the shipped tree, but their licenses are covered by the bundled
bundle's `LICENSE` and the upstream package metadata. The native Codex tree is
taken whole from the `@openai/codex` platform package
(`@openai/codex@<version>-<platform>`).

Because the bundled `bin/node` and `bin/codex` are third-party Mach-O binaries,
a signed/notarized host must codesign them during packaging. See
[`docs/architecture/adr-0004-codex-acp-packaging.md`](../../docs/architecture/adr-0004-codex-acp-packaging.md).

To update a component:

1. Edit the version and integrity hashes in `packaging/codex-acp/versions.env`.
2. Re-run `scripts/bundle-codex-acp.sh --target <triple>`.
3. Run the bundle smoke test (`.github/workflows/codex-acp-bundle.yml`).
4. Commit the updated `versions.env` and this table.
