---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffe80
project: spatial-nav
title: 'Grid row selector: Enter should open the inspector, not enter cell edit'
---
## What

When the row selector cell is spatially focused in the data-table grid and the user presses Enter, the grid enters edit mode on the first row's first cell. That's wrong: the row selector is a per-row affordance, and Enter on it should open the inspector for the row's entity — the same thing a card's double-click or the header "Inspect" button does.

### Root cause

The row selector's `FocusScope` in `kanban-app/ui/src/components/data-table.tsx` (around the `RowSelector` component, ~line 1063) passes `commands={[]}`. Empty commands on a child scope means the scope chain walks up to parent scopes for key resolution.

The parent scope chain includes a grid-level `CommandScopeProvider` (in `kanban-app/ui/src/components/grid-view.tsx`) whose `gridCommands` include `grid.editEnter` bound to `Enter` in vim mode and `grid.edit` bound to `Enter` in CUA mode (grid-view.tsx:266-267). Since the row selector scope doesn't shadow Enter, the parent's `grid.editEnter` fires — and `enterEdit()` defaults to the current cursor cell, which at init is (0, 0).

So pressing Enter on the row selector resolves to grid-level `grid.editEnter` → `grid.enterEdit()` → edits cell (0, 0). Never touches the inspector.

### Inspector command

`ui.inspect` — precedent in `kanban-app/ui/src/components/entity-card.tsx:294-310` (`InspectButton`):

```tsx
const dispatch = useDispatchCommand("ui.inspect");
// ...on click / activate:
dispatch({ target: moniker });
```

The explicit `target` matters: the comment at `entity-card.tsx:287-293` documents that the backend must use `ctx.target` rather than walking the scope chain (which might point to a previously-focused entity rather than this row).

`CommandDef` (in `kanban-app/ui/src/lib/command-scope.tsx:181-211`) supports both a `target?: string` field and an `execute?` callback. Either works for this case; the cleaner pattern (matching `InspectButton`) is an `execute` callback that calls the dispatcher with the explicit target.

### Approach

Update the `RowSelector` component in `kanban-app/ui/src/components/data-table.tsx` to pass a non-empty `commands` array on its FocusScope. The array binds Enter to `ui.inspect` with the row's entity moniker as the explicit target:

```tsx
function RowSelector({ entity, di, isCursorRow, onClick }: RowSelectorProps) {
  const dispatchInspect = useDispatchCommand("ui.inspect");
  const commands = useMemo<CommandDef[]>(() => [{
    id: "ui.inspect",
    name: "Inspect",
    keys: { cua: "Enter", vim: "Enter" },
    execute: () => dispatchInspect({ target: entity.moniker }),
  }], [dispatchInspect, entity.moniker]);

  return (
    <FocusScope
      moniker={fieldMoniker(entity.entity_type, entity.id, ROW_SELECTOR_FIELD)}
      commands={commands}
      renderContainer={false}
    >
      <RowSelectorTd ... />
    </FocusScope>
  );
}
```

Because the child scope's `Enter` binding exists, the scope chain walk stops at the row selector scope and `grid.editEnter` in the parent is shadowed — exactly the behavior documented in `CommandDef.target` / shadow-key resolution at `command-scope.tsx:200-207`.

### Files to modify

- `kanban-app/ui/src/components/data-table.tsx` — add `useDispatchCommand("ui.inspect")` + `useMemo`'d `commands` array inside `RowSelector`, pass through to its `FocusScope` (replacing the current `commands={[]}`)
- New imports likely needed: `useDispatchCommand` from `@/lib/command-scope`, `CommandDef` from `@/lib/command-scope`, `useMemo` from `react` (likely already imported)

## Acceptance Criteria

- [x] Focusing a row selector cell via spatial nav (h/j/k/l) and pressing Enter opens the inspector for that row's entity
- [x] The inspector that opens shows the correct entity (not some other row) — verified by the entity id in the inspector header matching the focused row's id
- [x] Pressing Enter on a regular data cell (not the row selector) still enters edit mode for that cell (existing behavior unchanged)
- [x] Pressing Enter on a row selector does NOT enter edit mode on (0, 0) or any other cell
- [x] The implementation uses the `ui.inspect` command with an explicit `target: entity.moniker`, matching the `InspectButton` precedent

## Tests

- [x] Add a vitest-browser test in `kanban-app/ui/src/components/data-table.test.tsx` (or a new focused test file) that:
  - Renders a grid with 3 rows, row selector column visible
  - Sets spatial focus to the row selector cell of row 2 via the test's focus-setter
  - Dispatches an Enter keydown
  - Asserts `ui.inspect` was dispatched with `target` equal to row 2's entity moniker
  - Asserts NO `grid.editEnter` / `grid.enterEdit` side effect fired (no edit mode transition)
- [x] Add a regression test for the existing behavior: focus a regular data cell, press Enter → edit mode activates (existing behavior protected)
- [x] Run `cd kanban-app/ui && npm test` — all 1301+ tests still pass, new tests green (1360 tests pass)
- [ ] Manual verification: open a grid, click the row selector of row 3, press Enter → inspector opens showing row 3's entity. Click cell (1, 1), press Enter → edit mode for that cell.

## Workflow

- Use `/tdd` — write the failing row-selector-Enter-opens-inspector test first, then implement.
- Keep the fix localized to `RowSelector`. Do NOT modify `grid.editEnter` / `grid.edit` global bindings — the override pattern (child commands shadow parent keys) is already in place; we just aren't using it here.

## Implementation Notes

Implementation landed:

- `kanban-app/ui/src/components/data-table.tsx` — `RowSelector` now builds a `useMemo`'d `commands` array with one `CommandDef` `{ id: "ui.inspect", keys: { vim: "Enter", cua: "Enter" }, execute: () => dispatchInspect({ target: entity.moniker }) }`, passed to its `FocusScope`. Docstring updated to explain the shadow-key rationale.
- `kanban-app/ui/src/components/data-table.test.tsx` — new `describe("DataTable row selector Enter opens inspector")` block with three tests (Enter on row 2 selector, Enter on row 3 selector, Enter on a data cell falls through to `grid.editEnter`). Uses `FixtureShell` + nested `CommandScopeProvider(gridCommands)` to mirror the real scope layering. Harness component `DataTableWithCellFocus` wires `onCellClick` to `setFocus` for the data-cell test.
