---
assignees:
- claude-code
depends_on:
- 01KMX2NJGVEVA175YBQZ6K88BC
position_column: done
position_ordinal: ffffffffffffffffd180
title: Update all menu item enabled state after every command dispatch
---
## What

After every command dispatch, iterate all cached menu MenuItem handles, check each command's `available()` against current context, and call `set_enabled()`. Universal — not special-cased for clipboard.

### Files to modify
- `kanban-app/src/menu.rs` — new `update_menu_enabled_state()` function
- `kanban-app/src/commands.rs` — call `update_menu_enabled_state()` after every `dispatch_command_internal`

### How it works
1. Build a `CommandContext` from current UIState scope chain + UIState ref (for clipboard flag etc.)
2. For each entry in `AppState.menu_items`:
   - Look up the `Command` impl from `AppState.command_impls`
   - Call `cmd_impl.available(&ctx)`
   - Call `menu_item.set_enabled(available)`
3. This is O(N) where N = menu items (~15-20), cheap

### CommandContext for availability checks needs:
- `scope_chain` from UIState (current focus)
- `ui_state` Arc for clipboard checks etc.
- No KanbanContext extension needed (availability checks only use `has_in_scope()` and UIState)

## Acceptance Criteria
- [ ] All menu items reflect their command's `available()` state
- [ ] Focus task → Cut/Copy enable; unfocus → disable
- [ ] Copy task → Paste enables immediately (clipboard flag set, column/board in scope)
- [ ] Works for non-clipboard commands too (e.g. commands requiring board scope)
- [ ] No full menu rebuild — just `set_enabled()` calls

## Tests
- [ ] Manual: focus task → Cut/Copy enabled in menu
- [ ] Manual: copy → Paste enabled without focus change
- [ ] Manual: click away from task → Cut/Copy disabled"
<parameter name="assignees">[]