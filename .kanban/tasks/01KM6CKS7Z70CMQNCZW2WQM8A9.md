---
assignees:
- claude-code
depends_on:
- 01KM6CH612XPADMZ6QRTMA4EFV
position_column: done
position_ordinal: ffffffffffd380
title: Wire grid-view entity commands from schema
---
## What

GridView has its own hardcoded entity commands in two places — the cursor-row `entityCommands` and the per-row `buildRowEntityCommands` factory. Both define `entity.inspect` and `entity.archive` inline. Replace both with the shared `useEntityCommands` hook / a non-hook variant for the factory.

### Files to modify
- `kanban-app/ui/src/components/grid-view.tsx` — replace `entityCommands` useMemo (line ~240) and `buildRowEntityCommands` (line ~270) with schema-driven commands
- `kanban-app/ui/src/lib/entity-commands.ts` — may need a non-hook `buildEntityCommands()` function for the row factory pattern (called outside React render cycle)

### Special considerations
- The per-row factory (`buildRowEntityCommands`) is called from DataTable's `RowSelectorWithScope` which wraps each row in its own CommandScopeProvider. It receives the entity as an argument, not from hooks. So we need a plain function variant, not just the hook.
- Grid commands (`grid.edit`, `grid.escape`, `grid.deleteRow`, etc.) stay hardcoded in the component — they are view-specific, not entity-specific. This is acceptable.

## Acceptance Criteria
- [ ] Grid row context menus show entity commands from YAML (\"Inspect Task\", \"Archive Task\" etc.)
- [ ] Per-row right-click resolves commands for that row's entity, not the cursor row
- [ ] Grid-specific commands (edit, escape, deleteRow, newBelow, newAbove) remain functional

## Tests
- [ ] `kanban-app/ui/src/components/grid-view.test.tsx` — verify context menu items come from schema
- [ ] `pnpm --filter kanban-app test` passes