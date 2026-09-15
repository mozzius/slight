---
# slight-wx2t
title: start sessions from chat surface
status: completed
type: feature
priority: normal
created_at: 2026-09-12T18:55:37Z
updated_at: 2026-09-12T18:57:59Z
---

Replace the New Session form with a session-shaped draft screen. Let users configure harness/model/effort and working directory in the session header, type immediately, and create the real host session with the first prompt before transitioning to the live session transcript.

## Summary of Changes

Replaced the modal `NewSessionSheet` form with `NewSessionDraftView`. The new session action opens a session-shaped draft surface, with title, harness, model, effort, working directory, and an immediately available composer. The first message calls `session.create` with `initial_prompt`; on success the draft transitions directly to the real attached session detail.

Desktop renders the draft in the detail column; iOS pushes it as a navigation destination. Verification: iOS Simulator `xcodebuild` succeeds.
