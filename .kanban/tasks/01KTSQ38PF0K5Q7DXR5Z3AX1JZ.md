---
assignees:
- claude-code
position_column: review
position_ordinal: '8180'
title: 'Stale drill wire-shape expectations: board-view.enter-drill-in.browser.test.tsx — 6 tests fail pre-existing'
---
## What

Six pre-existing failures in `apps/kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx`, discovered while verifying 01KTQ8KRJYX1DPHN76TZ654ZX2 (same architecturally-stale family; NOT caused by that card — its diff touches only `perspective-tab-bar.enter-rename.spatial.test.tsx` and `spatial-focus-context.test.tsx`, which this file does not import):

- enter_on_focused_column_drills_into_first_card
- enter_on_focused_column_with_remembered_focus_drills_into_remembered_card
- enter_on_focused_card_passes_snapshot_to_drill_in
- enter_on_focused_card_drills_into_first_field
- escape_on_focused_field_drills_out_to_parent_card
- escape_on_focused_card_passes_snapshot_to_drill_out

All fail like: "vim Enter on a focused column must dispatch spatial_drill_in exactly once: expected +0 to be 1". The tests expect the WEBVIEW to invoke `spatial_drill_in` / `spatial_drill_out` with an `fq` (and a geometry `snapshot`), then fan out `setFocus` with the kernel-returned moniker. After the nav-commands host-driven drill rework, drill executes in the plugin runtime (kernel pulls geometry host-side); the webview just dispatches the command id via `dispatch_command` (`cmd: "nav.drillIn"` / `"nav.drillOut"`).

## How (precedents)

- `app-shell.test.tsx` "dispatches nav.drillIn to the backend on Enter" pins the current contract: `dispatch_command` with `cmd: "nav.drillIn"`, plus asserting NO legacy `spatial_drill_in` invoke.
- 01KTQ8KRJYX1DPHN76TZ654ZX2 fixed the identical drift in `perspective-tab-bar.enter-rename.spatial.test.tsx` test #3 (and removed its stale `spatialDrillInCalls()` helper).
- The "passes_snapshot" tests are doubly stale: the webview no longer threads geometry snapshots into drill at all (host pulls geometry on demand) — those assertions need rethinking against the kernel-side e2e (`builtin_nav_commands_e2e.rs`), not just a wire-shape swap.

Do NOT weaken assertions — assert the real current contract (dispatch of the command id to the backend + no legacy client-side drill IPC + the DOM/focus outcomes that remain observable in the webview).

## Acceptance Criteria
- [x] The 6 tests assert the current host-driven drill contract (Enter/Escape → `dispatch_command` `nav.drillIn`/`nav.drillOut`; no webview-side fq/snapshot pre-resolution for drill)
- [x] Stale helpers (spatialDrillInCalls-style filters, drill-response mocks) removed or updated
- [x] Whole file green under `npx vitest run`; adjacent files not regressed; tsc clean

## Implementation notes (2026-06-12)

Repaired in the 7c5015141 style. The two `passes_snapshot` tests were renamed to `*_sends_no_snapshot_or_fq_on_drill_wire` — they now pin the NEGATIVE: the `dispatch_command` payload carries no `snapshot`/`fq`/`focused_fq`, and zero client-side drill IPC leaves the webview. The drill-in/out IPC helpers were repurposed as must-stay-empty no-legacy guards (matching `column-view.spatial.test.tsx`); a `spatialFocusCalls()` guard additionally pins that no webview-side `spatial_focus` fan-out fires (the kernel commits focus host-side). DOM outcomes retained: kernel `focus-changed` emissions are mimicked and `data-focused` transitions asserted (column→first card, column→remembered t2, card→field unfocus, field→parent card refocus). No shared-helper changes. Verified: `npx vitest run src/components/board-view.enter-drill-in.browser.test.tsx` → 9/9 pass; `npx tsc --noEmit` → exit 0.