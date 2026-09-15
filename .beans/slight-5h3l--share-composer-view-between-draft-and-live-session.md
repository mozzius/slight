---
# slight-5h3l
title: share composer view between draft and live sessions
status: completed
type: feature
priority: normal
created_at: 2026-09-13T12:56:44Z
updated_at: 2026-09-13T13:00:44Z
---

Unify the draft and live composer surfaces into one reusable SwiftUI view. Preserve live canCancel behavior and apply platform-aware enabled/disabled send button colors.


## Tasks

- [x] Replace duplicate draft/live composer implementations with one shared view
- [x] Apply adaptive enabled/disabled send button colors
- [x] Verify Apple build


## Summary of Changes

- Replaced the separate draft composer with the shared binding-based ComposerView.
- Preserved live send/cancel actions and disabled explanations through parent-provided state.
- Added adaptive button colors: black/white backgrounds by color scheme, inverted arrow color, and gray/white disabled state.
- Verified the macOS Xcode build succeeds.


## Follow-up

- [x] Invert enabled button contrast colors and soften the disabled system color


## Correction

Enabled buttons now use black with a white arrow in light mode and white with a black arrow in dark mode. Disabled buttons use a subtle semantic secondary color with a white arrow. macOS build reverified.


## Follow-up

- [x] Make the cancel button transparent with adaptive primary text color


## Correction

Cancel now uses a transparent background and the adaptive primary text color. macOS build reverified.
