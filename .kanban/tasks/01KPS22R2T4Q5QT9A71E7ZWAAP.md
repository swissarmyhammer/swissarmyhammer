---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8680
project: spatial-nav
title: 'Inspector: nav inside inspector breaks when opened over the grid (works over the board)'
---
## What

Inspector spatial navigation behaves correctly when opened from the board but breaks when opened from the grid:
- Header-section field rows are skipped
- j/k jumps past middle rows and lands on footer/action elements

The inspector is wrapped in `<FocusLayer name="inspector">`, which should isolate nav to the inspector layer regardless of what's behind it. Since symptoms depend on the parent view, something about layer isolation OR the inspector's own scope shape is susceptible to the lower-layer population.

### Root cause (confirmed by failing test)

**Hypothesis #1 in the original task description was correct.** The inspector's entity-level `FocusScope` at `kanban-app/ui/src/components/inspector-focus-bridge.tsx:108-120` used the default `spatial={true}`, which registers a large bounding rect enclosing the entire inspector body. During cardinal navigation inside the inspector (j from the last field, k from the first field edge cases, or transitioning between header and body sections under certain rect geometries), the Rust beam-test saw this outer container rect as a valid candidate and selected it instead of clamping or moving to the next field row.

The reported "skipping header fields" / "leap to footer" symptom is the same class of bug: the container rect attracted scoring over the smaller field-row rects.

Confirmed not already fixed by `01KPS1WCQRY8DEWQVA47PZ82ZC` (unified beam-test scoring): that task changed in-beam vs out-of-beam arbitration, but did not affect the container-rect shadowing case. A focused test (`spatial-nav-inspector-over-grid.test.tsx`) reproduced the failure at HEAD with the existing fix already landed; focus leaked from the last inspector field onto the `task:card-1-1` entity-level scope, not to a grid scope.

### Fix

Add `spatial={false}` to the inspector's entity-level FocusScope in `InspectorFocusBridge`. This mirrors the exact same pattern used by `DataTableRow` in `data-table.tsx:794`, where a row container is focus-aware (commands, claim callbacks, scope chain) but its rect is explicitly excluded from the beam-test graph because its cells are the real spatial targets.

### Files modified

