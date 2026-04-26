---
assignees:
- claude-code
depends_on:
- 01KMWS35SK4YDZ17FQG83PV1SH
- 01KMWS3KZAZHYCTF3KR86GG6BJ
position_column: done
position_ordinal: ffffffffffffffffcb80
title: Wire clipboard commands into native Edit menu with dynamic enable/disable
---
## What

Replace the OS predefined Cut/Copy/Paste/Select All menu items with command-system-aware menu items that dispatch through our command system and enable/disable based on focus chain state.

### Files to modify
- `kanban-app/src/menu.rs` — replace PredefinedMenuItem::cut/copy/paste/select_all with MenuItem::with_id for entity.cut/copy/paste; remove select_all
- `kanban-app/src/commands.rs` — add menu item enable/disable when scope chain changes (on ui-state-changed or scope-chain events)

### Current behavior (broken)
Lines 102-115 of menu.rs use `PredefinedMenuItem::cut/copy/paste/select_all` — these are OS-level text editing items, always enabled, completely disconnected from our command system.

### Required behavior
- Edit menu shows Cut (Cmd+X), Copy (Cmd+C), Paste (Cmd+V) — no Select All
- These dispatch `entity.cut`, `entity.copy`, `entity.paste` through the command system (via `menu-command` event → frontend `executeCommand`)
- Cut/Copy enabled only when task is in focus chain
- Paste enabled only when clipboard is non-empty AND column or board is in focus chain
- Menu items update their enabled state when focus changes

### Enable/disable approach
When `ui.setFocus` fires (scope chain changes), check availability of entity.cut/copy/paste against the new scope chain and call `set_enabled()` on the menu items. UIState already stores the scope chain and clipboard — both are readable synchronously via RwLock.

## Acceptance Criteria
- [ ] No Select All in Edit menu
- [ ] Cut/Copy/Paste in Edit menu dispatch entity.cut/copy/paste commands
- [ ] Cut/Copy grayed out when no task focused
- [ ] Paste grayed out when clipboard empty or no column/board focused
- [ ] Menu items update enabled state on focus change

## Tests
- [ ] Manual verification: focus task → Cut/Copy enabled; focus board only → Cut/Copy disabled
- [ ] Manual verification: copy task → Paste enabled when column focused; Paste disabled when clipboard empty
- [ ] `cargo nextest run -p kanban-app` passes"
<parameter name="assignees">[]