---
# slight-sij9
title: refine thinking status presentation
status: completed
type: feature
priority: normal
created_at: 2026-09-13T13:07:26Z
updated_at: 2026-09-13T14:02:33Z
---

Use the thinking disclosure as the only working indicator. Hide sub-five-second durations, show a spinner and thinking label while active, and a checkmark with thought duration when complete.


## Tasks

- [x] Update active and completed thinking labels
- [x] Remove the standalone agent-working indicator
- [x] Verify Apple build


## Summary of Changes

- Active thinking now shows a live spinner and `thinking`, adding the duration only from 5 seconds onward.
- Completed thinking shows a checkmark and `thought`, adding `for Ns` only from 5 seconds onward.
- Removed the standalone `Agent is working` row.
- Verified the macOS Xcode build succeeds.


## Follow-up

- [x] Show active thinking before reasoning content exists
- [x] Always show formatted thinking duration


## Correction

Active thinking now appears for streaming assistant messages even before reasoning content arrives. Durations are always visible: `10.5s` below one minute and `2m 32s` at or above one minute. macOS build reverified.


## Follow-up

- [x] Drive thinking activity from session working state instead of message streaming flag


## Correction

Thinking activity now comes from the session working state for the latest assistant message. Active labels show a spinner with `thinking` or a short `Thinking: ...` summary and no duration. Completed labels show a checkmark with `Thought: ... (2.9s)` or the formatted duration. macOS build reverified.


## Follow-up

- [x] Preserve Codex reasoning blocks when final assistant updates omit them


## Correction

Final assistant updates now preserve previously received reasoning blocks when the provider omits them from the final payload, keeping the completed thought summary visible. macOS build reverified.


## Correction

Thinking now activates only when the latest assistant block is reasoning while the session is working. Normal output streaming no longer displays the thinking spinner. macOS build reverified.


## Follow-up

- [x] Add host-measured reasoning timing to turn-ended events
- [x] Render host-measured duration in completed thought UI


## Correction

Reasoning timing is now measured by session-core from the first to last ACP thought chunk and emitted on `session.turn_ended`. The Apple client uses that host duration instead of message-view lifetime. Rust workspace and macOS build pass.
