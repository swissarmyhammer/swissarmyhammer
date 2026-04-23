---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff680
project: spatial-nav
title: 'Perspective bar: reachable from every view via top-edge nav'
---
## What

The perspective tab bar sits at the top of the main content area, above the active view (board or grid). The user expects universal spatial nav: from anywhere in the window layer, pressing `k` enough times eventually lands on a perspective tab. No view should trap focus.

Today, `ScopedPerspectiveTab` in `kanban-app/ui/src/components/perspective-tab-bar.tsx` uses `CommandScopeProvider` only — no FocusScope, no spatial entry. Beam test has zero perspective-tab candidates.

This task is the GENERIC version — ensure perspective tabs are reachable from the board, the grid, and any future view. Inspector excluded (modal layer — confirmed by `01KPNWPEMK` test).

### Harness / infrastructure already available (2026-04-20 19:45)

The vitest-browser spatial harness from `01KPNWGFTF` is landed and green:
- `kanban-app/ui/src/test/spatial-shim.ts`, `setup-spatial-shim.ts`
- `spatial-fixture-shell.tsx` — shared FixtureShell + FixtureKeybindingHandler
- `spatial-board-fixture.tsx`, `spatial-grid-fixture.tsx` — board / grid fixtures
- `spatial-nav-{board,grid,inspector,canonical}.test.tsx` — precedent

This task's fixture composes the perspective tab bar above the board/grid fixture. Suggested file: `kanban-app/ui/src/test/spatial-perspective-fixture.tsx` — mount `<PerspectiveTabBar />` above the existing fixture's view content. Reuse `spatial-fixture-shell.tsx` for keybinding wiring.

The `FocusScopeElementRefContext` pattern and `spatial?: boolean` prop on `FocusScope` already landed via `01KPNWH82X` (grid). PerspectiveTab can follow the same approach.

### TDD — failing tests

Under `kanban-app/ui/src/test/spatial-nav-perspective.test.tsx`:

```ts
describe("perspective bar reachable from all views", () => {
  it("k from top-row card in the board moves focus to the active perspective tab", async () => {
    // renderWithBoardAndPerspectiveFixture()
    // click cardEl("card-2-1"); keyboard("k") (already in top row)
    // expect focused moniker to match /^perspective:/
  });

  it("k from top-row cell in a grid moves focus to the active perspective tab", async () => {
    // renderWithGridAndPerspectiveFixture()
    // click cellEl(0, 0); keyboard("k")
    // expect focused moniker to match /^perspective:/
  });

  it("h/l between perspective tabs in the same bar", async () => {
    // clickPerspectiveTab("Default"); keyboard("l")
    // expect focused moniker matches /^perspective:/ AND not "perspective:Default"
  });

  it("j from an active perspective tab moves into the active view", async () => {
    // clickPerspectiveTab("Default"); keyboard("j")
    // expect non-/^perspective:/ focus (card or cell)
  });
});
```

### Approach

1. Change `ScopedPerspectiveTab` to wrap in `FocusScope` (`renderContainer=false`, `showFocusBar=false`) with `moniker("perspective", perspective.id)`.
2. Inner `PerspectiveTab` root `<div className="inline-flex items-center">` attaches the FocusScope elementRef via `useFocusScopeElementRef()` — same pattern as `DataTableRow`.
3. PerspectiveTab already has a `CommandScopeProvider` parent — preserve that behavior (right-click commands still resolve).
4. Filter/group popover buttons appear only on the active tab — they inherit the tab's scope chain, so their behavior is unchanged.

### Acceptance

- [x] All 4 E2E tests pass reliably
- [x] Existing perspective rename / context-menu behaviors unchanged
- [x] Filter formula bar still focuses on `onFilterFocus`
- [x] No regression in board / grid / inspector nav tests

### Implementation notes

- `FocusScope` replaces the prior `CommandScopeProvider` on `ScopedPerspectiveTab` — `FocusScope` owns command-scope registration AND spatial registration, so the explicit `CommandScopeProvider` wrapper was redundant.
- `PerspectiveTab`'s root `<div>` now carries `data-moniker`, `data-testid="data-moniker:<moniker>"`, and an `onClickCapture` that calls `setFocus(tabMoniker)`. Capture phase is required because the inner `FilterFocusButton` calls `e.stopPropagation()`; bubbling would miss.
- New fixture: `kanban-app/ui/src/test/spatial-perspective-fixture.tsx` — composes `<PerspectiveTabBar />` above the existing 3x3 board and 3x3 grid fixture content under a shared `EntityFocusProvider` + `FixtureShell`. Exports `AppWithBoardAndPerspectiveFixture` and `AppWithGridAndPerspectiveFixture`.
- New test file: `kanban-app/ui/src/test/spatial-nav-perspective.test.tsx` — 4 specs exactly as listed above; all four pass in vim mode (`h`/`j`/`k`/`l`).
- Minor re-exports added to `spatial-board-fixture.tsx` and `spatial-grid-fixture.tsx` (`FixtureColumn`, `FixtureRow`, `BOARD_COLUMNS`, `GRID_ROWS`) so the perspective fixture can reuse them without duplicating the board/grid trees.
- `perspective-tab-bar.test.tsx` helper now wraps the tree in `EntityFocusProvider` because `FocusScope` requires it. 28 existing specs still pass.
- Full suite: 1316 passes / 0 failures across 124 test files. `tsc --noEmit` clean.

### Interaction with multi-window (01KPNXYZZJ)

Multi-window Rust refactor has landed — each window owns its own `SpatialState`. The perspective tabs in window A only register into window A's state; window B's tabs don't leak. The test fixture remains single-window; no multi-window assertion needed here. If 01KPP5S6T6 (UI listener follow-up) hasn't landed yet, a manual two-window verification may show cross-window event fallout unrelated to this task.

## Review Findings (2026-04-20 16:21)

### Nits
- [x] `kanban-app/ui/src/components/perspective-tab-bar.tsx:354` — Section comment says "Inner tab component — rendered inside CommandScopeProvider so useContextMenu sees the perspective scope and builds the correct chain." The wrapper changed from `CommandScopeProvider` to `FocusScope` (which internally provides `CommandScopeContext.Provider`). The comment is now stale. Suggestion: update to "rendered inside a `FocusScope` (which installs `CommandScopeContext.Provider`) so useContextMenu sees the perspective scope…"
- [x] `kanban-app/ui/src/test/spatial-perspective-fixture.tsx:60-63` — Docstring for `FIXTURE_PERSPECTIVE_IDS` claims "Ordered `[active, inactive]` in the board variant and reversed in the grid variant for clarity — both variants are exposed so tests can assert nav direction independently." Neither the fixture nor the test mocks reverses the order — both board and grid variants render the same `["default", "archive"]` ordering. Suggestion: drop the "reversed in the grid variant" claim; describe the single shared ordering and why two perspectives are enough (so `h`/`l` has a sibling to move to).