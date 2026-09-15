---
# slight-dhoh
title: standardize bounded host logs
status: completed
type: task
priority: normal
created_at: 2026-09-14T17:28:37Z
updated_at: 2026-09-14T17:31:10Z
---

Give the host a standard default logging directory and bounded log retention.

- [x] Define the default log directory and rotation policy
- [x] Write host logs to the rotating file
- [x] Add regression coverage and verify the host build/tests

## Summary of Changes

Added host-owned JSONL logging at `~/.slight/logs/host.log` by default, with `--log-dir` for overrides. Logs rotate at 10 MiB and retain five total files (`host.log` plus `.1` through `.4`), preventing unbounded growth. Added rotation and host integration coverage. Rebuilt and restarted the local host, then verified `acp-host status --json`.
