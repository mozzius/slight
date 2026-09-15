---
# slight-m43h
title: support rich ACP content and tool updates
status: completed
type: task
priority: normal
created_at: 2026-09-12T19:15:39Z
updated_at: 2026-09-12T23:25:11Z
parent: slight-xnju
---

Preserve and normalize ACP image, audio, resource, and resource-link content plus tool-call diffs, terminal output, raw I/O, locations, and structured results. Expose safe gateway-v1 event payloads and Apple decoding/rendering models with real-agent fixtures.

## Checklist

- [x] Define normalized rich content types in acp-types (image/audio/resource/resource-link)
- [x] Normalize tool content (diffs, terminal refs), locations, raw I/O, structured results
- [x] Extend AgentEvent normalization to carry rich content
- [x] Extend session-core gateway payloads (ContentBlock, ToolCallPayload)
- [x] Update protocol/gateway-v1.md and add conformance fixture
- [x] Add Rust tests (acp-types, session-core, gateway-protocol)
- [x] Extend Apple decoding models and rendering
- [x] Add Swift tests for rich content and tool updates
- [x] Run cargo and swift test suites

## Summary of Changes

Preserved rich ACP content end to end instead of dropping everything but plain text.

- `acp-types`: new `content` module with `NormalizedContent` (text/image/audio/resource-link/embedded resource), `NormalizedToolContent` (content/diff/terminal), `DiffSummary`, `TerminalRef`, `ToolLocation`, and total conversions with unknown-variant and null-raw dropping. `ToolCallUpdate` and `AgentEvent::Message`/`Thought` now carry this content; normalization preserves content, locations, raw I/O, and sets the location-derived detail string.
- `session-core`: `ContentBlock` gained image/audio/resource-link/resource-text/resource-blob kinds and labelled base64/URI fields; `ToolCallPayload` gained `content[]`, `locations[]`, `raw_input`, `raw_output`; manager flattens normalized content into gateway payloads.
- `gateway-v1`: documented content-block and tool-call payload shapes; added `protocol/conformance/session-tool-call-rich.json` consumed by Rust and Swift conformance tests.
- Apple `SlightGateway`: `ContentBlock` decodes rich kinds/fields; new `ToolCallContent`/`Diff`/`TerminalRef`/`ToolLocation` models; `ToolCall` carries rich fields and merges partial updates; `ToolCallPayload` decodes/encodes all rich fields.
- Apple `SlightGatewayUI`: `MessageRowView` renders image (base64-decoded), audio, resource-link, and embedded-resource blocks safely (no implicit fetch/execute); `ToolCallRowView` renders diffs, content blocks, terminals, locations, and a collapsible raw result.
- Added real-agent coverage: the wire fake agent now emits an image chunk and a rich tool call, and `wire_session` gained a focused rich-content integration test.

Tests: `cargo test --workspace` all green; `swift test` 37 passed (6 new `RichContentTests`, updated conformance).

Boundary notes: terminal output is represented as an ACP terminal *reference* (terminal id); fetching terminal output requires the `terminal/output` client method, which is deliberately out of scope. Filesystem/session-control and negotiation changes were not touched. Binary media is preserved as labelled base64 for client rendering; MIME types are always included so clients can reject formats they do not understand.
