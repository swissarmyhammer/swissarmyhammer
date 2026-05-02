---
assignees: []
position_column: todo
position_ordinal: c180
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