---
# slight-mmll
title: preserve newlines in streamed markdown
status: completed
type: bug
priority: normal
created_at: 2026-09-12T23:30:08Z
updated_at: 2026-09-12T23:35:30Z
parent: slight-vewr
---

Ensure streamed assistant text preserves explicit newline characters through chunk aggregation and Markdown rendering. Add regression coverage for multiline streamed content.

## Summary of Changes

Preserved explicit line breaks through streamed assistant chunk aggregation and Markdown rendering by concatenating compatible text blocks and converting source newlines into Markdown hard breaks. The initial transcript load no longer receives a broad animation; only active streaming message content animates.

Verification: iOS Simulator and macOS `xcodebuild` builds succeed.
