---
# slight-ryiz
title: paginate transcript without scroll jumps
status: completed
type: feature
priority: normal
created_at: 2026-09-13T14:07:16Z
updated_at: 2026-09-13T14:44:15Z
---

Automatically fetch older transcript pages when scrolling to the top, preserve the prefetch anchor after prepending, and use larger history pages without moving the user's scroll position.


## Tasks

- [x] Automatically load older history at the transcript top
- [x] Preserve the scroll anchor while prepending
- [x] Increase history page size
- [x] Verify Apple build


## Summary of Changes

- Added an automatic top sentinel that fetches older history pages as the user scrolls upward.
- Captures the first existing transcript item before prepending and scrolls back to that item afterward, preventing jumps.
- Increased history pages from 100 to 200 events.
- Preserved interactive keyboard dismissal.
- Rust tests and Apple build pass.


## Correction

Older pages no longer rebuild the live transcript through an empty intermediate state. They are reduced separately and prepended in one assignment, so the iOS view remains populated while loading and the captured anchor can be restored without a bottom-scroll race. Apple build reverified.


## Correction

Replaced the top `onAppear` trigger with continuous scroll-offset preference tracking, so reaching the top reliably initiates the next page without requiring a down-and-up nudge. Apple build reverified.


## Follow-up

- [x] Only auto-scroll transcript updates when the user is already near the bottom
- [x] Verify prepend and live-message scroll behavior


## Correction

Transcript updates now auto-scroll only while the bottom spacer is within 40pt of the viewport. Users reading older content keep their position; prepend pagination remains independently anchored. Apple build reverified.


## Follow-up

- [x] Use role-specific default scroll anchors to avoid size-change jumps


## Correction

On iOS 18/macOS 15 and newer, default anchors are now bottom for initial placement, top for size changes, and bottom for short-content alignment. Older OS versions retain the existing bottom-anchor fallback. Apple build reverified.


## Follow-up

- [x] Prevent initial history pages from flashing at an intermediate scroll position
- [x] Disable automatic size-change repositioning and scroll once after initial history is ready


## Correction

Removed the `.top` size-change anchor as requested. Initial history now has an explicit loaded-state transition and one bottom-spacer scroll after the first page is installed; intermediate transcript mutations no longer trigger scroll commands. Apple build reverified.


## Correction

History prepends now rebuild the combined loaded event set through a temporary accumulator and publish the final transcript once. Reasoning/message chunks can merge across page boundaries without an empty intermediate transcript or split thought snippets. Apple build reverified.


## Correction

The prepend rebuild now assigns deterministic message IDs from event sequences when provider IDs are absent. Existing anchor IDs survive history reconstruction, preventing the scroll position from jumping to the top. Apple build reverified.


## Follow-up

- [x] Add targeted scroll geometry and anchor debug logging


## Correction

The top trigger is now edge-triggered only when the sentinel enters the `-10...40pt` window, and it is disarmed during anchor restoration. This prevents duplicate page requests and repeated scrollbar/content-height churn. Apple build reverified.


## Correction

Boundary merges now preserve the existing first transcript item and its ID, merging older blocks into it instead of removing the captured anchor. This prevents `scrollTo` from falling back to the top. Apple build reverified.


## Correction

Anchor and bottom scrolls now execute in a transaction with animation disabled. The debug trace showed the prior `scrollTo` walking through many frames, causing the visible jump and scrollbar motion. Apple build reverified.


## Correction

The debug trace showed the top preference resetting during the load and re-triggering pagination. The trigger now remains disarmed for the entire prepend/restore transaction, and anchor restoration is explicitly non-animated. Apple build reverified.


## Correction

Removed SwiftUI `defaultScrollAnchor` from the transcript entirely so framework size-change anchoring cannot compete with explicit prepend and bottom positioning. Apple build reverified.


## Correction

Removed the boundary merge that mutated the captured anchor row. Older items are now prepended without changing the existing first row, and pending bottom-scroll tasks abort after yielding when prepend mode is active. Apple build reverified.