- `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — added `spatial={false}` on the entity scope. One-line fix with rationale comment mirroring `DataTableRow`.
- `kanban-app/ui/src/test/spatial-inspector-over-grid-fixture.tsx` — **new** fixture that mirrors the production `InspectorFocusBridge` shape (entity-level FocusScope wrapping field rows) on top of a dense background grid. The existing `spatial-inspector-fixture.tsx` has no outer entity scope so it never reproduced the bug.
- `kanban-app/ui/src/test/spatial-nav-inspector-over-grid.test.tsx` — **new** 6-test regression suite that exercises j/k with a 50- and 100-row grid behind the inspector, verifies active-layer filtering, and guards the `j at the last field clamps` invariant that failed at HEAD.

Layer isolation and beam-test scoring were NOT changed — they were verified correct by the layer-filter assertion in the new test (`active layer is 'inspector' and candidate pool excludes grid scopes`).

### Files NOT modified (hypothesis ruled out)

- `swissarmyhammer-spatial-nav/src/spatial_state.rs` — layer isolation was already correct; the failing test proved the grid scopes never made it into the candidate pool. The bug was pure inspector-local geometry.
- `kanban-app/ui/src/components/focus-layer.tsx` — push/pop timing is fine.
- `kanban-app/ui/src/components/slide-panel.tsx` — no portal issues.

## Acceptance Criteria

- [x] Open an entity from the grid. In the resulting inspector, j/k navigates through every field row — including the header section — without skipping. New test `j walks every inspector field in order (no skipping)` over a 50-row background grid.
- [x] Same entity opened from the board shows identical nav behavior (no regression). Existing `spatial-nav-inspector.test.tsx` (single card in window layer) still green; new dense-grid test covers the analogous grid-background case.
- [x] j from the last field row does not leap to a footer action over the top of middle fields. New test `j at the last field clamps` was the failing test before the fix; green with `spatial={false}`.
- [x] k from the first field row stays at the first field row (doesn't leak into the parent view's scopes). New test `k at the first field clamps (does not leak to the grid beneath)` asserts both the positive (focus stays on first field) and negative (no `field:tag:*` or `column-header:*` monikers) conditions.
- [x] Inspecting from an even denser grid (e.g. 100+ rows) has the same nav behavior as inspecting from a sparse grid. New test `dense 100-row grid behaves identically to the 50-row baseline` scales the fixture to 100 rows and walks every field.
- [x] The grid is in a separate FocusLayer (window) beneath the inspector's FocusLayer; verified via the shim's `layersSnapshot` + `entriesSnapshot` that the active layer is "inspector" and the inspector's field monikers are registered on the inspector layer, not the window layer. New test `active layer is 'inspector' and candidate pool excludes grid scopes`.

## Tests

- [x] Added `spatial-nav-inspector-over-grid.test.tsx` — 6 vitest-browser tests covering the failure modes in the acceptance criteria. Uses `setupSpatialShim()` to route spatial invokes through the JS shim so the React tree and its ResizeObserver-driven rect registration are exercised end-to-end.
- [x] Added `spatial-inspector-over-grid-fixture.tsx` — a new fixture that reproduces the production `InspectorFocusBridge` shape (entity-level FocusScope) on top of a dense grid. The canonical `spatial-inspector-fixture.tsx` was kept unchanged; the new fixture is the regression guard for the container-scope geometry case.
- [x] `cargo test -p swissarmyhammer-spatial-nav` — 66 unit tests + 1 parity test all green.
- [x] `cd kanban-app/ui && npm test` — 1383 tests in 128 files all green, zero failures.
- [x] No Rust-side changes needed — the JS parity case list already covers the layer-isolation invariant (`navigate_layer_filter_excludes_inactive_layer_entries`) that this fix depended on.
- [x] The existing `spatial-nav-inspector.test.tsx` (inspector over sparse single-card window) still green — regression guard for the "from-board" working case per the task's requirement for a parallel test; the canonical fixture does not add an entity-level FocusScope so it continued to pass throughout.

## Workflow

- Followed the task's TDD plan exactly: wrote the failing dense-grid test first, watched `j at the last field clamps` fail with focus leaking to the entity scope (confirming hypothesis #1), applied the 1-LOC `spatial={false}` fix in `InspectorFocusBridge`, watched all 6 tests pass.
- Did NOT bundle with `01KPS1WCQRY8DEWQVA47PZ82ZC` — ran the failing test at HEAD (which already had that fix) and proved the bug is independent.
- The algorithm, layer isolation, and Rust core were all untouched — the fix is purely in the React inspector wrapper, the narrowest possible change.

## Review Findings (2026-04-21 19:06)

Scope: diff for this task (`kanban-app/ui/src/components/inspector-focus-bridge.tsx` plus new `spatial-inspector-over-grid-fixture.tsx` and `spatial-nav-inspector-over-grid.test.tsx`).

Verified locally:
- 6/6 tests in `spatial-nav-inspector-over-grid.test.tsx` pass
- Canonical `spatial-nav-inspector.test.tsx` + `inspector-focus-bridge.test.tsx` still pass (12/12)
- Wider spatial-nav sanity set (`board`, `grid`, `inspector`, `inspector-over-grid`) 24/24 green
- The `spatial={false}` idiom matches the DataTableRow precedent at `kanban-app/ui/src/components/data-table.tsx:794` with parallel rationale

### Nits
- [x] `kanban-app/ui/src/test/spatial-nav-inspector-over-grid.test.tsx:270` — comment updated to reflect that the entity-level scope is intentionally excluded via `spatial={false}`. New comment: "the inspector layer has at least the four field scopes (the entity-level scope is intentionally excluded via `spatial={false}`)". 6/6 tests still pass.
