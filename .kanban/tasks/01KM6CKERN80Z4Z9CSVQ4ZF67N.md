---
assignees:
- claude-code
depends_on:
- 01KM6CH612XPADMZ6QRTMA4EFV
position_column: done
position_ordinal: ffffffffffffa180
title: Add separator support to context menus
---
## What

Context menus currently render all items flat with no visual grouping. When the scope chain accumulates commands from nested entities (task → column → board), items from different scopes should be separated.

### Frontend changes (`kanban-app/ui/src/lib/context-menu.ts`)
- `useContextMenu` already has access to `CommandAtDepth` (which includes `depth`)
- Group items by depth, insert a sentinel `{ id: \"__separator__\", name: \"\" }` between groups
- Items within the same depth stay in declaration order (YAML order)

### Rust changes (`kanban-app/src/commands.rs`)
- Update `ContextMenuItem` to support separators — either a boolean `separator` field, or check for the sentinel `__separator__` id
- In `show_context_menu`, call `builder.separator()` when encountering a separator item instead of `builder.text()`

### Files to modify
- `kanban-app/ui/src/lib/context-menu.ts` — group by depth, insert separators
- `kanban-app/src/commands.rs` — handle separator items in `show_context_menu`

## Acceptance Criteria
- [ ] Right-clicking a task card on the board produces a menu with separators between task/column/board command groups
- [ ] A single-scope context menu (e.g. board background) has no spurious separators
- [ ] Empty groups don't produce double separators

## Tests
- [ ] `kanban-app/ui/src/lib/context-menu.test.ts` — test that items at different depths get separators inserted
- [ ] `kanban-app/ui/src/lib/context-menu.test.ts` — test single-depth produces no separators
- [ ] `pnpm --filter kanban-app test` and `cargo test -p kanban-app` pass