# ADR 0005: Self-contained Claude Agent ACP packaging

Status: accepted (first packaging slice)

## Context

The Claude Code CLI is not the ACP process. Slight uses the
`@agentclientprotocol/claude-agent-acp` broker, which drives Anthropic's Claude
Agent SDK and speaks ACP over stdio. Requiring a global npm install is not
acceptable for a shipped host or a launchd process.

## Decision

Ship a target-specific bundle next to the host executable:

```text
claude-agent-acp/<triple>/
├── bin/node
├── lib/claude-agent-acp/       # ACP broker package
├── node_modules/                # broker, SDK, and JS dependency graph
│   └── @anthropic-ai/.../claude # native SDK runtime
├── licenses/
├── NOTICE
└── bundle.json
```

`acp-adapters::claude_code` launches `bin/node` with the broker entrypoint and
resolves bundles beside the host, in a macOS app's `Resources`, or through
`SLIGHT_CLAUDE_CODE_BUNDLE_DIR`. An explicit executable remains an override.
Global `claude-agent-acp` lookup is disabled unless development mode or
`SLIGHT_CLAUDE_CODE_ALLOW_GLOBAL=1` enables it.

The bundle is produced by `scripts/bundle-claude-agent-acp.sh`, with versions
and npm/Node integrity pins in `packaging/claude-agent-acp/versions.env`.

## Authentication and limitations

Packaging removes the Node/npm/broker installation prerequisite, not the need
for Claude authentication. The broker still uses Anthropic's supported SDK
configuration and the user's Claude configuration or explicitly forwarded
`ANTHROPIC_*` credentials. The native platform runtime is third-party code and
must be included in macOS signing and notarization.

This is a runtime bundle beside the host binary, matching the Codex packaging
model. It is not a literal Rust single-file embedding; that would make native
SDK extraction, signing, and updates harder without improving the process
boundary.
