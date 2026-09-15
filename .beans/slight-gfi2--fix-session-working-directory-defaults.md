---
# slight-gfi2
title: fix session working directory defaults
status: completed
type: bug
priority: normal
created_at: 2026-09-13T13:56:02Z
updated_at: 2026-09-13T13:57:54Z
---

New sessions are created in the Slight process directory instead of the user-selected working directory. Ensure the requested directory is honored and provide a reasonable default location (the user home directory).

- [x] Trace session creation and working-directory propagation
- [x] Fix the working-directory fallback/default
- [x] Add or update tests
- [x] Verify with the relevant test suite

## Summary of Changes

- Propagated the requested session directory into ACP process launch and `session/new`.
- Resolved `~` and `~/...` against the host user home directory.
- Defaulted blank UI input and test fixtures to `~`.
- Added Rust coverage for empty, tilde, and explicit directory handling.
