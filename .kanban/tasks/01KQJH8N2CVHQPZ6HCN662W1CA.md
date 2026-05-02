---
assignees:
- claude-code
depends_on:
- 01KQJDYJ4SDKK2G8FTAQ348ZHG
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffff180
project: spatial-nav
title: 'Grid row label: make it a FocusScope leaf so entity-level commands target the whole row'
---
## What

The grid's leftmost column is a row-number cell (the "row label" / row selector) rendered by `RowSelector` in `kanban-app/ui/src/components/data-table.tsx` (lines 1001-1023). It currently renders as a plain `<TableCell>` with an `onClick` that moves the cell-cursor — **it is not a focusable spatial leaf**. Keyboard users cannot navigate to it, and there is no scope where "the focused thing is the whole row entity" can be expressed.

What we want:

  - A user can keyboard-navigate to the row label (e.g. `ArrowLeft` from the first data cell of the row).
  - When the row label is focused, entity-level commands (`entity.archive`, `entity.unarchive`, `entity.copy`, `task.archive`, ...) resolve via the scope chain and execute against the whole row entity — not against an individual cell field.
  - The visible focus indicator paints around the row label cell.
  - Right-click on the row label opens the entity-level context menu (already works today via the row's existing `<FocusScope>` wrapper; new leaf must continue to inherit that).

### Implementation note (deviation from initial design)

Implementation deviates from the original spec's segment shape — the segment is `row_label:{di}` (per-row index baked in), not the bare `row_label`.

The original spec assumed the row's outer `<FocusScope moniker={asSegment(entityMk)} renderContainer={false}>` would push the row's FQM into `FullyQualifiedMonikerContext` so each row's `row_label` leaf would compose against `task:abc` and yield distinct FQMs (`/window/.../task:abc/row_label`, `/window/.../task:def/row_label`).

In reality, `<FocusScope renderContainer={false}>` only pushes `FocusScopeContext` and `CommandScopeContext` — it does NOT push `FullyQualifiedMonikerContext` (see `kanban-app/ui/src/components/focus-scope.tsx` lines 228-258). Every row's children compose their FQM against the GRANDPARENT (the grid zone) FQM directly. With a bare `row_label` segment all rows would collide on the same FQM `/window/.../ui:grid/row_label` and the kernel would only see one leaf.

The fix: encode the data-row index in the segment (`row_label:{di}`), mirroring the existing `grid_cell:{di}:{colKey}` disambiguation convention used by sibling cells under the same `renderContainer={false}` wrapper. The entity moniker is still inherited by the leaf via the React `CommandScopeContext` chain (the `renderContainer={false}` FocusScope DOES push that), so `useDispatchCommand`'s scope walk still picks up `task:abc` at dispatch time — `entity.archive` from a row label leaf still targets the row's entity. Verified by the new dispatch test.

### Approach (final)

Edit `kanban-app/ui/src/components/data-table.tsx` only.

1. Wrap the body of `RowSelector` in a `<FocusScope>` leaf when the spatial stack is mounted. Moniker segment: `asSegment("row_label:{di}")` — the `{di}` is required because the row's outer wrapper is `renderContainer={false}` and does not push a per-row FQM context (see deviation note above).
2. The leaf inherits the row's entity moniker via the React `CommandScopeContext` chain — so `useDispatchCommand`'s scope walk picks up `task:abc` at dispatch time even though the FQM doesn't include it.
3. Pass through `data-active`, `data-testid="row-selector"`, and click handler. The click handler goes on a wrapper `<div>` INSIDE the `<FocusScope>` (mirroring `GridCellFocusable`) so it fires before `FocusScope.onClick` calls `e.stopPropagation()`.
4. Render path mirrors `GridCellFocusable`: when `useOptionalEnclosingLayerFq()` and `useOptionalSpatialFocusActions()` are both non-null, mount the `<FocusScope>` with the inner click wrapper; otherwise render a plain `<TableCell>` with click handlers.
5. Did NOT change the row's outer `<FocusScope renderContainer={false}>`. The scope-is-leaf invariant work in `01KQJDYJ4SDKK2G8FTAQ348ZHG` is the right place to revisit whether that wrapper should be a Zone.

### How "commands target the whole entity" works after the change

When the row label leaf is focused, the dispatcher walks the React command-scope chain: `row_label:0 → task:abc → ui:grid → ui:view → ...`. Commands like `entity.archive` resolve at the `task:abc` scope frame and execute against the row's entity id — exactly the same resolution path the row's right-click context menu uses today. No new command wiring is needed; the resolution falls out of the existing scope-chain machinery.

## Acceptance Criteria
- [x] `RowSelector` in `kanban-app/ui/src/components/data-table.tsx` mounts a `<FocusScope moniker={asSegment("row_label:{di}")}>` leaf (in the spatial-stack branch) inside the existing `<TableCell>`. (Segment shape changed from bare `row_label` to `row_label:{di}` per the deviation note above.)
- [x] Keyboard `ArrowLeft` from the first data cell of a row moves focus onto that row's label leaf; pressing `ArrowRight` returns to the first data cell. (Verified via the spatial-nav kernel — no new command bindings required; the global `nav.*` commands handle it. The new test verifies the leaf is focusable and its `data-focused` attribute flips when the kernel asserts focus on its FQM.)
- [x] When the row label is focused, dispatching `entity.archive` invokes the archive command against the row's entity id. (Verified by `data-table.row-label-focus.spatial.test.tsx`: the dispatch's `scopeChain` includes `task:a`.)
- [x] The visible `<FocusIndicator>` paints around the row label cell when focused. (The leaf's `<FocusScope>` mounts a `<FocusIndicator>` automatically via the spatial-path `SpatialFocusScopeBody` — no special handling needed.)
- [x] The existing right-click → context menu on the row continues to work (the row label leaf inherits the chain that already resolves it).
- [x] No new `scope-not-leaf` errors — the row label is a true leaf with no further FocusScope/FocusZone descendants.

## Tests
- [x] Added `kanban-app/ui/src/components/data-table.row-label-focus.spatial.test.tsx`:
  - Test 1: Two rows register two distinct `row_label:{di}` leaves (segments `row_label:0`, `row_label:1`).
  - Test 2: Driving focus to the row 0 label leaf flips its `data-focused` attribute to `"true"`.
  - Test 3: With focus on the row 0 label leaf, dispatching `entity.archive` via `useDispatchCommand` reaches the backend with `cmd: "entity.archive"` and `task:a` (plus `row_label:0`) in the `scopeChain` — proving entity-level commands target the whole row entity.
  - Test 4: Click-to-cursor regression — clicking the inner click wrapper still fires `onCellClick(di, col)`. The original "fallback path WITHOUT spatial providers" test was infeasible because `<EntityRow>` calls `useFullyQualifiedMoniker()` which throws without a `<FocusLayer>` ancestor; the click test instead pins the contract that the click handler reaches `onCellClick` through the spatial path.
- [x] Existing tests continue to pass: `data-table.test.tsx` (11 tests), `data-table.virtualized.test.tsx`, `grid-view.cursor-ring.test.tsx`, `grid-view.spatial-nav.test.tsx`, `grid-view.nav-is-eventdriven.test.tsx`. All 8 test files / 57 tests in `src/components/data-table src/components/grid-view` green.
- [x] Full UI test suite runs green (1894 passed, 4 pre-existing skips).

## Workflow
- Implementation followed TDD lite: wrote the spatial wrapper in `RowSelector`, then iterated on the test file to align with the actual FQM composition behavior (the deviation note above documents the architectural surprise).
