---
# slight-k34r
title: generate session names on device
status: in-progress
type: task
priority: normal
created_at: 2026-09-12T19:12:09Z
updated_at: 2026-09-12T19:12:35Z
---

Use Apple's FoundationModels on-device model to generate a short session name from the first prompt when available. If unavailable or generation fails, truncate the first message. Remove the title field from new-session creation and allow title editing from the session menu.

## Tasks

- [ ] Read AGENTS.md, protocol docs, and relevant Rust/Apple sources
- [ ] Add Rust gateway protocol rename-session command + event, persist via session-store
- [ ] Add session-core rename handling + tests
- [ ] Add Apple SessionNameGenerator (FoundationModels + truncation fallback) + tests
- [ ] Remove title from NewSessionDraftView / new-session creation
- [ ] Add title editing from live session menu/header
- [ ] Update conformance fixtures + protocol docs
- [ ] Run targeted Rust/Apple tests/builds
