---
# slight-47k2
title: redesign composer as glass control surface
status: completed
type: feature
priority: normal
created_at: 2026-09-12T19:04:44Z
updated_at: 2026-09-13T02:09:50Z
---

Replace the plain composer with a liquid-glass control surface containing text input, nested model and effort menus, and a send button that becomes stop while the agent is running. Remove model/effort metadata from the empty session body and use the same composer configuration pattern for draft sessions.

## Summary of Changes

Replaced the plain composer with a liquid-glass control surface. It now contains the message field, nested Model and Effort menus, and a primary action that changes from Send to Stop while the agent is working. Draft sessions use the same glass composer configuration pattern, and the standalone session metadata strip was removed.

Verification: iOS Simulator `xcodebuild` succeeds.

Follow-up: moved the live session composer into a bottom overlay with transcript content margins so it floats over chat instead of participating in vertical layout.
