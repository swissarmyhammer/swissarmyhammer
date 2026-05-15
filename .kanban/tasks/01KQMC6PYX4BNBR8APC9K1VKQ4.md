---
assignees: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffc680
title: resolve 4 pre-existing it.skip()s in kanban-app/ui — fix or delete
---
The frontend test suite reports 4 skipped tests on the `kanban` branch. Per the test skill rule "skipped tests are not acceptable. A skipped test is either broken (fix it) or dead (delete it)" — but these are pre-existing project-level decisions, so I am not removing them as part of a /test verification run. File for explicit resolution.

Skipped tests:

1. kanban-app/ui/src/lib/entity-focus.kernel-projection.test.tsx:229
   `setFocus(moniker) for an unknown moniker leaves the store untouched and logs an error`
   Inline note: requires `spatial_focus_by_moniker` rejection path. The kernel-side rejection mechanism described in the test (kernel emits `tracing::error!`, simulator mirrors as no-op) may not yet be exposed through the React adapter.

2. kanban-app/ui/src/components/focus-scope.test.tsx:772
   `useIsFocused ancestor: column gets data-focused when card inside is focused`
   Tests an ancestor-focus indicator behaviour — likely waiting on the FocusIndicator design that landed for cards.

3. kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx:1114
   `clicking a toolbar action focuses it and renders the indicator — production has no toolbar component today`
   Inline note explicitly says no production toolbar component exists. The test is tracking a future component class.

4. kanban-app/ui/src/components/board-view.spatial-nav.test.tsx:282
   `does not wrap in FocusZone when no SpatialFocusProvider is present`
   Tests a fallback shape that may have been removed when SpatialFocusProvider became mandatory.

Resolution plan: for each, decide whether (a) the feature is now implementable — un-skip and rewrite; (b) the feature is no longer planned — delete the test and any associated dead code.

Found during a /test run on commit 35a106634 (registry overlap-warning landed cleanly).

## Resolution (2026-05-09 — closed as obsolete)

All four items have already been resolved by intervening work; no `.skip` calls remain in `kanban-app/ui`:

1. `entity-focus.kernel-projection.test.tsx:229` — un-skipped to a real `it(`. The kernel-side rejection path now exists; the test runs.
2. `focus-scope.test.tsx:779` — un-skipped to a real `it(`. The ancestor `useIsFocused` indicator landed; the test runs.
3. `focus-on-click.regression.spatial.test.tsx` — `it.skip` block deleted; a comment block at lines ~1119–1123 documents the deferred toolbar-component class so the gap is still visible to future contributors. No production toolbar exists.
4. `board-view.spatial-nav.test.tsx` — `it.skip` deleted; a comment tombstone at line 266 records the removal.

Repo-wide grep for `it.skip(|test.skip(|describe.skip(` in `kanban-app/ui` returns the lone `board-view.spatial-nav.test.tsx` match, which is the comment text "Note: a former it.skip(...)", not an actual call. `pnpm -C kanban-app/ui test` reports `0 skipped` across 215 test files / 2068 tests.

Cleanup commits earlier this session: `15ca8f7a1` "test: drop dead it.skip placeholders in nav suites".

Closing.