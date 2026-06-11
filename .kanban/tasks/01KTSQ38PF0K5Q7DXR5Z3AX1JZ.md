---
assignees:
- claude-code
position_column: todo
position_ordinal: ea80
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
- [ ] The 6 tests assert the current host-driven drill contract (Enter/Escape → `dispatch_command` `nav.drillIn`/`nav.drillOut`; no webview-side fq/snapshot pre-resolution for drill)
- [ ] Stale helpers (spatialDrillInCalls-style filters, drill-response mocks) removed or updated
- [ ] Whole file green under `npx vitest run`; adjacent files not regressed; tsc clean