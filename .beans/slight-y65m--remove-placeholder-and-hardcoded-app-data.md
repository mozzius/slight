---
# slight-y65m
title: remove placeholder and hardcoded app data
status: in-progress
type: epic
priority: normal
created_at: 2026-09-12T18:48:55Z
updated_at: 2026-09-12T18:50:20Z
---

Map and replace placeholder, fixture-only, and hardcoded runtime data across the host, gateway contract, and Apple client. Keep intentional product defaults separate from values that must be advertised by the host or ACP agent.

## Inventory

### Must Become Host/ACP-Driven

- `clients/apple/SlightKit/Sources/SlightGatewayUI/Session/NewSessionSheet.swift`: hardcoded harness IDs, display labels, model IDs, and effort values.
- `rust/host-service/src/host.rs`: adapter registry currently exposes Fake plus OpenCode only; Claude/Codex remain absent from the real host.
- `rust/gateway-protocol/src/admin.rs` and host status: supported agents are bare string IDs with no display names, versions, availability, or configuration catalog.
- `protocol/gateway-v1.md`: `host.configuration` is explicitly documented as a placeholder alias for `HostStatusResult`.
- `rust/acp-types` and gateway ACP metadata: ACP `session/new` config options, model choices, custom model configuration, and thought/effort levels are not fully normalized to clients.
- `rust/session-core/src/types.rs`: `model` and `effort` are required fields, but current values are client-selected hardcoded strings rather than validated agent configuration values.

### Development/Infrastructure Defaults To Isolate

- `rust/host-service/src/config.rs` and `rust/host-cli/src/main.rs`: loopback bind, port `8787`, dev token `dev`, and development mode defaults.
- `clients/apple/SlightKit/Sources/SlightGatewayUI/Common/HostConnectionSettingsView.swift`: loopback URL prompt and Use Loopback action.
- `clients/apple/SlightKit/Sources/SlightGatewayUI/Development/*`: FakeHost and preview fixtures are intentional development/test data, not production placeholders.
- `rust/session-store`: SQLite implementation exists, but the running host still uses `InMemoryStore`; restart persistence is tracked separately.

### Documentation/Contract Drift

- `protocol/gateway-v1.md` still says only the fake agent ships and labels host configuration as placeholder.
- Gateway resume/replay aliases and older-host fallback behavior conflict with the repository's pre-release no-compatibility rule.

Child beans: `slight-cisd`, `slight-qrnd`, `slight-k17t`, `slight-0tgf`, `slight-tbok`. Existing related work is grouped here: `slight-sdsj`, `slight-uy6h`, `slight-bg6w`, and `slight-w8z7`.
