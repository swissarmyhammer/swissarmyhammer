---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffb980
title: Add row-level entity FocusScope to DataTable grid rows
---
## What

Grid rows in `data-table.tsx` lack a row-level entity scope. Each row represents an entity (like a card does in board view), but unlike EntityCard — which wraps everything in `FocusScope(moniker=entityMoniker, commands=entityCommands)` — the grid row is a bare `<TableRow>` with no entity moniker or commands in scope.

**Current state:**
- `RowSelectorWithScope` (line 608) wraps only the selector `<td>` in a `CommandScopeProvider` — the rest of the row's cells are outside it
- Cell-level `FocusScope` (inside `GridCellScope`, line 577) has `commands={[]}` and a cell moniker (`task:id.fieldName`), not an entity moniker
- Right-clicking the selector shows entity commands (correct), but right-clicking a cell or using palette inspect gives \"entity not found\" because the entity target isn't in scope
- The grid-level `entityCommands` wrapping (from `grid-view.tsx` line 493) only has the cursor row's entity — not the row you're interacting with

**Target state (modeled on EntityCard):**
Each `<TableRow>` should be wrapped in a `FocusScope` with `renderContainer={false}` (no wrapping div — preserves `<tbody>→<tr>` HTML structure) carrying the row entity's moniker and commands. This gives:
- Right-click context menu on any cell resolves entity commands
- Double-click inspect works from any cell
- Palette inspect resolves the focused row's entity
- `useIsFocused(moniker)` available for row highlight

**Key constraint:** `FocusScope` normally renders a `<FocusHighlight>` div wrapper. In a `<table>`, a div between `<tbody>` and `<tr>` breaks HTML. `renderContainer={false}` (already added to `focus-scope.tsx`) skips the div but also skips click/doubleclick/contextmenu handlers. The row component must attach those handlers to `<TableRow>` itself.

**Files to modify:**
- `kanban-app/ui/src/components/data-table.tsx` — wrap each data `<TableRow>` in `FocusScope(renderContainer=false, moniker=entityMk, commands=rowCommands)`. Create an `EntityRow` component rendered inside the scope that calls `useContextMenu()`, `useIsFocused()`, `useEntityFocus().setFocus`, and `useDispatchCommand(\"ui.inspect\")` to mirror `FocusScopeInner` behavior on a `<tr>`. Remove `RowSelectorWithScope` and `RowSelectorCell` (the row-level scope subsumes them). Row selector picks up focused state from `useIsFocused(moniker)` via the wrapping FocusScope — no grid cursor comparison needed. The existing `handleCellClick`, `onRowContextMenu`, `onCellClick` props and the grid-level `contextMenuHandler` need careful audit — some become dead code.
- `kanban-app/ui/src/components/grid-view.tsx` — remove the `entityCommands` `CommandScopeProvider` wrapping (line 493) since row-level scopes replace it

## Acceptance Criteria
- [ ] Right-click any cell in a grid row shows entity context menu (\"Inspect Task\", \"Archive Task\", etc.)
- [ ] Double-click a grid row opens the inspector for that row's entity
- [ ] Palette inspect (Cmd+Shift+P → \"Inspect\") targets the focused row's entity
- [ ] Row selector reads focused state from `useIsFocused(moniker)` hook, not grid cursor index
- [ ] Grid cell editing (click/double-click/Enter) still works
- [ ] Grid cell navigation (arrow keys, vim hjkl) still works
- [ ] `<table>` HTML structure valid — no `<div>` between `<tbody>` and `<tr>`
- [ ] `RowSelectorWithScope` and `RowSelectorCell` removed

## Tests
- [ ] `cd kanban-app/ui && pnpm vitest run` — all unit tests pass
- [ ] Manual: grid view — right-click any cell → context menu with entity commands
- [ ] Manual: grid view — double-click row → inspector opens for that entity
- [ ] Manual: grid view — Cmd+Shift+P \"Inspect\" → inspects focused row entity