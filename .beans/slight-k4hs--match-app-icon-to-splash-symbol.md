---
# slight-k4hs
title: match app icon to splash symbol
status: completed
type: task
priority: normal
created_at: 2026-09-14T22:33:56Z
updated_at: 2026-09-14T22:34:31Z
---

Replace the custom app-icon tree artwork with the exact tree.fill SF Symbol used by the splash screen, preserving the existing forest gradient.

## Summary of Changes

Replaced the custom tree SVG with a rendered `tree.fill` SF Symbol PNG matching `HostOnboardingView.swift`. The existing gradient background remains unchanged. Verified macOS and iOS builds.
