---
assignees:
- claude-code
position_column: todo
position_ordinal: ac80
project: spatial-nav
title: 'Grid keyboard navigation broken: grid commands shadow global nav.* with no-op broadcast'
---
## What

Keyboard navigation in the grid view is broken. Pressing arrow keys (or `j`/`k`/`h`/`l` in vim mode) does nothing while focus is inside a grid cell.

**Root cause**: `kanban-app/ui/src/components/grid-view.tsx` defines a local `CommandScopeProvider` with its own nav CommandDefs:

```
buildGridNavCommands  (kanban-app/ui/src/components/grid-view.tsx)
  - grid.moveUp     keys: { vim: "k", cua: "ArrowUp" }
  - grid.moveDown   keys: { vim: "j", cua: "ArrowDown" }
  - grid.moveLeft   keys: { vim: "h", cua: "ArrowLeft" }
  - grid.moveRight  keys: { vim: "l", cua: "ArrowRight" }
  - grid.moveToRowStart   keys: { vim: "0", cua: "Home" }
  - grid.moveToRowEnd     keys: { vim: "$", cua: "End"  }
  - grid.firstCell  keys: { cua: "Mod+Home" }
  - grid.lastCell   keys: { vim: "Shift+G", cua: "Mod+End" }
  - nav.first / nav.last aliases
```

Each grid command fires `broadcastRef.current(navEvent)`, where `broadcastNavCommand` resolves to the no-op declared in `kanban-app/ui/src/lib/entity-focus-context.tsx` lines 175–183:

```
broadcastNavCommand: (commandId) => false   // "no-op that always returns false. Real navigation lives in the Rust spatial-nav kernel."
```

The grid-scope commands win the scope-chain lookup over the global nav commands defined in `kanban-app/ui/src/components/app-shell.tsx` (`NAV_COMMAND_SPEC` / `buildNavCommands`, lines 226–301), which actually dispatch `spatial_navigate(focusedFq, direction)` to the Rust kernel. The global `nav.up/down/left/right/first/last` is therefore never reached while focus is inside the grid.

**Fix approach** (delete shadow, do not route the no-op):

1. Delete the redundant grid-scope nav commands. The global `nav.up`/`nav.down`/`nav.left`/`nav.right`/`nav.first`/`nav.last` already provide vim (`hjkl`, `Shift+G`, `gg` via sequence), cua (arrows, `Home`/`End`), and emacs bindings, and they correctly dispatch through the spatial-nav kernel.
2. In `useGridCommands`, remove `buildGridNavCommands` from the composed array so only `buildGridEditCommands` (edit/visual mode + row mutation) remains in the grid scope.
3. For the row-extreme bindings without a global counterpart (`grid.moveToRowStart` = `0`/`Home`, `grid.moveToRowEnd` = `$`/`End`): rewrite their `execute` to call the spatial-nav kernel directly via the `useOptionalSpatialFocusActions()` ref. They should compute the destination cell moniker (`grid_cell:R:firstColKey` or `grid_cell:R:lastColKey`) from the current cursor's row, then call `setFocus(composeFq(gridZoneFq, asSegment(cellMoniker)))`. Do not broadcast.
4. `broadcastNavCommand` is already a no-op; once the grid stops calling it, drop `broadcastNavCommand` from the `useGridNavigation` return shape (it has no other live consumer in the grid). The interface on `FocusActions` can stay until `board-view.tsx` is migrated as a follow-up.

**Out of scope**: `board-view.tsx` (line 1078, 1081, 1094) still uses `broadcastNavCommand` for board-specific moves — leave it alone in this task; track separately if also broken.

## Acceptance Criteria
- [ ] Inside a grid view, `ArrowUp`/`ArrowDown`/`ArrowLeft`/`ArrowRight` (cua) move the cell cursor between cells and update `data-cell-cursor` to the new `grid_cell:R:K`.
- [ ] Inside a grid view, vim mode `k`/`j`/`h`/`l` move the cursor in the same directions.
- [ ] `Home`/`End` move to the first/last cell of the current row; `Mod+Home`/`Mod+End` move to the first/last cell of the grid.
- [ ] `gg` (vim sequence) and `Shift+G` move to the first/last cell.
- [ ] Each navigation keystroke calls `invoke("spatial_navigate", { focusedFq, direction })` exactly once (or for row-extreme keys, calls `setFocus` against the kernel exactly once).
- [ ] No call to `broadcastNavCommand` is made from grid-view code paths after the fix.
- [ ] Existing `grid-view.nav-is-eventdriven.test.tsx` invariants still hold: nav must not trigger `list_entities`, `get_entity`, `get_board_data`, or `dispatch_command { cmd: "perspective.list" }`.

## Tests
- [ ] New test `kanban-app/ui/src/components/grid-view.keyboard-nav.spatial.test.tsx` mounting `GridView` inside the spatial-nav stack (mirror the harness in `grid-view.spatial-nav.test.tsx`):
  - Seeds entity focus on `grid_cell:0:<firstCol>`.
  - Dispatches a synthetic `keydown` for `ArrowDown`, `ArrowRight`, `k`, `l`, `Home`, `End`, `Mod+Home`, `Mod+End` (one per assertion block).
  - Asserts each press produces exactly one `mockInvoke("spatial_navigate", { focusedFq, direction })` call with the expected `direction` argument (`"down"`, `"right"`, `"up"`, `"right"`, …) and zero calls in test for `broadcastNavCommand`-style paths.
- [ ] Update `grid-view.nav-is-eventdriven.test.tsx` if needed so it still passes after the grid scope no longer registers `grid.moveUp`/`grid.moveDown`/`grid.moveLeft`/`grid.moveRight`. The eventdriven-nav contract (no data fetches on nav) must remain.
- [ ] Regression: existing `grid-view.spatial-nav.test.tsx`, `grid-view.cursor-ring.test.tsx`, `grid-view.test.tsx`, `grid-view.stale-card-fields.test.tsx` continue to pass.
- [ ] Run `cd kanban-app/ui && pnpm vitest run src/components/grid-view` and confirm all grid tests green.

## Workflow
- Use `/tdd` — write the failing `grid-view.keyboard-nav.spatial.test.tsx` first, watch arrow-key dispatches return zero `spatial_navigate` invocations, then delete the grid-scope nav commands and confirm the test goes green.
