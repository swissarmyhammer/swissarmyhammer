---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffa080
title: Add separator support to context menus
---
## What\n\nGroup context menu items by depth and insert visual separators between groups.\n\n### Files to modify\n- `kanban-app/ui/src/lib/context-menu.ts` — modify useContextMenu hook to group by depth, insert __separator__ sentinels\n- `kanban-app/src/commands.rs` — handle __separator__ items in show_context_menu, exclude from context_menu_ids\n\n### Changes\n1. Frontend: Loop through contextCommands (CommandAtDepth objects), track lastDepth, insert {id: '__separator__', name: ''} between depth groups\n2. Rust: Check item.id == '__separator__' in MenuBuilder loop, call builder.separator() instead of builder.text()\n3. Exclude separator IDs from context_menu_ids set\n4. Write tests for: separators inserted between depths, no separator for single depth, empty command list\n\n## Acceptance Criteria\n- [ ] Items at different depths get separators between them\n- [ ] Single-depth items produce no separators\n- [ ] Separator IDs excluded from pending handlers and context_menu_ids\n- [ ] All tests pass