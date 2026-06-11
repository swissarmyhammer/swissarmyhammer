---
assignees:
- claude-code
position_column: todo
position_ordinal: ed80
project: ui-command-cleanup
title: Fix pre-existing perspective-tab-bar.filter-migration.test.tsx failures (filter button click → empty spatial_focus fq)
---
## What
Two tests in `apps/kanban-app/ui/src/components/perspective-tab-bar.filter-migration.test.tsx` fail on branch `plugin`:

- `filter_button_click_dispatches_nav_focus_with_filter_editor_fq` — "spatial_focus.fq must end with filter_editor:p1 (got )"
- `filter_button_click_targets_the_currently_active_perspective` — "spatial_focus.fq must end with filter_editor:p2 when p2 is active (got )"

A `spatial_focus` IPC fires but its `fq` payload is empty, suggesting `FilterEditorDrillOutWiring`'s FQM-ref handoff to the Filter tab button yields null/empty at click time in this fixture (or the last matching IPC call has a different wire shape than the test unwraps).

## Evidence it pre-dates Card E (01KTED7PFKRS6GMAQKVDCQA07V)
Verified during Card E implementation (2026-06-11): restoring `perspective-tab-bar.tsx` to its HEAD version (before any Card E edits) reproduces the exact same 2 failures, so the regression was already present in the working tree / branch before the editor drill-in move. The test file itself is unmodified in git.

## Done means
- Root cause identified (empty `fq` in the `spatial_focus` IPC, or stale test unwrap shape).
- Both tests green without weakening their assertions.