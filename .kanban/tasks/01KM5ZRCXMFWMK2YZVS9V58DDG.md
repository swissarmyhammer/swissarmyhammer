---
assignees:
- claude-code
depends_on:
- 01KM5ZQ6520JTV136H2J2R89VY
position_column: done
position_ordinal: ffffffffffad80
title: Per-row entity command scope for context menu
---
## What

Wire each data row's context menu to resolve entity-specific commands for *that* row's entity, not just the grid-level cursor entity. Currently the context menu fires from DataTable's `useContextMenu()` which reads from the grid-level `CommandScopeContext`. Entity commands (inspect, archive) are registered via `GridFocusBridge` based on cursor position, but the cursor update and scope registration happen in a `useEffect` — creating a potential race with the synchronous context menu open.

### The problem

When a user right-clicks row 5 while the cursor is on row 2, the sequence is:
1. `onContextMenu` fires → `grid.setCursor(5, col)` → state update queued
2. `contextMenuHandler(e)` fires synchronously → reads scope chain → gets row 2's commands
3. React re-renders → `GridFocusBridge` effect runs → registers row 5's scope (too late)

### Solution

Wrap each data row's selector cell (from card 1) in a per-row `CommandScopeProvider` that provides entity commands for that specific row. The context menu handler on the selector cell then resolves commands from that row's scope, not the grid-level scope.

### Implementation approach

- In `GridView`, pass a `renderRowSelector` callback to `DataTable` that returns a `<FocusScope>` or inline `<CommandScopeContext.Provider>` with that row's entity commands
- Alternatively, have `DataTable` accept a `rowCommands?: (entity: Entity, rowIndex: number) => CommandDef[]` prop, and wrap each selector cell in a scope
- The selector cell's `onContextMenu` should call `useContextMenu()` from inside the per-row scope
- The `GridFocusBridge` continues to handle keyboard-driven focus; this card only fixes mouse right-click scoping

### Files
- `kanban-app/ui/src/components/data-table.tsx` — per-row scope wrapper around selector cell
- `kanban-app/ui/src/components/grid-view.tsx` — pass row-level command factory to DataTable

## Acceptance Criteria
- [ ] Right-clicking row N's selector shows context menu with row N's entity commands (inspect, archive)
- [ ] Right-clicking row 5 while cursor is on row 2 shows row 5's commands, not row 2's
- [ ] Context menu items include the entity moniker in the handler key (for dispatch)
- [ ] Keyboard-driven context menu (if any) still works via GridFocusBridge

## Tests
- [ ] Add test: right-clicking row selector calls `show_context_menu` with that row's entity commands
- [ ] Add test: right-clicking row 5 while cursor on row 2 scopes commands to row 5
- [ ] Run: `cd kanban-app/ui && npx vitest run` — all pass