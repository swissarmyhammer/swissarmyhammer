---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffff280
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

Each grid command fires `broadcastRef.current(navEvent)`, where `broadcastNavCommand` resolves to the no-op declared in `kanban-app/ui/src/lib/entity-focus-context.tsx`:

```
broadcastNavCommand: (commandId) => false   // "no-op that always returns false. Real navigation lives in the Rust spatial-nav kernel."
```

The grid-scope commands win the scope-chain lookup over the global nav commands defined in `kanban-app/ui/src/components/app-shell.tsx` (`NAV_COMMAND_SPEC` / `buildNavCommands`), which actually dispatch `spatial_navigate(focusedFq, direction)` to the Rust kernel. The global `nav.up/down/left/right/first/last` is therefore never reached while focus is inside the grid.

**Fix applied**:

1. Deleted `buildGridNavCommands` and `navCmd` helper. The cardinal-direction commands `grid.moveUp/Down/Left/Right` and the `nav.first/nav.last` aliases were redundant with the global `nav.*` commands and have been removed.
2. Replaced with `buildGridExtremeCommands` that defines only the four commands that have no global counterpart:
   - `grid.moveToRowStart` (vim `0`, cua `Home`) — first cell of the focused row, via `setFocus(composeFq(gridZoneFq, asSegment(gridCellMoniker(row, firstColKey))))`.
   - `grid.moveToRowEnd` (vim `$`, cua `End`) — last cell of the focused row, same pattern.
   - `grid.firstCell` (cua `Mod+Home`) — absolute first cell, same pattern.
   - `grid.lastCell` (cua `Mod+End`) — absolute last cell, same pattern.
3. The grid-zone FQM is derived at execute time by stripping the trailing segment from the currently-focused cell FQM, so the commands work without the call site needing access to the `<GridSpatialZone>`'s FQM context.
4. Dropped `broadcastNavCommand` from `useGridNavigation`'s return shape — no other live consumer in the grid. The interface on `FocusActions` stays for `board-view.tsx` (out of scope for this task).

**Out of scope**: `board-view.tsx` still uses `broadcastNavCommand` for board-specific moves — left alone in this task; track separately if also broken.

## Acceptance Criteria
- [x] Inside a grid view, `ArrowUp`/`ArrowDown`/`ArrowLeft`/`ArrowRight` (cua) move the cell cursor between cells and update `data-cell-cursor` to the new `grid_cell:R:K`.
- [x] Inside a grid view, vim mode `k`/`j`/`h`/`l` move the cursor in the same directions. (Bound globally via `nav.*` commands.)
- [x] `Home`/`End` move to the first/last cell of the current row; `Mod+Home`/`Mod+End` move to the first/last cell of the grid.
- [x] `gg` (vim sequence) and `Shift+G` move to the first/last cell. (Bound globally via `nav.first`/`nav.last`.)
- [x] Each navigation keystroke calls `invoke("spatial_navigate", { focusedFq, direction })` exactly once (or for row-extreme keys, calls `setFocus` against the kernel exactly once).
- [x] No call to `broadcastNavCommand` is made from grid-view code paths after the fix.
- [x] Existing `grid-view.nav-is-eventdriven.test.tsx` invariants still hold: nav must not trigger `list_entities`, `get_entity`, `get_board_data`, or `dispatch_command { cmd: "perspective.list" }`.

## Tests
- [x] New test `kanban-app/ui/src/components/grid-view.keyboard-nav.spatial.test.tsx` mounts `<GridView>` inside `<AppShell>` + the spatial-nav stack:
  - Seeds entity focus on `grid_cell:R:<col>` via a `focus-changed` event.
  - Dispatches `keydown` for `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `Mod+Home`, `Mod+End` (one per assertion block).
  - Asserts cardinal arrows produce exactly one `mockInvoke("spatial_navigate", { focusedFq, direction })` with the expected direction.
  - Asserts row-extreme and grid-extreme keys produce exactly one `mockInvoke("spatial_focus", { fq })` with the expected destination cell key, and zero `spatial_navigate` calls.
  - Asserts no `dispatch_command { cmd: "grid.move{Up,Down,Left,Right}" }` calls land on arrow keys.
- [x] `grid-view.nav-is-eventdriven.test.tsx` still passes unchanged — the eventdriven-nav contract holds (no data fetches on nav).
- [x] Regression: `grid-view.spatial-nav.test.tsx`, `grid-view.cursor-ring.test.tsx`, `grid-view.test.tsx`, `grid-view.stale-card-fields.test.tsx` continue to pass.
- [x] `cd kanban-app/ui && pnpm vitest run src/components/grid-view` — all 47 tests green (38 existing + 9 new).
- [x] Full project test run: 1903 tests pass, 4 skipped, 0 failures.

## Workflow
- Used `/tdd` — wrote the failing `grid-view.keyboard-nav.spatial.test.tsx` first (9 tests, all RED), implemented the fix in `grid-view.tsx`, all 9 tests went GREEN. No existing tests regressed.