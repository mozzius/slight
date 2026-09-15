---
# slight-fvno
title: fix draft send and dynamic model choices
status: completed
type: bug
priority: normal
created_at: 2026-09-12T18:59:38Z
updated_at: 2026-09-13T03:01:14Z
---

Fix the new-session draft composer send action and replace its hardcoded model list with model choices advertised by the selected harness from the host catalog.

## Progress

Fixed the immediate send failure: the draft no longer requires a manually entered working directory, uses `.` when blank, and displays why sending is disabled. Host-advertised model selection remains pending on the harness catalog work tracked by `slight-sdsj` and `slight-cisd`.

## Summary of Changes

Fixed the draft composer send blocker and completed dynamic configuration wiring. The draft and live composer now source model/thought-level choices from host/ACP-advertised catalog data; no static model list is used when the host advertises options.

Verification: Rust workspace tests, Swift package tests, and iOS Simulator build pass.
