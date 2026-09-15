---
# slight-lejk
title: implement desktop session hierarchy
status: completed
type: feature
priority: normal
created_at: 2026-09-12T17:28:30Z
updated_at: 2026-09-12T18:22:52Z
---

Implement the agreed session-first Apple UI hierarchy: NavigationSplitView desktop layout, searchable session sidebar, compact host settings toolbar action, session detail metadata header, and model/effort session configuration carried through the gateway models and create flow.

## Summary of Changes

Implemented the session-first Apple hierarchy. Desktop now uses `NavigationSplitView` with a searchable session sidebar and a detail column. Host connection state is represented by a compact toolbar menu with connect/disconnect and host settings actions. Session rows and detail headers show agent, model, effort, working directory, status, and activity context. New-session creation now requires model and effort values, which are carried through the Rust session and gateway contracts without compatibility fallbacks.

Verification: `cargo test --workspace` passes. The Apple multiplatform `xcodebuild` succeeds. `swift test` compiles and runs, with the pre-existing seven `SessionViewModelBehaviorTests` timeout failures remaining.
