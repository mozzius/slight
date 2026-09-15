---
# slight-bnbd
title: bundle Rust daemon with macOS app
status: todo
type: feature
priority: deferred
created_at: 2026-09-12T14:28:52Z
updated_at: 2026-09-12T14:28:52Z
parent: slight-4yk6
---

Package the canonical Rust host daemon inside the macOS Slight app and install/register it as a launchd service during app setup. The app remains an administration client and convenience shell; the bundled daemon remains the canonical host implementation. Cover signed binary packaging, version compatibility, install/uninstall/upgrade, launchd lifecycle, logs, status, recovery, user consent, and development overrides.\n\n- [ ] Define daemon packaging and app/daemon version contract\n- [ ] Add signed daemon binary to macOS app bundle\n- [ ] Implement install, upgrade, uninstall, and launchd registration\n- [ ] Add macOS admin UI lifecycle/status integration\n- [ ] Add logs, recovery, and migration behavior\n- [ ] Add packaging and launchd integration tests\n- [ ] Document release and support runbooks
