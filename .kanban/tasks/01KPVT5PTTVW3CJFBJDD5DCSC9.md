---
assignees:
- claude-code
position_column: todo
position_ordinal: df80
project: spatial-nav
title: 'Grid column header: Enter should toggle sort (currently a no-op on a focused header)'
---
## What

A focused grid column header currently has no Enter binding. Spatial nav can reach it (`k` from a body cell, `h`/`l` between headers) and clicking toggles sort — but pressing `Enter` while the header is focused does nothing. The user's contract is "Enter activates the focused scope," and on a column header that means toggle the sort for that column.

### Where the gap is

`kanban-app/ui/src/components/data-table.tsx`, `HeaderCell` component (lines 279–324):

```tsx
return (
  <FocusScope moniker={headerMoniker} commands={[]} renderContainer={false}>
    <HeaderCellTh
      ...
      onClick={handleClick}
      onContextMenu={handleContextMenu}
    >
      ...
    </HeaderCellTh>
  </FocusScope>
);
```

The `FocusScope` passes `commands={[]}`. Click already wires `handleClick` — which is either `header.column.getToggleSortingHandler()` (TanStack-native) or a `dispatchSortToggle({ args: { field, perspective_id } })` call (perspective-driven), built in `buildSortClickHandler` at line 265. Because `commands` is empty, Enter on the focused header falls through to parent scope and no sort-toggle command resolves.

### Fix approach — exact mirror of the established pattern

Every other focusable scope type has a per-instance namespaced `CommandDef` bound to Enter whose `execute` re-uses the click handler so mouse and keyboard converge. Precedents:

| Scope | Command id | Binds to |
|---|---|---|
| Row selector | `ui.inspect` (inline) | `data-table.tsx:1028` (RowSelector) |
| LeftNav button | `view.activate.<id>` | `left-nav.tsx` (ViewButton) |
| Perspective tab | `perspective.activate.<id>` | `perspective-tab-bar.tsx` (ScopedPerspectiveTab) |
| Card | `entity.activate.<moniker>` | `entity-card.tsx` (useCardCommands) |
| Toolbar buttons | `toolbar.*.activate` | `nav-bar.tsx` (useToolbarActions) |

The column header follows the same pattern. In `HeaderCell`, replace `commands={[]}` with a `useMemo`'d per-column array:

```tsx
const commands = useMemo<CommandDef[]>(
  () => [
    {
      id: `column-header.sort.${columnId}`,
      name: `Sort by ${columnId.replace(/_/g, " ")}`,
      keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
      execute: handleClick,
      contextMenu: false,
    },
  ],
  [columnId, handleClick],
);
```

`execute` is `handleClick` — the identical function the `onClick` already calls. That means:
- **Perspective-driven path** (a `perspectiveId` is present): `dispatchSortToggle({ args: { field: columnId, perspective_id: perspectiveId } })` — the same backend command click already dispatches. Correct.
- **TanStack-native path** (no `perspectiveId`): `header.column.getToggleSortingHandler()` — TanStack's own toggle function. Takes an optional `MouseEvent` argument but does not require one, so calling it with zero args from `execute` works. Verify by reading the TanStack signature if in doubt.

Pass `commands` through to the `FocusScope`. `contextMenu: false` is important so the per-column command doesn't clutter the right-click menu (click / right-click already dispatch sort / grouping directly).

### Files to modify

- `kanban-app/ui/src/components/data-table.tsx` — replace `commands={[]}` on `HeaderCell`'s `FocusScope` with the per-column `useMemo`'d array. Import `useMemo` (already imported). Import `CommandDef` type from `@/lib/command-scope` (likely already imported transitively; add if missing).
- `kanban-app/ui/src/components/data-table.test.tsx` — add tests following the precedent at `data-table.test.tsx:526` (`DataTable row selector Enter opens inspector` describe block).

### Out of scope

- Enter on a column header opening a filter popover — that's a different interaction, not in scope.
- Changing how clicks toggle sort — the click path is untouched; `handleClick` is the shared executor.
- Adding the column-header sort command to the global command palette — the namespaced per-column id (`column-header.sort.<field>`) is scope-local by design so it shows only when the corresponding header is focused.

## Acceptance Criteria

- [ ] Each data-column `<th>`'s enclosing `FocusScope` carries a non-empty `commands` array with a per-column entry bound to `Enter` across `vim`/`cua`/`emacs` keymaps
- [ ] With spatial focus on a column header in a grid that has an active perspective, pressing `Enter` dispatches `perspective.sort.toggle` with `{ field: <columnId>, perspective_id: <perspectiveId> }`
- [ ] With spatial focus on a column header in a grid without a perspective (TanStack-native sort path), pressing `Enter` toggles the column's sort direction — verify via the `sorting` state on the TanStack table instance or the re-rendered sort indicator
- [ ] Clicking the column header still toggles sort (regression protection — same `handleClick` reused as `execute`)
- [ ] The per-column command does not appear in the global context menu (`contextMenu: false`)
- [ ] Pressing `Enter` on a body cell, row selector, or any other focused grid scope is unaffected — existing scope-local Enter bindings continue to win over the new header-scope binding since they're nested deeper in the chain
- [ ] The row-selector `<th>` (leftmost spacer) remains unscoped per the existing design in `HeaderCell`'s intentional-skip case (already out of the `header.column` iteration)

## Tests

- [ ] New test in `kanban-app/ui/src/components/data-table.test.tsx` — mirror the structure of `describe("DataTable row selector Enter opens inspector", …)` at line 526:
  - Render a grid with 3 columns and `perspectiveId="default"`, focus the first data column's `<th>` (via `setFocus(columnHeaderMoniker(columnId))` on the fixture's `useEntityFocus`), press `userEvent.keyboard("{Enter}")`, assert the `dispatchSortToggle` mock received `args: { field: <col0-id>, perspective_id: "default" }` exactly once
  - Same fixture, focus the SECOND column header, assert the dispatch target is that column's id (guards against wrong-target bugs where the first column's handler fires for any header)
- [ ] New test for the TanStack-native path (no `perspectiveId`): focus a column header, press Enter, assert `table.getState().sorting` flipped to include that column — verify against the rendered `SortIndicator` or on the TanStack instance directly (whichever the existing header tests use as precedent)
- [ ] Regression test: focus a column header, click it (not keyboard), assert the same `dispatchSortToggle` fires — confirms `handleClick` is still the click executor
- [ ] Run `cd kanban-app/ui && npm test -- data-table --run` — all existing 20-ish data-table tests plus the new ones green
- [ ] Run `cd kanban-app/ui && npm test -- spatial-nav --run` — spatial-nav regression suite stays green (this change adds a per-scope command, it must not shadow anything)
- [ ] Run `cd kanban-app/ui && npm test` — full 1404+-test UI suite green

## Workflow

- Use `/tdd`. Write the two dispatch-assertion tests first. They will fail at HEAD because `commands={[]}` in `HeaderCell` means Enter has no binding and no sort dispatch fires. Add the `useMemo`'d `commands` array, wire it into the `FocusScope`, and the tests flip to green.
- Do NOT change `buildSortClickHandler` or `handleClick` — the whole point is that Enter and click share the same executor. Any refactor of the handler itself is out of scope.
- Keep the command id namespaced (`column-header.sort.<columnId>`) and `contextMenu: false`. Those two attributes together prevent the per-column entries from leaking into the global command palette or context menu.
